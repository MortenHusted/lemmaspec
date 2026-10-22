use std::{env, process::ExitCode};

use lemmaspec_intent::loader::{load, DirReader};

const USAGE: &str = "usage: lemmaspec-intent check [--dir DIR]";

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(USAGE)?;
    if command == "--help" || command == "-h" {
        println!("{USAGE}");
        return Ok(());
    }
    if command != "check" {
        return Err(format!("unknown command {command}"));
    }
    let mut dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" if dir.is_none() => dir = Some(args.next().ok_or("dir requires a path")?),
            _ => return Err(format!("unknown or duplicate option {arg}")),
        }
    }
    load(&DirReader(dir.unwrap_or_else(|| "intent".into()).into()))?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lemmaspec-intent: {error}");
            ExitCode::FAILURE
        }
    }
}
