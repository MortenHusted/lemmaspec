---
title: Intent Catalog as a Local Artifact - Plan
type: refactor
date: 2026-10-22
topic: intent-local-artifact
execution: code
status: ready
builds_on: docs/plans/2026-10-22-1005-feat-intent-catalog-plan.md
---

# Intent Catalog as a Local Artifact - Plan

## Context

PR #14 (branch `docs/intent-catalog-brainstorm`, head `54683f6` when this plan
was written) adds a CI job, `Intent catalog`, to `.github/workflows/ci.yml`. The
job runs `record` into a throwaway ledger, then `report`, and fails while
anything awaits approval, so the PR stays red until someone approves. KTD7 of
the intent catalog plan made it that way: green runs were never committed, so
only CI could regenerate them.

The owner decided on 2026-10-22 that the catalog is a self-sustained local
artifact with no CI gate: `intent/` carries its own evidence, and people run the
tool locally.

The code already supports this. The source identity leaves out
`intent/runs.intent` and `intent/approvals.intent`
(`crates/lemmaspec-intent/src/identity.rs`; covered by
`tests/identity.rs::editing_only_the_run_ledger_or_the_approval_log_leaves_the_identity_unchanged`).
A green run recorded on a clean tree can therefore be committed, and the commit
that adds it keeps the identity the run was recorded at. KTD7's reason, "a green
run at a commit cannot be committed in that same commit", does not hold under
KTD6.

After this change:

- `record` writes red and green runs to the committed ledger, and `report`
  reads only that ledger. Nothing runs in CI.
- When the source changes, the report shows statements as unverified until
  someone records green again.
- `report` still exits 1 only while something awaits approval.

## Changes

Three commits on `docs/intent-catalog-brainstorm`.

### 1. `ci(intent): drop the intent job and commit green runs`

- `.github/workflows/ci.yml`: delete the `intent` job.
- `INTRO` in `crates/lemmaspec-intent/src/main.rs`:
  - Add a "Record green" step: on a clean tree, run `lemmaspec-intent record`,
    then commit `intent/runs.intent`. Record again after the source changes.
  - Remove the "as CI runs them" line.
  - "Never approve only to make report or CI pass" becomes "...to make report
    pass".
- `crates/lemmaspec-intent/README.md`:
  - Walkthrough: drop `/ci/` from `.gitignore`. Green is `record`, then
    `report` exits 0, then commit the ledger.
  - "Green runs" section: drop the CI recipe and its `grep` gate, and describe
    the committed-ledger flow.
  - Known gaps: reword the hook gap. A hook change is a tracked change and
    changes the identity, so earlier green runs stop counting. Add one line
    saying the ledger grows by one run per example and channel each time green
    is recorded at a new identity.
- `docs/plans/2026-10-22-1005-feat-intent-catalog-plan.md`:
  - Revise KTD7.
  - Revise the "lemmaspec is the first adopter ... in its own CI" decision.
  - In U7, revise approach steps 4–5, the test scenarios and the verification.
  - Revise "Who finishes" and the Verification Contract's dogfood row.
  - Mark each revision as user-directed on 2026-10-22: a local artifact with no
    CI job.

### 2. `refactor(intent): read runs only from the catalog ledger`

`--ledger` existed only for the CI throwaway ledger.

- `src/main.rs`:
  - Drop `--ledger` from `record` and `report`, from `Args.ledgers` and from
    `USAGE`.
  - `record` always writes `dir/RUNS_FILE`.
- `src/report.rs`: drop `Options.ledgers`.
- `src/project.rs`:
  - Drop `Inputs.extra_ledgers` and the `ledgers` parameter of `Inputs::read`.
  - `ledgers()` returns the catalog ledger only, or is inlined at its two call
    sites (the run loops in `project.rs` and `report.rs`).
- Keep `recorder::record(.., ledger: &Path)` as it is; its tests use it.
- `tests/common/mod.rs`:
  - `record_green()` appends passing runs to `intent/runs.intent`, as
    `record_red` does.
  - `report(base)` loses its ledgers argument.
  - The fixture drops `/ci/` from `.gitignore`.
- `tests/report.rs`:
  - Convert `a_green_run_only_in_an_extra_ledger_verifies_a_definition` into
    `committing_green_runs_keeps_the_definition_verified_at_that_commit`. It
    checks that the definition is unverified before green, verified after
    `record_green`, and still verified after the ledger is committed, with the
    header saying the identity matches HEAD.
  - Update the other `record_green` call sites.
- `tests/cli.rs`:
  - `report_writes_markdown_html_and_the_bound_artifact_to_the_paths_given`
    reads the committed ledger.
  - Drop the `--ledger` case from
    `invalid_options_leave_existing_output_files_untouched`; `--frobnicate`
    already covers unknown options.
  - `record_with_no_examples_creates_no_ledger_and_the_report_reads_a_missing_one_as_empty`
    runs plain `record` and asserts that `intent/runs.intent` is not created.
- README: remove `--ledger` from the Commands section and the usage block.

### 3. `chore(intent): record lemmaspec's green runs`

As the last step, on the final clean tree:

1. Run `cargo run -q -p lemmaspec-intent -- record`.
2. Check that every outcome is `pass`.
3. Commit `intent/runs.intent`.

The report still exits 1, because nothing is approved yet. Approving lemmaspec's
own catalog stays with the owner and no longer blocks the PR.

### After the commits

- Rebase onto `origin/main`, re-run the gates, confirm
  `git merge-base --is-ancestor origin/main HEAD`, and push with
  `--force-with-lease`.
- Rewrite the PR #14 body, starting from `gh pr view 14 --json body -q .body`:
  - Drop "Before merge" and the CI job from the dogfooding line.
  - Mark the review-table row "the CI job fails on any fail or error" as
    withdrawn with the job.
  - Update the test plan.
- Merging PR #14 stays with the owner.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked --all-targets`. The converted report test is
  the proof that a committed green run keeps the report verified at the commit
  that adds it.
- `cargo package --locked -p lemmaspec`
- `lemmaspec walk` and `lemmaspec mutate` on
  `crates/lemmaspec-intent/intent_checker.lemmaspec` stay clean. The checker
  itself does not change.
- Dogfood, after commit 3:
  - `git status` is clean.
  - `report` names an identity that matches HEAD and counts the six examples'
    green runs.
  - `report` exits 1 (awaiting approval) and shows no ignored examples.
- `grep -rn -i -E "\bCI\b|RUNNER_TEMP|--ledger"` over the crate, its README,
  both plans and `plugins/lemmaspec/skills/lemmaspec/SKILL.md` finds no stale
  CI or ledger-option wording.
- On PR #14 the `Intent catalog` check is gone, and every remaining check
  passes.
