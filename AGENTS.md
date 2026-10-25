# AGENTS.md — LemmaSpec

Contribution rules, gates and release steps are in `CONTRIBUTING.md`.

## Moving to Stele

Decided 2026-10-25. Nothing moves yet; this records the direction.

- LemmaSpec will continue as part of **Stele**, a new open-source project
  from `MortenHusted` that combines LemmaSpec with codemap, a deterministic
  extractor of code facts. Spec files will take the `.stele` extension.
- This repository stays as its own repository, with its history and
  releases. Once Stele is published, new LemmaSpec updates are made in
  Stele, and this repository points there.
- Until Stele is published, changes still land here.
