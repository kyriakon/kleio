# Kleio — Project Proposal

**A cross-platform, `pass`-compatible password manager built on Rust and pure-Rust OpenPGP**

---

## 1. Overview

Existing GUI wrappers for `pass` (e.g. QtPass, pass-for-windows) shell out to the `gpg` binary and depend on `gpg-agent` for passphrase caching. This creates platform fragmentation — mobile platforms have no GPG binary or agent to shell out to, so mobile support would otherwise require a separate crypto implementation and key-management path from desktop — and a fragile process boundary (parsing subprocess output, agent state going out of sync).

Kleio's core architectural bet is to replace the GPG binary/agent dependency with **rPGP** (`pgp` crate), a pure-Rust OpenPGP implementation, so encryption, decryption, and key management run through one code path on both desktop and mobile.

**Non-negotiable constraint**: Kleio must remain a drop-in-compatible client for existing `pass` stores — no migration step, no proprietary format, round-trip interoperable with real `gpg`.

Beyond basic pass-store compatibility, Kleio is scoped around four capabilities `pass` doesn't provide out of the box: managing **multiple stores** side by side, **signing-key/user lifecycle** management (add/remove access as a first-class operation), a **security & notification center** that proactively surfaces things pass-store users otherwise have to notice themselves, and **sync that never exposes raw git conflicts**. Section 7 scopes which of these are in the initial release.

## 2. Design Principles

- **Non-technical users first, technical users second.** Kleio's primary audience is people who would benefit from `pass`'s security model but have no interest in git internals, GPG concepts, or command-line workflows. This governs several downstream decisions: sync never surfaces a raw merge conflict, signer management is scoped to the store root rather than exposing subtree permissions, and simplicity is weighted over configurability wherever the two trade off.
- **Standalone, genuinely open source.** Kleio works completely with any git remote, or none at all. It has no dependency — technical or in onboarding — on any particular hosting provider.
- **Pass-store compatibility is absolute.** Any store openable by standard `pass`/GnuPG must be openable by Kleio with zero migration, and vice versa.

## 3. Naming & Identity

