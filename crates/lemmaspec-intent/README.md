# lemmaspec-intent

An optional consumer of LemmaSpec. It keeps a typed catalog of what a system is
for, reads tagged `cargo test` integration tests as the examples that prove it,
records who approved each statement and example and which runs failed and
passed, and reports a standing for every statement. The LemmaSpec engine does
not depend on this crate. Only `record` runs project code; every other command
reads files and git objects.

Build it from a LemmaSpec checkout. Release binaries do not include it.

```sh
cargo install --path crates/lemmaspec-intent
```

The catalog must live in a git work tree. Run the commands from the
repository's top-level directory: `--dir` defaults to `intent` and `--tests` to
`tests`, both relative to the current directory.

## Walkthrough

Start from a git repository holding a Rust package named `greeter`, with
`/target` in `.gitignore`, and this `src/lib.rs`:

```rust
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

Write the root catalog, `intent/catalog.intent`:

```text
catalog greeter {
  channel library {
    text: "The greeter Rust library."
  }
  intent greets_people {
    text: "The greeter greets people by name."
  }
  definition greeting_names_the_person {
    refines: greets_people
    text: "A greeting is Hello, the person's name, and an exclamation mark."
  }
}
```

Tag a test in `tests/greeting.rs`:

```rust
/// intent: example hello_ada exemplifies greeting_names_the_person on library
#[test]
fn greets_ada_by_name() {
    assert_eq!(greeter::greet("Ada"), "Hello, Ada!");
}
```

Check the catalog, commit it, and read the report. `record` runs
`cargo test --locked`, so commit `Cargo.lock` too; `cargo generate-lockfile`
creates it.

```sh
lemmaspec-intent check     # exits 0
git add -A && git commit -m "Add the intent catalog"
lemmaspec-intent report    # exits 1: four items await approval
```

Record red. Edit `src/lib.rs` so `greet` returns `format!("Hi, {name}!")`,
then record the example, confirm it failed, and revert the break:

```sh
lemmaspec-intent record --example hello_ada    # hello_ada on library: fail
git checkout -- src/lib.rs
```

The run, with the diff of the break, is now in `intent/runs.intent`. Approve
the statements and the example, as whoever the project designates, and commit:

```sh
lemmaspec-intent approve library greets_people greeting_names_the_person hello_ada --by "Ada Lovelace"
git add -A && git commit -m "Approve the greeter catalog"
```

Record green on the clean tree, read the report, and commit the run:

```sh
lemmaspec-intent record    # hello_ada on library: pass
lemmaspec-intent report    # exits 0
git add intent/runs.intent && git commit -m "Record green for the greeter catalog"
```

The report now shows `greeting_names_the_person` and `greets_people` as
verified, and still does at the new commit, because the run ledger is outside
the source identity. Change any tracked file and they are unverified until you
record green again. Edit the test and `hello_ada` awaits approval again, so the
report exits 1.

## The catalog

The catalog declares four kinds of statement. Each has an ID and required
prose in `text`.

| Kind | Fields | States |
|---|---|---|
| `intent` | `text` | What the system is for. |
| `definition` | `refines`, `text` | What a domain term means for the system. It refines one intent. |
| `exception` | `narrows`, `kind`, `channels`, `text` | That one definition differs on the named channels. |
| `channel` | `text` | A way the system is used, such as its command line or its library. Examples run on channels. |

An exception has one of two kinds:

- `does_not_apply`: the definition does not hold on those channels. It needs no
  examples.
- `holds_instead`: the exception's own text holds on those channels, and
  examples prove it there.

A definition applies on every approved channel except those named by its
approved, non-stale exceptions of either kind.

### Syntax

A catalog file is `catalog NAME { ... }`: `use` imports first, then statement
blocks. A block is `KIND ID { field: value ... }`. Values are double-quoted
strings on one line, with the escapes `\n`, `\r`, `\t`, `\"` and `\\`; IDs;
`alias.id` references; and lists in brackets. `//` starts a comment. Fields
can come in any order, but an unknown or repeated field is an error, and
`channels` is a nonempty list without repeats.

The same statements split across files:

```text
// intent/catalog.intent: the root
catalog greeter {
  use "./channels.intent" as channels
  use "./greeting.intent" as greeting
}

// intent/channels.intent
catalog channels {
  channel library {
    text: "The greeter Rust library."
  }
  channel cli {
    text: "The greet command line."
  }
}

// intent/greeting.intent
catalog greeting {
  use "./channels.intent" as ch

  intent greets_people {
    text: "The greeter greets people by name."
  }
  definition greeting_names_the_person {
    refines: greets_people
    text: "A greeting is Hello, the person's name, and an exclamation mark."
  }
  exception cli_prints_a_line {
    narrows: greeting_names_the_person
    channels: [ch.cli]
    kind: holds_instead
    text: "The command line prints the greeting followed by a newline."
  }
}
```

