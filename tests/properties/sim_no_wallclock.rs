// keep wall-clock/thread APIs out of src/ entirely — the sim has to stay deterministic.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("src dir readable") {
        let entry = entry.expect("dir entry readable");
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn grep_gate_no_forbidden_apis() {
    let forbidden = [
        "std::time::Instant",
        "SystemTime",
        "thread::spawn",
        "tokio",
    ];

    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src_dir, &mut files);
    assert!(!files.is_empty(), "expected to find source files under src/");

    let mut violations = Vec::new();
    for file in &files {
        let contents = fs::read_to_string(file).expect("source file is valid utf8");
        for needle in forbidden {
            if contents.contains(needle) {
                violations.push(format!("{}: contains forbidden `{needle}`", file.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "forbidden wall-clock/thread APIs found:\n{}",
        violations.join("\n")
    );
}
