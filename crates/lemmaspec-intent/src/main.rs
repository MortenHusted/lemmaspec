use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use lemmaspec::{print_artifact, SourceContext};
use lemmaspec_intent::{
    approvals::{append, approve, reconfirm, remove},
    examples::scan,
    identity::Repo,
    loader::{load, DirReader, RUNS_FILE},
    recorder::{record, CargoTest, Selection},
    report::{report, Options},
    CHECKER,
};

const USAGE: &str = "usage: lemmaspec-intent check [--dir DIR]
       lemmaspec-intent approve ID... --by NAME [--dir DIR] [--tests DIR]
       lemmaspec-intent reconfirm ID... --by NAME [--dir DIR] [--tests DIR]
       lemmaspec-intent remove ID... --by NAME [--dir DIR] [--tests DIR]
       lemmaspec-intent record [--example ID] [--channel CHANNEL] [--dir DIR] [--tests DIR]
       lemmaspec-intent report [--dir DIR] [--tests DIR] [--base COMMIT] [--format md|html] [-o PATH] [--bound PATH]
       lemmaspec-intent checker
       lemmaspec-intent intro

DIR defaults to intent and --tests to tests. report exits 1 while anything awaits approval; every command exits 2 on an error.";

const INTRO: &str = r#"lemmaspec-intent, for an agent

What it is
  An intent catalog in intent/ states what a system is for: intents, domain
  definitions that refine them, exceptions that narrow a definition on named
  channels, and the channels themselves. Acceptance tests tagged as examples
  prove definitions and exceptions. The report gives every intent, definition
  and exception exactly one standing, checked in the order unapproved, stale,
  unverified, verified, and says why anything is not verified.

The catalog
  intent/catalog.intent is the root. A statement in another file is named
  <alias>.<id> after `use "./<file>" as <alias>`; IDs are unique across files.
    catalog shop {
      use "./channels.intent" as ch
      intent orders_are_honest {
        text: "A customer never pays for an order the shop cannot fill."
      }
      definition checkout_checks_stock {
        refines: orders_are_honest
        text: "Checkout refuses an order when a line exceeds the stock."
      }
      exception api_reserves_stock {
        narrows: checkout_checks_stock
        channels: [ch.api]
        kind: holds_instead
        text: "The API reserves the stock when the cart is created."
      }
    }
  intent/channels.intent declares each channel the same way, inside
  catalog channels { channel api { text: "The shop API." } }.
  An exception of kind does_not_apply states only that the definition does
  not hold on its channels.

The loop
  1. Draft statements   edit intent/catalog.intent and the files it imports;
                        lemmaspec-intent check validates imports, IDs and links
  2. Tag tests          one doc-comment line on a #[test] fn in tests/*.rs:
       /// intent: example <id> exemplifies <statement> on <channel>, <channel>
                        A test tagged for more than one channel must choose its
                        channel driver from LEMMASPEC_INTENT_CHANNEL; a test
                        that does not read it is tagged for one channel only.
  3. Record red         git add any new file, then break the behavior under
                        test on purpose, as a scratch edit to a tracked file
                        that still builds, run lemmaspec-intent record
                        --example <id>, and confirm every outcome is fail.
                        Revert only the files you broke (not git checkout .
                        or git stash, which also drop the new runs), then
                        commit intent/runs.intent.
                        error means the test did not run: a broken build, a
                        test path cargo cannot find, or a Cargo.lock that is
                        missing or out of date, since record runs cargo test
                        --locked --test <file> -- --exact <module::fn>. Run
                        that command to see why. error never counts.
  4. Record green       on a clean tree, run lemmaspec-intent record, confirm
                        every outcome is pass, and commit intent/runs.intent.
                        The run ledger is outside the source identity, so that
                        commit keeps the identity the runs were recorded at.
                        Record green again after the source changes.
  5. Report             lemmaspec-intent report
                        exits 0 when nothing awaits approval, 1 while something
                        does, 2 on an error. Passing runs count only at the
                        working tree's source identity.

Files only the tool writes
  intent/approvals.intent   approve, reconfirm and remove append to it
  intent/runs.intent        record appends to it
  Never hand-edit either file. The one exception is a rebase conflict in one:
  keep the base's entries first and the branch's entries after them.

Who approves
  The project decides who approves. Draft and change statements and examples
  freely, but run approve, reconfirm or remove only when the project has
  designated you as an approver:
    approve ID... --by NAME     approves each item as it reads now
    reconfirm ID... --by NAME   clears staleness for an item whose own text
                                has not changed
    remove ID... --by NAME      approves the removal of a deleted item
  If you are not a designated approver, report exiting 1 is the correct end
  state: hand its Awaiting approval list to an approver and stop. Never
  approve only to make report pass.

