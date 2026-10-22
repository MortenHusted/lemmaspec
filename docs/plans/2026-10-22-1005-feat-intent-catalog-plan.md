---
title: Intent Catalog - Plan
type: feat
date: 2026-10-22
topic: intent-catalog
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# Intent Catalog - Plan

## Goal Capsule

- **Objective:** A developer working with coding agents can tell, for any commit, which human-approved intents, domain definitions and exceptions are proven by examples that have been seen failing and that pass on every channel they apply to, and which are unapproved, stale or unverified.
- **Product authority:** The repository owner approves statements and examples. This plan covers the intent model, its verified-statement report, and run recording for one test framework. Execution binding beyond that framework, toolset packaging, code tie-in and an approval UI are not active scope.
- **Open blockers:** None.

---

## Product Contract

### Summary

A typed intent catalog lives in a project's repository next to its code. It holds human-approved intents, the domain definitions that refine them, exceptions that narrow a definition for named channels, and the project's channels, all joined by typed links. The project's own acceptance tests point at the statement they exemplify, and a checker reports which statements are verified at a given commit.

### Problem Frame

Coding agents now write more code than a person can review, so what keeps a system aligned with intent is a check that runs and that agents cannot quietly weaken. Practitioners report the opposite happening: agents delete failing tests or edit them until they pass. Markdown specs record intent but never run. Tests run, but no test is tied to an approved statement, and nothing notices when an approved check changes.

Two cases shaped this plan. In a CRM, the intent "a consultant always knows when to follow up on each open sales process" was first realized as a next-touch date on each sales process. Consultants actually steer by their next task, so the follow-up became the due date of the earliest open task. The intent survived; the meaning of "next follow-up" changed. The old date was read by the dashboard, the filters, inline editing, an import job and several MCP tools, so a browser channel and an agent channel both depended on the old meaning. Nothing recorded which places still used it or whether the passing tests still checked the right thing.

In the second case, an app that serves only browser-authenticated users adds API, MCP and agent clients, and later a public area. "Every request is authenticated by a browser session" turns out to be a mechanism. The intent that lasts is that private data is visible only to an allowed principal. Each new channel, and the public area, is a change someone must approve and that every existing definition must account for.

The existing tools each cover part of this. LemmaSpec evaluates evidence deterministically but has no notion of intent. In `crates/lemmaspec-plan`, requirements are labels, and acceptance claims can attach only to units and gates. memspec links slices and keeps hash-chained history, but it models coupled state, not intent. The surveyed landscape has the same gap: no tool links an approved statement to evidence that its check failed before it passed (`docs/research/2026-10-22-executable-spec-landscape.md`, cross-cutting observations).

### Key Decisions

