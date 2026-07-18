use aispekt::{build_input, print_human};
use aispekt_core::judge::{emit_brief, merge_brief, render_semantic};
use aispekt_core::{analyze, report_to_json_pretty};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/aispekt-judge/SKILL.md"
));

fn generator() -> String {
    format!("aispekt {}", env!("CARGO_PKG_VERSION"))
}

fn read_json(path: &str) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("{path} is not valid JSON: {e}"))
}

fn run_judge(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("emit") => {
            let mut target: Option<&str> = None;
            let mut out_path: Option<&str> = None;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--out" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => out_path = Some(p),
                            None => {
                                eprintln!("aispekt: --out requires a path");
                                return ExitCode::from(2);
                            }
                        }
                    }
                    a if !a.starts_with("--") && target.is_none() => target = Some(a),
                    a => {
                        eprintln!("aispekt: unknown argument \"{a}\"");
                        return ExitCode::from(2);
                    }
                }
                i += 1;
            }
            let Some(target) = target else {
                eprintln!("usage: aispekt judge emit <file-or-dir> [--out <path>]");
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
            let brief = emit_brief(&input, &report, &generator());
            let pretty = serde_json::to_string_pretty(&brief).expect("brief serializes");
            match out_path {
                Some(p) => {
                    if let Err(e) = fs::write(p, pretty + "\n") {
                        eprintln!("aispekt: cannot write {p}: {e}");
                        return ExitCode::from(2);
                    }
                    eprintln!("aispekt: judge brief written to {p} ({} tasks)", brief["tasks"].as_array().map_or(0, Vec::len));
                }
                None => println!("{pretty}"),
            }
            ExitCode::from(0)
        }
        Some("merge") => {
            let mut json_out = false;
            let mut paths: Vec<&str> = Vec::new();
            for a in &args[1..] {
                if a == "--json" {
                    json_out = true;
                } else if !a.starts_with("--") {
                    paths.push(a);
                } else {
                    eprintln!("aispekt: unknown argument \"{a}\"");
                    return ExitCode::from(2);
                }
            }
            let [brief_path, answers_path] = paths.as_slice() else {
                eprintln!("usage: aispekt judge merge <brief.json> <answers.json> [--json]");
                return ExitCode::from(2);
            };
            let loaded = read_json(brief_path).and_then(|brief| {
                let answers = read_json(answers_path)?;
                merge_brief(&brief, &answers)
            });
            match loaded {
                Ok(merged) => {
                    if json_out {
                        println!("{}", serde_json::to_string_pretty(&merged).expect("merged serializes"));
                    } else {
                        print!("{}", render_semantic(&merged));
                    }
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("aispekt: {e}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprintln!("usage: aispekt judge <emit|merge> ...");
            ExitCode::from(2)
        }
    }
}

fn run_skill(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("print") => {
            print!("{SKILL_MD}");
            ExitCode::from(0)
        }
        Some("install") => {
            let mut root = PathBuf::from(".claude/skills");
            let mut force = false;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--force" => force = true,
                    "--dir" => {
                        i += 1;
                        match args.get(i) {
                            Some(p) => root = PathBuf::from(p),
                            None => {
                                eprintln!("aispekt: --dir requires a path");
                                return ExitCode::from(2);
                            }
                        }
                    }
                    a => {
                        eprintln!("aispekt: unknown argument \"{a}\"");
                        return ExitCode::from(2);
                    }
                }
                i += 1;
            }
            let dir = root.join("aispekt-judge");
            let dest = dir.join("SKILL.md");
            // Clobber guard: byte-compare, refuse symlinks outright (fs::write
            // follows them), and treat any read error other than "not found"
            // as a refusal — never overwrite what we couldn't inspect.
            match fs::symlink_metadata(&dest) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    eprintln!(
                        "aispekt: {} is a symlink — refusing to write through it; remove it manually first",
                        dest.display()
                    );
                    return ExitCode::from(2);
                }
                Ok(_) => match fs::read(&dest) {
                    Ok(bytes) if bytes == SKILL_MD.as_bytes() => {
                        eprintln!(
                            "aispekt: skill already installed at {} (up to date)",
                            dest.display()
                        );
                        return ExitCode::from(0);
                    }
                    Ok(_) if !force => {
                        eprintln!(
                            "aispekt: {} exists and differs from this version — re-run with --force to overwrite",
                            dest.display()
                        );
                        return ExitCode::from(2);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!(
                            "aispekt: cannot inspect existing {}: {e} — refusing to overwrite",
                            dest.display()
                        );
                        return ExitCode::from(2);
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    eprintln!("aispekt: cannot inspect {}: {e}", dest.display());
                    return ExitCode::from(2);
                }
            }
            if let Err(e) = fs::create_dir_all(&dir).and_then(|()| fs::write(&dest, SKILL_MD)) {
                eprintln!("aispekt: cannot install skill: {e}");
                return ExitCode::from(2);
            }
            eprintln!("aispekt: skill installed at {}", dest.display());
            ExitCode::from(0)
        }
        _ => {
            eprintln!("usage: aispekt skill <install [--dir <skills-root>] [--force] | print>");
            ExitCode::from(2)
        }
    }
}

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
    // Subcommand dispatch is purely additive: anything else falls through to
    // the original analyze surface unchanged (a file literally named "judge"
    // or "skill" needs a ./ prefix).
    match args.first().map(String::as_str) {
        Some("judge") => run_judge(&args[1..]),
        Some("skill") => run_skill(&args[1..]),
        _ => run(&args),
    }
}
