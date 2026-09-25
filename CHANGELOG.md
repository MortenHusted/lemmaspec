# Changelog

All notable changes to LemmaSpec will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Symbol declarations attach human labels and optional source destinations to
  existing constants, including quoted symbol keys. Metadata appears in the
  graph projection without changing evaluation or proof identities.
- A `notes { text: "..." }` block keeps maintainer guidance separate from the
  question before `spec`. Printing and binding preserve labels and notes.
- A shared source-link policy permits repository-relative paths, fragments,
  and HTTPS destinations; unsafe destinations remain available as plain text.

## [0.3.1] - 2026-09-21

### Fixed

- The rendered view builds its step buttons from DOM nodes instead of an
  HTML string, so CodeQL no longer flags every rendered page with
  `js/xss-through-dom` in the repositories that commit them.

## [0.3.0] - 2026-09-13

### Added

- `lemmaspec check <checker> <evidence>` evaluates one artifact's relations
  and rules over another file's facts and expectations, so a self-tested
  checker can be applied to evidence about real code.
- `examples/state_as_records.lemmaspec`, a Rails checker for the convention
  that togglable state is a record rather than a boolean column.
- Walk reports carry the facts each expectation `found`, and the human
  output lists them under a failed expectation so a finding names its rows.
- `lemmaspec bind <checker> <evidence> [-o <path>]` writes the checker bound
  to the evidence as one self-contained artifact, so walk, mutate, project,
  and render work on the proof record unchanged. Behind it, a printer turns a
  parsed artifact back into source that parses to an equal artifact.

## [0.2.0] - 2026-09-04

### Added

- `lemmaspec intro` orients an agent: what LemmaSpec is for, the loop,
  evidence discipline, and how to write for the human who reads the render.
- `lemmaspec agent install` writes the skill embedded in the binary into a
  project's `.claude/skills` and `.codex/skills`, or runs the agents' plugin
  marketplace commands with `--marketplace`.
- `lemmaspec upgrade` recognises how the binary was installed, asks GitHub
  for the newest release, and runs the matching upgrade; `--check` only
  reports. It also notices when a project's installed skill is behind.
- Comments are kept: everything before `spec` documents the artifact, and a
  comment touching a declaration documents that declaration.
- Relations accept optional `roles` naming each argument and a `reads`
  sentence template; facts carry a `reading` in the projection.
- The HTML render is a journey beside the graph: the question, observations,
  assumptions with their blast radius, reasoning, conclusions with proof
  trees, claims, stress tests, and reference. Each step lights its part of
  the graph; a card and its node are one selection; open claims link to the
  facts found instead; labels are placed so they never overlap; the page
  explains itself behind `?`.

### Changed

- HTML graph view: force-directed layout clustered by relation, with pan,
  zoom, drag, neighbourhood highlighting, type and edge filters, search,
  focus mode, and recursive-cycle detection. Node size, fill opacity, and
  edge width encode connections, fact confidence, and evidence count.

## [0.1.0] - 2026-09-03

### Added

- Typed, self-contained `.lemmaspec` artifacts.
- Deterministic evaluation, explanations, and JSON reports.
- Omission-based mutation analysis with non-vacuous policy checks.
- Closed graph projection and dependency-free HTML rendering.
- Project-local Codex and Claude authoring skills.

[Unreleased]: https://github.com/MortenHusted/lemmaspec/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/MortenHusted/lemmaspec/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/MortenHusted/lemmaspec/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MortenHusted/lemmaspec/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MortenHusted/lemmaspec/releases/tag/v0.1.0
