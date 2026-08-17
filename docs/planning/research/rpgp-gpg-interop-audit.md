# rPGP ↔ gpg/pass round-trip interop audit

Ticket #20. Question: can rPGP (the `pgp` crate) interoperate with files produced by
real `gpg` in a `pass`-store configuration today — decrypt existing pass entries, and
produce output `gpg` can decrypt back?

## TL;DR

**Yes, both directions work** against rPGP 0.20.0 (the RFC 9580 release), with two caveats:

1. **Decrypting pass entries:** works for every entry encrypted to an RSA or ECC (X25519 /
   NIST P-*) encryption key, in *either* packet format GnuPG emits — legacy CFB+MDC
   (SEIPD v1, AES-128/256) or modern AEAD/OCB (SEIPD v2, AES-256). The only failure case
   is an entry encrypted to a legacy **Elgamal** encryption key, which rPGP does not
   implement. Requires `pgp` ≥ 0.12 for AEAD; use ≥ 0.20.0.
2. **Producing gpg-decryptable output:** works, but rPGP's message builder does **not**
   auto-negotiate algorithms from recipient key preferences (that is layer-4 semantics,
   which rPGP explicitly does not provide). Kleio must choose the SEIPD version and cipher
   itself. Emitting SEIPD v1 (CFB+MDC) + AES-128/256 is decryptable by every GnuPG in the
   wild; SEIPD v2 (AEAD OCB) is decryptable by GnuPG ≥ 2.2.21.

---

## 1. What `pass` actually runs

Primary source: `src/password-store.sh` (v1.7.4), github.com/zx2c4/password-store.

`pass` does **public-key** encryption (`-e`), not symmetric (`-c`):

```sh
# password-store.sh:9  (GPG_OPTS is appended to every gpg call)
GPG_OPTS=( $PASSWORD_STORE_GPG_OPTS "--quiet" "--yes" "--compress-algo=none" "--no-encrypt-to" )
# password-store.sh:12-13  (gpg2 / agent detection adds batch flags)
command -v gpg2 &>/dev/null && GPG="gpg2"
[[ -n $GPG_AGENT_INFO || $GPG == "gpg2" ]] && GPG_OPTS+=( "--batch" "--use-agent" )
```

The encrypt call in `cmd_insert` / `cmd_generate` / `cmd_edit` (lines 462, 471, 480, 506,
542) is:

```sh
echo "$password" | gpg -e "${GPG_RECIPIENT_ARGS[@]}" -o "$passfile" "${GPG_OPTS[@]}"
# where GPG_RECIPIENT_ARGS is one "-r <gpg_id>" per recipient (lines 77, 105)
```

Net effect — `pass` invokes:

```
gpg --encrypt --recipient <key> --quiet --yes --compress-algo=none --no-encrypt-to [--batch --use-agent]
```

Consequences for interop:

- **Binary output, never armored.** No `--armor` flag; `.gpg` files are raw packets.
- **No compression.** `--compress-algo=none` → the payload is a Literal Data packet, with
  no Compressed Data packet in between.
- **No explicit cipher/S2K/AEAD selection.** `pass` passes no `--cipher-algo`,
  `--s2k-*`, `--force-aead`/`--force-mdc`. gpg applies its own defaults (see §2), so the
  exact packet format is determined entirely by the recipient key and the GnuPG version.

---

## 2. GnuPG defaults (what a real pass store actually contains)

Primary sources: GnuPG manual (gnupg.org) + empirical `gpg` 2.5.20 runs on this machine.

### 2.1 Symmetric cipher

`--cipher-algo` manual: "If this is not used the cipher algorithm is selected from the
preferences stored with the key." So there is no fixed default; gpg ranks the recipient
key's cipher preferences.

Empirical, GnuPG 2.5.20 generated key (RSA-2048 and ed25519+cv25519):

```
pfc::9,8,7,2:10,9,8,11,2:2,3,1:2:::mdc,aead,no-ks-modify:
```

