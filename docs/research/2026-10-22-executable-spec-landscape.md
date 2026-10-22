# Survey: who and what Dave Farley cites on executable specs for AI coding

**Video:** Dave Farley, Modern Software Engineering, "I Said This Years Ago: Why Prompting AI Code Fails (And What Works)", posted 2026-10-21. <https://www.youtube.com/watch?v=EkH_uoybj8k>. The brief called it "the future of AI coding"; the page title is different, but the video ID and transcript match. All quotes attributed to Farley come from the auto-generated transcript and description on that page.

**Method:** I opened every URL listed under an entry, either as a page extract or a direct fetch. A verdict says whether the opened source supports the claim Farley attributes to it. *Not verified* means I could not find the claim in a primary source. Secondary sources are marked as such.

**Layer key:** INTENT (human-owned statements of behaviour or invariants), EXECUTABLE (runnable checks against the real system), DESIGN (structural or architectural constraints on the code), CODE-FACTS (facts extracted from code), EVIDENCE/TRACE (links between intent, runs and code; approval; drift), PROCESS (workflow discipline).

---

## 1. People and practices

### 1.1 Robert C. "Uncle Bob" Martin
- **Who:** Author of *Clean Code*. He now builds SwarmForge, a multi-agent coordinator.
- **What it does:** SwarmForge runs AI agents in their own git worktrees and tmux sessions. Agents pass committed work to each other, and a local dashboard lets the operator handle "approval gates" and clarifications. The "six-pack" demo runs these roles in order:
  - Specifier: turns tasks into Gherkin acceptance tests and a manual QA procedure.
  - Coder: writes the code and unit tests and builds the acceptance-test harness.
  - Cleaner: refactors until DRY and CRAP gates pass.
  - Architect: reviews the dependency graph and adds property-based tests.
  - Hardener: runs mutation testing at the language level and the Gherkin level.
  - QA: automates the QA procedure as UI scripts.
- **Farley's claim:** Martin no longer reads agent code and instead surrounds agents with unit tests, Gherkin tests, QA procedures, quality metrics and mutation testing. His swarm has a dedicated specifier role. **Supported.** On 23 Jul 2026 he wrote on X: "My current strategy is to not read any of the code written by my agents… surround the agents with extreme constraints. Unit tests, gherkin tests, QA procedures, quality metrics, mutation testing, test coverage…". The Specifier role is documented. Farley adds his own objection: he would rather the specifier were a human, or that a human signed off on its output.
- **Relates to:** PROCESS (role pipeline), EXECUTABLE (Gherkin, mutation), DESIGN (Architect and Cleaner gates), EVIDENCE/TRACE (approval gates only; no link from intent to evidence).
- **Sources:** <https://x.com/unclebobmartin/status/2080257779395154409> · <https://github.com/unclebob/swarm-forge> · <https://cleancoders.com/episode/agentic-discipline-6>

### 1.2 Adam Tornhill (CodeScene)
- **Who:** Founder and CTO of CodeScene, a code-health and behavioural code analysis tool. He wrote *Your Code as a Crime Scene*.
- **What he does:** His post "Practices I Abandoned with Agents: An Ode to Test-Driven Development" (Substack, shared on LinkedIn around Sep 2026) says he no longer uses TDD's fine-grained loop with agents. He iterates at feature level behind "human/agent abstraction boundaries", which are end-to-end tests. Code quality is enforced "via tooling, not manual inspection", meaning CodeScene's MCP servers.
- **Farley's claim:** Tornhill dropped red/green/refactor but kept one rule: drive every change through a failing test, now an end-to-end behaviour spec. **Supported, close to verbatim.** He writes "I always drive any change to the codebase via a failing test… those tests can typically be at a higher level than unit tests." He also writes "With agents, red gives me confidence that my test suite is capable of validating the changes I delegate", which is the same argument Farley makes for red. He gives the reason too: agents had deleted failing tests or changed their conditions to match buggy code.
- **Relates to:** PROCESS (red before green), EXECUTABLE (end-to-end tests), DESIGN (code health enforced by tools).
- **Sources:** <https://adamtornhill.substack.com/p/practices-i-abandoned-with-agents>

### 1.3 Sean Grove (OpenAI). Transcript: "Sha Grove"
- **Who:** Works in alignment research at OpenAI (per the talk page). He gave "The New Code" at the AI Engineer World's Fair 2025.
- **What he says:** Code is a "lossy projection" of intent, so the specification should be the durable artifact. His example is the OpenAI Model Spec: a set of Markdown files in which each clause has an ID and a separate file of challenging prompts that act as success criteria. "Executable" here means deliberative alignment. A grader model scores the responses against the policy, and the same document serves as training data and evaluation data. He also proposes spec tooling such as consistency checkers and ambiguity linters.
- **Farley's claim:** "Make the specification executable, feed the spec to the model and test against the spec." **Partly supported.** The direction matches. However, Grove's executable spec is Markdown prose with example prompts, checked by a model grader. It is not a deterministic test that runs against the system. That is the Markdown form Farley criticises elsewhere in the same video.
- **Relates to:** INTENT (a spec with clause IDs), EXECUTABLE (model-graded evaluations, not deterministic checks), EVIDENCE/TRACE (clause ID linked to example prompts).
- **Sources:** <https://ai.engineer/talks/8rABwKRsec4-specifications-are-the-new-code> (event page with talk summary and timestamps; video <https://www.youtube.com/watch?v=8rABwKRsec4>)

