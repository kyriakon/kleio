# Git sync over SSH — alternatives viable for mobile

> Reference material, not authoritative. Follow-on to
> [`gix-ssh-subprocess-transport.md`](./gix-ssh-subprocess-transport.md): `gix`
> cannot push, and its SSH transport shells out to a system `ssh` binary that
> mobile has no access to. This note surveys what *does* do push over SSH and
> what survives on mobile. Every claim cited to a primary source.

## TL;DR

- **There is no pure-Rust, off-the-shelf crate that does fetch + push + clone
  over SSH and is mobile-ready today.**
- **`git2` (libgit2)** is the *only* complete option — fetch/push/clone over SSH,
  in-process SSH (no system `ssh` binary). But it is a C dependency, so mobile is
  a cross-compile risk, not a given.
- **`russh`** is pure Rust and mobile-native, but it is SSH *transport only* — no
  git protocol. And `gix-protocol` (the layer the proposal pairs russh with) has
  **no send-pack**, so russh + gix-protocol still cannot push.
- The proposal's §6.2 custom-transport plan (russh + gix-protocol) covers fetch
  and clone, **but not push**, because the missing piece is the send-pack
  protocol itself, not the SSH transport.

---

## 1. `git2` / libgit2 — the only complete option

`git2` 0.21.0 (Rust bindings to libgit2) exposes full remote operations
including push:

- `Remote::push<Str>(&mut self, refspecs, opts) -> Result<()>` — "Perform a push"
- `Remote::fetch(...)`, `Remote::download(...)`, `Remote::connect(dir)`,
  `Remote::pushurl(...)` — [docs.rs `git2::Remote`](https://docs.rs/git2/latest/git2/struct.Remote.html).

SSH is in-process, not a subprocess:

- Feature `ssh` → `libgit2-sys/ssh` (libgit2's bundled **libssh2** transport) —
  [docs.rs git2 features](https://docs.rs/crate/git2/latest/features).
- `vendored-libgit2` / `vendored-openssl` build libgit2 and OpenSSL from source —
  no reliance on a system `ssh` binary, so mobile *can* link it.

**Mobile caveat (the risk):** libgit2 + libssh2 + OpenSSL are C, built via CMake.
Cross-compiling for iOS (Xcode/CMake) and Android (NDK/CMake) is the friction
point — real, but solved elsewhere in the ecosystem, not something to assert as
working without a spike. This is the trade the proposal's "pure Rust, no C
dependency" stance was explicitly trying to avoid (proposal §6.2).

## 2. `russh` — pure Rust, but SSH transport only

`russh` describes itself as a "Low-level Tokio SSH2 client and server
implementation" — pure Rust, async, crypto backends `aws-lc-rs` or `ring`, no C
dependency — [russh README](https://github.com/warp-tech/russh/blob/main/README.md).

Its surface is SSH primitives: authenticated channels, `exec`, port forwarding,
SFTP (via `russh-sftp`). It implements **no git protocol** — no ls-refs, no
fetch, no send-pack. It is exactly the "custom `Transport` backing" the proposal
envisages, but a transport alone does not produce a push.

## 3. The proposal's blind spot: `gix-protocol` has no push either

The proposal §6.2 planned `russh` (SSH) + `gix-protocol` (git protocol) for
mobile, reasoning that everything above `gix-transport` is protocol-agnostic.
That is true for **fetch/clone**, but push is absent at the protocol layer too:

- `gix-protocol` source contains only `fetch/`, `ls_refs.rs`, `handshake/`,
  `command.rs`, `remote_progress.rs` — **no `push` / `send-pack` module** —
  [gix-protocol source tree](https://github.com/GitoxideLabs/gitoxide/tree/main/gix-protocol/src).
- Confirmed at the crate level by gitoxide's own feature list: `[ ] push` —
  [README.md](https://github.com/GitoxideLabs/gitoxide/blob/main/README.md).

So swapping `russh` in for the subprocess SSH transport does **not** unlock push:
the missing piece is the send-pack *protocol implementation*, which gitoxide has
not written regardless of transport.

## 4. The landscape, summarized

| Path | Fetch | Clone | Push | SSH | Mobile |
|---|---|---|---|---|---|
| `gix` (subprocess ssh) | ✅ | ✅ | ❌ | system `ssh` | ❌ (no binary) |
| `gix` + `russh` transport (proposal §6.2) | ✅ | ✅ | ❌ | in-process | ✅ (fetch/clone only) |
| `git2` (libgit2 + libssh2) | ✅ | ✅ | ✅ | in-process | ⚠️ C cross-compile risk |
| `russh` alone | — | — | — | in-process | ✅ (SSH only, no git) |
| shell to `git` CLI | ✅ | ✅ | ✅ | system `ssh` | ❌ (no binary) |

There is no fourth, mature, pure-Rust git implementation with push: `gitoxide`
is *the* pure-Rust git, and it is fetch/clone-only.

## 5. Options to decide

1. **`git2` for the whole sync layer** (drop gix). One stack, full fetch/push/
   clone over SSH in-process. Cost: C dependency (libgit2 + libssh2 + OpenSSL),
   violating §6.2's "pure Rust, no C" rationale; mobile needs a cross-compile
   spike to de-risk.
2. **Desktop MVP on `git2` for push + `gix` for fetch/clone.** Two stacks,
   two SSH implementations — unnecessary complexity; rule out unless a concrete
   reason forces it.
3. **Stay pure-Rust, accept pull-only**: `russh` + `gix-protocol` gives
   fetch/clone over SSH on mobile today. Push is deferred until either gitoxide
   ships send-pack or Kleio implements it.
4. **Implement send-pack on `russh` + `gix-protocol`.** Pure Rust, mobile-native,
   but open-ended protocol work (pack negotiation, report-status, atomic push,
   force-update handling) — not a spike, a project.

## Open questions

- Does MVP sync actually require push (two-way), or is pull + a "sync is one-way
  for now" posture acceptable for v1? This is the same decision surfaced in the
  gix note, and it gates everything here.
- If push is required on mobile eventually, is the C-dependency trade (git2)
  acceptable, or is a pure-Rust send-pack worth the effort? This is an
  architecture decision, not a spike finding — likely an ADR.
- Prototype to confirm git2 cross-compiles for iOS/Android under Tauri v2's build
  pipeline (CMake toolchain + vendored libgit2/libssh2/openssl) before committing.
