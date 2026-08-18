# Semantics-layer spike (issue #21)

Throwaway prototype answering: how much of the OpenPGP layer-4 semantics
(revocation / expiry / key-flag) does rPGP give for free, and how much must
`kleio-crypto` hand-build?

Runs against real `gpg`-generated keys (fresh / expired / revoked), synthetic
fixtures generated at runtime in a temp GNUPGHOME — nothing committed.

```sh
cargo run --manifest-path spike/semantics-spike/Cargo.toml
```

Requires a `gpg` binary on PATH.