### 1.4 Kent Beck
- **Who:** Created XP and rediscovered TDD.
- **What he says:** On the Pragmatic Engineer podcast (Jun 2025) he called agents "unpredictable genies" and TDD a "superpower" with them, because agents introduce regressions. In a LinkedIn post (around Mar 2026) he wrote: "80% of a payroll system is the same as 0% of a payroll system."
- **Farley's claim:** TDD is a superpower; Beck struggles to stop agents deleting tests; the 80% payroll quote. **All three supported.** Gergely Orosz's write-up says Beck is "having trouble stopping AI agents from deleting tests in order to make them 'pass!'"
- **Relates to:** PROCESS, EXECUTABLE. The deleted-tests problem is the motivation for EVIDENCE/TRACE protection.
- **Sources:** <https://newsletter.pragmaticengineer.com/p/tdd-ai-agents-and-coding-with-kent> · <https://www.linkedin.com/posts/kentbeck_one-of-my-frustrations-with-scientific-activity-7434482205000716288-s3_e>

### 1.5 Emily Bache. Transcript: "Emily Beige", "Emily Bash"
- **Who:** Technical coach (Bache Consulting) and chair of the Samman Technical Coaching Society.
- **What she says:** "Test-Driven Development with Agentic AI" (24 Mar 2026) reports interviews with about half a dozen TDD coaches. Most no longer write code by hand but still read it, partly to avoid "cognitive debt". Agents tend to merge red and green into one step. The test list now lives in a Markdown "spec" that the agent edits. Mutation testing, formal methods, property-based testing and approval testing are now used routinely. One group treats the spec as source code and the LLM as the compiler.
- **Farley's claims:**
  - "All the tests together form the specification, whoever happened to write each kind." **Supported, verbatim**, in "Seven Testing Myths…" (Mar 2026). Her context is Continuous Delivery and team ownership, not agents specifically.
  - Agents delete failing tests. **Supported, same post:** "these AI agents have a nasty habit of deleting failing tests rather than fixing them". She suggests watching for unexpected coverage drops to catch it.
  - Her survey found most practitioners read the code and worry about cognitive debt. **Supported.**
- **Relates to:** PROCESS, EXECUTABLE. Using coverage dips to detect deleted tests is a weak EVIDENCE/TRACE signal.
- **Sources:** <https://coding-is-like-cooking.info/2026/03/test-driven-development-with-agentic-ai> · <https://coding-is-like-cooking.info/2026/03/seven-testing-myths-every-developer-should-stop-believing>

### 1.6 Anthropic: Claude Code best practices
- **What it is:** Anthropic's official best-practices page for Claude Code. It says to give Claude a check that returns pass or fail, so the loop closes without a human. It also recommends:
  - writing specs that end with an end-to-end verification step and running them in a fresh session;
  - using a verification subagent "so the agent doing the work isn't the one grading it";
  - having Claude show evidence (test output, commands run) rather than claim success.
- **Farley's claim:** "If you can't verify it, don't ship it." **Supported, verbatim**, under "The trust-then-verify gap": "Always provide verification (tests, scripts, screenshots). If you can't verify it, don't ship it."
- **Relates to:** PROCESS, EXECUTABLE, EVIDENCE/TRACE (show evidence; separate the grader from the worker).
- **Sources:** <https://code.claude.com/docs/en/best-practices>

### 1.7 Steve Yegge. Transcript: "Steve YG"
- **Who:** Former Amazon and Google engineer. He built the Beads agent memory tool and the Gas Town agent orchestrator, and co-wrote *Vibe Coding* with Gene Kim.
- **What he says:** On X (Sep 2025): "a quarter million lines of heavily reviewed, heavily tested… code in right around six days". In the Gas Town launch post: "17 days, 75k lines of code, 2000 commits" and "I've never seen the code, and I never care to."
- **Farley's claim:** Yegge produces 12,000 lines a day with agents. **Not verified in a primary source.** The 12,000 figure appears only in a secondary recap of a podcast. His own posts give other rates: roughly 40k lines a day in the X post (11 Sep 2025) and roughly 4.4k a day for Gas Town. The point Farley draws from it, that no human can review that volume, still holds.
- **Relates to:** PROCESS (throughput pressure). It is the motivation for EXECUTABLE checks rather than a practice in itself.
- **Sources:** <https://x.com/Steve_Yegge/status/1965991591367295403> · <https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04> · secondary: <https://alifeengineered.substack.com/p/vibe-coding-is-the-only-future-steve>

### 1.8 Dan Luu. Transcript: "Danlu"
- **Who:** Dan Luu, author of danluu.com, known for measurement-heavy essays.
- **What he says:** "Agentic test processes, LLM benchmarks, and other notes on agentic coding from Galapagos Island" (undated; it mentions GPT-5.5) benchmarks agent configurations over many runs. He argues that testing is the rate-limiting factor for highly agentic work, and that agents running in self-improving loops are bad at the benchmarking problem that follows.
- **Farley's claim:** "Anything that's not well tested gets stochastically degraded." **Supported, verbatim:** "testing is a rate-limiting factor in highly agentic workflows because when you let agents start doing a lot of things, anything that's not well tested gets stochastically degraded. The more you ship with agents, the worse this gets."
- **Relates to:** EXECUTABLE, PROCESS. Statistical evaluation over many runs is a form of EVIDENCE.
- **Sources:** <https://danluu.com/ai-coding/>