Reference: https://github.com/MortenHusted/lemmaspec/blob/main/crates/lemmaspec-intent/README.md"#;

struct Args {
    ids: Vec<String>,
    options: BTreeMap<&'static str, String>,
}

impl Args {
    fn parse(command: &str, mut args: impl Iterator<Item = String>) -> Result<Args, String> {
        let (accepted, takes_ids): (&[&'static str], bool) = match command {
            "check" => (&["--dir"], false),
            "approve" | "reconfirm" | "remove" => (&["--by", "--dir", "--tests"], true),
            "record" => (&["--example", "--channel", "--dir", "--tests"], false),
            "report" => (
                &["--dir", "--tests", "--base", "--format", "-o", "--bound"],
                false,
            ),
            "checker" | "intro" => (&[], false),
            _ => return Err(format!("unknown command {command}")),
        };
        let mut parsed = Args {
            ids: Vec::new(),
            options: BTreeMap::new(),
        };
        while let Some(arg) = args.next() {
            match accepted.iter().find(|option| **option == arg) {
                Some(&option) => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{option} requires a value"))?;
                    if parsed.options.insert(option, value).is_some() {
                        return Err(format!("unknown or duplicate option {arg}"));
                    }
                }
                None if takes_ids && !arg.starts_with('-') => parsed.ids.push(arg),
                None => return Err(format!("unknown or duplicate option {arg}")),
            }
        }
        Ok(parsed)
    }

    fn path(&self, option: &str, default: &str) -> PathBuf {
        self.options
            .get(option)
            .map_or(default, String::as_str)
            .into()
    }
}

fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let (command, rest) = args.split_first().ok_or(USAGE)?;
    let args = Args::parse(command, rest.iter().cloned())?;
    let dir = args.path("--dir", "intent");
    let tests = args.path("--tests", "tests");
    match command.as_str() {
        "check" => {
            load(&DirReader(dir))?;
        }
        "approve" | "reconfirm" | "remove" => {
            let by = args
                .options
                .get("--by")
                .ok_or_else(|| format!("{command} requires --by NAME"))?;
            if args.ids.is_empty() {
                return Err(format!("{command} requires at least one ID"));
            }
            let ids = &args.ids;
            append(&dir, &tests, |log, catalog, examples| {
                match command.as_str() {
                    "approve" => approve(catalog, examples, ids, by),
                    "reconfirm" => reconfirm(log, catalog, examples, ids, by),
                    "remove" => remove(log, catalog, examples, ids, by),
                    _ => unreachable!("an approval command"),
                }
            })?;
        }
        "record" => {
            let examples = scan(&tests, &load(&DirReader(dir.clone()))?)?.examples;
            let repo = Repo::of_catalog(&dir)?;
            let selection = Selection {
                example: args.options.get("--example").cloned(),
                channel: args.options.get("--channel").cloned(),
            };
            let ledger = dir.join(RUNS_FILE);
            let mut runner = CargoTest {
                dir: repo.root.clone(),
            };
            for run in record(&examples, &selection, &mut runner, &repo, &ledger)? {
                println!(
                    "{} on {}: {}",
                    run.example,
                    run.channel,
                    run.outcome.as_str()
                );
            }
        }
        "report" => return write_report(&args, dir, tests),
        "intro" => println!("{INTRO}"),
        "checker" => print!("{CHECKER}"),
        _ => unreachable!("Args::parse accepts only known commands"),
    }
    Ok(ExitCode::SUCCESS)
}

fn write_report(args: &Args, dir: PathBuf, tests: PathBuf) -> Result<ExitCode, String> {
    let html = match args.options.get("--format").map(String::as_str) {
        None | Some("md") => false,
        Some("html") => true,
        Some(other) => return Err(format!("unknown format {other}; expected md or html")),
    };
    let output = args.options.get("-o").map(PathBuf::from);
    let report = report(&Options {
        dir,
        tests,
        base: args.options.get("--base").cloned(),
    })?;
    let text = if html {
        let directory = output
            .as_deref()
            .and_then(Path::parent)
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        report.html(Some(&SourceContext::new(Path::new("."), directory)?))?
    } else {
        report.markdown.clone()
    };
    if let Some(path) = args.options.get("--bound") {
        fs::write(path, print_artifact(&report.bound)).map_err(|e| format!("{path}: {e}"))?;
    }
    match output {
        Some(path) => fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?,
        None => print!("{text}"),
    }
    Ok(if report.pending {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("lemmaspec-intent: {error}");
            ExitCode::from(2)
        }
    }
}
