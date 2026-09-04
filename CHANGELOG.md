# Changelog

All notable changes to LemmaSpec will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Comments are kept: everything before `spec` documents the artifact, and a
  comment touching a declaration documents that declaration.
- Relations accept optional `roles` naming each argument and a `reads`
  sentence template; facts carry a `reading` in the projection.
- The HTML render opens with a human guide: the question, observations,
  assumptions with their blast radius, relationships and vocabulary,
  reasoning, conclusions with proof trees, claims, and stress tests, each
  card linked to its graph node.

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

[Unreleased]: https://github.com/MortenHusted/lemmaspec/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MortenHusted/lemmaspec/releases/tag/v0.1.0