- **Name**: Kleio (Κλειώ), Muse of history and record-keeping.
- **App identifier**: `net.kyriakon.kleio` is the current working identifier. kyriakon.net (a separate project, an OpenBSD-based community hosting platform) is offered as one *suggested option* in Kleio's "add a remote" flow, alongside plain git-URL entry — it is not required and is not otherwise coupled to Kleio's core flows. Whether the app identifier should be neutral instead (given the project's standalone positioning) is an open question — see section 10.

## 4. Tech Stack

| Layer | Choice | Notes |
|---|---|---|
| Shell | Tauri v2 | desktop + mobile from one shell |
| Backend | Rust workspace | `kleio-crypto`, `kleio-store`, `kleio-git` |
| Frontend | React + TypeScript (strict) | no `any`, no non-null assertions, exhaustive switches, `noUncheckedIndexedAccess` |
| Package manager | Bun | |
| Crypto | `pgp` (rPGP) + `pgp-lib`, `zeroize` | in-process, no GPG binary/agent — see section 6.1 for open risk |
| Git | `gix` (plumbing crates: `gix-url`, `gix-transport`, `gix-protocol`) + `russh` for mobile | custom `Transport` implementation for SSH — see section 6.2; deferred past MVP per section 7 |
| Lint/format | ESLint (strict) + Prettier (import sorting) | |
| CI/CD | GitHub Actions | build/test matrix (Linux/macOS/Windows), lint, security audit, signed Tauri updater via GitHub Pages manifest |

## 5. Architecture

### 5.1 Crate layout
- `kleio-crypto` — OpenPGP operations, wrapping `pgp` (rPGP)
- `kleio-store` — pass-store file layout, tree traversal, `.gpg-id` handling
- `kleio-git` — git operations, remotes, sync, built directly on `gix` plumbing crates

### 5.2 Key abstractions

**`KeyStore` trait** — abstracts key persistence. `FileKeyStore` stores armoured keys under `~/.kleio/keys/` (desktop) or the app data directory (mobile), with `0700` permissions.

**`PassphraseProvider` trait** — decouples crypto operations from UI. `TauriPassphraseProvider` fires a Tauri event to the frontend and awaits the response. Optional session-only passphrase caching, in-memory, never persisted.

### 5.3 Pass-store compatibility
Round-trip tests decrypt Kleio-encrypted files with real `gpg`, and decrypt real-`gpg`-encrypted files with Kleio. Existing `~/.password-store` directories must open with zero migration step.

### 5.4 Key management
Public/private key generation, import, and export are unified across desktop and mobile — implemented once in `kleio-crypto` and invoked identically from both platforms' UI.

### 5.5 Multi-store management *(post-MVP — see section 7)*

Kleio manages multiple independent `pass`-style store directories, each represented as its own tab in the sidebar. This keeps each store a plain `PASSWORD_STORE_DIR`-compatible directory that any standard `pass` install could also open directly — no virtualization, no proprietary aggregation layer.

- A `StoreRegistry`, sitting above `kleio-store`, tracks each registered store's root path, git remote (if any), and display label.
- Each store is fully independent: its own recipient set (`.gpg-id`), its own git remote and sync state, its own security-center findings.
- Search is scoped to the active store by default. Searching across stores the active key can't decrypt would leak "an entry with this name exists" without decrypt access, so cross-store search is not a default behavior.

### 5.6 Signing-key / user lifecycle *(post-MVP — see section 7)*

Managing "who can decrypt this store" as a first-class operation rather than a manual `.gpg-id` edit.

Removing a signer is two separable operations with different guarantees:

1. **Re-keying** (automatable): once `.gpg-id` is updated and the store re-encrypted, the removed user cannot decrypt anything committed afterward.
2. **Rotation** (cannot be automated, must be tracked): the removed user retains their private key and can still decrypt every ciphertext blob reachable in git history up to the point of removal, since git history is immutable by default. More fundamentally, they already saw the plaintext of anything that existed before removal — re-encrypting the file doesn't undo that. Only changing the underlying secret value does.

Re-keying without rotation must never be presented as equivalent to revocation.

**Add signer**: add the public key to the store-root `.gpg-id`, re-encrypt the whole store atomically (write-to-temp-then-rename per entry, or a transaction log, so an interrupted operation can't leave a partially re-encrypted store).

**Remove signer**: update the store-root `.gpg-id` and re-encrypt immediately (the re-keying half); open a rotation task in the security center for every entry in the store (the tracked, not-automatable half). Kleio can offer to generate fresh random values to make rotation easy but cannot push new values to most external services itself. The UI must surface both halves distinctly at the moment of removal — e.g. "re-keyed immediately; 11 entries still need rotation."

**Scope**: add/remove operates at the store root only — one recipient list, one re-encrypt-everything action. Kleio still reads nested `.gpg-id` files correctly (a native pass feature, required to open existing stores unmodified), but does not offer subtree-scoped management; a team needing that level of separation is better served by a second store than by subtree permissions within one.

**Audit trail**: a plaintext, git-committed membership log (`.kleio/audit.log`) records who added or removed whom and when. Public key fingerprints aren't sensitive, so this lives in the repository itself — every member's install then sees the same access history.

**Who can remove a signer**: pass/GPG stores have no central authority — whoever has git push access can rewrite `.gpg-id` directly, with or without going through Kleio's UI. Any in-app restriction is therefore a courtesy rather than an enforced boundary unless removal commits are cryptographically signed by a designated admin key and Kleio verifies that signature before trusting a `.gpg-id` change. Proposed model for v1: no in-app restriction, full visibility — anyone with write access can remove anyone, the action is unmistakably recorded in `.kleio/audit.log`, and real access control is left to the git remote's push permissions. A soft in-app "admin" role, and eventually signed-approval removal, are possible later escalations if a real deployment needs stronger guarantees than tier 1 provides.

**Escape hatch**: git history rewrite (squash/filter-repo) for the rare case where the existence of a secret is itself sensitive enough that old, superseded ciphertext shouldn't remain recoverable. This forces every collaborator to re-clone and should be a deliberate, heavily-warned action, not part of the normal removal flow.

### 5.7 Security & notification center *(partially post-MVP — see section 7)*

Four checks:

1. **Reused passwords across entries** — fully local.
2. **Entries still encrypted to a removed signer's key** — fully local; cross-references each entry's recipient list against the store's current authorized-signer list.
3. **Unresolved sync conflicts** — see 5.8. When the same entry is touched on two devices between syncs, one side is kept live and the other preserved as a flagged, recoverable copy.
4. **Breach-checked credentials** — optional and manual-only, never automatic. Uses the k-anonymity range-query pattern (as pioneered by Have I Been Pwned's API): only the first five characters of a SHA-1 hash of the password ever leave the device, with the full comparison done locally against the returned candidate set. This is the only feature in the app that talks to a third party, and it is opt-in and clearly disclosed.

Checks 1–3 are fully local and can run on launch or on a schedule without a "why is this app talking to the network" concern; check 4 stays explicit and user-triggered every time.

### 5.8 Sync & conflict handling

Kleio never surfaces a raw git merge conflict. This isn't purely a UX preference: pass-store entries are individually-encrypted, opaque ciphertext blobs, and git's default line-based text merge is meaningless — and actively corrupting — when applied to them. Collisions are detected and resolved at the tree level rather than by ever invoking git's generic text-merge machinery on entry files.

**Different entries touched on both sides since last sync** — not a real conflict. Detected via tree-level diffing against the common ancestor and applied automatically and silently; this covers the large majority of cases (e.g. different people editing different accounts).

**The same entry touched on both sides** — a real conflict, handled without ever blocking sync or asking a question in the moment. One side is kept as the live entry (most-recent-edit wins, acknowledging clock skew as a known limitation), the other is preserved as a recoverable copy and flagged in the security center. The person reviews and resolves it whenever they next open the app.

Related cases and their handling:

- **Rename vs. edit race**: git has no native rename — it's a delete+add pair. Kleio only re-encrypts on actual content change, never on a pure move; since OpenPGP encryption is non-deterministic, a pure rename leaves the ciphertext blob byte-identical, which lets `gix`'s blob-identity rename detection identify it correctly and automatically. A rename on one side combined with a content edit at the old path on the other is a genuine conflict, flagged for the person to confirm the result.
- **Edit vs. delete race**: an edit since the common ancestor is treated as evidence the entry is still wanted — the deletion is vetoed and the entry stays live, with a security-center note. This asymmetry is deliberate: a surviving unwanted entry is a two-click fix, a silently vanished edit is unrecoverable.
- **`.gpg-id` merges**: handled as a structured three-way set merge (`final = (ancestor ∪ addedA ∪ addedB) − removedA − removedB`) rather than git's text merge, since a "clean" line merge on this file can still produce a recipient list that reflects neither side's actual intent. A concurrent add/remove of the same key resolves by favoring the removal, flagged for review.
- **Audit log merges**: stored as one event per line with a timestamp and stable ID; merged as a union of events (deduplicated by ID) and re-sorted by timestamp, rather than trusting a positional text merge.
- **Interrupted operations**: sync is performed in a scratch area and only touches the real working state with a single atomic step at the end (a ref update is one atomic filesystem operation). A crash before that step leaves the live store untouched; the next launch simply retries.
- **First-sync race**: store creation has no separate force-push path — it always fetches first and, if the remote already has content, runs the standard merge machinery using an empty tree as the common ancestor.

For MVP, the different-entry auto-merge and a minimal version of same-entry conflict handling are in scope (see section 7); the full set of related edge cases above should be implemented but can be prioritized after the core flow works end to end.

## 6. Technical Risks & Rationale

### 6.1 rPGP: OpenPGP semantics layer

This is the highest-priority open risk in the stack, and the first thing scheduled in section 9.

rPGP's own documentation describes OpenPGP as four layers: (1) wire format, (2) composite objects (certificates, messages), (3) crypto operations, and (4) **semantics** — expiration and revocation handling, key flags governing what a given key may be used for, algorithm preference signaling. rPGP implements layers 1–3 and **explicitly does not implement layer 4**, leaving applications to build that themselves or depend on another library. This is directly relevant to Kleio: the security center's "entries still encrypted to a removed signer's key" check, and correct handling of a revoked or expired key, sit entirely in that layer. A companion crate, `rpgpie`, is attempting to build this semantics layer on top of rPGP, but its own documentation describes it as "a relatively early stage of development... use with caution," with an API that is "still decidedly incomplete."

Two production projects doing work close to Kleio's both hedge rather than commit fully to rPGP:
- **`prs`** — a Rust, pass-inspired, git-synced password manager — supports rPGP as an optional backend but ships GnuPG shell-out as its default.
- **GpgFrontend** — a mature dual-engine OpenPGP desktop application — states in its own release notes that "GnuPG remains the recommended default engine, while rPGP can be selected... and used as a fallback."

Both are more mature than Kleio will be at v1, built by people who evaluated this exact tradeoff and chose GnuPG as the trusted default.

**Alternative considered**: Sequoia (`sequoia-openpgp`), the other serious Rust-adjacent OpenPGP implementation, has fuller spec coverage including the semantics layer. It doesn't resolve the mobile problem cleanly, however — its mature, recommended cryptographic backends (Nettle, OpenSSL, Botan) are C libraries, reintroducing the cross-compilation burden rPGP was chosen to avoid. Sequoia does offer a pure-Rust backend, but its own documentation states that backend isn't recommended for general use and gates it behind experimental-crypto opt-in flags.

**Recommendation**: an early technical spike (Phase 1, section 9) to prototype the actual semantics Kleio needs (revocation checking, expiry checking, key-flag validation feeding the recipient-resolution logic in 5.6) and measure how much `rpgpie` provides versus how much `kleio-crypto` must build from scratch. Depending on the result, weigh the hybrid both `prs` and GpgFrontend converged on — system `gpg` on desktop, rPGP reserved for mobile only — against the "one code path everywhere" architecture this project is currently built around. The hybrid isn't free (it reintroduces platform branching) but is the option two more-mature comparable projects both independently chose.

### 6.2 Git: `gix` + `russh`

Git integration is built on `gix` (the gitoxide project) for the object/protocol layer — pure Rust, no C dependency, mature enough that Cargo itself uses it for dependency fetching — paired with a custom `Transport` implementation backed by `russh` (a pure-Rust, async SSH client library used in several production Rust projects) for the SSH connection itself, since `gix`'s own SSH support shells out to the system `ssh` binary, which mobile platforms don't have.

**Integration approach**: `gix-url::parse()` produces a structured URL, normally passed to `gix-transport::client::connect()`, a scheme-dispatching function that selects a built-in transport and returns a `Box<dyn gix_transport::client::Transport>`. Everything above that — `gix-protocol::handshake()`, ref negotiation, pack transfer — operates purely against that trait. `kleio-git` bypasses `connect()`'s built-in dispatch for `ssh://` URLs, implements `Transport` itself with `russh` driving the SSH session, and hands that directly to `gix-protocol::handshake()` — a confirmed, intended extension point. This clean extension point exists at the plumbing-crate level; the convenient top-level `gix::Repository`/`gix::Remote` API resolves transports through the same built-in dispatch and has no documented "bring your own transport" hook, so `kleio-git` is built directly against the plumbing crates (`gix-url`, `gix-transport`, `gix-protocol`) and implements its own fetch/push/clone orchestration.

As an additional, independent motivation: the subprocess-based SSH approach this design avoids has a disclosed vulnerability class (RUSTSEC-2024-0335), a code-execution bug via a maliciously crafted remote URL smuggling arguments into a spawned SSH process. The `russh`-backed transport removes that entire class of bug, a security improvement independent of the mobile-portability argument.

**For MVP specifically**: since MVP is desktop-only (section 7), the default subprocess SSH transport `gix` already provides is sufficient — desktop platforms have a system `ssh` binary and typically an ssh-agent. The custom `russh` transport is real, scoped work that only becomes necessary once mobile targets enter the picture, so it's deferred rather than built up front.

### 6.3 Tauri v2 mobile maturity
Tauri's mobile targets are newer and less battle-tested than its desktop story, and mobile-specific issues (deep linking, background execution limits, keychain access patterns) are exactly where a security-sensitive app can least afford surprises. Desktop is the MVP target, with mobile as an explicit later phase.

### 6.4 Bun with Tauri's mobile build tooling
Tauri's mobile builds shell out to Xcode/Gradle rather than Bun/Node, making this the least-traveled combination in the stack. Not relevant to MVP scope; worth a smoke test before the mobile phase begins.

### 6.5 `KeyStore` / `PassphraseProvider` abstractions
Both traits currently have exactly one planned implementation, so their value (e.g. enabling an OS-keychain-backed `KeyStore` later) is still theoretical. Reasonable to keep as designed without over-investing in flexibility for backends that may never be built.

## 7. MVP v1 Scope

**In scope:**
- Single pass-store (no multi-store tabs)
- Desktop only — Linux, macOS, Windows via Tauri v2
- Core operations: browse/search entries, reveal/copy a secret, add/edit/delete entries
- `kleio-crypto`: rPGP-based encrypt/decrypt, `FileKeyStore`, `TauriPassphraseProvider` with optional session caching
- Pass-store compatibility: open an existing `~/.password-store` unmodified, round-trip interop with real `gpg`
- Git sync via `gix` over SSH, using the default subprocess transport (see 6.2)
- Sync conflict handling: different-entry auto-merge (required for basic multi-device use), and a minimal version of same-entry conflict handling — a flagged, recoverable-copy list is sufficient; it doesn't need the full security-center UI at this stage
- "Add a remote" flow, including kyriakon.net as a suggested (not required) option

**Deferred past MVP:**
- Multi-store (`StoreRegistry`, sidebar tabs)
- Signer lifecycle management (add/remove signer flows, rotation tracking, `.kleio/audit.log`) — recipient changes can be made by hand-editing `.gpg-id` outside the app for now, exactly as with plain `pass`
- Full security & notification center — reused-password detection is the cheapest check and the best candidate to bring forward first if there's spare capacity; removed-signer detection and breach-checking depend on features that are themselves deferred
- Custom `russh` SSH transport (needed only once mobile targets exist)
- Mobile targets

**Rationale**: this scope proves the core value proposition — a genuinely simple, safe, pass-compatible GUI — without requiring the higher-effort features that differentiate Kleio from other pass GUIs but aren't needed to validate the concept. It also meaningfully de-risks the schedule: deferring the custom `russh` transport alone removes one of the two hardest Rust spikes from the MVP critical path, leaving the rPGP semantics-layer spike as the one item that has to be resolved early.

## 8. Team & Task Allocation

Two contributors with different capacity and skill profiles:

- **Oliver** — mid-level, strong Rust, available around 25 hours/week for the next several weeks.
- **Marios** — entry-level, new to Rust and to the project's development workflow, available roughly 4–5 hours/week.

Task allocation follows skill and dependency lines rather than trying to split work evenly, but it's also designed with Marios's growth in mind — this project is a genuinely good opportunity for him to pick up Rust, and the plan below tries to make that real rather than incidental.

**Oliver — Rust-heavy, critical-path work:** `kleio-crypto` (rPGP integration, the semantics-layer spike), `kleio-store` (pass-store compatibility, round-trip tests), `kleio-git` (`gix` integration, sync and conflict logic), the Tauri command layer connecting the Rust backend to the frontend, and CI/build pipeline setup.

**Marios — frontend work now, growing into Rust over time:**
- *To start*: React/TypeScript UI components built against mocked data first (unlock screen, entry list, entry detail view, add/edit forms), so this work isn't blocked waiting for the Rust backend to land; styling and design-system application; documentation (README, contributing guide, in-app help text); manual QA and bug triage once a working build exists.
- *Rust on-ramp*: Marios doesn't need to wait for `kleio-crypto`/`kleio-store` to land before touching Rust — a fully standalone task, decoupled from the crypto and sync critical path, is a better first step and can start in the first couple of weeks. The password-generator utility is a good fit: no dependency on the rest of the backend, easy to spec, easy to test and review in isolation, and nothing breaks if it takes a while to get right. From there, once `kleio-crypto`/`kleio-store` have real, tested functions to work against, he can move into writing unit tests for them from templates and examples Oliver provides, and then further small, well-scoped tasks — parsing a `.gpg-id` file into a list of recipient key IDs, `Display`/error-message implementations for an already-defined error enum, small `From`/`TryFrom` conversions between types, or simple validation functions (e.g. checking an entry name/path is well-formed before it's used). These should be picked deliberately (clear spec, small blast radius, easy to review, never on the critical path) and reviewed closely by Oliver — the goal at this stage is learning, not throughput.

At 4–5 hours a week, Marios's realistic throughput is roughly one small task per week, and less than that while he's still learning the workflow. The schedule in section 9 treats his contributions as parallel and supplementary rather than on the critical path — the MVP timeline should hold even if his tasks land later than planned, and that's fine; the point isn't to maximize his output early on, it's to build toward him being a genuine Rust contributor over time. Onboarding overhead (git/GitHub workflow, code review norms, basic Tauri project structure) is budgeted explicitly in week 1 rather than assumed to happen alongside his first task for free.

## 9. Scheduling & Phased Plan

Assumes Oliver at around 25 hours/week for an initial four-week block, tapering afterward — this duration is an assumption and should be adjusted to whatever's actually planned.

**Phase 1 — Weeks 1–2: De-risking spikes**
- *Oliver*: the rPGP semantics-layer spike (prototype revocation/expiry/key-flag checking, measure `rpgpie` coverage, round-trip interop tests against real `gpg`-encrypted stores) and basic `gix` fetch/push/clone over SSH using the default subprocess transport. The outcome of the rPGP spike should directly inform whether the rPGP-only architecture holds for MVP or needs the hybrid fallback from section 6.1, so this comes before anything else is built on top of it.
- *Marios*: onboarding (repo structure, git/PR workflow, coding conventions); begin the Tauri + React shell and static UI components against mocked data; in parallel, his first Rust task — the standalone password-generator utility, fully decoupled from Oliver's work this phase.

**Phase 2 — Weeks 3–4: Core implementation**
- *Oliver*: `kleio-crypto` (encrypt/decrypt/sign/verify, `FileKeyStore`, `TauriPassphraseProvider`); `kleio-store` (pass-store tree read/write, `.gpg-id` resolution, round-trip interop tests); `kleio-git` sync logic, including different-entry auto-merge and the minimal same-entry conflict handling from section 5.8.
- *Marios*: continue UI components (unlock screen, entry list/detail, add/edit forms); wrap up the password-generator task; start the next Rust on-ramp step — writing tests for Oliver's already-landed `kleio-crypto`/`kleio-store` functions from provided templates.

**Phase 3 — Weeks 5–6: Integration**
- Wire the React frontend to the Rust backend via Tauri commands; end-to-end unlock → browse → edit → sync flow.
- *Marios*: continue component work and Rust-test writing; a second small, self-contained Rust task if things are going well — `.gpg-id` parsing or one of the other candidates above — with a clear spec and close review; begin documentation and manual QA once a working build exists.

**Phase 4 — Ongoing: MVP polish and dogfooding**
- Error handling, first-run onboarding, packaging and CI, alpha use against a real (initially sanitized/test) store.

**Post-MVP**, prioritized against the open questions in section 10: reused-password check (cheapest security-center item), multi-store, signer lifecycle and audit log, remaining security-center checks, mobile targets and the `russh` transport work that unlocks them.

## 10. Open Questions

1. Whether `net.kyriakon.kleio` remains the right app identifier given the project's standalone positioning, or whether a neutral identifier better fits before any store submission.
2. Whether the ~25 hours/week, four-week assumption in section 9 matches what's actually planned, and what cadence follows it.
3. Whether the sync-conflict handling in 5.8 — auto-merge for different entries, keep-one-flag-the-other for same-entry conflicts, and the specific handling proposed for rename/delete races, `.gpg-id` and audit-log merges, interrupted operations, and first-sync races — is the right default before implementation begins, and how much of the "related cases" list needs to ship in MVP versus after.
4. Whether store-root-only signer management (5.6) is sufficient, with finer-grained access needs handled by creating a second store.
5. Whether the breach-check feature (5.7) ships at all, given it's the one third-party network dependency in an otherwise local/self-hosted application.
6. Whether `KeyStore` should plan for OS-keychain-backed implementations now or defer that.
7. `PassphraseProvider` session-cache eviction policy: fixed timeout, app-background trigger, explicit lock action, or a combination.
8. Test corpus for pass-store interoperability: synthetic edge-case fixtures versus a sanitized copy of a real store.project proposal