- **The scope is the intent model.** Execution binding and toolset packaging follow as separate plans that reuse its vocabulary. (session-settled: user-approved — chosen over starting with execution binding or toolset shape: both depend on the intent model's terms.)
- **Domain definitions are a human-approved layer between intent and code.** A change of meaning needs sign-off; a change of code behind an unchanged definition does not. Governs R2, R21. (session-settled: user-approved — chosen over rewriting the intent on every meaning change, or leaving definitions to agents: the follow-up case changed meaning without changing intent.)
- **A definition holds on every channel unless an approved exception scopes it out.** Governs R3, R4. (session-settled: user-approved — chosen over per-channel opt-in, or keeping channels out of intent: a new channel must not silently skip an intent.)
- **The first deliverable is the verified-statement report.** Governs R17, R18, R19, R20, R21, R22, R23, R24. (session-settled: user-approved — chosen over change impact, orphan detection and a channel coverage view: it needs no code facts.)
- **The catalog is a typed, linked format, not Markdown, and examples are the project's own tagged tests.** Typed links let a change be traced to everything that depends on it. Governs R5, R6, R7, R8, R9. (session-settled: user-directed — chosen over a Markdown catalog: references must be checkable so dependent changes can be tracked.)
- **Exceptions narrow definitions, never intents.** An intent that needs an exception is stated at the mechanism level, as "authenticated by a browser session" was. Governs R3.
- **Exceptions come in two kinds.** One states that a definition does not apply on a channel; the other states what holds there instead and carries its own examples. Governs R3, R18, R19, R21. (session-settled: user-approved — chosen over requiring a tested exception for every channel a definition does not concern: each new channel would force placeholder examples onto unrelated definitions.)
- **Run recording for one test framework is in scope.** Without it the plan could be finished on fixture runs while no real project ever gets a report. Governs R11. (session-settled: user-approved — chosen over narrowing the objective to fixture-only runs: the objective is a report for any real commit.)
- **A failing run may come from a deliberately broken scratch commit.** Behavior that already works has no natural failing run, so requiring red on an earlier commit would leave it unverified forever. Governs R20. (session-settled: user-approved — chosen over requiring red on an earlier commit or a killed mutant: old commits may no longer build, and mutation testing stays deferred.)
- **Agents draft; only humans approve.** The catalog must stay agent-driven without letting an agent's edit count as approved. Governs R12, R13, R14, R15, R16.
- **The catalog is an optional companion to LemmaSpec.** A project adopts it one statement at a time, and LemmaSpec without the catalog is unchanged. Governs R25, R26.

### Requirements

```mermaid
flowchart TB
  I[Intent]
  D[Domain definition] -->|refines| I
  X[Exception] -->|narrows| D
  X -->|scoped to| C[Channel]
  D -.->|applies on every channel unless excepted| C
  E["Example: tagged acceptance test"] -->|exemplifies, on channels| D
  E -->|exemplifies, on channels| X
  R["Recorded run: digest, channel, commit, outcome"] -->|of| E
  S["Other specs: plans, memspec slices, checkers"] -->|cite| I
```

**Catalog statements**

- R1. The catalog declares four kinds of statement: intent, domain definition, exception and channel. Each statement has required prose text and a stable ID that is unique across every file of the catalog.
- R2. A domain definition refines one intent and states what a domain term means for the system, such as "a process's next follow-up is the due date of its earliest open task".
- R3. An exception narrows one definition for named channels, either by stating that the definition does not apply there or by stating what holds there instead.
- R4. A definition applies on every declared channel except the channels named by its approved exceptions.

**Links and files**

- R5. Statements link to each other with typed links, and each link points from the dependent statement to the statement it depends on.
- R6. A catalog can span several files that import one another, and a reference across files resolves and is checked exactly like one within a file.
- R7. An unknown reference, a cycle, a duplicate ID across files, or a link between the wrong kinds of statement is an error that names the offending reference.
- R8. Artifacts outside the catalog (examples, plans, memspec slices, LemmaSpec checkers) may cite a statement by its bare ID, while the catalog's only references to other files are the imports R6 allows.

**Examples and runs**

- R9. An example is an acceptance test in the project's own test code that names the definition or exception it exemplifies and the channels it runs on.
- R10. The report reads recorded runs, each naming the example, the example's digest at run time, the channel, the commit and the outcome, and failing runs are kept as evidence.
- R11. The first version reads tagged examples from one test framework, chosen by the first project to adopt the catalog, computes each example's digest, and records every run with its outcome, failures included.

**Approval**

- R12. Agents may draft and change statements and examples, and approval is a separate, explicit step that records the approver, the digest of the approved text, and the digest of each statement the item links to.
- R13. A statement or example whose current text differs from its approved digest is unapproved.
- R14. An item is stale when a statement it links to no longer has the digest recorded at the item's last approval or re-confirmation, and it stays stale until a human re-confirms it.
- R15. Removing an approved statement or example is a change that needs approval, and until it is approved the report treats the item as present with no passing run and names the removal as the reason.
- R16. Approvals and recorded runs count only when they come from a declared source that the project's agents cannot write to under the owner's identity, and the report treats anything else as missing and says why.

**Verified-statement report**

- R17. For a given commit, the report gives every intent, definition and exception exactly one standing: verified, unverified, unapproved or stale.
- R18. A definition, or an exception that states what holds instead, is verified when it and its examples are approved as they read now and none is stale, each channel it applies to has at least one example, and every one of those examples has failed with its current text and passes at the commit.
- R19. An exception that states only that a definition does not apply is verified when it is approved and not stale.
- R20. The failing run that R18 requires may come from any commit, including a scratch commit that deliberately breaks the behavior, and need not come before the passing run.
- R21. An intent is verified when it is approved and every definition refining it, and every exception narrowing those definitions, is verified, and an intent with no definitions is unverified.
- R22. Every standing other than verified states its reason, such as a missing approval, example, channel or failing run, traced back to the facts it rests on.
- R23. The same catalog, recorded runs and commit always produce the same report.
- R24. The report renders for a human reader, showing each statement's prose, links and standing.

**Adoption**

- R25. A catalog with a single statement produces a useful report, and statements without examples show as unverified without blocking the rest.
- R26. LemmaSpec stays usable without the catalog, and its core gains no dependency on it.

### Acceptance Examples

- AE1. **Covers R18.**
  - **Given:** the definition "next follow-up is the due date of the earliest open task" and its example are approved; the example failed with its current text and passes at the commit on the browser and MCP channels; no exception applies.
  - **When:** the report runs.
  - **Then:** the definition is verified.
- AE2. **Covers R13, R18.**
  - **Given:** AE1, except that an agent edited the example after its failing run, and the edited example passes.
  - **When:** the report runs.
  - **Then:** the example is unapproved and the definition is not verified.
- AE3. **Covers R13, R14.**
  - **Given:** the follow-up definition changes from "the next-touch date the consultant sets" to "the due date of the earliest open task".
  - **When:** the report runs.
  - **Then:** the definition is unapproved, and its examples and the exception narrowing it are stale. After a human approves the new definition text, they stay stale until a human re-confirms each.
- AE4. **Covers R4, R18.**
  - **Given:** an MCP channel is added to the catalog and approved.
  - **When:** the report runs.
  - **Then:** every definition without an exception for MCP becomes unverified, with the missing MCP example as its reason, until it has an MCP example that has failed and passes.
- AE5. **Covers R3, R4, R18.**
  - **Given:** an approved exception "public pages need no principal and show no private data" narrows the private-data definition for the public channel and has its own approved example that has failed and passes.
  - **When:** the report runs.
  - **Then:** the private-data definition needs no example on the public channel, and the exception is verified.
- AE6. **Covers R7.**
  - **Given:** an exception links to a definition ID that does not exist.
  - **When:** the catalog is checked.
  - **Then:** the check fails with an error naming the unknown reference.
- AE7. **Covers R21, R25.**
  - **Given:** a catalog holding one approved intent and nothing else.
  - **When:** the report runs.
  - **Then:** the report completes and shows the intent as unverified because no definition refines it.
- AE8. **Covers R15, R18.**
  - **Given:** the private-data definition has two approved browser examples, one passing and one failing, and an agent deletes the failing one without an approved removal.
  - **When:** the report runs.
  - **Then:** the definition is unverified, and the reason names the removed example.
- AE9. **Covers R19, R4.**
  - **Given:** an approved exception states that the follow-up definition does not apply on the public channel, and no public example exists.
  - **When:** the report runs.
  - **Then:** the exception is verified by its approval, and the follow-up definition needs no example on the public channel.
- AE10. **Covers R20.**
  - **Given:** an MCP tool already returns the next follow-up correctly, and its new approved example passes at every real commit but failed at a scratch commit where that tool was deliberately broken.
  - **When:** the report runs.
  - **Then:** the follow-up definition is verified on the MCP channel.

### Success Criteria

- AE1 through AE10 run as the catalog checker's own fixture and produce the stated standings.
- The checker's mutation policies leave no surviving mutants, so every rule and condition is load-bearing for that fixture.
- On the first project to adopt the catalog, the report at a real commit shows at least one definition verified from that project's own tagged tests and recorded runs, and editing one of those tests turns it unapproved.

### Scope Boundaries

**Deferred for later**

- How each test framework tags examples and records runs, beyond the one framework the first version needs.
- Packaging: whether the catalog tool lives with LemmaSpec, with the code-fact extractor, or in a merged toolset.
- Change impact into code and detection of code no approved statement reaches. Both need code facts, and attribute reads such as an ORM column accessor are not resolved call edges today.
- A channel coverage view beyond what the report's standings already show.
- An approval ledger that records why each approval was given; git history is the record for now.
- Semantic comparison of statement versions, such as Cedar's check of whether a new policy version permits more than the old one.
- A TUI or GUI for walking the catalog and approving pending changes.
- Code mutation testing as evidence of how strong an example is.

**Outside this product's identity**

- Generating code from the catalog.
- Counting a model grader's verdict as verification.
- Authenticating approvers.

### Dependencies / Assumptions

- The existing status producer turns only passing gates into `observed` facts, so the recorder in R11 (one-framework run recording) is new work.
- The repository owner is the only approver at first.
- Agents commit under the owner's identity, so the source R16 (trusted approvals and runs) declares must be a step they cannot take, such as CI on a protected branch or a protected path. The owner's commits alone do not qualify. LemmaSpec's basis metadata likewise does not authenticate authorship.
- Test code an example calls outside its digest, such as shared steps, helpers, fixtures and channel drivers, is not protected by approval until the example-digest question below is settled.
- Cross-file references follow memspec's import model: `use "<path>" as <alias>`, qualified IDs, cycles rejected, and a file imported along two paths loaded once. `alias.id` is only how one catalog file refers into another; artifacts outside the catalog cite the bare ID.

### Outstanding Questions

**Deferred to Planning**

- Does the catalog get its own syntax with first-class statement kinds, or LemmaSpec syntax with a fixed relation vocabulary? Readability favors first-class kinds that project to LemmaSpec evidence.
- Where does the catalog tool live? The default is a crate beside `crates/lemmaspec-plan` unless the packaging work decides otherwise.
- How does a human approve or re-confirm: a command, an approval file, or a signed commit?
- What does a statement digest cover: prose only, or prose and links, and is whitespace normalized?
- Does an example's digest cover only its body and tags, or also the test code it calls, such as shared steps, helpers, fixtures and channel drivers? A body-only digest lets a weakened helper go unnoticed.
- Which test framework does the first version read examples from? The first project to adopt the catalog decides.

<!-- ce-section: work-relationships -->
### How This Work Fits Together

This plan covers the intent model, its verified-statement report, and run recording for one test framework. The areas below are the current understanding, not a committed roadmap.

- **Execution binding:** Depends on this plan's statement and example IDs and its first-framework recorder. It extends run recording to further test frameworks and channels.
- **Toolset shape:** Can proceed independently of execution binding. Still to decide: whether the catalog tool lives with LemmaSpec, with the code-fact extractor, or in a merged toolset.
- **Code tie-in** (change impact, unreached code, channel coverage): Depends on this plan and on code facts. Shares this plan's link direction, with code-level facts pointing up at statements.
- **Approval UI:** Depends on this plan's approval step and its unapproved and stale standings.

### Sources / Research

- `docs/research/2026-10-22-executable-spec-landscape.md`: a survey of the people, tools and practices Dave Farley cites on executable specifications, plus adjacent tools, mapped to intent, executable, design, code-fact, evidence and process layers.
- `docs/research/2026-10-22-behavior-checker-sketch.lemmaspec`: a working LemmaSpec sketch of approval by digest, red then green with unchanged text, orphan scenarios and unrequested code. `walk` is clean, and `mutate` kills 33 mutants with none surviving.
- `crates/lemmaspec-plan/README.md`: the projector, checker and status-composition pattern this work extends.
- memspec `docs/grammar-v0.md`, "Imports": the cross-file reference model.
