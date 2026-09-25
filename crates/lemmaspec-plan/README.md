# lemmaspec-plan

An optional consumer of LemmaSpec. It projects an explicit Markdown plan into
labelled, source-addressed evidence. The LemmaSpec engine does not depend on this
crate. Projection and checking never execute project commands.

```sh
cargo run -p lemmaspec-plan -- project crates/lemmaspec-plan/tests/fixtures/plan.md -o /tmp/plan.lemmaspec
cargo run -p lemmaspec-plan -- checker -o /tmp/plan-checker.lemmaspec
cargo run -p lemmaspec -- check /tmp/plan-checker.lemmaspec /tmp/plan.lemmaspec
cargo run -p lemmaspec -- bind /tmp/plan-checker.lemmaspec /tmp/plan.lemmaspec -o /tmp/bound-plan.lemmaspec
cargo run -p lemmaspec -- mutate /tmp/plan-checker.lemmaspec
```

## Input contract

This is a narrow machine-readable subset of `ce-unified-plan/v1`, not a general
Markdown or natural-language interpreter. Existing plans must normalize their
structural declarations to this contract before projection. Narrative prose
outside structural sections is retained in the source document but not inferred
as facts. A successful projection establishes facts about the declarations,
not whether a plan's narrative is adequate or its work is complete.

The plan starts with `---` front matter containing exactly
`artifact_contract: ce-unified-plan/v1`, followed by a closing `---`. Other front
matter entries are ignored. Exactly one `# Plan title` and these four case-sensitive
sections are required: `Requirements`, `Gates`, `Deferred`, `Semantic inputs`.
Sections can use Markdown heading levels 2–6. They contain blank lines, `- ID. Label` items, and optional standalone
`**Bold subsection labels**`, without nested headings. Gates, Deferred and Semantic inputs
may instead contain the sole item `- None`. Requirements must be nonempty.

- Requirements use `R` followed by optional uppercase letters and a number,
  such as `R1`, `RA1`, `RX3`.
- Gates use `G1`; deferred shapes use `D1`; semantic inputs use `S1`.
- All IDs have positive numbers from 1 through 10000, without leading zeroes.
- Unit declarations use `### U1. Unit label`, outside the four list sections.
  At least one unit is required. The source IDs become lowercase symbols.
- Every unit must declare exactly one `**Requirements:**` and one
  `**Dependencies:**` field, on separate lines or the same line. Fields take
  comma-separated IDs or `None`, with an optional final period.
- Requirement fields support ascending, same-prefix ranges such as `RA1-RA3`.
  Every expanded ID must exist. Dependencies may name units (`U1`) or gates (`G1`), including an external
  prerequisite explicitly declared as a gate. Dependency ranges are unsupported.
  References can point forward, but must exist; unit dependencies must be acyclic.

```markdown
---
artifact_contract: ce-unified-plan/v1
---
# Example plan
## Requirements
- RA1. Read the source
- RA2. Validate the result
## Work
### U1. Extract facts
**Requirements:** RA1-RA2. **Dependencies:** None.
## Gates
- G1. Run verification
## Deferred
- D1. Additional formats
## Semantic inputs
- S1. Input contract
```

Each fact cites the given path and its enclosing heading anchor. Symbols retain
item labels and the same citation. Unit coverage/dependency facts cite the unit
heading. Anchors lowercase letters, preserve alphanumeric characters, `_` and
`-`, replace spaces with `-`, and remove other punctuation. Duplicate or empty
anchors are rejected; renderer-specific duplicate-heading suffixes and explicit
HTML anchors are unsupported. Renaming a heading changes its citations.
Every generated fact has `basis: snapshot` and the SHA-256 digest of the exact
Markdown bytes. This identifies the observed document, not a repository tree.

