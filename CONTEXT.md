# Kleio

The domain glossary for Kleio, a cross-platform, `pass`-compatible password manager. This context is the single source of shared vocabulary; terms that affect code semantics are also mirrored in `AGENTS.md`.

## Language

**Re-keying**:
Removing a signer's key from `.gpg-id` and re-encrypting the store. Automatable and immediate.
_Avoid_: rotation, revocation

**Rotation**:
Actually changing a secret's value. Cannot be automated; must be tracked, because the removed signer already saw the plaintext and keeps their private key.
_Avoid_: re-keying

**Recoverable copy**:
The sync-conflict pattern: never silently discard either side of a conflicting edit — keep one live, preserve the other, flag it for review.
_Avoid_: losing side, merge result

**Semantics layer**:
OpenPGP layer 4 — expiration, revocation, key flags, algorithm-preference signaling. rPGP parses the data; Kleio's `kleio-crypto` hand-builds the decisions.
_Avoid_: policy layer

**Recipient validation**:
The encrypt-side semantics checks: a revoked or expired recipient is not encrypted to, and only key-flag-valid encryption subkeys receive session keys.
_Avoid_: recipient check, key validation

**Signer lifecycle**:
Managing who can decrypt the store — add/remove signer, re-keying, rotation tracking. Post-MVP; using the current `.gpg-id` set is MVP, managing it is not.
_Avoid_: key management
