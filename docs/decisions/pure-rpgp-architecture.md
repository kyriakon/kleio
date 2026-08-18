# Pure-rPGP architecture for MVP, hybrid deferred to mobile phase

The project's central bet (proposal §1, §6.1) is replacing the GPG binary/agent with rPGP (`pgp` crate) so encryption, decryption, and key management run through one code path. rPGP implements OpenPGP layers 1–3 but not the layer-4 semantics (expiration, revocation, key flags); the spike and research (semantics capability audit, rpgpie audit, round-trip interop, semantics spike on real gpg keys) measured that gap: parsing is free, the semantics *decisions* are ~14 hand-built lines, and round-trip interop with real gpg is byte-identical. We decided: **kleio-crypto stays pure-rPGP for the desktop MVP** — one code path, hand-built semantics layer owned by kleio-crypto. The hybrid (system `gpg` on desktop / rPGP on mobile) is rejected for MVP; it becomes a trigger for the mobile phase, not an MVP bet.

**Status**: accepted.

**Considered options**

- **Hybrid (gpg shell-out on desktop, rPGP on mobile)** — rejected for MVP: mobile is post-MVP (proposal §7), so the rPGP path would be dead code for the entire MVP; two backends double the test matrix and bug surface in the de-risking phase; the process boundary and gpg-agent coupling is exactly what §1 rejected as fragile. `prs` and GpgFrontend both converged on gpg-default, but both are desktop-first with mature user bases — informative, not binding.
- **Sequoia** — already set aside (§6.1): mature backends are C libraries, reintroducing the cross-compilation burden.
- **rpgpie** — correct reference model but read-only and self-declared unstable; does not remove kleio-crypto's layer-4 work.

**Consequences**

- "Drop-in `pass` compatibility" means file-format interop with round-trip both ways — not gpg-identical runtime behavior. Semantics decisions are Kleio's own.
- No `CryptoBackend` trait now: kleio-crypto exposes a narrow module surface (encrypt/decrypt, the semantics checks). kleio-crypto is a leaf crate with few call sites (kleio-store entry encrypt/decrypt), so introducing a gpg backend later is a mechanical refactor behind that boundary.
- **Known issue**: Elgamal-encrypted entries cannot be decrypted — rPGP marks Elgamal "not planned". Rare, legacy; MVP fails such decrypts with a clear non-technical error.
- Encrypt must choose cipher explicitly: SEIPD v1 + AES-256 for universal gpg compatibility.
- The hybrid is not foreclosed: if the mobile phase measures rPGP as unable to carry it, the gpg backend is added then. This ADR records the MVP decision only.
