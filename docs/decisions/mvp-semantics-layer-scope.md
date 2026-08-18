# MVP semantics-layer scope for kleio-crypto

The pure-rPGP architecture is accepted (see `pure-rpgp-architecture.md`); rPGP parses layer-4 data but every semantics *decision* is hand-built by kleio-crypto. This ADR locks what the semantics layer must own for the desktop MVP (single store) versus what defers with signer lifecycle (post-MVP).

**MVP — kleio-crypto owns:**
- **Encrypt-side recipient validation, full.** Before encrypting to a `.gpg-id` recipient: revoked? → don't encrypt to it; expired? → don't encrypt to it; key-flag-valid encryption-subkey selection feeds recipient resolution (which subkeys receive the session key). All three checks are hand-built (~14 lines, spike measurement).
- **Ungated decrypt.** Open any store entry regardless of recipient key state — real pass stores legitimately contain entries encrypted to keys that were later revoked, expired, or removed, and `pass`/gpg decrypt those. This is the file-format interop commitment.

**Deferred with signer lifecycle (post-MVP, proposal §7):**
- Decrypt-side warnings and removed-signer detection (security center §5.7) — their real value needs `.gpg-id` history, which only exists once lifecycle management lands; without it, warnings would be noise about old expired keys.
- Signer lifecycle management itself (add/remove/re-key).

**Decided out:**
- Algorithm-preference signaling — cipher is fixed SEIPD v1 + AES-256 (architecture ADR).

**Status**: accepted.

**Consequences**: kleio-crypto's public surface is `encrypt_to_recipients` (validates), `decrypt` (ungated), and internal semantics checks; recipient resolution picks valid subkeys among the current `.gpg-id` set — the deferred part is *managing* that set, not *using* it. Elgamal-encrypted entries fail decrypt with a clear non-technical error (known issue, architecture ADR).
