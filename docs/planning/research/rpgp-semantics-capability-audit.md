# rPGP semantics-layer capability audit

**Ticket:** #18 — Research: rPGP semantics-layer capability audit
**Audited artifact:** `pgp` crate (rPGP) **v0.20.0**, commit `82aae1f` (HEAD at clone time).
**Primary sources:** rPGP source tree (`github.com/rpgp/rpgp`), rPGP README + `docs/` (`docs.rs/pgp` mirrors the crate), RFC 4880 (`rfc-editor.org/rfc/rfc4880`).

---

## TL;DR

rPGP explicitly implements OpenPGP **layers 1–3** (wire format, composite objects, crypto operations) and explicitly does **not** implement **layer 4** (semantics: expiration, revocation, key flags, algorithm preferences). Its README states this in so many words, and its `IMPL_STATUS.md` marks the "High Level API" as **"Not yet started"**.

However — and this is the crux of the audit — rPGP **does parse every relevant signature subpacket** into typed Rust values and exposes them through public accessor methods on `pgp::packet::Signature`. So the raw *data* for all four concerns is reachable. What rPGP does **not** provide is the *semantic layer*: no "is this key expired?", no "is this key/subkey/User ID revoked?", no "which self-signature is authoritative", no "is this revocation by an authorized revoker". Those decisions must be hand-built in `kleio-crypto` on top of the parsed subpackets.

**Bottom line:** all four concerns are **parse-level YES, semantics-level NO**. `kleio-crypto` gets the parsed subpacket data for free but must implement every layer-4 *decision* itself.

---

## Framing: rPGP's own position on layer 4

rPGP's README defines the four layers and states its boundary:

> 4. OpenPGP semantics (e.g.: *Expiration* and *Revocation* of Certificates and their components, *Key Flags* that define which semantical operations a given component key may be used for, signaling of algorithm preferences, ...)
>
> Analogous to the RFC, rPGP handles layers 1-3, but explicitly does not deal with 4.
> Applications that need OpenPGP semantics must implement them manually, or rely on additional libraries to deal with that layer.
> NOTE: The [`rpgpie`] library implements some of these high level OpenPGP semantics.

— `README.md:160-178` (`github.com/rpgp/rpgp/blob/main/README.md`)

`docs/IMPL_STATUS.md` confirms the high-level (semantic) API is unimplemented:

> ## High Level API
>
> Not yet started

— `docs/IMPL_STATUS.md:112-114`

The README also points to **`rpgpie`** ("An experimental OpenPGP semantics library", `README.md:58`) as the project that layers semantics on top of rPGP. This is worth evaluating as an alternative to hand-building in `kleio-crypto` (see "Recommendation" at the end) — but it is a separate crate, not part of `pgp`.

---

## Where the parsed data lives

The object graph of a parsed certificate:

- `pgp::composed::SignedPublicKey { primary_key, details, public_subkeys }` — `src/composed/signed_key/public.rs:32-36`
- `pgp::composed::SignedPublicSubKey { key, signatures }` — `src/composed/signed_key/public.rs:268-271`
- `pgp::composed::SignedKeyDetails { revocation_signatures, direct_signatures, users, user_attributes }` — `src/composed/signed_key/shared.rs:22-27`
- `pgp::types::SignedUser { id, signatures }` — `src/types/user.rs:16-19`

All four concerns surface as **`Signature` subpackets**, typed in `pgp::packet::SubpacketData` — `src/packet/signature/subpacket.rs:283-326` — and read via accessor methods on `pgp::packet::Signature`.

**Critical security-relevant detail:** every accessor below reads only the **hashed** subpacket area (`hashed_subpackets()`), i.e. the cryptographically signed area. Unhashed (attacker-mutable) subpackets are ignored by these getters. E.g. `key_expiration_time()` iterates `config().hashed_subpackets()` — `src/packet/signature/types.rs:843-850`.

---

## Per-concern findings

### 1. Key / subkey expiration