Unknown references, duplicate IDs/fields/references, malformed ranges,
self-dependencies, cycles, empty labels, unsupported lines inside structural
sections, and structural declarations outside their sections fail closed.
Balanced backtick or tilde fenced code blocks (including language tags) are
ignored; apparent structural declarations inside them are never projected.
Unterminated fences are errors.
Tables, checkboxes, setext headings, HTML headings, bold-wrapped IDs, multiline
items, inline annotations on references, implicit gate prose, and semantic or
deferred declarations under alternative section names are unsupported. Field
spelling and punctuation are exact. Unstructured prose cannot declare any of
these relations; all four structural sections are mandatory even when empty.

## Checking structure and acceptance

Projection emits `plan`, `plan_item`, `scheduled`, `requirement`, `covers`,
`depends_on`, `gate`, `deferred`, and `semantic_input`. Every unit, gate and
requirement belongs to its plan through `plan_item`; ownership isolates plan
acceptance. Deferred shapes and semantic inputs preserve scope and context,
without adding acceptance gates.

The checker derives `covered(requirement)` and `uncovered(requirement)` from
coverage by scheduled units. `unit_covered(unit)` and `uncovered_unit(unit)` detect
empty unit coverage. `omitted(shape)` means a deferred shape is not scheduled.
Generated evidence requires zero uncovered requirements and zero uncovered
units, plus positive expectations for declared requirements, units, dependencies,
gates, deferred shapes and semantic inputs. It never copies a derived answer
count into its own expectation. A unit explicitly declaring `Requirements: None`
projects successfully and produces a failing coverage finding.

Missing status is incomplete. `blocked(unit, dependency)` identifies incomplete
dependencies, including gates without acceptance evidence; `ready(unit)` means
only that no declared dependency is incomplete.
It does not mean the unit itself has been implemented or accepted.

Optional status composition uses:

```sh
cargo run -p lemmaspec-plan -- project plan.md --status status.lemmaspec -o /tmp/evidence.lemmaspec
```

Status is a separate evidence artifact with only these facts:

- `acceptance_claim(claim, item)` declares required acceptance evidence for a
  known unit or gate. Each claim has exactly one owner.
- `observed(claim, identity)` records a producer's observation.
- `target_identity(identity)` supplies exactly one target.

The status producer must declare all required gates, restrict acceptance claims
to acceptance-capable probes, and bind observations to the actual repository,
source, plan, probe and tool inputs. The projector does not execute or verify
those probes. File-presence or other progress observations can use `observed`,
but must not be promoted into acceptance claims. Metadata records the producer's
declared evidence basis; it does not authenticate a producer.

The checker admits an item only with at least one acceptance claim and all of
its declared claims observed at the target. No target means no matched claims;
a stale observation does not match. Missing claims, absent status, open gates,
and uncovered requirements/units create `acceptance_blocker(item)` facts.
`accepted(plan)` requires no blocker among the plan's owned items. Deferred
work does not block acceptance. Status composition adds explicit expectations
for `accepted(plan)`, exactly one target, and zero blocked declared dependencies.
Dependency findings bring their positive witnesses into the answer-first report; a structural-only projection makes
no acceptance assertion.

Status cannot add relations, rules, mutations, structural facts, unknown owners,
or conflicting labels/fact IDs/expectation IDs. Identical duplicate declarations
are retained once; repeated acceptance claim IDs and repeated target facts are
errors. Labels for new observation symbols and status notes survive composition.

## Library and self-test

`project_plan(markdown, source_path)` returns `Result<lemmaspec::Artifact, String>`.
`compose_status(plan, status)` validates and combines parsed status evidence.
`CHECKER` exposes the bundled checker. Use LemmaSpec's `print_artifact`,
`check_artifact` and `bind_artifact` APIs for serialization and evaluation.

The generic checker keeps its own synthetic fixtures, independent positive and
negative expectations, and drop-rule/drop-condition mutation policies. They
exercise complete, incomplete, stale, partly observed, uncovered and deferred
cases, including two isolated plan scopes. `check` replaces the fixture with
generated evidence; `bind` creates a closed artifact and drops self-test mutation
policies. Mutate the checker to stress-test its rules; render a bound artifact to
explain a particular plan. Run the public suite with `cargo test -p lemmaspec-plan`.
