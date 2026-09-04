# LemmaSpec

LemmaSpec is an experimental command-line tool for typed, self-contained
software specifications evaluated by a deterministic deductive engine.

```text
.lemmaspec artifact -> typed validation -> logic engine -> executable report
```

An artifact keeps relation schemas, asserted facts, derivation rules,
expectations, and optional mutation policies in one reviewable file. The same
artifact can produce a human report or a deterministic graph for another tool
to consume.

## Install

Versioned GitHub releases provide prebuilt binaries for macOS, glibc-based
Linux on x86_64 or ARM64, and x86_64 Windows.

### Homebrew (macOS or Linux)

```sh
brew install MortenHusted/tap/lemmaspec
```

### Linux (glibc)

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/MortenHusted/lemmaspec/releases/latest/download/lemmaspec-installer.sh | sh
```

### Windows (x86_64)

Run in PowerShell:

```powershell
irm https://github.com/MortenHusted/lemmaspec/releases/latest/download/lemmaspec-installer.ps1 | iex
```

The shell and PowerShell installers place `lemmaspec` in Cargo's binary
directory. Ensure `%USERPROFILE%\.cargo\bin` on Windows or `$HOME/.cargo/bin`
on macOS and Linux is on `PATH`, then verify the installation:

```sh
lemmaspec --version
```

To check for a newer release later, or to upgrade the way the binary was
installed (Homebrew, the release installer, or cargo):

```sh
lemmaspec upgrade --check
lemmaspec upgrade
```

To build the latest development version from source instead:

```sh
git clone https://github.com/MortenHusted/lemmaspec.git
cd lemmaspec
cargo install --path .
```

## Quick start

Inspect the language and evaluate an example:

```sh
lemmaspec syntax
lemmaspec walk examples/release_readiness.lemmaspec
lemmaspec walk examples/release_readiness.lemmaspec --json
```

The example contains the complete model:

```text
spec release_readiness {
  relation depends_on { args: [symbol, symbol] }
  relation incomplete { args: [symbol] }
  relation blocked { args: [symbol] }

  fact release_needs_tests {
    relation: depends_on
    args: [release, tests]
  }

  fact tests_are_incomplete {
    relation: incomplete
    args: [tests]
    provenance: ["plan:test-gate"]
  }

  rule blocked_by_incomplete_dependency {
    derive: "blocked(Item)"
    when: [
      "depends_on(Item, Dependency)",
      "incomplete(Dependency)",
    ]
  }

  expect release_is_blocked {
    query: "blocked(\"release\")"
    count: 1
  }
}
```

`walk` rejects unknown relations, wrong arity, value-type mismatches, and rule
variables used with incompatible types. Its report separates asserted and
derived facts and carries a deterministic `why` witness for every fact.
Repeated walks over the same artifact produce byte-stable JSON.

## Commands

```text
lemmaspec walk <path.lemmaspec> [--json]
lemmaspec mutate <path.lemmaspec> [--json]
lemmaspec check <checker.lemmaspec> <evidence.lemmaspec> [--json]
lemmaspec project <path.lemmaspec> [--json]
lemmaspec render <path.lemmaspec> [-o <path.html>]
lemmaspec syntax
```

- `walk` validates and evaluates the artifact.
- `mutate` tests whether its expectations notice deliberate omissions.
- `check` evaluates one artifact's rules over another file's facts.
- `project` emits its deterministic, internally closed graph.
- `render` writes a dependency-free HTML report beside the artifact.
- `syntax` prints the supported artifact and rule language.

Exit status `0` means the requested check passed, `1` means the artifact is
valid but an expectation, mutation policy, or baseline remains incomplete, and
`2` means the command or artifact is invalid.

## Mutation analysis

Mutation policies can remove rules, named rule conditions, or facts:

```text
mutation semantic_rules_are_observable {
  operator: drop_rule
  except: [known_equivalent_rule]
}