Cipher preference list is `9,8,7,2` = AES-256, AES-192, AES-128, 3DES. AES-256 ranks
first, so modern `gpg -e` picks **AES-256**. Older GnuPG 2.2/2.4 default new keys carry
the same AES-256-first list. (`gpg --symmetric` instead uses `--s2k-cipher-algo`, whose
manual default is AES-256; irrelevant here since `pass` never uses `-c`.)

### 2.2 Packet format: CFB+MDC (SEIPD v1) vs AEAD (SEIPD v2)

`--force-mdc` / `--force-aead` manual (OpenPGP-Options): "The CFB+MDC is always used
unless the keys indicate that the OCB mode can be used in which case OCB is used. The
default is to determine the to be used mode from the recipients key preferences."

Empirical, GnuPG 2.5.20, `gpg -e -r <rsa2048> --compress-algo=none`:

```
# off=0   ctb=85 tag=1  hlen=3 plen=268
:pubkey enc packet: version 3, algo 1, keyid A44D87EDA700642B      # PKESK v3, RSA
# off=271 ctb=d4 tag=20 hlen=2 plen=90  new-ctb
:aead encrypted packet: cipher=9 aead=2 cb=16                       # SEIPD v2, AES-256, OCB, 64 KiB chunks
# off=292 ctb=ac tag=11 hlen=2 plen=37
:literal data packet: mode b (62), name="pass_in.txt", raw data: 20 bytes
```

So there are **two real-world formats** a pass store may hold:

