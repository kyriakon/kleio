# git2 (libgit2) tree-level conflict plumbing — does it cover §5.8?

> Reference material, not authoritative. Raw investigation notes for the
> wayfinder question behind §5.8 ("Sync & conflict handling"): whether `git2`
> (the Rust binding to libgit2) exposes the plumbing needed to implement
> Kleio's tree-level, text-merge-free sync-conflict handling in-process.
> Every claim cited to a primary source; verify before building.

## TL;DR

- **`git2` exposes every primitive §5.8 needs**, including the hard one: a
  custom three-way merged tree computed and written directly from blob OIDs,
  without ever entering libgit2's text-merge machinery.
- **Blob-identity rename detection exists as a first-class flag** —
  `DiffFindOptions::exact_match_only` — which is exactly what §5.8's
  rename-vs-edit race relies on ("pure rename leaves the ciphertext blob
  byte-identical").
- **The text merge is opt-in, not forced.** It is entered only via
  `Repository::merge_trees` / `merge_commits` / `merge`. The building blocks
  for the custom merge (`diff_tree_to_tree`, `Diff::deltas`, `TreeBuilder`,
  `commit`, `reference`/`Reference::set_target`) never touch it.
- **The one gap is architectural, not capability:** `git2` is a C dependency
  (libgit2), which directly contradicts the proposal's "pure Rust, no C
  dependency" rationale in §6.2. This is a policy decision, not a missing API.

---

## 1. Object read/write: commit, tree, blob, index

All present in git2 0.21.0, straight from the [`Repository`](https://docs.rs/git2/0.21.0/git2/struct.Repository.html)
method list:

- **Blob write:** `Repository::blob(&[u8]) -> Oid` ("Write an in-memory buffer
  to the ODB as a blob"), `Repository::blob_path`, `Repository::blob_writer`.
  Low-level equivalent: `Odb::write(kind, data) -> Oid` — [Odb](https://docs.rs/git2/0.21.0/git2/struct.Odb.html).
- **Blob read:** `Repository::find_blob(oid) -> Blob` (`Blob::content()` for the bytes).
- **Tree build:** `Repository::treebuilder(Option<&Tree>) -> TreeBuilder`;
  `TreeBuilder::insert(filename, oid, filemode)` / `remove` / `write() -> Oid`
  ("Write the contents of the TreeBuilder as a Tree object") — [TreeBuilder](https://docs.rs/git2/0.21.0/git2/struct.TreeBuilder.html).
- **Tree read:** `Repository::find_tree`, `Tree::get_path` (recursive),
  `Tree::iter`, `TreeEntry` — [Tree](https://docs.rs/git2/0.21.0/git2/struct.Tree.html).
- **Commit write:** `Repository::commit(update_ref, author, committer, message,
  tree, parents) -> Oid`; `commit_create_buffer` for a commit without touching a ref.
- **Commit read:** `Repository::find_commit`, `Commit::tree()`, `Commit::parents()`.
- **Index read/write:** `Repository::index() -> Index`; `Index::read`,
  `Index::write` ("using an atomic [rename]"), `Index::add_frombuffer`,
  `Index::add`, `Index::write_tree`, `Index::write_tree_to` — [Index](https://docs.rs/git2/0.21.0/git2/struct.Index.html).

The in-process merge path does not even need the index: build a `TreeBuilder`
from blob OIDs, `write()` it into the ODB, `find_tree()` the result, and hand
it to `commit()`. No workdir, no index, no checkout.

## 2. Common-ancestor / merge-base

Present — [`Repository`](https://docs.rs/git2/0.21.0/git2/struct.Repository.html):

- `Repository::merge_base(one, two) -> Oid` — "Find a merge base between two commits".
- `Repository::merge_bases(one, two) -> OidArray` — "Find all merge bases between two commits"
  (criss-cross histories; §5.8 wants the tree diff against the common ancestor).
- `merge_base_many`, `merge_base_octopus`, `merge_bases_many`.
- `Repository::graph_descendant_of(commit, ancestor) -> bool` — ancestry check,
  useful for the edit-vs-delete veto ("an edit since the common ancestor is
  evidence the entry is still wanted").

## 3. Ref manipulation

Present — [`Repository`](https://docs.rs/git2/0.21.0/git2/struct.Repository.html)
and [`Reference`](https://docs.rs/git2/0.21.0/git2/struct.Reference.html):

- **Create direct ref:** `Repository::reference(name, id, force, log_message) -> Reference`;
  `reference_matching(name, id, force, current_id, log_message)` is the
  compare-and-swap variant.
- **Symbolic refs:** `Repository::reference_symbolic(name, target, force, log_message)`,
  `reference_symbolic_matching`; `Reference::symbolic_target()` to read; `set_head`
  for `HEAD`.
- **Update existing ref:** `Reference::set_target(id, reflog_msg)` — a lockfile
  rename under the hood, which is the "single atomic filesystem operation"
  §5.8's interrupted-operation case asks for.
- **Delete / rename:** `Reference::delete()`, `Reference::rename()`.
- **Multi-ref grouping:** `Repository::transaction() -> Transaction`
  (`lock_ref`, `set_target`, `set_symbolic_target`, `commit`) —
  [Transaction](https://docs.rs/git2/0.21.0/git2/struct.Transaction.html).
  Caveat, from that page: "committing is not atomic: if an operation fails, the
  transaction aborts, but previous successful operations are not rolled back" —
  fine for grouping, not a rollback guarantee.

## 4. Blob-identity rename detection

Present, as a first-class flag. §5.8 needs to spot a pure rename (delete+add of
a byte-identical ciphertext blob) without ever misreading a re-encrypted edit
as a rename.

- `Repository::diff_tree_to_tree(old, new, opts) -> Diff`, then
  `Diff::find_similar(Option<&mut DiffFindOptions>)` — [Diff](https://docs.rs/git2/0.21.0/git2/struct.Diff.html),
  [git2-rs source](https://raw.githubusercontent.com/rust-lang/git2-rs/master/src/diff.rs).
- `DiffFindOptions::exact_match_only(bool)` — documented "Measure similarity
  only by comparing SHAs (fast and cheap)" — [DiffFindOptions](https://docs.rs/git2/0.21.0/git2/struct.DiffFindOptions.html).
  This is blob-identity matching: two files are a rename only if their OIDs are
  equal, which is precisely the §5.8 invariant (a pure move does not re-encrypt,
  so the OID is unchanged; any re-encryption produces a different OID and is
  therefore never a rename).
- libgit2 underlying flag: `GIT_DIFF_FIND_EXACT_MATCH_ONLY = (1u << 14)`,
  "Measure similarity only by comparing SHAs (fast and cheap)" —
  [libgit2 diff.h](https://raw.githubusercontent.com/libgit2/libgit2/main/include/git2/diff.h).
- Result status: `DiffDelta::status()` returns `Delta::Renamed` /
  `Delta::Copied` / etc., with `old_file()`/`new_file()` exposing the OIDs and
  modes — [Delta](https://docs.rs/git2/0.21.0/git2/enum.Delta.html),
  [DiffDelta](https://docs.rs/git2/0.21.0/git2/struct.DiffDelta.html),
  [DiffFile](https://docs.rs/git2/0.21.0/git2/struct.DiffFile.html).

Two notes. First, libgit2 only runs similarity detection when flags are set:
the header warns "if you don't explicitly set this, `diff.renames` could be set
to false, resulting in `git_diff_find_similar` doing nothing" — Kleio must call
`.renames(true).exact_match_only(true)` explicitly, not rely on defaults.
Second, libgit2's pluggable similarity metric (`git_diff_similarity_metric`,
the custom-callback escape hatch) is declared in diff.h but not surfaced in
git2-rs 0.21's `DiffFindOptions` (no `metric` builder method). Irrelevant here:
`exact_match_only` is the stronger, cheaper primitive and is bound.

## 5. Custom three-way merged tree, written directly, no text merge

**Supported.** The merge is computed by the caller at tree level; libgit2's text
merge is only entered through its merge API, which Kleio simply never calls.

- Enumerate changes vs. the common ancestor with
  `Repository::diff_tree_to_tree(ancestor, ours)` and
  `diff_tree_to_tree(ancestor, theirs)`, then iterate `Diff::deltas()`.
- For each delta, `DiffDelta::status()` gives `Added`/`Deleted`/`Modified`/
  `Renamed`; `DiffFile::id()` and `mode()` give the blob OID and file mode to
  place in the result tree. This is enough to implement the different-entry
  auto-merge (union of non-overlapping changes) and same-entry keep-one-flag-other.
- Assemble the result with `TreeBuilder::insert(filename, oid, filemode)` and
  `TreeBuilder::write()`, then `Repository::commit(...)`, then update the ref
  via `Reference::set_target`/`reference`.

The text merge lives behind `Repository::merge_trees(ancestor, ours, theirs,
opts) -> Index` / `merge_commits` / `merge` ("Merge two trees, producing an
index that reflects the result of [the merge]"), and the free function
`git2::merge_file`. None of the diff / `TreeBuilder` / `commit` / reference
primitives above invoke it. "Compute the tree myself and write it" is the
documented happy path of `TreeBuilder`, not a hack — the struct's own doc calls
it the low-level tree-update facility.

The structured merges in §5.8 are application logic, not a missing git2 API:
`.gpg-id` set-merge and the audit-log union are read the blobs
(`find_blob` → `Blob::content`), compute the result in Rust, write it back
(`Repository::blob` → `TreeBuilder::insert`). git2 provides the byte-level
read/write; Kleio owns the policy.

## 6. Verdict and gaps

**Verdict: yes.** `git2` 0.21.0 (libgit2) supports §5.8's in-process tree-level
approach end to end: object read/write, merge-base, ref manipulation (including
atomic single-ref update and symbolic refs), blob-identity rename detection via
`exact_match_only`, and a custom three-way tree merge written directly without
invoking libgit2's text merge.

Gaps, all architectural or application-level rather than missing plumbing:

1. **C dependency.** `git2` links libgit2 (C), which contradicts §6.2's "pure
   Rust, no C dependency" rationale. This is the same tradeoff the gix note
   flagged; choosing `git2` for sync means accepting the C dependency the
   proposal explicitly avoided, or shelling out to the `git` CLI instead.
2. **Text merge must be deliberately avoided, not disabled.** There is no flag
   that makes `merge_trees` itself tree-only; Kleio must not call
   `merge_trees`/`merge`/`merge_commits`/`git2::merge_file` on entry files.
3. **`Transaction::commit` is not atomic** (its own doc says so). For §5.8 this
   is fine — the required atomicity is a single ref update, which
   `Reference::set_target` provides via lockfile rename.
4. **Custom similarity metric not bound** in git2-rs 0.21 (`git_diff_similarity_metric`
   exists in libgit2 headers but has no git2-rs builder). Not needed: `exact_match_only`
   is the exact primitive §5.8 requires.

## Open questions

- Does §5.8's "scratch area + one atomic ref update" need `HEAD` and the branch
  ref updated together? If so, note the `Transaction` rollback caveat above.
- Whether accepting the libgit2 C dependency is acceptable versus the gix path
  (which the gix note found cannot push) or a `git` CLI shell-out for push.