### Root and imports

- The catalog is `intent/catalog.intent` and every file it imports,
  transitively.
- `use "PATH" as ALIAS` resolves `PATH` relative to the importing file. An
  absolute path, or one that climbs out of the catalog directory, is an error.
- A bare ID refers to a statement in the same file. A statement in another file
  is `alias.id`, where `alias` is one of this file's own imports.
- IDs are unique across every file of the catalog.
- An import cycle is an error. A file reached along two paths loads once. A
  missing file is reported at the `use` that names it.
- An `.intent` file anywhere under the catalog directory that the root does not
  reach is an error. The exceptions are `approvals.intent` and `runs.intent`,
  which the tool reads by fixed path.
- An unknown reference, or a link to the wrong kind, is an error that names the
  file and line: `refines` names an intent, `narrows` a definition, and
  `channels` channels.

Moving a statement to another file or renaming an alias leaves its digest
unchanged, so splitting a file changes no standing. Plans, memspec slices and
LemmaSpec checkers outside the catalog cite a statement by its bare ID.

## Examples

An example is a `#[test]` function in a file directly inside `tests/` with this
doc-comment line:

```text
/// intent: example <id> exemplifies <statement> on <channel>, <channel>
```

- `<id>` names the example. It is unique among examples and statements, and is
  what `approve` and `record --example` take.
- `<statement>` is a definition or a `holds_instead` exception. For an
  exception, every channel must be one it names.
- Tests in inline modules are found, with the path `module::function`. Tests in
  `src/` and in subdirectories of `tests/` are not read.
- A doc-comment line that starts with `intent:` but does not parse, or that
  names an unknown statement or channel, a statement an example cannot
  exemplify, or a channel outside the exception's own, is listed under "Ignored
  examples" with its reason, and the rest of the report still renders. So is a
  tagged test whose `module::function` path another test in the same file
  shares, because `cargo test --exact` cannot tell the two apart.
- Two tags that claim one example ID are an error.
- A missing `tests/` directory holds no examples.

`record` runs an example once per channel it is tagged for, with
`LEMMASPEC_INTENT_CHANNEL` set to that channel. A test tagged for more than one
channel must choose its channel driver from that variable. A test that does not
read it is tagged for one channel only; otherwise every channel's run exercises
the same code.

```rust
/// intent: example hello_grace exemplifies greeting_names_the_person on library, cli
#[test]
fn greets_grace_by_name() {
    let greeting = match std::env::var("LEMMASPEC_INTENT_CHANNEL").as_deref() {
        Ok("library") | Err(_) => greeter::greet("Grace"),
        Ok("cli") => greet_on_the_command_line("Grace"),
        Ok(channel) => panic!("no driver for channel {channel}"),
    };
    assert_eq!(greeting, "Hello, Grace!");
}
```

Name every channel the test drives and panic on any other. Plain `cargo test`
sets no channel, so the `Err(_)` arm picks the driver it runs. A catch-all arm
would let a tag for a new channel pass by running another channel's driver.

## Digests

Digests are SHA-256 over a versioned canonical rendering, `intent-digest/1`.

- A statement's digest covers its kind, its prose with runs of whitespace
  collapsed, its links as resolved IDs, and an exception's kind and sorted
  channels. It leaves out the file, the position, the alias spelling and
  comments.
- An example's digest covers the test function's tokens, its attributes and
  doc comments included, so the tag is covered. Formatting and comments are
  not. The attributes of the file and of every module around the test are
  covered too, since a `#[cfg]` there decides whether the test is compiled.
- Helpers, fixtures and channel drivers that a test calls are not covered.
  `greet_on_the_command_line` above can change without any approval.

Approvals and runs store the digest version. Those recorded under another
version are listed under "Not evaluated" and do not count.

## Approvals: `intent/approvals.intent`

Only `approve`, `reconfirm` and `remove` write this file, and they only append
to it. Each `approval { ... }` entry names the item and its kind, the action,
the approver, the item's digest and digest version, the digest of each
statement the item links to, an example's channels, and a statement's prose.
Entries count in file order, and the last entry for an item wins.

- Anyone, human or agent, may draft and change statements and examples.
  Approval is a separate step, and who may take it is the project's policy. The
  tool records the name given to `--by` and does not authenticate it.
