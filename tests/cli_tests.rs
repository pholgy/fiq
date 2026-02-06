use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fiq_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_fiq"));
    if !path.exists() {
        // Fallback for debug builds
        path = PathBuf::from("target/debug/fiq");
        if cfg!(windows) {
            path.set_extension("exe");
        }
    }
    path
}

fn create_test_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();

    // Create some test files
    fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();
    fs::write(dir.path().join("readme.md"), "# Readme\nSome content").unwrap();
    fs::write(
        dir.path().join("main.rs"),
        "fn main() { println!(\"hi\"); }",
    )
    .unwrap();
    fs::write(
        dir.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    fs::write(dir.path().join("data.json"), r#"{"key": "value"}"#).unwrap();

    // Create a subdirectory with files
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("nested.txt"), "nested file content").unwrap();

    // Create duplicate files
    fs::write(dir.path().join("copy1.txt"), "duplicate content here").unwrap();
    fs::write(dir.path().join("copy2.txt"), "duplicate content here").unwrap();

    dir
}

#[test]
fn test_help_output() {
    let output = Command::new(fiq_bin())
        .arg("--help")
        .output()
        .expect("failed to run fiq");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fast file intelligence"));
    assert!(stdout.contains("stats"));
    assert!(stdout.contains("duplicates"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("organize"));
}

#[test]
fn test_stats_command() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args(["stats", dir.path().to_str().unwrap()])
        .output()
        .expect("failed to run fiq stats");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Total files:"));
    assert!(stdout.contains("Total size:"));
}

#[test]
fn test_stats_top_flag() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args(["stats", dir.path().to_str().unwrap(), "--top", "3"])
        .output()
        .expect("failed to run fiq stats --top");

    assert!(output.status.success());
}

#[test]
fn test_duplicates_command() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args(["duplicates", dir.path().to_str().unwrap()])
        .output()
        .expect("failed to run fiq duplicates");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Duplicate Files"));
    // Should find our duplicate pair
    assert!(stdout.contains("copy1.txt") || stdout.contains("copy2.txt"));
}

#[test]
fn test_duplicates_min_size() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args([
            "duplicates",
            dir.path().to_str().unwrap(),
            "--min-size",
            "999999",
        ])
        .output()
        .expect("failed to run fiq duplicates --min-size");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // With a very high min-size, no duplicates should be found
    // Note: bold formatting inserts ANSI escapes between label and value,
    // so we check for the value portion separately.
    assert!(stdout.contains("0 B"));
}

#[test]
fn test_search_by_name() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args(["search", dir.path().to_str().unwrap(), "--name", "*.rs"])
        .output()
        .expect("failed to run fiq search --name");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("lib.rs"));
}

#[test]
fn test_search_by_content() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args(["search", dir.path().to_str().unwrap(), "--content", "Hello"])
        .output()
        .expect("failed to run fiq search --content");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("hello.txt"));
}

#[test]
fn test_search_by_size() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args([
            "search",
            dir.path().to_str().unwrap(),
            "--min-size",
            "1",
            "--max-size",
            "100",
        ])
        .output()
        .expect("failed to run fiq search --min-size --max-size");

    assert!(output.status.success());
}

#[test]
fn test_organize_dry_run() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args([
            "organize",
            dir.path().to_str().unwrap(),
            "--by",
            "type",
            "--dry-run",
        ])
        .output()
        .expect("failed to run fiq organize --dry-run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("dry run") || stdout.contains("Preview"));

    // Verify no files were actually moved
    assert!(dir.path().join("hello.txt").exists());
    assert!(dir.path().join("main.rs").exists());
}

#[test]
fn test_organize_by_type() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args([
            "organize",
            dir.path().to_str().unwrap(),
            "--by",
            "type",
            "--dry-run",
        ])
        .output()
        .expect("failed to run fiq organize --by type");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Should mention Code and Documents categories
    assert!(stdout.contains("Code") || stdout.contains("Documents"));
}

#[test]
fn test_organize_by_size() {
    let dir = create_test_dir();

    let output = Command::new(fiq_bin())
        .args([
            "organize",
            dir.path().to_str().unwrap(),
            "--by",
            "size",
            "--dry-run",
        ])
        .output()
        .expect("failed to run fiq organize --by size");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Tiny") || stdout.contains("Small"));
}

#[test]
fn test_no_command_exits_with_error() {
    let output = Command::new(fiq_bin()).output().expect("failed to run fiq");

    assert!(!output.status.success());
}
