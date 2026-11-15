---
name: drive-app
description: Drive git-ex-machina headless to see a change working. Use when running, snapshotting or recording the app, writing or debugging a journey, reading the element tree, or checking a visual change.
---

# Driving git-ex-machina

The app runs headless with Rune's real renderer, on a fixed virtual clock, inside a **world** (a sandboxed home with fixture repositories). Every command below builds its world fresh, so runs are repeatable.

## See the app

- `cargo xtask snap [WORLD] [--theme light] [--size 1200x800]` writes `target/proof/snap-WORLD-THEME.png`, plus the element tree beside it as `.json`. Open the PNG with Read. The default world, `mockup`, is the demo workspace.
- To check a visual change, render the same moment before and after (`cargo xtask snap`, or a journey's marks on both branches) and compare them zoomed in (`ffmpeg -vf crop=W:H:X:Y,scale=iw*6:ih*6:flags=neighbor`). To find a layout offset, scan a pixel column of both renders for where colours change: rules and row edges land on exact pixels, so a one-point difference shows as a number.
- `cargo xtask app WORLD -- <rune flags>` passes any of Rune's flags through: `--snap`, `--tree`, `--script`, `--video`, `--at`, `--scene`, `--list-scenes`. Rune documents them in its README, under "Agent tooling". The app reads git off its UI thread in real time, so pass `--realtime` with `--at`, or the render can catch it before its reads land (`xtask snap` and `xtask journey` already do).
- `cargo xtask gxm WORLD -- repos | log atlas-web` shows the same data as text.

## Journeys

1. Add the steps to `crates/gxm-journeys/src/catalog.rs`. Steps are `tap(key)`, `tap_text`, `type_text`, `press`, `scroll_to(key)` (wheels until it's in view, as a person scrolls to something below the fold), `wait` and `mark`. Expectations are `see`, `see_in(key, text)`, `not_see`, `gone(key)` and `world_check`. Put a `mark` at every moment worth a screenshot.
2. A new journey also needs its `#[test]` in `crates/gxm-journeys/tests/journeys.rs`, and a world: a fixture in `gxm-world` or inline in the catalog.
3. `cargo test -p gxm-journeys` runs it. A failure names the step and prints the screen as text: `[key] "text"`, one element per line.
4. `cargo xtask journey NAME` writes `target/proof/NAME/NN-mark.png` and `.json` for each mark, plus `script.json`. Adding `--video` also writes the keycapped `NAME.mp4` and a GIF small enough to embed.

## The world's GitHub

Every world serves a fake GitHub GraphQL API on 127.0.0.1 (`gxm_world::hub`), and its `Env` carries the URL and token, so the app's real GitHub client talks to it over HTTP. Fixtures add pull requests (`fixtures/pulls.rs`: atlas-web's #131 open, #128 merged) pointing at the world's real commits. `Hub::of(w).reviews(owner, repo, n)` returns the reviews the app sent, for `world_check`. `cargo xtask gxm mockup -- prs atlas-web` lists them as text. A world built by xtask keeps serving only while xtask holds it, so keep the `World` alive until the app binary exits.

## Reading the element tree

The tree JSON (`--tree`, or the `.json` written next to each mark) lists every element with its `path` (keys joined by `/`), `role`, `key`, `text` and `rect` as `[x, y, w, h]` in points. Use it to answer layout questions exactly: an element with a 0-height `rect` collapsed, two rects that overlap clash. Then fix the cause and re-snap.

## Gotchas

- A text field's placeholder isn't in the element tree; a hint a journey must read is a text of its own (see the review summary in `view/cockpit_side.rs`).
- A Rune key can't contain `/`, because `/` separates key paths. `keys::repo` and `keys::changed_file` swap it for `:`.
- `.grow(1.0)` inside an auto-height column resolves to 0 pt. Give the column a bounded height and `.min_h(0.0)`.
- Marks pause the timeline briefly, so the next step doesn't leak into a mark's screenshot.
- Work off the UI thread (fetch, pull, push, the provider's API) runs in real time while the journey's clock is virtual: follow it with `wait_for(key, text)`, never a fixed `wait`. Every replay rebuilds the world, since a journey's git operations change it.
- `hold("shift")` / `hold("cmd")` before a tap makes it a ⇧- or ⌘-click; `hold("")` lets go.
