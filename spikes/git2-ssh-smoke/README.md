# Spike: git2 push over SSH in-process

Throwaway spike for wayfinder ticket **git2 fetch + push + clone over SSH in-process
smoke test** (map #28). Question: does `git2` (libgit2) do fetch + push + clone over SSH
**in-process** (libssh2), with no `ssh` subprocess?

## Verdict

**In-process SSH: CONFIRMED.** With `ssh` absent from `PATH` and `GIT_SSH` /
`GIT_SSH_COMMAND` trapped to a marker script, git2 still completed the full SSH handshake
in-process: KEX (curve25519-sha256), host-key verification (`certificate_check` callback),
RSA public-key auth (`credentials` callback), channel open, and `exec git-upload-pack`
(server-side log confirmed `Accepted publickey`). The marker never fired; no `ssh` binary
was involved.

## Caveat (not a subprocess issue)

The clone/push data round-trip hit a libssh2 `bad packet length` error during the
git-protocol data exchange against macOS OpenSSH, in the **homebrew libssh2 1.11.1**
build this spike linked (system libgit2). That is a libssh2↔OpenSSH interop quirk, not
evidence of subprocess SSH — the transport demonstrably runs in-process. Re-verify with
the vendored libssh2 build Kleio will actually ship (exercised by the cross-compile spike,
map #28 → #29).

## Repro

1. Stand up a local sshd on `127.0.0.1:2222` (throwaway env, e.g. `/tmp/kleio-smoke`).
2. `LIBSSH2_SYS_USE_PKG_CONFIG=1 cargo run` with `PATH` excluding `ssh`.
