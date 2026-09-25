---
name: lemmaspec
description: Author, validate, explain, and graph typed self-contained .lemmaspec artifacts for implementation plans and code relationships. Use when translating requirements, observed facts, dependencies, constraints, or acceptance criteria into deterministic logic; not for mutable runtime state or persistence adapters.
---

# LemmaSpec

Use LemmaSpec as an executable specification. The agent owns the model and the evidence; the engine owns deterministic derivation and expectation checking.

## Discover the current language

The plugin and the CLI are installed separately. Verify the CLI first:

```sh
lemmaspec --version
lemmaspec intro
lemmaspec syntax
```

`intro` is the orientation for this workflow, kept in the binary so it always
matches the installed version. `lemmaspec agent install` writes this skill into
a project's `.claude/skills` and `.codex/skills` from the same binary, and
`lemmaspec upgrade --check` says whether a newer release exists.

If `lemmaspec` is unavailable, tell the user to install it from the public
release instructions at `https://github.com/MortenHusted/lemmaspec#install`.
Do not run Cargo in the user's current project as a substitute.

Only when deliberately testing source changes from a LemmaSpec repository
checkout, use the repository build:

```sh
cargo run --quiet -- --help
cargo run --quiet -- syntax
```

The remaining examples use the preferred installed binary. Substitute
`cargo run --quiet --` only in that source-development case. If a documented
command is rejected, report the installed version before changing the artifact.

## Author an artifact

1. Decide the question the artifact must answer. Keep one coherent implementation, feature, or analysis boundary per file.
2. Translate verified or explicitly supplied observations into `fact` blocks. Never promote a guess to a base fact merely to make an expectation pass.
3. Declare every predicate as a typed `relation`. Prefer relations that remain meaningful outside one sentence, such as `requires`, `changes`, `calls`, `blocked_by`, `implemented`, or `satisfies`.
4. Express consequences and invariants as `rule` blocks. Use stable descriptive IDs for facts, rules, and expectations.
5. Express acceptance criteria as `expect` blocks with exact counts. An expectation is a claim to test, not a desired value to force.
6. Keep the evidence's standing explicit. Facts may declare `basis: snapshot`, `policy`, `observed`, or `reviewer_declared`. Each basis requires a `source` string; `snapshot` and `observed` also require an `identity` string. This metadata states the producer's claim and does not authenticate authorship or prove truth. Untyped facts remain reviewer-declared, including facts with provenance or full confidence. Neither citations nor confidence upgrade LLM-authored claims: those stay `reviewer_declared`. Generated observations must come from the named deterministic tool.
7. Write for the reader, not only the engine. Comments before `spec` state the question the artifact answers; keep maintainer guidance in `notes { text: "..." }`. A comment directly above a relation, fact, rule, expectation, or mutation explains it in the rendered guide. Give relations `roles` naming each argument and a `reads` sentence template such as `"{item} depends on {dependency}"` so facts and rule conditions render as prose. Give symbols human labels and source links; unlabelled symbols remain visibly marked.

Run `lemmaspec syntax` rather than guessing the grammar. Important boundaries:

- Variables begin with uppercase; lowercase or quoted values are symbols.
- Every relation used by a fact, rule, or expectation must be declared.
- One relation cannot be both directly asserted and derived.
- `count`, `min`, and `max` are available in rule heads. `sum` and `now` are not available in self-contained artifacts.
- Time or revisions must be ordinary explicit values if the model needs them.

## Evaluate and repair

Run:

```sh
lemmaspec walk path/to/spec.lemmaspec --json
```

Interpret the exit status before changing the file:

- `0`: the artifact is valid and every expectation is satisfied.
- `1`: the artifact is valid, but at least one expectation is unsatisfied. Treat this as meaningful specification or implementation evidence. Do not change facts or counts solely to obtain green output.
- `2`: invocation, file reading, parsing, or validation failed. Repair the syntax or model using the diagnostic on stderr.

Inspect the JSON facts and their `why` trees before trusting a derived conclusion. Distinguish asserted facts from derived facts in any report.

## Mutation-test the specification

When an artifact declares `mutation` policies, run:

```sh
lemmaspec mutate path/to/spec.lemmaspec --json
```

Mutation analysis requires a clean baseline and evaluates every mutant from a fresh parsed artifact clone. Supported operators are:

- `drop_rule`, optionally excluding rule IDs with `except`;
- `drop_condition`, optionally excluding whole rule IDs or named conditions with
  `except`;
- `drop_fact`, scoped by required `relation` and optionally excluding fact IDs.

Name conditions when one condition needs a durable identity or individual
exclusion. Use a keyed `when` map and reference the condition as a quoted
`rule_id.condition_id` value:

```text
rule select_release {
  derive: "selected(Item)"
  when: {
    item_exists: "release(Item)"
    item_is_ready: "ready(Item)"
  }
}

mutation condition_coverage {
  operator: drop_condition
  except: ["select_release.item_is_ready"]
}
```

