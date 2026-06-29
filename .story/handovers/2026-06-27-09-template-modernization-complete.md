# Session Handover — Template Modernization complete (22/22)

## Summary
Completed the entire **Template Modernization** phase: all 22 tickets (T-001…T-022) are
`complete` and committed on branch `storybloq/modernize-template`. Each ticket went through
the full autonomous pipeline (plan → codex/agent review → implement → review → commit), with
adversarial review verdicts addressed before commit. The phase brings `rust-service-template`
to fleet parity with hare/keystore/aviary/metricstore.

## What shipped (by area)
- **Build/lint gate (T-003, T-004, T-007):** `clippy-strict` cargo alias; bacon clippy job
  points at it; `[workspace.lints]` (unsafe_code=deny, clippy all=warn) with all 3 members
  opted in; fixed the 5 pre-existing clippy::all warnings so the gate is green on buildable
  members.
- **Toolchain/repro (T-002):** `rust-toolchain.toml` pinned to 1.96.0.
- **CI (T-001, T-008, T-009, T-016, T-017, T-018):** merge-queue branch guard; on-merge
  coverage job + shared `rust-ci` cache + action pin bumps; on-push clippy job + concurrency
  cancel + pin bumps; `claude.yml` @claude assistant; `dependabot.yml`; `CODEOWNERS`.
- **cargo-make (T-006):** `monitor`, `coverage`/`coverage-llvm-cov`, `generate-schema`,
  `clippy-flow` (→ clippy-strict), and `integ` tasks.
- **Docs (T-005, T-010, T-011):** real README; CLAUDE.md rewritten as a runbook with an
  accurate structure tree; `src/controllers/CLAUDE.md`.
- **Testing/layout (T-013, T-014, T-015, T-021):** `.story/` scaffold verified at fleet
  parity; cucumber BDD harness (`tests/cucumber/**`, `spec/**`, in-memory mocks, `cargo make
  integ` green); committed `tern/schema.sql` snapshot; migration round-trip test (DB-gated,
  skips green).
- **build.rs / init (T-019, T-020):** provenance warning gated behind `CI`; init-service.sh
  renames `crates/client/` → `crates/<service>-client/` (verified end-to-end with a full
  `cargo build`).
- **Policy (T-022):** decided NOT to commit `openapi.json`/add a drift check (aviary-aligned);
  rationale in note **N-002**.

## Open follow-ups (issues filed this session — NOT yet fixed)
- **ISS-001 (high):** `generate-openapi` / CI build with `-p __SERVICE_NAME__` but the package
  is `__service_name__` → the un-instantiated template can't build the workspace. This is the
  root blocker that several tickets had to work around (substituted-name verification). **Best
  next task.** Fix: use the snake `__service_name__` consistently in `Makefile.toml`
  generate-openapi and `.github/workflows/*` build/clippy/fmt `-p` flags.
- **ISS-007 (medium):** `src/middleware/mocks.rs` `item_store_mock_middleware` injects
  `Box<dyn ItemStore>` but controllers obtain `Arc<dyn ItemStore>` → it's broken/dead. Fix:
  inject `Arc`. (T-014's BDD harness injects its own Arc mock to avoid it.)
- **ISS-008 (medium):** BDD items step builds app/store inline; fine now, revisit if multi-
  request flows need shared `TestWorld` state.
- **ISS-006 (low):** `OPENTELEMETRY.md` Middleware Order section is stale vs
  `src/controllers/mod.rs` (CLAUDE.md now documents the correct order).
- **ISS-003 (low):** fleet-wide `session-start.sh` git-auth hardening (ssh shorthand +
  idempotent `--add`).

## State / next steps
- Branch `storybloq/modernize-template` (22 ticket commits, head `9b45fed`). Not yet pushed /
  no PR opened — the user committed the initial `.story/` scaffold manually at session start;
  open the PR when ready (`gh pr create --base trunk`).
- Verification ran locally throughout: `cargo make integ`, the migration round-trip, and
  `generate-schema` all green; `cargo clippy --all-targets -p __service_name__ -p migration`
  clean. Full `--workspace` clippy/build remains gated by ISS-001 (the client crate).
- Recommended next session: fix **ISS-001** first (unblocks full-workspace build/clippy/CI),
  then **ISS-007**.
