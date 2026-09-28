use std::{env, fs, process::ExitCode};

use lemmaspec::{parse_artifact, print_artifact};
use lemmaspec_plan::{compose_status, project_plan, CHECKER};

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(
        "usage: lemmaspec-plan project PLAN [--status STATUS] [-o OUTPUT] | checker [-o OUTPUT]",
    )?;
    if command == "--help" || command == "-h" {
        println!("usage: lemmaspec-plan project PLAN [--status STATUS] [-o OUTPUT] | checker [-o OUTPUT]");
        return Ok(());
    }
    let input = if command == "project" {
        Some(args.next().ok_or("project requires a Markdown path")?)
    } else if command == "checker" {
        None
    } else {
        return Err(format!("unknown command {command}"));
    };
    let mut output = None;
    let mut status = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" if output.is_none() => {
                output = Some(args.next().ok_or("output requires a path")?)
            }
            "--status" if status.is_none() && input.is_some() => {
                status = Some(args.next().ok_or("status requires a path")?)
            }
            _ => return Err(format!("unknown or duplicate option {arg}")),
        }
    }
    let text = if let Some(input) = input {
        let markdown = fs::read_to_string(&input).map_err(|e| format!("{input}: {e}"))?;
        let mut plan = project_plan(&markdown, &input)?;
        if let Some(status) = status {
            let text = fs::read_to_string(&status).map_err(|e| format!("{status}: {e}"))?;
            plan = compose_status(plan, parse_artifact(&text).map_err(|e| e.to_string())?)?;
        }
        print_artifact(&plan)
    } else {
        CHECKER.to_string()
    };
    if let Some(output) = output {
        fs::write(&output, text).map_err(|e| format!("{output}: {e}"))?;
    } else {
        print!("{text}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lemmaspec-plan: {error}");
            ExitCode::FAILURE
        }
    }
}
