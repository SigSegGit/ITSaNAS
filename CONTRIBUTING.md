# Contributing

## Governance

ITSaNAS has a single validator: the repository owner (`@SigSegGit`) reviews
and merges every change. `main` is protected — no direct pushes, all changes
go through a pull request, CI must be green, and the owner must approve
before merge. `CODEOWNERS` names the owner for the entire tree so review is
always requested automatically.

## Contributor License Agreement (CLA)

Every external contribution requires a signed CLA before it can be merged.
This is enforced by the CLA Assistant bot on each pull request. The CLA
grants the project maintainer the rights needed to relicense or
commercialize the project in the future (e.g. paid hosted storage, a
marketplace) while keeping the codebase itself under AGPL-3.0. Contributions
without a signed CLA will not be merged, no exceptions.

## Code standards

1. **Small, single-purpose functions.** No huge chunks of logic. Public
   functions get doc comments. Every crate has a doc header (`//!` at the
   top of `lib.rs`) explaining its role in the system.
2. **Documentation maps to code.** `ARCHITECTURE.md` has one section per
   crate, linking design decisions to the modules that implement them.
   `STATUS.md` is kept current after each significant step so any
   contributor (or future session) can resume cold.
3. **Versioned releases, CI on every push.** Semantic-versioned tags,
   cross-compiled release binaries. `fmt`, `clippy`, build, and the full
   test suite must pass — nothing merges on red CI.
4. **Two layers of anti-regression tests:**
   - In-code: unit tests per function, plus simulation-mode tests that
     replay scripted failure scenarios (node dies mid-upload, shard
     corrupted, storage permission change, quota exceeded, version restore)
     against an in-memory fake network/disk. These run in CI on every
     commit.
   - Deployment-level: an acceptance harness that runs the same scenario
     scripts against real deployed nodes.
5. **Boring, proven components.** Justify every new dependency in
   `ARCHITECTURE.md`. No home-grown cryptography.
6. **Security is never deferred.** If a change touches key handling, shard
   integrity, or the trust boundary between nodes, say so explicitly in the
   PR description.

## Pull requests

Use the PR template. Every PR must:

- Pass CI (`scripts/ci.sh`, which is exactly what CI runs — you can and
  should run it locally first).
- Include or update tests for the behavior it changes.
- Update `ARCHITECTURE.md` / `STATUS.md` if it changes design or project
  state.
