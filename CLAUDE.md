# git-ex-machina

A native git client in Rust for macOS and Linux: a Rune GUI (`gxm-app`) and a CLI (`gxm`) over one domain crate. It works with GitHub, GitLab and Bitbucket; SSH is the default transport and HTTPS is opt-in.

## Layout

- `crates/gxm-core`: the domain (repositories, history, graph layout, remotes, git). No UI.
- `crates/gxm-cli` (`gxm`) and `crates/gxm-app` (Rune): thin layers over core.
- `crates/gxm-world`: hermetic **worlds** (sandboxed home, fixture repositories, local remotes).
- `crates/gxm-journeys`: **journeys**, which are the tests, the demo videos and the PR proof.
- `xtask`: every check and render. `cargo xtask help` lists them.
- `design/pr-flow/`: the approved mockups for the pull-request viewer, until it ships.

Rust comes from `rust-toolchain.toml` through rustup. Rune is pinned in `Cargo.toml`: its `rev` and the four `[patch.crates-io]` entries move together, in a PR of their own that re-runs every journey and golden.

## The loop

Every change goes through these steps, in order:

1. **Journey first.** Extend the journey for the workflow you're changing (or start one, if the person's goal is new). Run it with `cargo test -p gxm-journeys` and watch it go red for the reason you expect.
2. **Make it green.** Put logic in `gxm-core`, where the CLI and the app can both verify it.
3. **See it.** `cargo xtask journey <name>`, then open every mark PNG in `target/proof/<name>/`. For a visual change, render the same mark before and after (the base branch too) and look at both zoomed in. This step is done when you've looked at every mark and each one shows what the journey claims.
4. **Verify.** `cargo xtask verify` is green.
5. **Proof.** Open the PR with its proof (see "Pull requests").

How to drive the app headless (snapshots, element trees, scripts, video): `.claude/skills/drive-app/SKILL.md`.

## Look and feel

- `gxm-app/src/theme.rs` holds every colour, shadow, radius and type style, by token name; `ui.rs` holds the components (buttons, fields, segmented controls, badges, tiles). New UI composes these. A new look starts as a token in `theme.rs`, never a literal in a view.
- Visual feedback goes straight into code. Show the result as before-and-after renders.
- Rune details that bite: Rune paints later shadows on top (the reverse of CSS); a border takes layout space only with `.border_layout()`; `.paint_order(n)` is `z-index`; text is drawn with `FontSmoothing::Off`.

## Journeys: write fewer, longer tests

Follow Kent C. Dodds' [Write fewer, longer tests](https://kentcdodds.com/blog/write-fewer-longer-tests):

- A journey is a manual tester's script: one per workflow, with expectations after every step. When the person's goal is the same, add steps to the existing journey.
- Each journey builds its own world inline. Helpers return values; state lives in the journey.
- Git is always real, inside a world. Remotes are local bare repositories behind real URLs (`insteadOf` in the world's `.gitconfig`). Fakes belong only at the network edge (provider HTTP APIs).
- Target controls by stable keys from `gxm_app::keys`, and content by the text a person reads.
- Unit tests are for pure algorithms with crisp inputs and outputs (graph layout, parsers, dates), in a `#[cfg(test)]` module beside the code.
- A journey that passes on its first run is unproven. Break one expectation, watch it go red, then restore it.
- A failure names the step and prints the screen as text. Keep the runner's messages that useful.

## Determinism

- Time, home, config paths and git's environment come only from `gxm_core::env::Env`. `clippy.toml` enforces this.
- Worlds use fixed authors and dates, so commit ids are stable across runs and machines, and goldens rely on that.
- Goldens change only through `cargo xtask bless`. Every changed golden appears in the PR with the reason for the change.

## Architecture

- Behaviour with phases (network operations, wizard steps, loading) is a state machine: one `enum State`, and one `step` function that returns effects (`rune::machine`).
- Slow work runs off the UI thread (`rune::Latest`). Nothing is read before the first frame.
- The UI shows only what works. An unfinished control is disabled, with a tooltip saying what's coming.
- Tests and worlds read only their sandbox. The real `~/.ssh` and `~/.gitconfig` belong to the person.

## Garden

Slop grows faster than weeds, so tend as you go. `cargo xtask verify` checks all of this:

- Keep files under 600 non-test lines (1000 is the hard cap), split by capability.
- Delete dead code in the same PR that orphans it.
- Silence a lint with `#[expect(lint, reason = "…")]`.
- A TODO carries its issue: `// TODO(#123): …`.
- Comments say why; names say what.

## Pull requests

Every PR carries **proof** that the change works, using `.github/pull_request_template.md`:

- **Video** for any user-visible change: `cargo xtask journey <name> --video`. Embed the GIF inline and attach the mp4.
- **Screenshots**: the journey's mark PNGs. For a visual change, show before and after (render the base branch too).
- **Commands run**, with their results, `cargo xtask verify` included.
- **Goldens**: each changed golden and why it changed.

A PR without proof isn't ready for review. CI re-runs every journey on its own runner and attaches the renders it made.