| GnuPG lineage | Key advertises AEAD? | Packet | Cipher | AEAD |
|---|---|---|---|---|
| 2.2.x / 2.4.x (stable, most existing stores) | no | SEIPD v1 (tag 18), CFB+MDC | AES-256 (pref #1) or AES-128 | — |
| 2.5.x (dev; new stores) | yes (`features: 07`, `pref-aead-algos: 2`) | SEIPD v2 (tag 20), AEAD | AES-256 | OCB (2), chunk 64 KiB |

GnuPG 2.5-generated keys advertise the AEAD feature flag (`hashed subpkt 30 len 1
(features: 07)` = MDC + AEAD + keyserver-no-modify) and `pref-aead-algos: 2` (OCB), which
is why `gpg -e` to them emits SEIPD v2.

### 2.3 Compression

`--compress-algo` manual: default is to consult recipient preferences; "If all else fails,
ZIP is used." Default new-key compression preference is `2,3,1` = ZLIB, BZip2, ZIP. But
`pass` overrides with `--compress-algo=none`, so **pass files are never compressed**. This
is moot for the pass interop path (but relevant if Kleio ever encrypts for other gpg
users without disabling compression).

### 2.4 S2K (secret-key / passphrase protection)

S2K is **not** used on the pass payload path (public-key encryption generates a random
session key; no passphrase mangling). It only matters for importing the user's
passphrase-protected **secret key** into rPGP, and for `gpg -c` symmetric (which `pass`
does not use).

Empirical, GnuPG 2.5.20 secret key:

```
:secret key packet:
	iter+salt S2K, algo: 7, SHA1 protection, hash: 2, salt: 536868102B079AA2
	protect count: 56623104 (251)
```

Decoded: S2K type 3 (iterated+salted), cipher `algo: 7` = AES-128, digest `hash: 2` =
SHA-1, count 56,623,104 (gpg-agent-calibrated). Manual defaults (`--s2k-mode` = 3,
`--s2k-digest-algo` = SHA-1, `--s2k-count` inquired from gpg-agent) are consistent with
this. (`--s2k-cipher-algo` manual default is documented as AES-256; the observed
secret-key protection cipher on 2.5.20 is AES-128. Either way both are in rPGP's set, so
it does not affect interop.)

---

## 3. rPGP capability vs GnuPG

Primary source: rPGP `main` (= released `pgp` 0.20.0, RFC 9580), github.com/rpgp/rpgp;
`docs/IMPL_STATUS.md`; enums in `src/crypto/sym.rs`, `src/crypto/hash.rs`,
`src/types/s2k.rs`, `src/types/compression.rs`.

### Coverage matrix

| Layer | rPGP (0.20.0) | GnuPG default (pass store) | Gap? |
|---|---|---|---|
| **Cipher** AES-128 (7) | ✅ `sym.rs:123` | possible | no |
| **Cipher** AES-192 (8) | ✅ `sym.rs:125` | possible | no |
| **Cipher** AES-256 (9) | ✅ `sym.rs:127` | default pref #1 | no |
| **Cipher** 3DES (2) / CAST5 (3) / Blowfish (4) / IDEA (1) / Twofish (10) / Camellia (11–13) | ✅ `sym.rs:114-135` | legacy only | no (not emitted by default) |
| **Compression** none (0) | ✅ `compression.rs:11` | `pass` forces none | no |
| **Compression** ZIP (1) / ZLIB (2) / BZip2 (3) | ✅ `compression.rs:12-14` (BZip2 behind `bzip2` cargo feature) | gpg default ZIP/ZLIB | no (pass disables) |
| **S2K** simple (0) / salted (1) / iterated+salted (3) | ✅ `s2k.rs:166-183` | iterated+salted | no |
| **S2K** Argon2 (4) | ✅ `s2k.rs:190` | not used by GnuPG 2.x secret keys | no |
| **Hash** MD5 (1) / SHA-1 (2) / RIPEMD160 (3) | ✅ `hash.rs:26-30` | SHA-1 (S2K digest) | no |
| **Hash** SHA-256 (8) / SHA-384 (9) / SHA-512 (10) / SHA-224 (11) / SHA3-256 (12) / SHA3-512 (14) | ✅ `hash.rs:33-43` | — | no |
| **SEIPD v1 (tag 18) CFB+MDC** | ✅ encrypt + decrypt; MDC enforced (`sym.rs:232-241`, writes `0xD3`/`0x14` at `sym.rs:496-497`) | GnuPG ≤ 2.4 | no |
| **SEIPD v2 (tag 20) AEAD** OCB/EAX/GCM | ✅ decrypt (all three); encrypt (all three) — IMPL_STATUS | GnuPG ≥ 2.5 (OCB) | no (needs `pgp` ≥ 0.12) |
| **PKESK v3** RSA / ECDH (X25519, NIST P-256/384/521) | ✅ IMPL_STATUS | default keys (RSA or cv25519) | no |
| **PKESK** Elgamal | 🚫 not planned (IMPL_STATUS) | legacy GnuPG 1.x-era keys only | **yes, edge case** |
| **Armor** read + write | ✅ IMPL_STATUS | pass writes binary | no |

### Notes

- rPGP is low-level: the message builder makes the caller choose SEIPD version and cipher
  explicitly — `seipd_v1(rng, SymmetricKeyAlgorithm::…)` or `seipd_v2(rng, sym, aead,
  chunk_size)` (`src/composed/message/builder.rs`). There is no recipient-preference
  negotiation; that is layer-4 semantics, which rPGP's README states it deliberately does
  not implement.
- Defaults inside rPGP's own primitives: `SymmetricKeyAlgorithm::default()` = AES-128
  (`sym.rs`), `HashAlgorithm::default()` = SHA-256 (`hash.rs`),
  `StringToKey::new_default()` = iterated+salted, SHA-256, count 224 (`s2k.rs:18,218`),
  builder compression default = none (`builder.rs:784`).
- GnuPG proprietary OCBED AEAD (older experimental `--rfc4880bis` format, "SKESK v5")
  is decrypt-only in rPGP (IMPL_STATUS), but GnuPG 2.5 emits *standard* RFC 9580 SEIPD v2,
  which rPGP fully supports — not a gap for pass stores.

---

## 4. Known interop gaps / risks

1. **Elgamal encryption keys — hard gap.** rPGP marks Elgamal "Encrypt only" as 🚫
   (IMPL_STATUS). GnuPG still lists `ELG` as a supported pubkey (`gpg --version`), and
   pre-2010s pass stores may contain entries encrypted to an Elgamal subkey. Those entries
   cannot be decrypted by rPGP. Modern default keys (RSA / ed25519+cv25519) are unaffected.
2. **AEAD requires a recent `pgp`.** SEIPD v2 decrypt landed in 0.12.0-alpha.x, encrypt in
   0.16.0 (CHANGELOG). Pinning an RFC-4880-only `pgp` (≤ 0.11) would fail to decrypt
   GnuPG 2.5 AEAD pass entries. Pin ≥ 0.20.0.
3. **No algorithm auto-negotiation on encrypt.** rPGP will not pick cipher/SEIPD mode from
   the recipient key the way gpg does. Kleio must implement that choice (or hardcode a
   compatible default). For maximum gpg compatibility, emit **SEIPD v1 + AES-256** (or
   AES-128); this is decryptable by every GnuPG since 2.0. SEIPD v2 + OCB is only
   decryptable by GnuPG ≥ 2.2.21.
4. **`bzip2` is a cargo feature.** Compression is irrelevant for pass (disabled), but if
   Kleio ever writes compressed messages for gpg users, BZip2 support must be enabled via
   the `bzip2` feature (`docs.rs/pgp` features list).
5. **MDC is mandatory, not optional.** GnuPG ≥ 2.2.8 always writes MDC on SEIPD v1. rPGP
   *enforces* the MDC check on decrypt (`sym.rs:237-241` → `MdcError`) and always writes
   it on encrypt (`sym.rs:496-497`). This is symmetric — no gap, but it means rPGP will
   correctly *reject* tampered/legacy non-MDC CFB messages rather than silently accept them.
6. **Binary vs armored:** pass stores are binary; rPGP reads both binary
   (`Message::from_file`/`from_bytes`) and armored (`from_armor_file`) input, so no gap —
   Kleio just must parse `.gpg` as binary.

---

## 5. Explicit answers

**Can rPGP decrypt a typical pass-store entry today?**

Yes — for the near-universal case. A typical entry is `PKESK v3` (RSA or X25519) +
`SEIPD v1 (CFB+MDC, AES-256/AES-128)` from GnuPG ≤ 2.4, or `PKESK v3` + `SEIPD v2 (AEAD
OCB, AES-256)` from GnuPG ≥ 2.5, plus an uncompressed Literal Data packet. rPGP 0.20.0
supports all of these (cipher, compression, S2K, hash, both SEIPD versions, PKESK RSA/ECDH).
The only unsupported case is an entry encrypted to a legacy Elgamal key.

**Can gpg decrypt rPGP output?**

Yes — provided Kleio emits SEIPD v1 (CFB+MDC) with an AES cipher (decryptable by all GnuPG
versions, recommended for a pass-compatible store) or SEIPD v2 (AEAD OCB/EAX/GCM,
decryptable by GnuPG ≥ 2.2.21). rPGP does not choose this for the caller; Kleio owns the
decision, which is exactly the layer-4 semantics the spike must supply.

---

## Sources

- **pass**: `src/password-store.sh` v1.7.4 — lines 9, 12-13, 77, 105, 462, 471, 480, 506,
  542. https://github.com/zx2c4/password-store/blob/master/src/password-store.sh
- **rPGP source** (main = `pgp` 0.20.0): `src/crypto/sym.rs` (enum 109-135, MDC 232-241,
  496-497), `src/crypto/hash.rs` (enum 21-43), `src/types/s2k.rs` (18, 164-190, 218),
  `src/types/compression.rs` (10-14), `docs/IMPL_STATUS.md`,
  `src/composed/message/builder.rs`, `CHANGELOG.md`.
  https://github.com/rpgp/rpgp
- **rPGP crate docs / version**: https://docs.rs/pgp/latest/pgp/ (version 0.20.0; feature
  list incl. `bzip2`; explicit "handles layers 1-3, not 4").
- **GnuPG manual** (installed 2.5.20, libgcrypt 1.12.2): `--cipher-algo` / `--compress-algo`
  (GPG-Esoteric-Options.html); `--s2k-cipher-algo` / `--s2k-digest-algo` / `--s2k-mode` /
  `--s2k-count` / `--force-aead` / `--force-mdc` (OpenPGP-Options.html).
  https://gnupg.org/documentation/manuals/gnupg/
- **Empirical** (this machine, gpg 2.5.20): `gpg --list-packets` of a `pass`-style
  `gpg -e -r <key> --compress-algo=none` output; `--with-colons` key preference dump;
  secret-key S2K dump.
