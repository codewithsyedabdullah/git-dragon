//! End-to-end checks: snapshot -> stage -> coat -> files.

use std::fs;
use std::path::PathBuf;

use git_dragon::coat::DragonCoat;
use git_dragon::history::{AccountSnapshot, LanguageShare};
use git_dragon::languages::language_hex;
use git_dragon::pipeline::render_snapshot;
use git_dragon::render::{render_loop, render_still, LOOP_FRAMES};
use git_dragon::stage::Stage;

fn snapshot(contributions: u64, languages: &[(&str, u32)]) -> AccountSnapshot {
    AccountSnapshot {
        login: "test-user".to_owned(),
        account_id: 1,
        observed_on: chrono::NaiveDate::from_ymd_opt(2026, 11, 1).expect("date"),
        total_contributions: contributions,
        monthly_contributions: 0,
        current_streak: 0,
        longest_streak: 0,
        languages: languages
            .iter()
            .map(|(name, repos)| LanguageShare {
                name: (*name).to_owned(),
                hex_color: language_hex(name),
                repo_count: *repos,
            })
            .collect(),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("git-dragon-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn contribution_counts_select_the_right_stage() {
    let cases = [
        (0, Stage::Egg),
        (100, Stage::Baby),
        (300, Stage::Young),
        (800, Stage::Adolescent),
        (2_000, Stage::Adult),
        (3_000, Stage::Legendary),
    ];
    for (n, stage) in cases {
        let dir = temp_dir(&format!("stage-{n}"));
        let report = render_snapshot(&snapshot(n, &[]), &dir, false).expect("render");
        assert_eq!(report.stage, stage, "{n} contributions");
        fs::remove_dir_all(dir).ok();
    }
}

#[test]
fn every_stage_produces_a_looping_animation() {
    for stage in Stage::ALL {
        let frames = render_loop(stage, None).expect("frames");
        assert_eq!(frames.len(), LOOP_FRAMES);
        assert!(
            frames.iter().any(|f| *f != frames[0]),
            "{stage:?} never moves"
        );
    }
}

#[test]
fn stages_look_different_from_each_other() {
    let stills: Vec<_> = Stage::ALL
        .iter()
        .map(|s| render_still(*s, None).expect("still").rgba)
        .collect();
    for i in 0..stills.len() {
        for j in i + 1..stills.len() {
            assert!(stills[i] != stills[j], "stages {i} and {j} are identical");
        }
    }
}

#[test]
fn python_and_javascript_profiles_get_different_colours() {
    let py = DragonCoat::from_snapshot(&snapshot(2_000, &[("Python", 9)]), false).expect("coat");
    let js =
        DragonCoat::from_snapshot(&snapshot(2_000, &[("JavaScript", 9)]), false).expect("coat");
    assert_eq!(py.primary.language, "Python");
    assert_eq!(js.primary.language, "JavaScript");
    let a = render_still(Stage::Adult, Some(&py)).expect("py");
    let b = render_still(Stage::Adult, Some(&js)).expect("js");
    assert!(a.rgba != b.rgba);
}

#[test]
fn mixed_languages_blend_into_separate_zones() {
    let coat = DragonCoat::from_snapshot(
        &snapshot(900, &[("Python", 6), ("JavaScript", 3), ("C++", 1)]),
        false,
    )
    .expect("coat");
    assert_eq!(coat.primary.language, "Python");
    assert_eq!(coat.secondary.language, "JavaScript");
    assert_eq!(coat.tertiary.language, "C++");
    assert!(coat.primary.strength > coat.tertiary.strength - 0.2);
}

#[test]
fn missing_language_data_gives_the_default_emerald_dragon() {
    let none = snapshot(2_000, &[]);
    assert!(DragonCoat::from_snapshot(&none, false).is_none());
    let dir = temp_dir("default");
    let report = render_snapshot(&none, &dir, false).expect("render");
    assert!(report.tinted_by.is_empty());
    // `--canon` is identical to having no language data.
    let canon =
        render_snapshot(&snapshot(2_000, &[("Rust", 4)]), &temp_dir("canon"), true).expect("canon");
    assert!(canon.tinted_by.is_empty());
    assert_eq!(
        fs::read(&report.gif).expect("a"),
        fs::read(&canon.gif).expect("b")
    );
    fs::remove_dir_all(dir).ok();
}

#[test]
fn bad_language_colours_never_crash_the_render() {
    let mut snap = snapshot(500, &[("Weird", 3)]);
    snap.languages[0].hex_color = "not-a-colour".to_owned();
    let dir = temp_dir("bad-colour");
    let report = render_snapshot(&snap, &dir, false).expect("render");
    assert!(report.tinted_by.is_empty());
    fs::remove_dir_all(dir).ok();
}

#[test]
fn outputs_are_real_files_ready_for_a_readme() {
    let dir = temp_dir("outputs");
    let report =
        render_snapshot(&snapshot(1_500, &[("Go", 5), ("Rust", 2)]), &dir, false).expect("render");

    let gif = fs::read(&report.gif).expect("gif");
    assert_eq!(&gif[..6], b"GIF89a");
    let mut decoder = gif::DecodeOptions::new()
        .read_info(gif.as_slice())
        .expect("decode");
    let mut frames = 0;
    while decoder.read_next_frame().expect("frame").is_some() {
        frames += 1;
    }
    assert_eq!(frames, LOOP_FRAMES);
    assert!(gif.len() < 4_000_000, "GIF is {} bytes", gif.len());

    assert_eq!(&fs::read(&report.png).expect("png")[1..4], b"PNG");
    assert_eq!(&fs::read(&report.share_card).expect("card")[1..4], b"PNG");
    let html = fs::read_to_string(&report.preview).expect("html");
    assert!(html.contains("dragon.gif") && html.contains("@test-user"));
    fs::remove_dir_all(dir).ok();
}

#[test]
fn identical_input_gives_identical_bytes() {
    let snap = snapshot(2_600, &[("TypeScript", 5), ("Rust", 2)]);
    let a = render_snapshot(&snap, &temp_dir("repro-a"), false).expect("a");
    let b = render_snapshot(&snap, &temp_dir("repro-b"), false).expect("b");
    assert_eq!(fs::read(a.gif).expect("a"), fs::read(b.gif).expect("b"));
}

#[test]
fn snapshot_json_round_trips() {
    let snap = snapshot(42, &[("Python", 1)]);
    let json = serde_json::to_string(&snap).expect("json");
    let back: AccountSnapshot = serde_json::from_str(&json).expect("parse");
    assert_eq!(back.total_contributions, 42);
    assert_eq!(back.languages[0].name, "Python");
}