### 1.9 Gojko Adzic. Transcript: "Go Adzi"
- **Who:** Author of *Specification by Example*.
- **What he says:** In the LinkedIn article "Spec Driven Development – revenge of Waterfall or BDD taken to new level?" he reports trying Spec Kit with Claude Code at Citcon. The generated "spec" had given/when/then acceptance criteria but at the level of a scope of work. The agent then "tried to do too much", which made keeping a human in the loop impossible. He concludes that Spec Kit "does not, really" relate to Specification by Example.
- **Farley's claim:** The Markdown spec lacked detail, and the real spec ended up in unit and integration tests. **Supported, verbatim:** "This is not a spec, it lacks a ton of detail. The real 'spec' then ends up being in unit and integration tests… readable only for developers."
- **Relates to:** INTENT (asks for a human-readable source of truth "detailed enough for people to approve/complain about"), EXECUTABLE, PROCESS (a missing scoping phase).
- **Sources:** <https://www.linkedin.com/pulse/spec-driven-development-revenge-waterfall-bdd-taken-gojko-adzic-imquf/> · context: <https://tessl.io/blog/a-look-at-spec-kit-githubs-spec-driven-software-development-toolkit>

### 1.10 Birgitta Böckeler (Thoughtworks). Transcript: "Burgittita Berkeler", "Gita Berkeler"
- **Who:** Distinguished Engineer at Thoughtworks. She writes the "Exploring Gen AI" series on martinfowler.com.
- **What she says:** "Understanding Spec-Driven-Development: Kiro, spec-kit, and Tessl" (15 Oct 2025) sorts SDD into three levels:
  - spec-first: a spec is written before the task;
  - spec-anchored: the spec is kept and maintained after the task;
  - spec-as-source: only the spec is edited by humans.

  She compares spec-as-source to model-driven development (MDD), and finds Kiro spec-first, Spec Kit nominally spec-anchored and Tessl the only one exploring spec-as-source.
- **Farley's claims:**
  - The agent did not follow instructions. **Supported, verbatim:** "Even with all of these files and templates and prompts and workflows and checklists, I frequently saw the agent ultimately not follow all the instructions."
  - Small work packages seem to contradict SDD. **Supported, verbatim:** "small work packages almost seem counter to the idea of SDD."
- **Relates to:** PROCESS, INTENT. Her taxonomy is a useful axis for the INTENT→EVIDENCE question.
- **Sources:** <https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html>

### 1.11 Colin Eberhardt (Scott Logic). Transcript: "Colin Eberhart"
- **Who:** CTO of Scott Logic.
- **What he says:** "Putting Spec Kit Through Its Paces: Radical Idea or Reinvented Waterfall?" (26 Nov 2025) used Spec Kit to add one small feature to a real app. It took about 33m30 of agent time and produced 2,577 lines of Markdown for 689 lines of code. He found iterative prompting more effective.
- **Farley's claim:** "Code is law… Markdown specs lack formality." **Supported, verbatim:** "Code is law because it is formal language you can reason about. You can test it. You can prove it is right or wrong. Specifications, or at least ones expressed in the markdown format of Spec Kit, lack this formality."
- **Relates to:** INTENT (critique of informal intent), PROCESS.
- **Sources:** <https://blog.scottlogic.com/2025/11/26/putting-spec-kit-through-its-paces-radical-idea-or-reinvented-waterfall.html>

### 1.12 Markus Eisele, writing for O'Reilly. Transcript: "Marcus is writing for O'Reilly"
- **Who:** Developer advocate and author. The article is "Why AI Coding Agents Still Need Clear Specs", O'Reilly Radar, 8 Jul 2026, republished from his newsletter *The Main Thread*.
- **What he says:** Total cost follows a U-shaped curve as the spec becomes more complete, and the minimum sits around acceptance criteria and BDD scenarios. Multi-agent pipelines need typed contracts plus validators. Agents can draft and stress-test specs, but a human reviews them. "Validate your spec before you scale the implementation."
- **Farley's claim:** BDD collapses spec writing and spec validation into one artifact. **Supported, verbatim:** "Behavior-driven development, when done well, collapses spec writing and spec validation into the same artifact. A Gherkin scenario is simultaneously a description of intent and an executable test." Eisele's section is titled "How BDD *partially* solves this". He warns that Gherkin can carry semantic ambiguity, and he is more relaxed than Farley about agents drafting specs.
- **Relates to:** INTENT, EXECUTABLE, PROCESS.
- **Sources:** <https://www.oreilly.com/radar/why-ai-coding-agents-still-need-clear-specs>