mutation violations_are_guarded {
  operator: drop_fact
  relation: violation
  must_fail: nothing_unexpected
}
```

Each mutant starts from a fresh parsed artifact. Results distinguish
expectation-`killed`, valid `survived`, validation-`rejected`, and
policy-`excluded` mutants. Rejected and excluded targets are not counted as
executions; a policy without a valid execution is `vacuous` and exits `1`.
Evaluation failures are analysis errors and exit `2`.

Without `must_fail`, any previously passing expectation can kill a mutant.
With `must_fail`, that exact expectation must fail, so an unrelated failure
cannot mask a surviving oracle. Duplicate policies are rejected rather than
counting the same mutants twice.

See [examples/mutation_analysis.lemmaspec](examples/mutation_analysis.lemmaspec)
for a complete artifact.

## Checking evidence

A checker is an ordinary artifact whose facts are a fixture and whose
expectations and mutation policies are its self-test. `check` keeps the
checker's relations and rules, replaces its fixture with the facts of an
evidence file, and evaluates the evidence file's expectations instead:

```sh
lemmaspec check examples/state_as_records.lemmaspec .lemmaspec/state_as_records.lemmaspec
```

The evidence file is a `spec` with facts and expectations only; a relation or
rule in it is rejected, and its facts are validated against the checker's
relations. The result is one closed artifact evaluated exactly like a walk,
with a `why` witness for every derived fact, so a finding traces back to the
evidence and its provenance. Evidence is written by whoever can establish it,
an agent reading a schema today or an extractor later; LemmaSpec does not
inspect source code.

[examples/state_as_records.lemmaspec](examples/state_as_records.lemmaspec)
judges a Rails schema fixture against the convention that togglable state is
a record rather than a boolean column.

## Graph and HTML projections

`project` emits nodes for the spec, relations, facts, rules, mutations,
expectations, and symbols, plus typed edges such as `asserts`, `derives`,
`depends_on`, `proves`, `matches`, `expects`, `targets`, and `references_symbol`.
A satisfied expectation is `proves`-linked from the facts that match its query;
an open one is `matches`-linked from the facts found instead.

The graph is internally closed: every ID is spec-namespaced and every edge
endpoint is emitted in the same projection. Symbols that may later resolve to
external definitions remain local anchor nodes. Concrete proof edges retain
one deterministic witness for each producing rule; recursive rules may create
cycles, so consumers should traverse with a visited set.

The projection intentionally has no persistence envelope, revision identity,
or clock-derived timestamp. Those are responsibilities of a separate adapter,
which can translate the stable projection into a datastore-specific contract.
[examples/persistence_adapter_readiness.lemmaspec](examples/persistence_adapter_readiness.lemmaspec)
models that boundary as a deliberately incomplete executable specification.

`render` writes one offline HTML file with the graph on stage and a journey
beside it: the question the artifact answers, then observations, assumptions,
reasoning, conclusions with proof trees, claims, stress tests, and reference.
Each step lights its part of the graph; selecting a card or a node follows one
thread through the argument. The page carries its own guide: press `?` or the
guide button for how to read it. Comments in the source become the prose. Relations can declare `roles` for their arguments and a
`reads` sentence template so facts and rule conditions render as sentences:

```text
// Can the release ship today?
spec release_readiness {
  relation depends_on {
    args: [symbol, symbol]
    roles: [item, dependency]
    reads: "{item} depends on {dependency}"
  }
  ...
}
```

Facts with `provenance` are observations; facts without it, or below full
confidence, are assumptions, listed with everything that would fall if they
turned out to be wrong. Render with:

```sh
lemmaspec render examples/release_readiness.lemmaspec
```

## Agent use

The optional `lemmaspec` agent plugin teaches Codex and Claude Code to author
artifacts from verified observations, preserve failed expectations as useful
evidence, inspect derivations, and emit graph or HTML projections. Install the
CLI first using one of the methods above; the plugin does not bundle the
executable.

Start an agent with the orientation the binary carries:

```sh
lemmaspec intro
```

Install the skill next to the project the agent works in. The binary embeds
the skill it was built with, so skill and CLI never drift:

```sh
lemmaspec agent install            # .claude/skills and .codex/skills in the current directory
lemmaspec agent install --codex --dir ~/work/app
```

Or install through the agents' own plugin marketplaces, which is what
`lemmaspec agent install --marketplace` runs for you:

```sh
claude plugin marketplace add MortenHusted/lemmaspec
claude plugin install lemmaspec@lemmaspec

codex plugin marketplace add MortenHusted/lemmaspec
codex plugin add lemmaspec@lemmaspec
```

Every path loads the same canonical skill from
`plugins/lemmaspec/skills/lemmaspec`.

## Scope

LemmaSpec currently packages a static, replayable subset of its engine:
asserted facts, stratified rules, incremental fixpoint evaluation, confidence
and provenance annotations, explanations, hypotheticals, and change feeds.
The `.lemmaspec` artifact adds typed schemas, executable expectations, graph
projection, and isolated omission-based mutation analysis.

Mutable sessions, persistence, retrieval, clocks, and application-specific
adapters are deliberately outside the artifact format. The direct engine API
also retains one upstream boundary: uninstalling a rule batch containing inline
fact clauses cannot distinguish that batch's support from an independently
declared base fact. Artifacts do not install facts through rule batches.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the complete local gate.

## Lineage and license

LemmaSpec is an independent project inspired by
[Lemmalog](https://github.com/JordyZomer/lemmalog); it is not affiliated with or
endorsed by the Lemmalog project or its author.

Several engine and engine-test files are modified from Lemmalog 0.2.0 at commit
[`74d428a`](https://github.com/JordyZomer/lemmalog/commit/74d428a2497066795f6328946457f22d713fcbd5).
The exact files, upstream copyright, and MIT terms are preserved in
[NOTICE](NOTICE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

LemmaSpec is released under the [MIT License](LICENSE).
