//! CLI-level smoke tests.
//!
//! The substantive behavior of each subcommand is exercised by
//! the library's own integration tests.  This file only checks
//! that the binary builds, the help/version flags work, and the
//! subcommand dispatch produces a non-zero exit on bad input
//! (so CI catches a broken command registration).

use std::{path::PathBuf, process::Command};

fn get_binary_path() -> PathBuf {
  let mut path =
    std::env::current_exe().expect("Failed to get current executable path");
  path.pop();
  path.pop();
  path.push("automation-simulator-cli");
  if !path.exists() {
    path.pop();
    path.pop();
    path.push("debug");
    path.push("automation-simulator-cli");
  }
  path
}

#[test]
fn test_help_flag() {
  let output = Command::new(get_binary_path())
    .arg("--help")
    .output()
    .expect("run");
  assert!(output.status.success(), "--help must succeed");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Usage:"));
  // The subcommand should appear in the top-level help so users
  // discover it without extra flags.
  assert!(
    stdout.contains("seed"),
    "top-level help must advertise the 'seed' subcommand, got:\n{stdout}"
  );
}

#[test]
fn test_version_flag() {
  let output = Command::new(get_binary_path())
    .arg("--version")
    .output()
    .expect("run");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("automation-simulator-cli"));
}

#[test]
fn test_missing_subcommand_is_error() {
  // With subcommands required, bare invocation should exit non-zero.
  let output = Command::new(get_binary_path()).output().expect("run");
  assert!(
    !output.status.success(),
    "bare invocation must fail now that subcommands are required"
  );
}

#[test]
fn test_seed_help() {
  let output = Command::new(get_binary_path())
    .args(["seed", "--help"])
    .output()
    .expect("run");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("--property"));
  assert!(stdout.contains("--catalog"));
  assert!(stdout.contains("--db"));
}