- Channels are approved like other statements, and a definition applies only
  on approved channels.
- An item is unapproved when it was never approved, or when its digest differs
  from its latest approval. Links and channel tags are inside the digest, so
  changing either makes the item unapproved.
- An item is stale when a statement it links to now reads differently, or was
  last approved with a different digest, than when the item was approved.
  Reverting the linked statement does not clear this. `reconfirm` records the
  current digests of the linked statements for an item whose own text has not
  changed.
- Deleting an approved statement, example or channel tag is itself a change.
  Until `remove` records the item's removal, or `approve` records the example
  without the tag, the report treats it as still present, with no passing run.
  `remove` is refused while approved items still link to the target, unless
  the same call removes them too.

Never edit the file by hand. On a rebase conflict in it, keep the base's
entries first and the branch's entries after them: order decides which entry
wins, and `report --base` needs the base's file as a prefix of the current one.

## Runs: `intent/runs.intent`

Only `record` writes this file, and it only appends to it. Each `run { ... }`
entry names the example, its digest and digest version when it ran, the
channel, the source identity, the base commit (HEAD) and the outcome. A failing
run also stores the diff of the tracked files against the base commit, without
the approval log and the run ledger. Never edit the file by hand. On a rebase
conflict in it, keep the base's entries first and the branch's entries after
them.

`record` runs each example from the top-level directory of the git work tree:

```sh
cargo test --locked --test <file stem> -- --exact <module::function>
```

It reads the outcome from the exit status and libtest's last summary line. One
test passed is `pass`, one failed is `fail` and one ignored is `skip`; anything
else, such as a build failure or a path that matches no test, is `error`.
`record` exits 0 whatever the outcomes, and prints one line per run:
`<example> on <channel>: <outcome>`.

The source identity is SHA-256 over the sorted path, mode and git blob ID of
every tracked file, without `approvals.intent` and `runs.intent`. A submodule
counts as the commit it has checked out. A clean
checkout has exactly its commit tree's identity on every platform, so
committing runs and approvals leaves the identity as it was, and an uncommitted
scratch edit gets an identity of its own. Untracked files are neither in the
identity nor in the stored diff, so `git add` new files before recording.
`record` appends nothing when the identity changes while it runs.

### Red runs

Each example must have failed, with its current text, on every channel it is
tagged for where its statement applies. The failing run may come from any
source, including an uncommitted scratch break, and need not come before the
passing run. Only a run in which the test itself ran and failed counts: a build
error, a skip or a missing test does not.

1. Break the behavior the example checks, on purpose, in a tracked file, so the
   test still builds and fails.
2. Run `lemmaspec-intent record --example <id>`, adding `--channel <channel>` to
   narrow it, and confirm every printed outcome is `fail`.
3. Revert the break and commit `intent/runs.intent`.

The stored diff keeps the break reviewable: `report --base <commit>` lists each
failing run added to the committed ledger since that commit, with its diff.
Editing an example after its red run leaves the old runs at the old digest, so
record red again.

### Green runs

A passing run counts only at the source identity the report evaluates, which is
the working tree's. Record green on a clean tree, confirm every printed outcome
is `pass`, and commit the ledger:

```sh
lemmaspec-intent record
git add intent/runs.intent && git commit -m "Record green"
```

The run ledger is outside the source identity, so the commit that adds the runs
keeps the identity they were recorded at, and the report at that commit counts
them. Any later change to a tracked file gives a new identity, and every
statement that needs a passing run is unverified until green is recorded again.

A failing or erroring run at the target keeps the example from passing there.
`record` exits 0 whatever the outcomes and `report` exits 1 only while
something awaits approval, so a regression shows as an unverified statement,
not as an exit status. Read the outcomes `record` prints.

## Standings

Every intent, definition and exception gets exactly one standing, the first of
these that holds:

1. `unapproved`: its text, links or channel tags differ from its latest
   approval, or it was deleted without an approved removal.
2. `stale`: a statement it links to changed since it was approved.
3. `unverified`: one of the conditions below fails.
4. `verified`: otherwise.

- A definition, or a `holds_instead` exception, is verified when it and its
  examples are approved as they read now and none is stale, it applies on at
  least one channel, each such channel has at least one example tagged for it,
  and every example tagged for that channel has failed there with its current
  text and passes there cleanly at the target.
- A `does_not_apply` exception is verified when it is approved and not stale.
- An intent is verified when it is approved and not stale, at least one
  definition refines it, and every definition refining it and every exception
  narrowing those definitions is verified.

Every standing other than verified lists its reasons. Channels and examples get
no standing; they appear under "Awaiting approval" while unapproved or stale.

