# AGENTS.md

Kleio is a cross-platform, `pass`-compatible password manager (Tauri v2 + Rust +
React/TypeScript).

## Orientation

Planning docs, specs, research notes, and the project glossary live in this repo under
`docs/planning/`. Read `docs/planning/README.md` first for anything beyond a small,
well-scoped change.

Finalized architecture decisions that matter for how code is written live under
`docs/decisions/` — check there before assuming you're missing context; it may already
be written down.

## Shared vocabulary

Full glossary: `docs/planning/glossary.md`. The two terms most likely to matter while
writing code:

- **Re-keying vs. rotation**: re-keying = removing a signer's key from `.gpg-id` and
  re-encrypting (automatable, immediate). Rotation = actually changing a secret's
  value (cannot be automated, must be tracked). Never conflate the two in code,
  comments, or UI copy — a re-keyed-but-not-rotated entry is not secure against the
  removed signer.
- **Recoverable copy**: the pattern used for sync conflicts. Never silently discard
  either side of a conflicting edit — keep one live, preserve the other, flag it for
  review.

## Build & test

- Rust: `cargo build --workspace` / `cargo test --workspace` / `cargo clippy --workspace -- -D warnings`
- Frontend: `bun install` / `bun run dev` / `bun run lint` / `bun run typecheck`
- Pass-store interop suite (round-trips against real `gpg`, slow, not run by default):
  `cargo test -p kleio-store --test interop -- --ignored`
- Coverage: `cargo llvm-cov --workspace` (enforced in CI per-crate; `kleio-crypto` and
  `kleio-store` hold a high bar, everything else doesn't — see CI config for current
  thresholds, not this file).

## Code style

- Rust lint levels (`unwrap`/`expect` outside tests, etc.) are enforced via
  `[workspace.lints.clippy]` in the root `Cargo.toml` — `cargo clippy` is the source of
  truth, not this file.
- TypeScript strictness (no `any`, no non-null assertions, exhaustive `switch`,
  `noUncheckedIndexedAccess`) is enforced via ESLint/tsconfig — `bun run lint` /
  `bun run typecheck` is the source of truth, not this file.
- Anything that ever holds key material or a passphrase must be wrapped in `zeroize` —
  not mechanically enforceable, no tool will catch a miss.
- Push side effects (I/O, randomness, time, subprocess/network calls) behind a trait at
  the boundary; don't introduce a new trait unless there's a real external boundary.

## Tickets

Tickets live in GitHub Issues. **Work only on the ticket explicitly given to you for
the current session — do not browse the issue tracker, list open issues, or self-select
work.** If a task doesn't map to an existing ticket, ask before creating one; don't
create tickets speculatively.

```
gh issue view <number>
gh issue create --title "..." --body-file <path> --label <label>   # only when asked
```

Issues generated from a spec include a `Spec:` line pointing to the corresponding file
in `docs/planning/specs/`. Read that file before starting non-trivial ticket work — the
issue body alone is a summary, not the full context.

## Git

- Conventional commits, with scope: `type(scope): description` — e.g. `feat(kleio-store): resolve nested .gpg-id`, `fix(kleio-crypto): handle empty passphrase`. Types: `feat`, `fix`, `chore`, `docs`, `test`. Scope is the crate or area touched (`kleio-crypto`, `kleio-store`, `kleio-git`, `frontend`, `ci`).
- Never force-push to `main`. Never rewrite shared history without explicit human
  sign-off.
- Squash merge only.

## Never

- Never commit private key material, real passphrases, or real `.gpg-id`/pass-store
  content — test fixtures use synthetic keys only.
- Never edit `docs/decisions/*.md` (ADRs) as a side effect of an unrelated task — these
  record decisions, not implementation notes, and changes need explicit human review.

## Agent skills

### Issue tracker

Issues live in GitHub Issues, driven via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical triage labels, strings equal to their names: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. ADRs live in `docs/decisions/`, shared vocabulary in `docs/planning/glossary.md`. See `docs/agents/domain.md`.
