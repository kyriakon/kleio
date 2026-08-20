# gix fetch/push/clone over SSH — default subprocess transport

> Reference material, not authoritative. Raw investigation notes for the Phase 1
> `kleio-git` spike ("basic `gix` fetch/push/clone over SSH using the default subprocess
> transport"). Every claim cited to a primary source; verify before building.

## TL;DR

- `gix` (gitoxide) connects over SSH by **spawning the system `ssh` binary as a subprocess** — confirmed at source level.
- **Clone and fetch over SSH work.** **Push does not exist in gix** — the `push` feature is unimplemented (`[ ] push`).
- The subprocess approach's known vulnerability (RUSTSEC-2024-0335) is fixed in `gix >= 0.62.0`; current stable `gix` is `0.86.0`, well clear.
- **Blocking finding:** the proposal assumes `gix` can fetch *and push* over SSH. It cannot push. This invalidates the "fetch/push/clone" spike item as scoped.

---

## 1. SSH transport is a subprocess of the system `ssh`

`gix-transport`'s blocking connector dispatches `ssh://` URLs to a dedicated ssh
module, which builds a `std::process::Command` — a subprocess — not an in-process
SSH client.

- Dispatch: `gix_url::Scheme::Ssh => crate::client::blocking_io::ssh::connect(...)`
  — [gix-transport/src/client/blocking_io/connect.rs](https://github.com/GitoxideLabs/gitoxide/blob/main/gix-transport/src/client/blocking_io/connect.rs).
- The ssh connector resolves the program via `Options::ssh_command()`, which
  **defaults to `ssh`** (`ssh.exe` on Windows), builds a `SpawnProcessOnDemand::new_ssh(...)`
  (a `std::process::Command`), and runs a `-G` feature-probe command against it.
  Built-in program variants: `Ssh`, `Plink`, `Putty`, `TortoisePlink`, `Simple`.
  — [gix-transport/src/client/blocking_io/ssh/mod.rs](https://github.com/GitoxideLabs/gitoxide/blob/main/gix-transport/src/client/blocking_io/ssh/mod.rs).
- The ssh program is overridable via `connect::Options.command` (an `OsString`);
  the higher-level `gix` layer maps git config/env (`core.sshCommand`, `GIT_SSH`,
  `GIT_SSH_COMMAND`, `GIT_SSH_VARIANT`) into this — [ssh/mod.rs `connect::Options`](https://github.com/GitoxideLabs/gitoxide/blob/main/gix-transport/src/client/blocking_io/ssh/mod.rs).

Confirmation that this is a real subprocess of an *external* program (not a Rust
SSH implementation) is also in the security advisory, which describes the attack
as smuggling arguments into "the external `ssh` command" — see §4.

## 2. Connection API and feature flags

Two layers:

- **Plumbing:** `gix_transport::client::connect()` (blocking) / the async
  equivalent — returns a `Box<dyn Transport>`. `gix_url::parse()` produces the
  structured URL.
- **High-level (what kleio-git should use):** `gix::open()` a repo, then
  `repo.remote()` / `repo.remote_at(url)`, and `Remote::connect(Direction)` →
  `Connection`. Clone via `gix::prepare_clone`.

Feature flags on the `gix` meta-crate (55 flags, 30 default) — [docs.rs features page](https://docs.rs/crate/gix/latest/features):

- **`blocking-network-client`** — pulls in `gix-transport` + `gix-protocol`; this is what enables SSH (and the git:// daemon) transport. Required.
- **`blocking-http-transport-curl`** or **`blocking-http-transport-reqwest-*`** — needed only for `https://` remotes; not needed for SSH.
- **`async-network-client`** / **`async-network-client-async-std`** — async transport; not needed for a blocking sync loop.

There is **no `ssh` feature flag**: the subprocess SSH transport is compiled
unconditionally into `gix-transport` (it just needs an `ssh` binary on PATH at
runtime). That matches the proposal's desktop-only assumption.

## 3. Clone and fetch: supported

From the gitoxide README's own feature list — [README.md](https://github.com/GitoxideLabs/gitoxide/blob/main/README.md):

```
* [x] clone
* [x] fetch
* [ ] push
```

- **Clone:** `gix::prepare_clone(url)` → `clone::PrepareFetch` → `.receive()`.
- **Fetch:** `Remote::connect(Direction::Fetch)` → `Connection::prepare_fetch(...)` —
  [docs.rs `Remote`](https://docs.rs/gix/latest/gix/struct.Remote.html),
  [docs.rs `Connection`](https://docs.rs/gix/latest/gix/remote/struct.Connection.html).

## 4. Push: NOT implemented — the blocking risk

`push` is unchecked in the README feature list (`[ ] push`). The high-level API has
**no push operation**:

- `Remote` exposes push *configuration* — `push_url()`, `with_push_url()`,
  `Direction::Push`, refspecs for both directions — but no `push()` method —
  [docs.rs `Remote`](https://docs.rs/gix/latest/gix/struct.Remote.html).
- `Connection`'s methods stop at `ref_map()` and `prepare_fetch()`; there is no
  `send_pack`/`push` — [docs.rs `Connection`](https://docs.rs/gix/latest/gix/remote/struct.Connection.html).

The send-pack protocol (git's push) is not implemented in gitoxide. Push config
plumbing (`pushUrl`, `push.default`, `branch@{push}`) exists, but the wire
operation does not.

**Consequence for Kleio:** the Phase 1 spike item ("`gix` fetch/**push**/clone over
SSH using the default subprocess transport") cannot be delivered as written.
`gix` covers fetch and clone; push must come from elsewhere (see §6).

## 5. Security: RUSTSEC-2024-0335

- **Advisory:** [RUSTSEC-2024-0335](https://rustsec.org/advisories/RUSTSEC-2024-0335.html) —
  "gix-transport indirect code execution via malicious username"
  (CVE-2024-32884, GHSA-98p4-xjmm-8mfh).
- **Nature:** a malicious `ssh://-Fconfigfile@host/…` URL smuggles ssh options
  through the username, which is passed verbatim-ish to the spawned `ssh`.
  Exploitable when an attacker can place a file (e.g. `configfile@example.com`)
  in the process's CWD.
- **Fix:** `gix-transport >= 0.42.0`; first `gix` crate with the fix **0.62.0**;
  first `gix` CLI **v0.35.0** (all per the advisory).
- **Kleio minimum:** any `gix >= 0.62.0` is safe. Current stable is **0.86.0**, so
  pinning current is fine. Still, treat clone/fetch URLs as untrusted input —
  the advisory's exploit class is "user pasted a malicious URL", which a password
  manager's "add remote" flow can encounter.

## 6. Recommendation

- **Pin `gix = "0.86"`** (current stable, 2026-07-23) with features
  `blocking-network-client` (+ `blocking-http-transport-curl` if https remotes are
  wanted). Subprocess SSH works with no extra feature.
- **Fetch + clone over SSH:** unblocked — proceed.
- **Push over SSH:** not available in gix. Options to decide in the spike:
  1. Shell out to the `git` CLI for push only (pragmatic, keeps everything else in gix).
  2. Use `git2` (libgit2) for push — different stack, C dependency, contradicts the
     "no C dependency" rationale in §6.2 of the proposal.
  3. Re-scope MVP to pull-only sync (clone + fetch), defer push.
  4. Implement send-pack against `gix-protocol` ourselves — real, open-ended work;
     not a spike.

## Open questions

- Does MVP sync actually require push, or is one-way (pull) sync acceptable for v1?
- Which push path (git CLI vs git2 vs defer) fits the "no C dependency, pure Rust" constraint in the proposal §6.2?
- Confirm `gix` honors `GIT_SSH`/`GIT_SSH_COMMAND`/`core.sshCommand` at the high level (source shows `Options.command`; the env/config mapping lives in the `gix` layer and is worth a 10-line verification before relying on it).