## Report

The Markdown report has these sections:

1. A header naming the evaluated source identity and whether it matches the
   tree of HEAD.
2. Awaiting approval: every changed, added or removed statement or example, and
   every stale dependent, with reasons.
3. Statements: each intent, definition and exception in ID order, with its
   prose, file, links, examples, standing and reasons.
4. Ignored examples and Not evaluated, when there are any.
5. Since base, with `--base COMMIT`: the approval entries added since that
   commit with their approvers, and the failing runs added to the committed
   ledger with their diffs. When a file's content at the base is not a prefix
   of its content now, the report says it was altered.

The same catalog, approval log, run ledger, tests and source always produce
the same report.

## Commands

```text
lemmaspec-intent check [--dir DIR]
lemmaspec-intent approve ID... --by NAME [--dir DIR] [--tests DIR]
lemmaspec-intent reconfirm ID... --by NAME [--dir DIR] [--tests DIR]
lemmaspec-intent remove ID... --by NAME [--dir DIR] [--tests DIR]
lemmaspec-intent record [--example ID] [--channel CHANNEL] [--dir DIR] [--tests DIR]
lemmaspec-intent report [--dir DIR] [--tests DIR] [--base COMMIT] [--format md|html] [-o PATH] [--bound PATH]
lemmaspec-intent checker
lemmaspec-intent intro
```

- `check` loads and validates the catalog. Exits 0 when it is valid.
- `approve` appends an entry approving each item as it reads now, with the
  current digest of each statement it links to. Exits 0.
- `reconfirm` does the same for items whose own digest has not changed since
  their latest approval, which clears staleness. Exits 0.
- `remove` appends an approved removal of each approved item. Exits 0.
- `record` runs the selected examples, every example on every channel it is
  tagged for by default, and appends one run each to `DIR/runs.intent`. Exits 0
  whatever the outcomes.
- `report` prints the report, or writes it to `-o`. It reads runs only from
  `DIR/runs.intent`; a missing ledger holds no runs. `--format html` renders
  through LemmaSpec's HTML renderer. `--bound` writes the checker bound to the
  projected evidence, which `lemmaspec walk` and `lemmaspec render` accept.
  Exits 1 while anything awaits approval, and 0 otherwise, even when statements
  are unverified.
- `checker` prints the bundled checker. Exits 0.
- `intro` prints the workflow for coding agents. Exits 0.

Every command exits 2 on an error and names it on stderr. `approve`,
`reconfirm` and `remove` write nothing when any ID fails, and `report` builds
the whole report before writing any file, so an error leaves existing files
untouched.

## Agents

`lemmaspec-intent intro` prints the agent workflow: draft statements, tag
tests, record red on a deliberate scratch break and revert it, run `report`,
and never hand-edit the approval log or the run ledger except to resolve a
rebase conflict. An agent approves only when the project designates it. The
LemmaSpec skill points agents at `intro` when a repository has an `intent/`
catalog.

## Known gaps

- Forged approvals and runs on a branch are not rejected. Change is surfaced,
  not policed: a forged approval still shows, with its approver, among the
  approvals since the base.
- The recorder is not isolated from test hooks or harness configuration at the
  version under test. A hook that turns failures into passes is outside example
  digests. A hook is a tracked file, so changing one changes the source
  identity: green runs recorded before the change stop counting, and the change
  itself is in the diff a reviewer reads.
- The run ledger grows by one run per example and channel each time green is
  recorded at a new source identity. Nothing prunes it.
- Shared helpers, fixtures and channel drivers are not in an example's digest.
  A weakened helper changes no digest and needs no approval.
- `record` finds an example's test target by its file stem. A `[[test]]` entry
  in `Cargo.toml` that gives another file that name runs other code, and
  `Cargo.toml` is in no example's digest.
- A failing run stores the diff of every tracked file against HEAD, not only
  the break. Record red from a tree whose only uncommitted change is the break.
- Scratch breaks are not kept under a git ref. A failing run stores the break's
  diff for review instead.
- Approvers are not authenticated, who may approve is not enforced, and
  approvals are not signed.
- Only `cargo test` integration tests in `tests/*.rs` are read as examples.

## Library and self-test

`lemmaspec_intent::CHECKER` is the bundled checker, `intent_checker.lemmaspec`.
It decides standings over the projected evidence. Its synthetic fixture covers
the standing rules, and its `drop_rule` and `drop_condition` mutation policies
leave no survivors under `lemmaspec mutate`. Run the suite with
`cargo test -p lemmaspec-intent`.
