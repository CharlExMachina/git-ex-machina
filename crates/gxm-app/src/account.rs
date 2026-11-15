//! Signing in to GitHub, as a machine on the sheet: asking GitHub for a one-time code, waiting
//! while the person enters it on github.com, then signed in; or, opened while signed in, who it
//! is and Sign out. The network runs off the UI thread; Cancel stops the wait.
//!
//! A sign-in's token goes into the environment's forge access, and the open repository's pull
//! requests connect again with it; signing out goes back to the token the process started with
//! (`GH_TOKEN`), if any.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gxm_core::auth::{self, AuthError, DeviceCode};
use gxm_core::env::Env;
use gxm_core::forge::model::User as Account;
use rune::{Latest, Tx, Waker};

use crate::sheet::Sheet;
use crate::state::App;

/// Where a sign-in is, on the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignIn {
    /// Asking GitHub for a code.
    Asking,
    /// Waiting for the person to enter the code on github.com.
    Code(DeviceCode),
    Failed(String),
    /// Signed in: who it is.
    Account(Account),
    SigningOut(Account),
}

pub enum AccountEvent {
    Open,
    Retry,
    SignOut,
    CodeLanded(Result<DeviceCode, String>),
    /// A sign-in or sign-out finished: who's signed in now, and their token.
    Done(Result<Option<(Account, String)>, String>),
}

enum Effect {
    AskForCode,
    Wait(DeviceCode),
    Copy(String),
    SignOut,
    SetAccount(Option<(Account, String)>),
}

fn step(sheet: &mut Sheet, signed_in: Option<&Account>, event: AccountEvent) -> Vec<Effect> {
    match (sheet, event) {
        (sheet, AccountEvent::Open) => match signed_in {
            Some(a) => {
                *sheet = Sheet::Account(SignIn::Account(a.clone()));
                vec![]
            }
            None => {
                *sheet = Sheet::Account(SignIn::Asking);
                vec![Effect::AskForCode]
            }
        },
        (Sheet::Account(state @ SignIn::Failed(_)), AccountEvent::Retry) => {
            *state = SignIn::Asking;
            vec![Effect::AskForCode]
        }
        (Sheet::Account(state @ SignIn::Account(_)), AccountEvent::SignOut) => {
            if let SignIn::Account(a) = state {
                *state = SignIn::SigningOut(a.clone());
            }
            vec![Effect::SignOut]
        }
        (Sheet::Account(state @ SignIn::Asking), AccountEvent::CodeLanded(code)) => match code {
            Ok(code) => {
                *state = SignIn::Code(code.clone());
                vec![Effect::Copy(code.user_code.clone()), Effect::Wait(code)]
            }
            Err(why) => {
                *state = SignIn::Failed(why);
                vec![]
            }
        },
        (sheet, AccountEvent::Done(Ok(account))) => {
            if matches!(sheet, Sheet::Account(_)) {
                *sheet = Sheet::Closed;
            }
            vec![Effect::SetAccount(account)]
        }
        (Sheet::Account(state @ (SignIn::Code(_) | SignIn::SigningOut(_))), AccountEvent::Done(Err(why))) => {
            *state = SignIn::Failed(why);
            vec![]
        }
        // Answers for a sheet that's gone (cancelled) change nothing.
        _ => vec![],
    }
}

/// The background work: asking for a code, waiting for it to be entered, signing out.
#[derive(Default)]
pub struct AccountJobs {
    code: Latest<Result<DeviceCode, String>>,
    done: Latest<Result<Option<(Account, String)>, String>>,
    /// Read at startup: who's signed in and their token, as last remembered.
    pub remembered: Latest<Option<(Account, String)>>,
    cancel: Arc<AtomicBool>,
    /// The token the process environment gave (`GH_TOKEN`), which signing out goes back to.
    pub from_env: Option<String>,
}

impl AccountJobs {
    /// The jobs, remembering `from_env`: the token the process environment gave, if any.
    pub fn new(from_env: Option<String>) -> AccountJobs {
        AccountJobs { from_env, ..AccountJobs::default() }
    }

    pub fn set_waker(&self, waker: Waker) {
        self.code.set_waker(waker.clone());
        self.done.set_waker(waker.clone());
        self.remembered.set_waker(waker);
    }

    /// The remembered account is still being read.
    pub fn reading(&self) -> bool {
        self.remembered.is_pending()
    }

    /// Stops a wait for a code to be entered.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl App {
    pub fn account_event(&mut self, event: AccountEvent, tx: Option<&mut Tx>) {
        let mut tx = tx;
        for effect in step(&mut self.sheet, self.account.as_ref(), event) {
            match effect {
                Effect::AskForCode => self
                    .account_jobs
                    .code
                    .request(self.env.clone(), |env| auth::start(&env).map_err(|e| sentence(&e.to_string()))),
                Effect::Wait(code) => {
                    self.account_jobs.cancel.store(false, Ordering::Relaxed);
                    let cancel = self.account_jobs.cancel.clone();
                    self.account_jobs.done.request((self.env.clone(), code, cancel), |(env, code, cancel)| {
                        sign_in(&env, &code, &|| cancel.load(Ordering::Relaxed)).map(Some).map_err(|e| match e {
                            AuthError::Declined => "You declined the sign-in on GitHub".to_owned(),
                            AuthError::Expired => "The code expired before it was entered".to_owned(),
                            other => sentence(&other.to_string()),
                        })
                    });
                }
                Effect::Copy(code) => {
                    if let Some(tx) = tx.as_deref_mut() {
                        tx.copy(code);
                    }
                }
                Effect::SignOut => self.account_jobs.done.request(self.env.clone(), |env| {
                    auth::sign_out(&env).map(|()| None).map_err(|e| sentence(&e.to_string()))
                }),
                Effect::SetAccount(signed_in) => self.signed_in(signed_in),
            }
        }
    }

    /// Who's signed in now. Their token opens the forge, unless the process environment gave
    /// one; the open repository's pull requests connect again either way.
    fn signed_in(&mut self, signed_in: Option<(Account, String)>) {
        let token = match &signed_in {
            _ if self.account_jobs.from_env.is_some() => self.account_jobs.from_env.clone(),
            Some((_, token)) => Some(token.clone()),
            None => None,
        };
        self.account = signed_in.map(|(account, _)| account);
        if token != self.env.forge.github_token {
            self.env.forge.github_token = token;
            if let Some(r) = self.repo() {
                let (path, origin) = (r.summary.path.clone(), r.summary.origin.clone());
                self.pulls.open(&self.env, path, origin.as_ref());
            }
        }
    }

    /// Takes whatever the account's jobs have answered. True when the state changed.
    pub fn land_account(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(remembered) = self.account_jobs.remembered.take() {
            self.signed_in(remembered);
            changed = true;
        }
        if let Some(code) = self.account_jobs.code.take() {
            self.account_event(AccountEvent::CodeLanded(code), Some(tx));
            changed = true;
        }
        if let Some(done) = self.account_jobs.done.take() {
            self.account_event(AccountEvent::Done(done), Some(tx));
            changed = true;
        }
        changed
    }
}

/// Waits for the code to be entered, then keeps the sign-in.
fn sign_in(env: &Env, code: &DeviceCode, cancelled: &dyn Fn() -> bool) -> Result<(Account, String), AuthError> {
    let token = auth::wait(env, code, cancelled)?;
    let account = auth::whoami(env, &token)?;
    auth::remember(env, &account, &token)?;
    Ok((account, token))
}

/// `couldn't reach GitHub` → `Couldn't reach GitHub`.
fn sentence(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}