| concern | rPGP provides? | exact API | must hand-build |
|---|---|---|---|
| Read key expiration (v4/v6) | **Partial** — raw subpacket, parsed | `pgp::packet::Signature::key_expiration_time() -> Option<Duration>` — `src/packet/signature/types.rs:843-850`; subpacket `pgp::packet::SubpacketData::KeyExpirationTime(Duration)` — `src/packet/signature/subpacket.rs:288-289`; subpacket type `pgp::packet::SubpacketType::KeyExpirationTime` — `src/packet/signature/subpacket.rs:34` | absolute expiry + "is expired" |
| Read key creation time | **Yes** | `pgp::types::KeyDetails::created_at() -> Timestamp` (trait, implemented by `SignedPublicKey`/`SignedPublicSubKey`) — `src/types/key_traits.rs:17-35`, impl at `src/composed/signed_key/public.rs:181-183, 350-352` | — |
| Read subkey expiration | **Partial** — same subpacket, on the subkey binding signature | `Signature::key_expiration_time()` on `SignedPublicSubKey.signatures` — `src/composed/signed_key/public.rs:268-271` | which subkey sig is authoritative |
| v3 key expiration | **Yes** | `KeyDetails::legacy_v3_expiration_days() -> Option<u16>` — `src/types/key_traits.rs:32`; stored on `packet::PublicKey` — `src/packet/key/public.rs:260-261` | — (v3 is legacy; gpg pass stores use v4) |

**What the API actually returns:** `key_expiration_time()` returns `Some(Duration)` = **seconds after the key's creation time**, per RFC 4880 §5.2.3.6 "Key Expiration Time" (subpacket type 9). It is **not** an absolute `Timestamp` and rPGP has **no** `expires_at()` on any key type. To compute expiry: `expiry = key.created_at() + sig.key_expiration_time()`. A return of `None` *or* `Some(Duration(0))` both mean "no expiration" — the doc comment at `src/packet/signature/types.rs:835-842` states these are semantically equivalent.

**Must hand-build:**
- Adding the `Duration` to `created_at()` to obtain an absolute `Timestamp`.
- Deciding **which** self-signature's `KeyExpirationTime` is authoritative (a v4 key carries expiration on its primary-User-ID self-signature / subkey binding signature; rPGP does not select "latest valid self-signature" for you).
- The "expired as of *now*" comparison and any clock-skew policy.

---

### 2. Revocation (key, subkey, User ID)

| concern | rPGP provides? | exact API | must hand-build |
|---|---|---|---|
| Key revocation signatures, separated | **Partial** — parsed + grouped, not *evaluated* | `pgp::composed::SignedKeyDetails.revocation_signatures: Vec<Signature>` — `src/composed/signed_key/shared.rs:23` | "is key revoked" decision |
| Subkey revocation signatures, retained | **Partial** — parsed + retained | `SignedPublicSubKey.signatures` retains `SignatureType::SubkeyRevocation` alongside bindings — `src/composed/signed_key/public.rs:274-287` | "is subkey revoked" decision |
| Cert revocation (User ID) signatures | **Partial** — present among `users[].signatures` | `pgp::types::SignedUser.signatures: Vec<Signature>` — `src/types/user.rs:18`; signature types `CertRevocation = 0x30` — `src/packet/signature/types.rs:1214` | "is User ID revoked" decision |
| Verify a revocation signature | **Yes** | `Signature::verify_key(&key)` — `src/packet/signature/types.rs:763`; `SignatureType::KeyRevocation = 0x20` / `SubkeyRevocation = 0x28` — `src/packet/signature/types.rs:1198, 1205` | which revoker is authorized |
| Revocation reason | **Yes** (parsed) | `Signature::revocation_reason_code() -> Option<&RevocationCode>` / `revocation_reason_string() -> Option<&Bytes>` — `src/packet/signature/types.rs:950-966`; enum `pgp::packet::RevocationCode` — `src/packet/signature/types.rs:1651-1679` | hard/soft revocation policy |
| Revocation key (designated revoker) | **Yes** (parsed) | `Signature::revocation_key() -> Option<&RevocationKey>` — `src/packet/signature/types.rs:1037-1044`; `pgp::types::RevocationKey` — `src/types/revocation_key.rs:13-24` | check "revoker is authorized" |

**What rPGP does vs. does not:** the parser separates key-revocation signatures into `details.revocation_signatures` and keeps subkey/cert revocations in their component's `signatures` vec, and `Signature::verify_key()` / `verify_subkey_binding()` / `verify_certification()` can cryptographically validate each one. But there is **no** method returning "this key is revoked". The only bundled "check" is `SignedPublicKey::verify_bindings()` — `src/composed/signed_key/public.rs:116-123` — which calls the **private** `SignedKeyDetails::verify_revocation_signatures` (`src/composed/signed_key/shared.rs:84-93`) and **errors if any signature fails to verify**; it does not answer the semantic question "is there a valid revocation in effect".

RFC 4880 defines the revocation signature types and the "only by the key itself or an authorized revocation key" rule in §5.2.1 (0x20 key revocation, 0x28 subkey revocation, 0x30 certification revocation), and the "Reason for Revocation" subpacket in §5.2.3.23 (type 29). The "Revocation Key" (designated revoker) subpacket is §5.2.3.15 (type 12).

