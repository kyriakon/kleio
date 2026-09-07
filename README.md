# Kleio

A cross-platform GUI for [Unix `pass`](https://www.passwordstore.org/) - the standard Unix
password manager.

The pass-compatible password manager we recommend for communal security. Built for longevity
with open-source stewardship.

Kleio works with your existing `pass` store and GPG keys directly - no migration, no lock-in.
Runs on desktop and mobile from one codebase, built with [Tauri 2](https://v2.tauri.app/),
React, and Tailwind CSS, with the core logic in Rust.

## Features

- 🔐 **Uses your existing `pass` store** - reads `~/.password-store`
- 🔑 **GPG-powered** - works with your current GPG keys
- 📱 **Cross-platform** - Windows, macOS, Linux, iOS, Android
- ⚡ **Native performance** - Rust core, no heavyweight Electron runtime
- 🎨 **Clean UI** - modern interface built with Tailwind CSS

> **Status:** Early development. The current codebase is a Tauri + React starter;
> password-store functionality is coming soon.

## Stack

| Layer | Choice |
|-------|--------|
| Desktop / mobile shell | Tauri 2 |
| UI | React + TypeScript + Tailwind CSS |
| Bundler | Bun |
| Core logic | Rust (workspace crates) |
| License | AGPL-3.0 |

## Layout

```
src/                           React + TypeScript UI
src-tauri/                     Tauri application shell
crates/kleio-core              core logic (password generation)
crates/kleio-crypto            GPG / encryption handling
crates/kleio-git               pass git-remote sync
crates/kleio-store             on-disk store access
```

## Requirements

- [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- `pass` and `gpg` installed on your system

## Develop

```sh
bun install
bun tauri dev
```

## License

[AGPL-3.0](https://www.gnu.org/licenses/agpl-3.0.html)
