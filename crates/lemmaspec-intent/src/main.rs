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
       lemmaspec-intent record [--example ID] [--channel CHANNEL] [--ledger PATH] [--dir DIR] [--tests DIR]
       lemmaspec-intent report [--dir DIR] [--tests DIR] [--ledger PATH]... [--base COMMIT] [--format md|html] [-o PATH] [--bound PATH]
       lemmaspec-intent checker

DIR defaults to intent and --tests to tests. report exits 1 while anything awaits approval; every command exits 2 on an error.";

/// A command line split into item IDs, single-valued options and the
/// repeatable `--ledger` of `report`.
struct Args {
    ids: Vec<String>,
    options: BTreeMap<&'static str, String>,
    ledgers: Vec<PathBuf>,
}

impl Args {
    fn parse(command: &str, mut args: impl Iterator<Item = String>) -> Result<Args, String> {
        let (accepted, takes_ids): (&[&'static str], bool) = match command {
            "check" => (&["--dir"], false),
            "approve" | "reconfirm" | "remove" => (&["--by", "--dir", "--tests"], true),
            "record" => (
                &["--example", "--channel", "--ledger", "--dir", "--tests"],
                false,
            ),
            "report" => (
                &[
                    "--dir", "--tests", "--ledger", "--base", "--format", "-o", "--bound",
                ],
                false,
            ),
            "checker" => (&[], false),
            _ => return Err(format!("unknown command {command}")),
        };
        let mut parsed = Args {
            ids: Vec::new(),
            options: BTreeMap::new(),
            ledgers: Vec::new(),
        };
        while let Some(arg) = args.next() {
            match accepted.iter().find(|option| **option == arg) {
                Some(&option) => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{option} requires a value"))?;
                    if command == "report" && option == "--ledger" {
                        parsed.ledgers.push(value.into());
                    } else if parsed.options.insert(option, value).is_some() {
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
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    if command == "--help" || command == "-h" {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let args = Args::parse(&command, args)?;
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
                    _ => remove(log, catalog, examples, ids, by),
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
            let ledger = args
                .options
                .get("--ledger")
                .map_or_else(|| dir.join(RUNS_FILE), PathBuf::from);
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
        _ => print!("{CHECKER}"),
    }
    Ok(ExitCode::SUCCESS)
}

/// Build the whole report before writing any output, so an error leaves
/// existing files untouched.
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
        ledgers: args.ledgers.clone(),
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