**Must hand-build:**
- Iterating revocation signatures, verifying each with `verify_key()`, and checking the signer matches the key itself **or** a `RevocationKey` designated-revoker (rPGP does not enforce this authorization — `RevocationKey` is just parsed data).
- Deciding revocation "wins": a revocation is invalid if a *later* self-signature supersedes it; rPGP has no such ordering logic.
- Hard-revocation (reason 1/2/3) vs. soft-revocation semantics and policy (e.g. a `KeySuperseded` revocation may still permit decryption of old pass entries).
- The same logic separately for primary key, each subkey, and each User ID.

---

### 3. Key-usage flags

| concern | rPGP provides? | exact API | must hand-build |
|---|---|---|---|
| Read key flags from a self-signature | **Partial** — parsed + bit accessors | `Signature::key_flags() -> KeyFlags` — `src/packet/signature/types.rs:930-939`; subpacket `SubpacketData::KeyFlags(KeyFlags)` — `src/packet/signature/subpacket.rs:300`; type `SubpacketType::KeyFlags` — `src/packet/signature/subpacket.rs:45` | authoritative self-sig selection |
| Individual flags | **Yes** | `KeyFlags::certify()` / `sign()` / `encrypt_comms()` / `encrypt_storage()` / `authentication()` / `shared()` / `group()` / `adsk()` / `timestamping()` — `src/packet/signature/types.rs:1348-1397` | operation→key mapping |
| Parse raw flags | **Yes** | `KeyFlags::try_from_reader(&mut reader)` — `src/packet/signature/types.rs:1272-1306` | — |
| Flag → allowed crypto operation | **No** | — | full semantic mapping |

RFC 4880 §5.2.3.21 "Key Flags" (subpacket type 27) defines the bits; rPGP's `KeyFlags` struct (`src/packet/signature/types.rs:1251-1258`) models the known bits and preserves unknown/extra bytes for round-tripping (`rest` + `original_len`).

**What rPGP does vs. does not:** `KeyFlags` is a fully populated typed bitfield with getters/setters. But it is returned by `Signature::key_flags()`, which requires the caller to first identify the correct self-signature (primary-key flags live on the primary-User-ID self-sig; subkey flags on the subkey binding signature). rPGP has no "given this operation (sign/encrypt/authenticate), which subkey should I use?" — that is exactly the layer-4 mapping the README disclaims.

