# Kleio glossary

Shared project vocabulary. Kept short and current; terms that affect code
semantics are also mirrored in `AGENTS.md`. See `docs/decisions/` for
decisions, `docs/planning/` for working material.

## Terms

- **Re-keying** — removing a signer's key from `.gpg-id` and re-encrypting the
  store. Automatable and immediate; commits nothing to the removed signer's
  future access. Never conflate with rotation.
- **Rotation** — actually changing a secret's value. Cannot be automated and
  must be tracked, because the removed signer already saw the plaintext and
  keeps their private key. A re-keyed-but-not-rotated entry is not secure
  against the removed signer.
- **Recoverable copy** — the sync-conflict pattern: never silently discard
  either side of a conflicting edit. Keep one live, preserve the other,
  flag it for review.
- **Semantics layer** — OpenPGP layer 4: expiration, revocation, key flags,
  algorithm-preference signaling. rPGP parses the data; Kleio's
  `kleio-crypto` hand-builds the decisions (see
  `docs/decisions/mvp-semantics-layer-scope.md`).
- **Recipient validation** — the encrypt-side semantics checks: a recipient
  that is revoked or expired is not encrypted to, and only key-flag-valid
  encryption subkeys receive session keys.
- **Signer lifecycle** — managing who can decrypt the store: add/remove
  signer, re-keying, rotation tracking. Post-MVP (proposal §7); recipient
  *resolution* (using the current `.gpg-id` set) is MVP, lifecycle
  *management* is not.
