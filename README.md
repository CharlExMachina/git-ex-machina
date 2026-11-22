<img width="1040" height="426" alt="Asset-2@2x" src="https://github.com/user-attachments/assets/7c6119f2-be00-4f33-b977-e90f444e8ae2" />


# Git Ex Machina

A clean, no-nonsense Git GUI client, plus a no-nonsense, simple-to-use CLI. It lets you manage your repos, review PRs, clone projects, push, pull, and do everything else you'd expect from a Git client, with world-class performance. It's stress-tested to handle your biggest repos.

For macOS and Linux.

![Git Ex Machina](docs/first-look.png)

## Install

On Linux, download `git-ex-machina-x86_64.AppImage` from the [latest release](https://github.com/CharlExMachina/git-ex-machina/releases/latest), make it executable and run it. Keep it somewhere you can write to, such as `~/Applications`: it checks for a newer version when it starts, offers it in the corner of the sidebar, and replaces itself once you take it. It needs `git` installed.

On a Mac with Apple silicon, download `git-ex-machina-aarch64.dmg` from the same release, open it, and drag git-ex-machina onto Applications. It's signed and notarized by Apple, so macOS opens it after asking once whether you meant to open an app from the internet. It uses the `git` that comes with Apple's command line tools (`xcode-select --install`). On the Mac, updating means downloading the new release yourself, for now.

Releases come from a `vX.Y.Z` tag: `.github/workflows/release.yml` builds the AppImage with `cargo xtask appimage` and the disk image with `cargo xtask macapp`, then publishes both with `docs/releases/vX.Y.Z.md` as their notes, which the app shows when it offers the update.

## Contributions

I'm not accepting contributions right now, except from people I've specifically authorized to contribute to this project. If you're thinking of contributing, don't, unless you're someone I've told is free to contribute. You know who you are.

I do accept issues. If they're usability bugs or the app not working right, I'll do my best to work on them. I also accept ideas in the Issues tab, if they fit my vision for this project. But if you open a pull request and you're not one of the people I've allowed to contribute to this repo, I'll close it right away.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option. The bundled fonts (Hanken Grotesk and Geist Mono) are under the SIL Open Font License; their licenses are in `crates/gxm-app/assets/fonts/`.