### 1.13 Ian Cooper
- **Who:** Software engineer, known for the talk "TDD, Where Did It All Go Wrong?". Its message is that tests should be triggered by behaviours, not by methods or classes.
- **What he says:** In "Is AI a Silver Bullet?" he writes that "TDD becomes a powerful tool when you ask the AI to implement code for your tests." On X (3 Oct 2026): "AI will write tests that couple to details rather than behaviors, making your test suites an obstacle to refactoring… You have to drive here."
- **Farley's claim:** "If the AI writes the tests, it isn't TDD", which Cooper said "years ago". **Exact wording not verified.** The position is supported: the human writes behaviour tests and the AI implements against them. I found no source with that sentence.
- **Relates to:** PROCESS, INTENT (tests express behaviour, not implementation).
- **Sources:** <https://ian-cooper.writeas.com/is-ai-a-silver-bullet> · <https://x.com/ICooper/status/2106371001164460052>

### 1.14 Martin Fowler. Likely misattributed; the source is Birgitta Böckeler on martinfowler.com
- **What the source says:** Böckeler's "Maintainability sensors for coding agents" (martinfowler.com, May 2026) runs a set of sensors: linting, dependency rules, coupling data, coverage and incremental Stryker mutation testing. One file showed 100% statement coverage but 13 surviving mutants, because a large acceptance test executed it without checking its effect. She concludes that mutation testing is "crucial" once most testing is left to AI.
- **Farley's claim:** "Martin Fowler… [says acceptance tests] are often not very assertion-heavy, giving us a false sense of security", and that Fowler recommends mutation testing. **The quote is verbatim, but the author is Böckeler**: "acceptance tests increase coverage, but are often not very assertion-heavy, giving us a false sense of security in test effectiveness". I found no passage by Fowler himself on this in his Fragments page.
- **Counter-evidence:** Böckeler's "TDD inside the agent loop – theater or actual value?" ran a small exploratory experiment. Telling agents to do TDD produced no quality advantage, "no meaningful difference in mutation scores", and 3–8.5× the tokens. She has "stopped telling my coding agents to write tests first". This applies to fine-grained TDD, not to acceptance-level ATDD.
- **Relates to:** EXECUTABLE (mutation testing as a measure of test strength), CODE-FACTS (coupling data), DESIGN (dependency rules).
- **Sources:** <https://martinfowler.com/articles/sensors-for-coding-agents.html> · <https://martinfowler.com/articles/exploring-gen-ai/tdd-in-the-agent-loop.html> · checked: <https://martinfowler.com/fragments>

---

## 2. Spec-driven tools

### 2.1 GitHub Spec Kit
- **What it is:** An open-source toolkit. The `specify` CLI installs slash commands into the user's coding agent. The flow is: write a project constitution once, then specify → plan → tasks → implement for each feature. All artifacts are Markdown files in the repository.
- **What it does:** The plan step generates data models, API contracts and test scenarios, and tasks are derived from them. The methodology document `spec-driven.md` calls specs "the source of truth, with code as the generated output". Its example constitution includes a "Test-First Imperative": "No implementation code shall be written before: 1. Unit tests are written 2. Tests are validated and approved by the user 3. Tests are confirmed to FAIL (Red phase)." That rule is written as instructions to the LLM. The tool does not enforce it or record that it happened.
- **Farley's claim:** "Intent is the source of truth." **Supported, verbatim**, in Den Delimarsky's launch post (GitHub Blog, 2 Sep 2025): "We're moving from 'code is the source of truth' to 'intent is the source of truth.'" Farley's broader charge, that SDD is "ATDD with the executable part left out", is too strong for Spec Kit's own methodology document. That document asks for human-approved tests that are seen failing first. The charge is fair for what practitioners report actually getting (Adzic, Eberhardt).
- **Relates to:** INTENT (Markdown spec), PROCESS (phases and constitution). EVIDENCE/TRACE is aspired to in prose but not captured.
- **Sources:** <https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/> · <https://github.com/github/spec-kit> · <https://github.com/github/spec-kit/blob/main/spec-driven.md>

### 2.2 Amazon Kiro. Transcript: "Kirao"
- **What it is:** An AWS agentic IDE and CLI. Each spec produces three files:
  - `requirements.md` (or `bugfix.md`): user stories plus acceptance criteria in **EARS**, written as `WHEN [condition/event] THE SYSTEM SHALL [expected behavior]`;
  - `design.md`;
  - `tasks.md`.
- **What it does:**
  - **Correctness (IDE only):** each acceptance criterion becomes a "for any…" property and a Hypothesis property-based test. The developer can navigate from the spec text to the test. When the test finds a counterexample, the developer decides whether to fix the code, the spec or the test (blog, 17 Nov 2025).
  - **Bugfix specs:** these hold Current, Expected and Unchanged Behavior sections. Kiro generates property-based tests for all three, including tests "that verify the current implementation has the bug" (18 Feb 2026).
  - **Requirements analysis:** an LLM translates the EARS criteria into SMT-LIB. WHEN, IF and WHILE clauses become antecedents of implications. A solver then finds contradictions (as minimal conflicting sets), completeness gaps, and accepted or rejected scenarios that may be surprising (12 May 2026).
