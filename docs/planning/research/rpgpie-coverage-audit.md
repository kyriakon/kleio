# rpgpie coverage audit

Research for ticket #19 (rPGP semantics-layer spike). Answers: what the `rpgpie` crate
provides for the OpenPGP layer-4 semantics Kleio needs, how mature it is, what it leaves to
the application, and how it relates to rPGP.

> **Correction to ticket:** the ticket guessed `github.com/sts10/rpgpie`. The crate's
> canonical repository is **`https://codeberg.org/heiko/rpgpie`** (source of truth per the
> `repository` field in `Cargo.toml:11` and the crates.io metadata). There is no
> `sts10/rpgpie` repository for this crate.

## Sources (primary)

- Crate source: `https://codeberg.org/heiko/rpgpie` (branch `main`, commit
  `f35e239881596deb3fac3098a02ff51cdd641a3d`)
- `Cargo.toml`, `Cargo.lock`, `README.md` — codeberg raw, branch `main`
- API docs: `https://docs.rs/rpgpie/latest/rpgpie/` (renders `0.11.0`)
- crates.io metadata: `https://crates.io/crates/rpgpie`

Unless noted, file/line citations below are against the `main` branch source at the commit
above. Raw-file reads do not show line numbers, so line numbers were recovered by grepping
the same raw files.

## Version and provenance

- Latest release **0.11.0** (2026-07-07), license **MIT OR Apache-2.0**
  (`Cargo.toml:7-8`; crates.io "Recent Versions").
- crate description: `"Experimental high level API for rPGP"` (`Cargo.toml:6`).
- 66K total / 5.6K recent downloads (crates.io).

## Short answer

rpgpie **does** implement the four semantics-layer concerns Kleio cares about — expiration,
revocation, key flags, and algorithm preference — but as a **hardcoded, read-only, and
explicitly unstable** policy layer on top of rPGP. It is a real, working implementation of
layer 4, not a stub, and it already backs the `rsop` SOP tool. The gaps for Kleio are not
"missing expiration/revocation logic" but rather: (a) policy is not configurable per
application, (b) certificate/TSK *mutation* (re-key, revoke, add/remove user ID, change key
flags) is out of scope, and (c) the API is self-described as incomplete and unstable.

---

## Coverage table

| Concern | rpgpie provides? | API (public unless marked) | Gap |
|---|---|---|---|
| Expiration | **Yes** | `Checked::primary_valid_at(reference) -> Result<bool, Error>`; `Checked::primary_expiration_time` (private); subkey expiry inside `is_subkey_valid_at` (private); `SignatureVerifier::temporal_validity` (private); `signature::signature_validity_expiration` (private) | Encryption-key selection has no historical-reference variant — `valid_encryption_capable_component_keys()` always uses "now", so decrypting an old message with a then-valid-but-now-expired key is not supported out of the box |
| Revocation | **Yes** (hard + soft) | `Checked::revoked_at(reference) -> bool`; subkey revocation inside `is_subkey_valid_at` (private); `signature::is_revocation` / `is_hard_revocation` (private) | Third-party revocations/certifications are dropped from the `Checked` view; no simple "whole cert is hard-revoked" flag (open TODO); only self-revocations are evaluated |
| Key flags | **Yes** | `Checked::valid_encryption_capable_component_keys()`, `valid_signing_capable_component_keys_at(reference)`, `valid_authentication_capable_component_keys(reference)`; internals `key_flags_at` / `is_*_capable` (private) | Read-only: cannot set/modify key flags (cert/TSK mutation out of scope). Signing-capable selection additionally requires a valid back-signature |
| Algorithm preference | **Yes** (partial) | `Checked::preferred_symmetric_key_algo`, `preferred_aead_algo`, `preferred_hash_algo`, `features` (read cert prefs); `policy::PREFERRED_*` constants; negotiation in `message::encrypt` | Policy is hardcoded constants, not app-configurable; acceptance is timestamp-coupled (see risk below); `PREFERRED_COMPRESSION_ALGORITHMS` is empty |

---

## Concern-by-concern findings

### 1. Expiration — implemented

- Primary-key validity folds in expiration: `primary_valid_at` rejects the key if
  `primary_expiration_time(reference) < reference` (`src/checked.rs:370`,
  `src/checked.rs:369-406`). Expiration is read from the active self-signature's
  `key_expiration_time` subpacket (`primary_expiration_duration`, `src/checked.rs:353-367`).
- Subkey expiration is enforced in `is_subkey_valid_at`: an active binding whose
  `key_expiration_time` plus subkey creation time is before the reference is invalid
  (`src/key.rs:633-669`).
