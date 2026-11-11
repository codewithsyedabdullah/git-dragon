//! Command-line interface for `git-dragon`.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use git_dragon::coat::DragonCoat;
use git_dragon::collect::collect_account_snapshot_with_window;
use git_dragon::history::{AccountSnapshot, ContributionWindow, LanguageShare};
use git_dragon::languages::language_hex;
use git_dragon::output::encode_gif;
use git_dragon::pipeline::{render_snapshot, RenderReport};
use git_dragon::render::render_loop;
use git_dragon::stage::Stage;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("git-dragon: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Options shared by the subcommands.
struct Options {
    output: Option<PathBuf>,
    output_dir: PathBuf,
    window: ContributionWindow,
    canon: bool,
    commits: u64,
    languages: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: None,
            output_dir: PathBuf::from("dragon"),
            window: ContributionWindow::Annual,
            canon: false,
            commits: 2_800,
            languages: vec!["TypeScript".into(), "Rust".into(), "Python".into()],
        }
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let mut value = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value after {name}"))
        };
        match flag {
            "--output" | "-o" => opts.output = Some(PathBuf::from(value(flag)?)),
            "--output-dir" | "-d" => opts.output_dir = PathBuf::from(value(flag)?),
            "--window" | "-w" => opts.window = parse_window(&value(flag)?)?,
            "--commits" | "-c" => {
                opts.commits = value(flag)?
                    .parse()
                    .map_err(|_| "invalid --commits value".to_owned())?;
            }
            "--languages" | "-l" => {
                opts.languages = value(flag)?
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--canon" => opts.canon = true,
            other => return Err(format!("unknown option `{other}`\n\n{}", usage())),
        }
        i += 1;
    }
    Ok(opts)
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "collect" => {
            let login = args.get(1).ok_or("missing <login> for `collect`")?;
            let opts = parse_options(&args[2..])?;
            let snapshot = collect_account_snapshot_with_window(login, opts.window)?;
            let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
            match opts.output {
                Some(path) => {
                    fs::write(&path, format!("{json}\n"))
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    println!("Saved account snapshot for @{login} to {}", path.display());
                }
                None => println!("{json}"),
            }
            Ok(())
        }
        "render" => {
            let file = args.get(1).ok_or("missing <snapshot.json> for `render`")?;
            let opts = parse_options(&args[2..])?;
            let raw = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            let snapshot: AccountSnapshot =
                serde_json::from_str(&raw).map_err(|e| format!("{file}: {e}"))?;
            report(&render_snapshot(&snapshot, &opts.output_dir, opts.canon)?);
            Ok(())
        }
        "generate" => {
            let login = args.get(1).ok_or("missing <login> for `generate`")?;
            let opts = parse_options(&args[2..])?;
            let snapshot = collect_account_snapshot_with_window(login, opts.window)?;
            report(&render_snapshot(&snapshot, &opts.output_dir, opts.canon)?);
            Ok(())
        }
        "demo" => {
            let opts = parse_options(&args[1..])?;
            let snapshot = demo_snapshot(opts.commits, &opts.languages)?;
            report(&render_snapshot(&snapshot, &opts.output_dir, opts.canon)?);
            Ok(())
        }
        "gallery" => {
            let opts = parse_options(&args[1..])?;
            let snapshot = demo_snapshot(0, &opts.languages)?;
            let coat = DragonCoat::from_snapshot(&snapshot, opts.canon);
            fs::create_dir_all(&opts.output_dir)
                .map_err(|e| format!("{}: {e}", opts.output_dir.display()))?;
            for stage in Stage::ALL {
                let frames = render_loop(stage, coat.as_ref())?;
                let path = opts.output_dir.join(format!("{}.gif", stage.slug()));
                fs::write(&path, encode_gif(&frames)?)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                println!("{stage}: wrote {}", path.display());
            }
            Ok(())
        }
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{}", usage())),
    }
}

fn report(r: &RenderReport) {
    let colours = if r.tinted_by.is_empty() {
        "default emerald".to_owned()
    } else {
        r.tinted_by.join(", ")
    };
    println!(
        "{} ({} contributions, colours: {colours})\n  wrote {}\n        {}\n        {}\n        {}",
        r.stage,
        r.contributions,
        r.gif.display(),
        r.png.display(),
        r.share_card.display(),
        r.preview.display()
    );
}

/// Offline snapshot for demos and tests. Languages are weighted in the order given.
fn demo_snapshot(commits: u64, languages: &[String]) -> Result<AccountSnapshot, String> {
    let observed_on = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).ok_or("invalid demo date")?;
    let count = languages.len().max(1) as u32;
    let langs = languages
        .iter()
        .enumerate()
        .map(|(i, name)| LanguageShare {
            name: name.clone(),
            hex_color: language_hex(name),
            repo_count: count - i as u32 + 1,
        })
        .collect();
    Ok(AccountSnapshot {
        login: "demo-user".to_owned(),
        account_id: 1,
        observed_on,
        total_contributions: commits,
        monthly_contributions: commits.min(31),
        current_streak: 0,
        longest_streak: 0,
        languages: langs,
    })
}

fn parse_window(s: &str) -> Result<ContributionWindow, String> {
    match s.to_lowercase().as_str() {
        "annual" | "year" => Ok(ContributionWindow::Annual),
        "rolling" | "rolling365" | "365" => Ok(ContributionWindow::Rolling365),
        _ => Err(format!(
            "unknown window `{s}` (expected `annual` or `rolling`)"
        )),
    }
}

fn usage() -> String {
    "Usage:
  git-dragon generate <login> [--window annual|rolling] [--output-dir dragon] [--canon]
  git-dragon collect  <login> [--window annual|rolling] [--output snapshot.json]
  git-dragon render   <snapshot.json> [--output-dir dragon] [--canon]
  git-dragon demo     [--commits 2800] [--languages TypeScript,Rust,Python] [--output-dir dragon] [--canon]
  git-dragon gallery  [--languages A,B,C] [--output-dir dragon] [--canon]

`generate` = `collect` + `render` in one step. `--canon` ignores language colours."
        .to_owned()
}
