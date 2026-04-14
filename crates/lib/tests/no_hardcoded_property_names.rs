//! Compliance test: the domain model must stay catalog-driven.
//!
//! Phase 2 of the project plan commits to keeping brand names and
//! property-specific labels out of `crates/lib/src/sim/`.  This test
//! walks every Rust source file under the lib crate's `src/` tree
//! and fails if any forbidden token appears, preserving the
//! invariant that v0.3 can ship support for new controllers and new
//! property shapes without touching the domain types.
//!
//! This test intentionally excludes `src/sim/zone.rs` references to
//! catalog ids like `"1gph-pc"` — those are example emitter-spec ids
//! used by the soft-warn list, not brand or property names.

use std::fs;
use std::path::{Path, PathBuf};

/// Case-insensitive substrings that must not appear anywhere inside
/// `crates/lib/src/`.  Each is either a brand name (which belongs in
/// the catalog, not the domain model) or a property-specific label
/// (which belongs in a fixture, not the domain model).
const FORBIDDEN: &[&str] = &[
  "opensprinkler",
  "ecowitt",
  "os-pi",
  "gw2000",
  "wh51",
  "ws90",
  "rain bird",
  "rachio",
  "hunter pro-c",
  "orbit b-hyve",
  "front yard",
  "back yard",
  "six zones",
  "6 zones",
];

fn crate_src_dir() -> PathBuf {
  // CARGO_MANIFEST_DIR points at crates/lib when this test builds.
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn walk_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
  for entry in fs::read_dir(dir).expect("read src dir") {
    let entry = entry.expect("dir entry");
    let path = entry.path();
    if path.is_dir() {
      walk_rust_files(&path, files);
    } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
      files.push(path);
    }
  }
}

#[test]
fn domain_model_has_no_brand_or_property_specific_names() {
  let mut rs_files = Vec::new();
  walk_rust_files(&crate_src_dir(), &mut rs_files);
  assert!(
    !rs_files.is_empty(),
    "found no .rs files under {}",
    crate_src_dir().display()
  );

  let mut offenses: Vec<(PathBuf, usize, String, &'static str)> = Vec::new();
  for file in &rs_files {
    let contents = fs::read_to_string(file).expect("read source");
    for (lineno, line) in contents.lines().enumerate() {
      let lower = line.to_lowercase();
      for token in FORBIDDEN {
        if lower.contains(token) {
          offenses.push((
            file.clone(),
            lineno + 1,
            line.trim().to_string(),
            token,
          ));
        }
      }
    }
  }

  if !offenses.is_empty() {
    let mut message = String::from(
      "domain model must not reference brand or property-specific \
       names — move the reference into data/catalog or a fixture:\n",
    );
    for (path, line, text, token) in &offenses {
      message.push_str(&format!(
        "  {}:{} contains '{}': {}\n",
        path.display(),
        line,
        token,
        text
      ));
    }
    panic!("{message}");
  }
}