Omit `must_fail` when any failed expectation may kill a mutant. Set `must_fail: EXPECTATION_ID` when that exact oracle must fail; unrelated failures do not count as a kill. Treat `survived` as an unresolved specification gap, `rejected` as a validation boundary, and `excluded` as explicit policy—not as interchangeable successful outcomes.

Rejected and excluded targets are not valid executions. Every policy must execute at least one valid mutant or it is `vacuous`; a report containing a vacuous policy exits `1`. An evaluation failure is an analysis error and exits `2`, not a rejected mutant.
Policies with identical configuration are invalid because counting the same
mutants twice would make the report misleading.

## Check evidence against a checker

A checker is an artifact whose facts are a fixture and whose expectations and mutation policies prove the rules bite. To apply it to real code, produce an evidence file: a `spec` containing facts over the checker's relations, source citations and declared bases from their producers, plus the expectations that should hold for that codebase. Labels and notes may accompany the evidence. Agent-authored facts stay reviewer-declared even when they cite code. Then run:

```sh
lemmaspec check path/to/checker.lemmaspec path/to/evidence.lemmaspec --json
```

The checker's fixture and expectations are replaced by the evidence file's; the report and exit status read as for `walk`. Assert only facts you established from the code; never invent a fact to satisfy an expectation. A failed expectation with its `why` witness is the finding: report the derived fact, the rule, and the sources and standings of the asserted facts beneath it. An exemption is a reviewer-declared fact citing the review that granted it, not a deleted piece of evidence.

To keep, render, or project the result, bind it to a file:

```sh
lemmaspec bind path/to/checker.lemmaspec path/to/evidence.lemmaspec
```

This writes `path/to/evidence.bound.lemmaspec` (or the `-o` path), a self-contained artifact that `walk`, `mutate`, `project`, and `render` accept unchanged. It records that checker over that codebase; render it when a human needs to read the finding. Keep private evidence, reports and identifying source references local unless publication is explicitly authorized.

## Project the graph

After the walk represents the intended model, run:

```sh
lemmaspec project path/to/spec.lemmaspec --json
```

The projection is the deterministic, internally closed graph for that artifact:

- IDs are spec-namespaced and every edge endpoint is a node in the same projection.
- Edge relations have fixed endpoint types.
- Symbols are local anchor nodes. Do not replace them with external repository or datastore IDs inside this projection.
- Concrete proof edges retain one deterministic body witness for every producing rule. They are not an exhaustive retraction-impact graph.
- Recursive rules may produce cyclic `depends_on` edges; graph traversals need a visited set.
- The output is a generic inner graph, not a persistence event. It deliberately has no ingest envelope, revision, or clock-derived timestamp.

When identity stability matters, run `project --json` twice in separate processes and compare the bytes.

## Render the human view

Generate the standalone view after the graph represents the intended model:

```sh
lemmaspec render path/to/spec.lemmaspec
lemmaspec render path/to/spec.lemmaspec --source-root . --output path/to/report.html
lemmaspec render path/to/spec.lemmaspec --format md
```

The default output replaces `.lemmaspec` with `.html`; `--format md` uses `.md`. Both formats open on the answers, with failed expectations first, numbered witness steps, and the premises' declared standings and sources. Maintainer notes stay outside this brief. The offline HTML graph starts on the selected answer's witness; the full graph and walkthrough are opt-in. Tell a human reader to press `?` for the guide, and point them to the failed answer or reviewer-declared premise that needs attention.

When source citations are repository-relative, pass `--source-root` with that repository's directory so links resolve from the report's output location. `--source-root .` uses the invocation directory. Without this option, relative links resolve beside the report; no repository root is inferred. HTTPS links and fragments keep their authored destinations.

Observed status says which identity it was recorded against. Pass `--target-identity ID` to compare with an explicit target; a mismatch puts a not-current banner first and exits `1`. A static report does not monitor source changes. Exit `1` still writes the report: preserve and report its open claims or stale observations rather than treating them as a rendering error.

## Report the result

Return:

- the artifact path and the question it models;
- asserted facts versus derived conclusions;
- each expectation and whether it passed;
- node and edge counts when projected;
- the rendered HTML or Markdown path when a human view was requested;
- any failed expectation as unresolved work, not as a parser failure;
- any modeling choice that could materially change the answer.

## Current Lemmalog correspondence

LemmaSpec currently packages a static, replayable subset of Lemmalog:

- Lemmalog assertion (`+`) -> `fact`
- Lemmalog rule installation -> `rule`
- Lemmalog query (`?`) -> `expect` for an exact-count executable claim
- Lemmalog `run` -> implicit in `walk` and `project`
- Lemmalog `dump` and `why` -> the `walk --json` report
- isolated static omission analysis -> `mutation` policies plus `mutate`

Mutation policies clone and reevaluate the static artifact; they do not expose mutable engine state. Do not invent artifact syntax for runtime Lemmalog actions. Retraction, ad hoc queries, authored hypotheticals, batch install/uninstall, change feeds, clocks, and persistence are not part of the current `.lemmaspec` artifact. They are candidate future capabilities whose durable semantics must be designed explicitly.