**Must hand-build:**
- Selecting the authoritative self-signature per component before reading flags.
- The `certify/sign/encrypt/authenticate` flag → allowed-operation policy (e.g. which subkey to encrypt a pass entry to, which to use for decryption, which for signing).
- Handling multiple self-signatures with differing flags (rPGP's `SignedKeyDetails::as_unsigned()` just grabs the first primary-User-ID signature — an explicitly documented heuristic, `src/composed/signed_key/shared.rs:118-251`).

---

### 4. Algorithm preferences

| concern | rPGP provides? | exact API | must hand-build |
|---|---|---|---|
| Preferred symmetric algorithms | **Yes** (parsed) | `Signature::preferred_symmetric_algs() -> &[SymmetricKeyAlgorithm]` — `src/packet/signature/types.rs:875-884` | choose algorithm for outbound ops |
| Preferred hash algorithms | **Yes** (parsed) | `Signature::preferred_hash_algs() -> &[HashAlgorithm]` — `src/packet/signature/types.rs:897-906` | choose hash for signing |
| Preferred compression algorithms | **Yes** (parsed) | `Signature::preferred_compression_algs() -> &[CompressionAlgorithm]` — `src/packet/signature/types.rs:908-917` | choose compression |
| Preferred AEAD algorithms | **Yes** (parsed) | `Signature::preferred_aead_algs() -> &[(SymmetricKeyAlgorithm, AeadAlgorithm)]` — `src/packet/signature/types.rs:886-895` | — |

RFC 4880 defines these in §5.2.3.7 "Preferred Symmetric Algorithms" (type 11), §5.2.3.8 "Preferred Hash Algorithms" (type 21), §5.2.3.9 "Preferred Compression Algorithms" (type 22), with the *selection semantics* (how a sender should pick from the recipient's list) in §13.2 "Symmetric Algorithm Preferences" and §13.3 "Other Algorithm Preferences". rPGP parses the lists into typed arrays (`SubpacketData::PreferredSymmetricAlgorithms` etc. — `src/packet/signature/subpacket.rs:292-298`) but does not implement the selection logic from §13.2/§13.3.

**What rPGP does vs. does not:** the ordered preference lists are returned intact. rPGP does **not** offer "given a recipient key, pick the symmetric/hash/compression algorithm to use" — no intersection-with-sender-preference, no fallback ordering.

**Must hand-build:**
- Choosing the symmetric algorithm for encrypting a pass entry to a recipient key (intersect recipient's list with Kleio's own supported set, in recipient's stated order).
- Choosing hash algorithm for signing.
- Fallback behavior when lists are empty (RFC 4880 §13.2/§13.3 specify defaults — e.g. tripledes for symmetric if no preference).

---

## Exact API surface (consolidated reference)

All paths are public crate paths in `pgp` v0.20.0. Note the name collision: `pgp::types::KeyDetails` is a **trait** (metadata accessor); `pgp::composed::KeyDetails` is a **struct** (used only for *generating* keys).

**Types:**
- `pgp::composed::SignedPublicKey { primary_key, details, public_subkeys }`
- `pgp::composed::SignedPublicSubKey { key, signatures }`
- `pgp::composed::SignedKeyDetails { revocation_signatures, direct_signatures, users, user_attributes }`
- `pgp::types::SignedUser { id, signatures }`
- `pgp::packet::Signature` / `pgp::packet::SignatureType` / `pgp::packet::SignatureConfig`
- `pgp::packet::Subpacket` / `pgp::packet::SubpacketData` / `pgp::packet::SubpacketType`
- `pgp::packet::KeyFlags` / `pgp::packet::Features` / `pgp::packet::RevocationCode`
- `pgp::types::RevocationKey` / `pgp::types::RevocationKeyClass`
- `pgp::types::KeyDetails` (trait)

**Methods that matter (on `pgp::packet::Signature`):**

| Method | Returns | Source |
|---|---|---|
| `key_expiration_time()` | `Option<Duration>` (secs since creation) | `types.rs:843` |
| `signature_expiration_time()` | `Option<Duration>` | `types.rs:852` |
| `key_flags()` | `KeyFlags` | `types.rs:930` |
| `features()` | `Option<&Features>` | `types.rs:941` |
| `preferred_symmetric_algs()` | `&[SymmetricKeyAlgorithm]` | `types.rs:875` |
| `preferred_hash_algs()` | `&[HashAlgorithm]` | `types.rs:897` |
| `preferred_compression_algs()` | `&[CompressionAlgorithm]` | `types.rs:908` |
| `preferred_aead_algs()` | `&[(SymmetricKeyAlgorithm, AeadAlgorithm)]` | `types.rs:886` |
| `revocation_reason_code()` | `Option<&RevocationCode>` | `types.rs:950` |
| `revocation_reason_string()` | `Option<&Bytes>` | `types.rs:959` |
| `revocation_key()` | `Option<&RevocationKey>` | `types.rs:1037` |
| `is_primary()` | `bool` | `types.rs:968` |
| `is_revocable()` | `bool` | `types.rs:979` |
| `embedded_signature()` | `Option<&Signature>` | `types.rs:990` |
| `typ()` | `Option<SignatureType>` | `types.rs:358` |
| `verify_key(&key)` | `Result<()>` | `types.rs:763` |
| `verify_subkey_binding(&signer, &subkey)` | `Result<()>` | `types.rs:660` |
| `verify_primary_key_binding(&signer, &key)` | `Result<()>` | `types.rs:715` |
| `verify_certification(&key, tag, &id)` | `Result<()>` | `types.rs:538` |

Line references are to `src/packet/signature/types.rs` in the rPGP source tree.

---

## Consolidated: what `kleio-crypto` must hand-build

rPGP supplies parsed, typed subpackets. `kleio-crypto` must implement, on top of them, every **decision**:

1. **Expiration** — absolute expiry (`created_at() + key_expiration_time()`), authoritative self-signature selection, "expired as of now".
2. **Revocation** — for key/subkey/User ID: verify each revocation (`verify_key` etc.), enforce the "self or designated revoker" authorization rule (using `revocation_key()` + fingerprint comparison), supersession ordering, and hard/soft-revocation policy (`RevocationCode`).
3. **Key flags** — authoritative self-signature selection, then flag→allowed-operation mapping (which subkey to encrypt/decrypt/sign with).
4. **Algorithm preferences** — sender-side selection logic (RFC 4880 §13.2/§13.3) against the parsed preference lists.

**Also evaluate `rpgpie`** (`github.com/rpgp/rpgpie`, referenced at `README.md:58` as "An experimental OpenPGP semantics library"): it implements some of layer 4 on top of rPGP and powers `rsop` (a SOP CLI). Adopting it (or reading it as a reference for the exact semantics algorithms) is likely cheaper and more correct than reimplementing the revocation/expiration/flag logic from scratch — but it is a separate, "experimental" crate with its own maintenance risk, so the tradeoff is a separate decision, not part of this audit.
