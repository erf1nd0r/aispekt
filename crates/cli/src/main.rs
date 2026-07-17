use aispekt::{build_input, print_human};
use aispekt_core::{analyze, report_to_json_pretty};
use std::path::Path;
use std::process::ExitCode;

fn run(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::from(0);
    }
    let mut json = false;
    let mut min = 60.0f64;
    let mut target: Option<&str> = None;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--json" {
            json = true;
        } else if a == "--min" {
            i += 1;
            // JS `Number("nan")` is NaN and the TS CLI rejects it; Rust's f64
            // parser would accept it and make `score >= NaN` always false —
            // filter it out so both CLIs exit 2 on non-numeric input.
            let parsed = args.get(i).and_then(|v| {
                let t = v.trim();
                if t.is_empty() {
                    None
                } else {
                    t.parse::<f64>().ok().filter(|v| !v.is_nan())
                }
            });
            match parsed {
                Some(v) => min = v,
                None => {
                    eprintln!("aispekt: --min requires a numeric value");
                    return ExitCode::from(2);
                }
            }
        } else if !a.starts_with("--") && target.is_none() {
            target = Some(a);
        } else {
            eprintln!("aispekt: unknown argument \"{a}\"");
            return ExitCode::from(2);
        }
        i += 1;
    }
    let Some(target) = target else {
        eprintln!("usage: aispekt <file-or-dir> [--json] [--min <score>]");
        return ExitCode::from(2);
    };
    let input = match build_input(Path::new(target)) {
        Ok(input) => input,
        Err(err) => {
            eprintln!("aispekt: {err}");
            return ExitCode::from(2);
        }
    };
    let report = analyze(&input);
    if json {
        println!("{}", report_to_json_pretty(&report));
    } else {
        print!("{}", print_human(&report));
    }
    if (report.score as f64) >= min {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}
