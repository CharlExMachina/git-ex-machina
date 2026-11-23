//! `gxm tags` and `gxm tag`: whether pushing a tag releases, the version that comes next, and
//! tagging a commit (and pushing the tag), as the app's tag sheet does.

use std::io::Write;
use std::path::PathBuf;

use gxm_core::env::Env;
use gxm_core::history;
use gxm_core::release::NotesFrom;
use gxm_core::tag::{self, NewTag};

fn w(out: &mut impl Write, line: impl std::fmt::Display) -> Result<(), String> {
    writeln!(out, "{line}").map_err(|e| e.to_string())
}

/// `gxm tags [PATH]`: what pushing a tag starts, and what the next tag would be called.
pub fn tags(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let path = args.first().map_or_else(|| PathBuf::from("."), PathBuf::from);
    let t = tag::survey(env, &path);
    match &t.trigger {
        Some(trigger) => {
            let which = if trigger.patterns.is_empty() {
                "any tag".to_owned()
            } else {
                format!("a tag matching {}", trigger.patterns.join(" or "))
            };
            w(out, format!("Releases: pushing {which} starts {} ({})", trigger.what, trigger.file))?;
            let notes = match &trigger.notes {
                NotesFrom::Tag => "the tag's message (gxm tag NAME -m \"$(cat notes.md)\")".to_owned(),
                NotesFrom::File(file) => file.clone(),
                NotesFrom::Generated => "GitHub, from the merged pull requests".to_owned(),
                NotesFrom::Unknown => "the workflow doesn't say".to_owned(),
            };
            w(out, format!("Notes: {notes}"))?;
        }
        None => w(out, "Releases: nothing here runs when a tag is pushed")?,
    }
    match t.suggest() {
        Some(next) => w(out, format!("Next tag: {next}")),
        None => Ok(()),
    }
}

/// `gxm tag NAME [COMMIT] [-C PATH] [-m MESSAGE] [--push]`: tags COMMIT (HEAD by default).
pub fn tag(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let mut words = Vec::new();
    let (mut path, mut message, mut push) = (PathBuf::from("."), String::new(), false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-C" => path = PathBuf::from(it.next().ok_or("-C needs a path")?),
            "-m" | "--message" => message = it.next().ok_or("-m needs a message")?.clone(),
            "--push" => push = true,
            _ => words.push(a.clone()),
        }
    }
    let name = words.first().ok_or("name the tag: gxm tag NAME [COMMIT]")?.clone();
    let rev = words.get(1).map_or("HEAD", String::as_str);
    let commit = tag::resolve(env, &path, rev).map_err(|e| e.to_string())?;
    let surveyed = tag::survey(env, &path);
    if let Ok(info) = history::info(env, &path, &commit) {
        for warning in surveyed.warnings(&info.contained_in, push) {
            w(out, format!("Note: {warning}"))?;
        }
    }
    let short = commit.get(..7).unwrap_or(&commit).to_owned();
    let new = NewTag { name: name.clone(), commit, message, push };
    let pushed = tag::create(env, &path, &new).map_err(|e| e.to_string())?;
    w(out, format!("Tagged {name} on {short}"))?;
    if let Some(remote) = pushed {
        w(out, format!("Pushed {name} to {remote}"))?;
        if let Some(trigger) = surveyed.trigger.filter(|t| t.runs_for(&name)) {
            w(out, format!("That starts {} ({})", trigger.what, trigger.file))?;
        }
    }
    Ok(())
}