- Signature-level temporal validity (not-before-key-creation, not-from-the-future) in
  `SignatureVerifier::temporal_validity` (`src/certificate.rs`); signature expiry computed in
  `signature_validity_expiration` (`src/signature.rs:149`).

**Gap:** `valid_encryption_capable_component_keys()` hardcodes `Timestamp::now()` and has no
`_at` sibling (`src/checked.rs:500-511`). The test suite confirms this is an acknowledged
missing capability: *"TODO: if we had a [valid_encryption_capable_component_keys_at] fn, we
could test for historical validity"* (`src/checked.rs`, test module, `test_dsa`).

### 2. Revocation — implemented (hard vs. soft distinction)

- `Checked::revoked_at(reference)` returns whether the certificate is revoked, with the doc
  note *"This takes into account the semantics of hard and soft revocation"*
  (`src/checked.rs:417-425`).
- Revocation classification: `is_revocation` matches `KeyRevocation` / `CertRevocation` /
  `SubkeyRevocation` signature types (`src/signature.rs:33-38`); soft reasons are
  `KeyRetired`, `CertUserIdInvalid`, `KeySuperseded`; everything else is hard
  (`src/signature.rs:43-63`).
- The active-signature resolver `SigStack::active_at` returns a hard revocation first, else
  the latest soft revocation, else the latest regular signature
  (`src/signature.rs:216-300`).

**Gaps:**
- `Checked::new` carries an explicit TODO: *"go look for any (valid/correct) hard revocations
  for the full cert, and encode the result in a simple 'cert is hard revoked' flag"*
  (`src/checked.rs:71-74`). The flag is not implemented; revocation is instead inferred
  through `active_certificate_self_signature_at`.
- Third-party signatures are stripped: *"The checked representation currently removes all
  third-party signatures (because they can't be cryptographically checked in the context of
  looking at an individual certificate)"* (`src/checked.rs:32-35`). So a revocation issued by
  a designated revoker or web-of-trust third party is not evaluated.

### 3. Key flags — implemented (read-only)

- Key flags are extracted from the hashed subpackets of the active binding signature:
  `key_flags_at(reference)` (`src/key.rs:673-696`), with capability predicates
  `is_encryption_capable` (`encrypt_comms || encrypt_storage`), `is_signing_capable`
  (`sign`), `is_authentication_capable` (`authentication`) (`src/key.rs:699-719`).
- Public selection APIs on `Checked`:
  - `valid_encryption_capable_component_keys()` (`src/checked.rs:500`)
  - `valid_signing_capable_component_keys_at(reference)` — additionally requires a valid
    embedded back-signature (`src/checked.rs:517`, `has_valid_backsig_at`
    `src/key.rs:576-631`)
  - `valid_authentication_capable_component_keys(reference)` (`src/checked.rs:534`)
- There are also deliberately *lenient* variants on `Certificate`
  (`validation_capable_component_keys`, `decryption_capable_component_keys`) that check only
  key flags, not validity, and are documented as "very lenient"
  (`src/certificate.rs`).

**Gap:** all key-flag logic is read-only. Setting flags is part of certificate/TSK mutation,
which is explicitly out of scope (see § Maturity / gaps).

### 4. Algorithm preference — implemented, but hardcoded

- rpgpie ships its own fixed preference constants: `PREFERRED_SYMMETRIC_KEY_ALGORITHMS`
  (AES-256/192/128), `PREFERRED_AEAD_ALGORITHMS`, `PREFERRED_HASH_ALGORITHMS`,
  `PREFERRED_HASH_ALGORITHMS_V6`, `PREFERRED_COMPRESSION_ALGORITHMS` (empty),
  `PREFERRED_SEIPD_MECHANISMS` (`src/policy.rs:39-88`).
- Acceptance policy is timestamp-coupled to the *claimed* signature creation time: MD5 cut
  off 2010-01-01, SHA-1 (data) 2014-01-01, SHA-1 (structural) 2023-02-01, RSA < 2048-bit after
  2013-12-31, DSA after 2023-02-03 (`src/policy.rs:90-108`, `acceptable_pk_algorithm`
  `:111`, `acceptable_hash_algorithm` `:140`, `accept_for_signatures` `:203`).
- Recipient certificate preferences are *read* via `Checked::preferred_symmetric_key_algo`,
  `preferred_aead_algo`, `preferred_hash_algo`, `features` (`src/checked.rs:430-456`).
- Negotiation happens in `message::encrypt`: it intersects rpgpie's own defaults with each
  recipient's advertised preferences (`src/message.rs:337-390`), then picks SEIPDv1/v2 and a
  symmetric/AEAD algorithm from the intersection.