- **Farley's claim:** Kiro is built around structured acceptance criteria. **Supported, and the criteria are EARS.** Kiro also does more than Farley credits: it turns criteria into executable property-based tests, and for bugs it shows the test failing before the fix.
- **Relates to:** INTENT (EARS), EXECUTABLE (property-based tests), EVIDENCE/TRACE (spec-to-test navigation, bug reproduced before the fix), CODE-FACTS (none).
- **Sources:** <https://kiro.dev/docs/specs/> · <https://kiro.dev/docs/specs/feature-specs/> · <https://kiro.dev/blog/property-based-testing/> · <https://kiro.dev/blog/specs-bugfix-and-design-first/> · <https://kiro.dev/blog/deep-spec-analysis/>

### 2.3 Tessl. Transcript: "Tesla"
- **What it is:** A startup that in 2025 shipped the beta Tessl Framework, a CLI and MCP server for spec-driven development. Its definition: "specs — not code — are the primary artifact". Böckeler observed per-file specs with `@generate` and `@test` tags, code headed `// GENERATED FROM SPEC - DO NOT EDIT`, and non-deterministic regeneration. Tessl's current docs describe a different product: a platform for managing agent context (rules, skills, plugins, a registry, evaluations). Its registry still offers a `spec-driven-development` package, where specs are `.spec.md` files with YAML `targets` (code globs), inline `[@test]` links to the tests that verify each requirement, and a `check-spec-links.sh` script that checks those links resolve to real files.
- **Farley's claim:** The spec, not the code, is the maintained artifact, and code can be regenerated. **Supported for the 2025 framework** (via Böckeler and Tessl's own definition). The company has since moved its focus to agent context and skills.
- **Relates to:** INTENT, EVIDENCE/TRACE (spec to test via `[@test]`; spec to code via `targets`; existence checks only, not run results).
- **Sources:** <https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html> · <https://tessl.io/registry/tessl-labs/spec-driven-development> · <https://docs.tessl.io/introduction-to-tessl/core-concepts.md>

### 2.4 Thoughtworks Technology Radar: spec-driven development
- **What it is:** Thoughtworks' twice-yearly technology assessment.
- **What it says:** The SDD blip appeared in **Assess** in Nov 2025. It warns that workflows "remain elaborate and opinionated" and that some tools "generate lengthy spec files that are hard to review". It adds: "We may be relearning a bitter lesson", namely that "handcrafting detailed rules for AI ultimately doesn't scale." The page is now marked "not on the current edition".
- **Farley's claim:** SDD was placed in Assess. **Supported.**
- **Relates to:** PROCESS.
- **Sources:** <https://www.thoughtworks.com/radar/techniques/spec-driven-development>

---

## 3. Farley's own model

### 3.1 Four-layer acceptance test model and LMAX
- **What it is:** The four layers, from top to bottom:
  1. test cases, written as executable specifications in the language of the problem domain;
  2. a DSL that turns behaviour into reusable operations;
  3. protocol drivers, one per channel (UI, public API, FIX API and so on);
  4. the system under test.

  Farley's 2015 BCS slides show the parts: the DSL ("Separation of 'What' from 'How'"), channel drivers over the system under test, and stubs for external systems. One test is annotated `@Channel(fixApi, dealTicket, publicApi)`, so **the same spec runs across several client channels**. Thierry de Pauw's notes on the talk quote "Tests are written against the Driver… the Driver talks the protocol of the system." Farley's training prospectus names the "Four-Layer Model" / "Four-Layer Approach". It also lists "Using Acceptance Testing as a precise language to write prompts" and "Verifying AI-generated code with Acceptance Tests".
- **LMAX:** LMAX Exchange open-sourced **Simple-DSL**: "a style for writing acceptance tests used at LMAX Exchange that aims to balance human and machine readability". Farley's book *Continuous Delivery Pipelines* has an LMAX case study with sections titled "The Commit Stage in 4 Minutes" and "The Acceptance Stage in 40 Minutes".
- **Farley's claim:** Built at LMAX in 2007, with "tens of thousands of these specifications against every commit". **Model supported. The 2007 date and the test count are not verified:** none of the pages I opened states them.
- **Relates to:** INTENT (domain-language test cases), EXECUTABLE (drivers against the real system), DESIGN (the layering separates what from how). The channel annotation maps directly onto auth rules that must hold across browser-session, API, MCP and agent clients.
- **Sources:** <https://edinburgh.bcs.org/events/2015/acceptance_testing_for_continuous_delivery.pdf> · <https://thinkinglabs.io/notes/2021/07/18/acceptance-testing-for-continuous-delivery-dave-farley.html> · <https://continuous-delivery.co.uk/downloads/Acceptance_Testing_3_Courses_Prospectus.pdf> · <https://github.com/LMAX-Exchange/Simple-DSL> · <https://leanpub.com/cd-pipelines>

### 3.2 Farley's ATDD skill for AI coding agents
- **What it is:** A Claude Code skill bundled with his paid course "Acceptance Testing: BDD From Stories to Executable Specifications" (cd.training). The course page calls it a "FREE Claude ATDD Skill trained on Dave Farley's methodology". His LinkedIn article (16 Oct 2026) says it "coaches you in this approach, reviews your acceptance tests, and helps you to build the" supporting infrastructure. The video's description link redirects (via bit.ly and a mailing tracker) to the course with coupon `atdd30`.
- **Farley's claim:** The skill walks you through building the layers and holds the agent to the principles. **Partly verified.** The skill exists and is sold with the course. It is not public, so I could not see what it enforces. Third-party skills that copy the four-layer model exist (for example `rkhozinov/claude-skills` on mcpmarket.com) but are not Farley's.
- **Relates to:** PROCESS, INTENT, EXECUTABLE.
- **Sources:** <https://courses.cd.training/courses/atdd-from-stories-to-executable-specifications-team> · <https://www.linkedin.com/pulse/what-300-developers-told-us-working-coding-agents-dave-farley-njxre> · <https://mcpmarket.com/tools/skills/atdd-four-layer-acceptance-testing>