**Gaps:**
- Policy is a set of `pub const`s and `pub(crate)` functions — there is no policy object an
  application can configure or override. Kleio cannot relax or tighten rpgpie's choices
  without forking or upstreaming.
- The timestamp-coupling is an acknowledged tradeoff with an attack surface. From the module
  doc: *"the downside is that an attacker may trick users with weak, new (or newly modified)
  artifacts that show 'old' signature creation timestamps"* (`src/policy.rs:13-16`).
- `PREFERRED_COMPRESSION_ALGORITHMS` is empty (`src/policy.rs:85`); several constants carry
  `FIXME` markers (e.g. `AEAD_CHUNK_SIZE` "FIXME: what's a good default?", `Seipd` "FIXME:
  where should this go? -> upstream to rpgp?", `src/policy.rs:41,29`).

---

## Maturity (exact quotes)

rpgpie's own documentation is unambiguous that it is early-stage and that neither API
stability nor completeness is a current goal.

From `README.md`, section **"Warning, early-stage project!"**:

> rpgpie is in a relatively early stage of development. Use with caution!
>
> In particular:
>
> - Its interface will undergo regular changes, as development continues (the API is also
>   still decidedly incomplete).
> - OpenPGP semantics are notoriously underspecified. It's not (yet) fully clear what exact
>   semantics a library like this needs to implement to achieve the best user-facing outcomes.
> - Some of the implemented business logic is not currently optimized for efficiency, and may
>   be noticeably slower than it should be.

From `README.md`, section **"Limitations and non-objectives"**:

> API stability is not a focus in the current phase of development.
>
> Error handling is not currently well-elaborated.
>
> The rpgpie API currently limits itself to using certificates (also known as "OpenPGP public
> keys") and TSKs (also known as "OpenPGP secret/private keys") as they are. Updating or
> altering certificates or TSKs is currently out of scope.
>
> rpgpie does not currently process messages in an efficient, streaming manner. Messages that
> are too large to be conveniently processed in RAM can currently not be handled with rpgpie.

From the crate docs front page (`docs.rs/rpgpie/latest/rpgpie/`):

> rpgpie is an experimental higher level OpenPGP library based on rPGP.

From the `key` module doc (`src/key.rs:6-7`):

> NOTE: This module is particularly experimental, the API may change drastically.
> (This current implementation does a lot of cloning and is limited to read-only operations.)

Other maturity signals:

- `Error` is `#[non_exhaustive]` and coarse: `Rpgp`, `Io`, `Message(String)`, `InvalidPrimary`
  (`src/lib.rs:46-55`).
- Clippy lints treat `unimplemented`, `todo`, `unwrap`, `expect`, `panic` as warnings, not
  denied (`Cargo.toml`, `[lints.clippy]`).
- Numerous `FIXME`/`TODO` comments throughout the semantics paths (e.g. `src/checked.rs:71`,
  `:119` "TODO: store bad self-sigs", `src/policy.rs:29`, `src/message.rs`).

---

## What rpgpie leaves to the application (gaps Kleio would still fill)

1. **Certificate/TSK mutation.** Re-keying (change expiration, add/remove user IDs, update
   key flags), issuing a revocation, and adding/removing a subkey are out of scope — "Updating
   or altering certificates or TSKs is currently out of scope" (`README.md`). This is directly
   the signer-lifecycle work Kleio's proposal §5.6 requires (add/remove signer, rotation
   tracking). Key *generation* is covered (`Tsk::generate`, `src/tsk.rs:244`), but nothing
   that mutates an existing key.
2. **Application-configurable policy.** Algorithm acceptance and preference are hardcoded
   constants with no override hook (`src/policy.rs`). If Kleio wants stricter or different
   policy than rpgpie's built-in cutoffs, it must fork or maintain a policy layer itself.
3. **Third-party signatures / web of trust.** The `Checked` view drops all third-party
   signatures (`src/checked.rs:32-35`). Third-party certifications are only surfaced raw via
   `user_id_third_party_certifications` / `direct_third_party_certifications`
   (`src/checked.rs:568-612`) — no verification or trust model. Designated-revoker and
   web-of-trust semantics are unhandled.
4. **Historical key selection for decryption.** No `valid_*_at` for encryption keys; only
   `_at` variants exist for signing/authentication (`src/checked.rs:500`). Decrypting a
   message encrypted to a key that was valid at encryption time but has since expired/been
   rotated requires Kleio-side logic.
5. **Streaming / large messages.** `unpack`/`encrypt` operate on whole buffers; "Messages
   that are too large to be conveniently processed in RAM can currently not be handled with
   rpgpie" (`README.md`). Kleio entries are small, so this is a non-issue for pass-store
   compatibility but matters if attachments or large secrets are ever in scope.
6. **API-stability churn.** 0.x with rapid releases (0.8.2→0.11.0 between 2025-12 and
   2026-07, crates.io); "API stability is not a focus." Adopting rpgpie means tracking
   breaking changes.
7. **Error-handling granularity.** `Error` is non-exhaustive and coarse; error handling is
   "not currently well-elaborated" (`README.md`). Kleio's UX needs finer-grained failure
   reasons (e.g. distinguishing expired vs revoked vs no-binding) than rpgpie exposes today —
   note `primary_valid_at` collapses these into a single `Ok(false)`.

## Dependency relationship to rPGP

- Declared: `pgp = { version = "0.20", default-features = false }` (`Cargo.toml:28`). This is
  a semver *range* (`>=0.20.0, <0.21.0`), not an exact pin.
- Resolved in `Cargo.lock:1366`: `pgp` **0.20.0** (checksum
  `1cfa4743b28656065ff4c0ba09e46b357a65e8c00fc2341e89084b82f87cbdf1`).
- rpgpie exposes the build-time rPGP version: `pub const RPGP_VERSION: &str = pgp::VERSION`
  (`src/lib.rs:44`).
- Feature forwarding: `pqc = ["pgp/draft-pqc"]`, `wasm = ["pgp/wasm"]` (`Cargo.toml:22-24`).
  rpgpie uses `default-features = false` on rpgp and re-exposes the features it needs.
- Which rPGP APIs it builds on (from `use` blocks across the crate):
  - `pgp::composed` — the high-level composed object layer: `SignedPublicKey`,
    `SignedSecretKey`, `SignedPublicSubKey`, `SignedKeyDetails`, `Message`, `MessageBuilder`,
    `Encryption`, `TheRing`, `DecryptionOptions`, `ArmorOptions`, `CleartextSignedMessage`,
    `DetachedSignature`, `Deserializable`, `PublicOrSecret`, `RawSessionKey`,
    `SecretKeyParamsBuilder`, `SubkeyParamsBuilder`, `KeyType`, `EncryptionCaps`,
    `VerificationResult` (`src/message.rs`, `src/certificate.rs`, `src/tsk.rs`).
  - `pgp::packet` — `Signature`, `SignatureConfig`, `SignatureType`, `Subpacket`,
    `SubpacketData`, `KeyFlags`, `RevocationCode`, `UserId`, `Features`, `Packet`,
    `PacketTrait` (`src/key.rs`, `src/signature.rs`).
  - `pgp::types` — `Fingerprint`, `KeyId`, `KeyVersion`, `Timestamp`, `Duration`, `KeyDetails`,
    `PublicParams`, `Password`, `SigningKey`, `VerifyingKey`, `EncryptionKey`, `EskType`,
    `StringToKey`, `SecretParams`, `EncryptedSecretParams`, `PlainSessionKey`, `Esk`
    (widespread).
  - `pgp::crypto` — `AeadAlgorithm`, `ChunkSize`, `HashAlgorithm`, `PublicKeyAlgorithm`,
    `SymmetricKeyAlgorithm` (`src/policy.rs`, `src/message.rs`).
  - `pgp::ser::Serialize`, `pgp::armor`, `pgp::errors::Error`.

  In short: rpgpie is a *wrapping* layer over rpgp's `composed` (layer-3) API plus the
  `packet`/`types`/`crypto` primitives. It does not reimplement crypto or wire format; it
  adds the layer-4 validity/policy reasoning on top.

## Bottom line for Kleio

rpgpie is a credible, source-available implementation of the exact semantics layer the
proposal (§6.1) flags as the highest-priority open risk — it is not vaporware. But adopting it
would not eliminate Kleio's layer-4 work: the signer-lifecycle mutations Kleio needs are
explicitly out of scope, policy is not configurable, and the API is self-declared unstable.
The realistic choice is either (a) build `kleio-crypto`'s semantics on rpgp directly (copying
rpgpie's hard/soft-revocation and key-flag reasoning, which is the correct reference model),
or (b) vendor rpgpie and maintain a fork for the mutation + configurable-policy + historical
decryption gaps. Either way, the hard semantics rpgpie already got right — expiration,
hard-vs-soft revocation, key-flag gating, and timestamp-coupled algorithm policy — is the
specification Kleio should converge on, not re-derive.