---

## 4. Sponsors (brief)

### 4.1 Equal Experts
A product-development consultancy ("3,600+ consultants across 5 continents"); the video description says it embraces Continuous Delivery from the outset. It is not specific to the layer model. **Relates to:** PROCESS. <https://www.equalexperts.com/>

### 4.2 TransFICC. Transcript: "Transfix", "Transfig"
A fixed-income e-trading connectivity and workflow-automation fintech; Farley says it applies continuous delivery practices. Not tool-relevant. **Relates to:** none directly. <https://transficc.com/>

### 4.3 Octopus Deploy
Continuous delivery and deployment orchestration for multi-cloud, Kubernetes and on-premises targets. **Relates to:** PROCESS (pipeline), and EVIDENCE/TRACE only for release records. <https://octopus.com/>

### 4.4 Zügel by hello2morrow. Transcript: "Zoo"
- **What it is:** An **MCP server** from hello2morrow, the makers of Sonargraph, that "teaches your architecture to your AI coding agent".
- **What it does:** The agent asks Zügel before adding a dependency. Each proposed dependency "is checked against your rules and for new cycles before any code is written", and a refusal "comes with a correct alternative". Rules are written in Sonargraph's architecture DSL. Zügel keeps a full in-memory dependency model (Java, C#, Python), lists every violation with permitted alternatives, computes breakup sets for cycles, and keeps a "prevention ledger". In a jPOS case study, component cyclicity fell 79%.
- **Farley's description:** "Sidekick for your AI coding agent… architectural conscience… prevents structural erosion… cleans up legacy codebases." **Matches the vendor page.**
- **Relates to:** DESIGN, CODE-FACTS (dependency model), EVIDENCE/TRACE (prevention ledger).
- **Sources:** <https://www.hello2morrow.com/products/zugel>

### 4.5 SmartBear BearQ
- **What it is:** SmartBear's agentic QA product.
- **What it does:** Agents map an application's pages, flows and states, run real user journeys end to end through the UI and API, create and maintain tests, and re-explore when the app changes. Humans choose "where human approval is required" and can review or stop agent activity. It integrates with Jira and Linear. The page does not say tests are linked to requirements.
- **Relates to:** EXECUTABLE (generated tests), PROCESS (approval controls). Note the tension with Farley's argument: tests discovered by exploring today's application describe what it does now, not what it was meant to do.
- **Sources:** <https://smartbear.com/product/bearq/>

---

## 5. Adjacent tools

### 5.1 Cucumber / Gherkin
"Gherkin uses a set of special keywords to give structure and meaning to executable specifications." Each step's text is matched to a step definition in code, and scenarios can be grouped under `Rule`. This is the standard BDD binding from prose scenario to automation. **Relates to:** INTENT, EXECUTABLE. Step definitions roughly correspond to Farley's DSL layer. <https://cucumber.io/docs/gherkin/reference/>

### 5.2 Serenity BDD: Screenplay pattern
Tests model **actors** who have **abilities** (for example, interacting with a web browser or querying a database), perform **tasks**, and answer **questions** about system state. It is a concrete, maintained implementation of a DSL plus protocol-driver split, and the ability abstraction maps onto client channels. **Relates to:** EXECUTABLE, DESIGN (test architecture). <https://serenity-bdd.github.io/docs/screenplay/screenplay_fundamentals>

### 5.3 Concordion
A Specification by Example tool. Natural-language specifications (HTML or Markdown) are instrumented so that the specification itself runs as the check, and the output is "living documentation… generated whenever the checks are run". The rendered spec turns green as behaviour is implemented. **Relates to:** INTENT, EXECUTABLE, EVIDENCE/TRACE (the run result is rendered into the intent document). <https://concordion.org/>

### 5.4 FitNesse
"The fully integrated standalone wiki and acceptance testing framework." Stakeholders write wiki pages containing tables, and those pages run as tests through fixtures. Still maintained (v20260313). **Relates to:** INTENT, EXECUTABLE. <https://fitnesse.org/>

### 5.5 Pact
"A code-first tool for testing HTTP and message integrations using contract tests." Contract tests "assert that inter-application messages conform to a shared understanding that is documented in a contract", replacing brittle end-to-end integration tests. I did not verify the broker and deployment-gating features. **Relates to:** EXECUTABLE (at service boundaries). <https://docs.pact.io/>

### 5.6 Specmatic
"Turns API specifications into executable contracts, continuously verifying that every change made by AI coding agents and engineers stays true to the desired behaviour." Works from OpenAPI and similar specs, without writing test code. **Relates to:** INTENT (the API spec), EXECUTABLE. <https://specmatic.io/>

### 5.7 ArchUnit
"A free, simple and extensible library for checking the architecture of your Java code using any plain Java unit test framework." It checks dependencies between packages and classes, layers and slices, and cycles. These are architecture fitness functions written as unit tests. **Relates to:** DESIGN, CODE-FACTS (bytecode import). <https://www.archunit.org/>

### 5.8 Mutation testers (one per ecosystem)
All four seed small faults into the code and check whether the tests notice. A surviving mutant means a hole in the spec. This is the use Farley, Martin (SwarmForge's Hardener, including Gherkin-level mutation) and Böckeler describe.
- **PIT (JVM):** "Faults (or mutations) are automatically seeded into your code, then your tests are run. If your tests fail then the mutation is killed." <https://pitest.org/>
- **Stryker (JS/TS, C#, Scala):** more than 30 mutators and reports on surviving mutants. Böckeler's script reads its JSON report to rank tests as "weak, unused, or top-killer", which maps tests to the mutants they kill. <https://stryker-mutator.io/>
- **cargo-mutants (Rust):** "finding places where bugs can be inserted without causing any tests to fail." <https://mutants.rs/>
- **mutant (Ruby):** tagline "AI writes your code. AI writes your tests. But who tests the tests?". It has an incremental mode for CI and is "free for open source" (`--usage opensource`). Each alive mutation means either the original code is redundant or the tests are missing semantics. <https://github.com/mbj/mutant>

**Relates to:** EXECUTABLE (strength of the checks), EVIDENCE/TRACE (per-test kill data), CODE-FACTS (mutant locations).

### 5.9 CodeQL
"A language and toolchain for code analysis." It "extracts a single relational representation of each source file" into a database, which is then queried in QL, a declarative logic query language. Mainly used for security analysis. **Relates to:** CODE-FACTS, DESIGN (custom queries can enforce structural rules). <https://codeql.github.com/docs/codeql-overview/about-codeql/>

### 5.10 Glean
"An open-source code indexing system that stores typed, schema-defined facts about source code in a queryable database… queried with Angle, a Datalog-style query language." Facts cover definitions, references, calls, inheritance and imports. The site says coding agents query it "instead of relying on grep". **Relates to:** CODE-FACTS. <https://glean.software/>

### 5.11 AWS Cedar and Cedar Analysis
- **What it does:** Cedar is an authorization policy language with `permit` and `forbid` policies plus `when`/`unless` conditions over principal, action, resource and context. Its decision rule is: "If any forbid policy evaluates to true, then the final result is Deny. Else, if any permit policy evaluates to true… Allow", and the default is deny. A schema validates policies when they are written.
- **Cedar Analysis** (open-sourced 16 Jun 2025) contains a symbolic compiler, implemented in Lean with soundness and completeness proofs, that turns policies into SMT formulas checked by CVC5. It can compare two policy sets (equivalent, more permissive, less permissive, incomparable) and answer "Does this change to my policies grant any new, unintended permissions?". It also finds shadowed permits, impossible conditions and complete denials.
- **Relates to:** INTENT (policy as code), EXECUTABLE (the authorizer), EVIDENCE/TRACE (semantic diff between policy versions). It is the most direct fit for an auth rule that grows from browser-session-only to API, MCP, agent and public access.
- **Sources:** <https://docs.cedarpolicy.com/> · <https://docs.cedarpolicy.com/auth/authorization.html> · <https://docs.cedarpolicy.com/policies/syntax-policy.html> · <https://aws.amazon.com/blogs/opensource/introducing-cedar-analysis-open-source-tools-for-verifying-authorization-policies>

### 5.12 Open Policy Agent (OPA) and Rego
A general-purpose policy engine that "decouples policy decision-making from policy enforcement". Callers send JSON input and get decisions back. Rego "was inspired by Datalog and extends it to support structured document models such as JSON". I did not verify any semantic policy-diff analysis comparable to Cedar's. **Relates to:** INTENT (policy as code), EXECUTABLE (decisions). <https://www.openpolicyagent.org/docs> · <https://www.openpolicyagent.org/docs/policy-language>

### 5.13 TLA+ and Quint
TLA+ (Lamport; now run by the TLA+ Foundation) is "a high-level language for modeling programs and systems—especially concurrent and distributed ones", and used to eliminate "fundamental design errors" before code. Quint is a typed language with TLA-style semantics that calls itself "the executable spec that turns your system's intended behavior into checkable, verifiable evidence". It has a simulator and the Apalache model checker. Its `quint-connect` crate generates ITF traces that are replayed against real Rust code, which is model-based conformance testing. **Relates to:** INTENT (invariants), EXECUTABLE (model checking), EVIDENCE/TRACE (trace replay against the implementation, in Quint's case). <https://lamport.azurewebsites.net/tla/tla.html> · <https://quint-lang.org/>

### 5.14 Alloy
"An open source language and analyzer for software modeling." It finds counterexamples with its model-finding engine, Kodkod. Alloy 6 (Jan 2025) adds mutable state and temporal logic. It suits small relational models, such as an access-control model, where you want a counterexample to a claimed invariant. **Relates to:** INTENT, EXECUTABLE (bounded analysis of the model, not of the code). <https://alloytools.org/>

---

## Cross-cutting observations

- **Where they agree:** Prose intent alone fails. Martin, Tornhill, Eisele, Adzic, Eberhardt, Böckeler, Anthropic and Dan Luu all end up with a runnable check as the real contract. Agents attack that contract: Beck, Bache and Tornhill all report agents deleting or weakening tests. So the check must be protected or human-owned. Farley, Cooper and the Spec Kit constitution say so; Eisele and Martin are fine with agents drafting it if a human reviews it.
- **Where they conflict:** Who writes the spec and the tests: Farley and Cooper (human), Martin (a Specifier *agent* plus approval gates), Spec Kit (LLM writes, user approves). Step size: Farley wants one scenario per cycle, Tornhill takes deliberately larger agentic steps, and Böckeler measured no gain from TDD inside the agent loop. Format: Grove's flagship executable spec is Markdown with a model grader, the format Farley rejects. Reading code: Martin and Yegge don't; most of Bache's practitioners do.
- **(a) Approved behaviour → runnable check:** Partially covered. Gherkin, Concordion and FitNesse make the spec text the test, but they have no notion of human approval. Kiro links each EARS criterion to a generated property-based test you can navigate to. Tessl's `[@test]` links and Spec Kit's "tests validated and approved by the user" express the link, but the link is only checked to resolve (Tessl) or is an instruction to the LLM (Spec Kit). SwarmForge and BearQ have approval gates, but the approval is not tied to a specific behaviour statement.
- **(b) Evidence the check failed before it passed:** Everyone preaches it: Farley ("red is the proof that your specification is capable of catching a wrong answer"), Tornhill, Spec Kit Article III. The only tool I found that mechanises it is **Kiro's bugfix spec**, which generates tests confirming the current code has the bug. None of the sources say that red→green evidence is stored as a durable artifact attached to the approved behaviour.
- **(c) The code a check reaches:** Not covered. Tessl's `targets` declare which files a spec covers but do not measure it. Stryker's per-test kill data (as Böckeler uses it) and coverage tools measure which code the suite reaches, but do not tie it to an intent statement. CodeQL, Glean, Zügel and ArchUnit produce rich code facts with no link back to behaviours. **No surveyed tool closes the triangle (approved intent ↔ red-then-green run evidence ↔ reached code).**
- **Drift detection is indirect everywhere:** Adzic's "real spec ends up in tests", Bache's coverage dips as a signal of deleted tests, and mutation survivors as "spec holes" are all after-the-fact signals. None of these tools flags that an approved behaviour's check was edited, weakened or deleted, which is the exact failure Beck, Bache and Tornhill report.
- **Invariants with context-dependent exceptions:** Cedar comes closest for auth. Forbid overrides permit, `when`/`unless` conditions carry context, and Cedar Analysis can show that version N+1 is "more permissive" than N, or prove the two equivalent. Kiro's SMT analysis encodes WHEN/IF/WHILE antecedents and finds contradictions and gaps, but only within one snapshot of the requirements. TLA+, Quint and Alloy can model state and temporal invariants, but exceptions are something the modeller encodes by hand.
- **Exceptions that change over time:** Not covered as a first-class concept. Cedar diffs two policy versions but does not record *why* an exception was approved or tie it to an intent statement or acceptance scenario. Nothing in the survey models "invariant + approved exception set + history of exception changes + the check per exception". Example: "only browser sessions may mutate X", later amended to "…or API, MCP and agent clients with scope S", plus a public read-only area.
- **Channel-parameterised specs exist but are under-tooled:** Farley's `@Channel(fixApi, dealTicket, publicApi)` and Serenity's actors-with-abilities let one behaviour run across several client types. That is what an auth rule spanning browser, API, MCP and agent clients needs. But no tool joins the channel matrix to a policy engine such as Cedar or OPA, so that a policy diff tells you which acceptance scenarios must change.
- **Layer coverage summary:** INTENT is well served (Gherkin, EARS, Cedar, TLA+/Quint). EXECUTABLE is well served (Cucumber, Pact, Specmatic, property-based tests, mutation testing). DESIGN and CODE-FACTS are well served (ArchUnit, Zügel, CodeQL, Glean). PROCESS is crowded (Spec Kit, Kiro, SwarmForge, Farley's skill). **EVIDENCE/TRACE is the thin layer.** Its fragments are Kiro's spec-to-test navigation and bug reproduction, Tessl's link checks, Concordion's rendered results, SwarmForge's approval gates and Cedar's version diff, and none of them connects to the others.

---

### Unresolved or unverified
- **Steve Yegge, "12,000 lines/day":** only a secondary podcast recap. His primary posts give about 250k lines in roughly 6 days (Sep 2025) and 75k lines in 17 days (Gas Town).
- **Ian Cooper, "If the AI writes the tests it isn't TDD":** I found no source with this wording. His position is supported.
- **Martin Fowler, "assertion-light acceptance tests" and mutation testing:** the quote is Birgitta Böckeler's, published on martinfowler.com. I found no statement by Fowler himself.
- **LMAX "2007" and "tens of thousands of specifications against every commit":** the model and LMAX Simple-DSL are verified. The date and the count are not.
- **Farley's ATDD skill contents:** it exists and is bundled with the course, but its contents are not public.
