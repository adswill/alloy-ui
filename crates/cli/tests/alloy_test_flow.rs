//! `alloy test` end to end: a failing golden must leave expected, actual, diff and compare
//! images in `--out`, name them, and exit non-zero; `--update` must accept the new look.

use std::path::PathBuf;
use std::process::Command;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn alloy(out: &std::path::Path, update: bool) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alloy"));
    cmd.current_dir(workspace())
        .args(["test", "-p", "alloy-ui-render", "--out"])
        .arg(out);
    if update {
        cmd.arg("--update");
    }
    // Everything after `--` is cargo's; the second `--` is libtest's.
    cmd.args([
        "--",
        "--lib",
        "--",
        "alloy_test_fixture_mismatch",
        "--ignored",
    ]);
    cmd.output().expect("run alloy test")
}

#[test]
fn failing_snapshot_is_reported_with_images_and_update_accepts_it() {
    let out = std::env::temp_dir().join(format!("alloy-test-flow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    let failed = alloy(&out, false);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&failed.stdout),
        String::from_utf8_lossy(&failed.stderr)
    );
    assert!(
        !failed.status.success(),
        "a mismatching snapshot fails the run:\n{text}"
    );
    for part in ["expected", "actual", "diff", "compare"] {
        let f = out.join(format!("fixture.{part}.png"));
        assert!(f.exists(), "{} is written\n{text}", f.display());
    }
    assert!(text.contains("1 snapshot(s) differ"), "{text}");
    assert!(text.contains("fixture.compare.png"), "{text}");
    assert!(text.contains("alloy test --update"), "{text}");

    // `--update` rewrites the snapshot instead of comparing, so the same test now passes,
    // and the old report is cleared.
    let updated = alloy(&out, true);
    assert!(
        updated.status.success(),
        "--update accepts the new look:\n{}{}",
        String::from_utf8_lossy(&updated.stdout),
        String::from_utf8_lossy(&updated.stderr)
    );
    assert!(!out.join("fixture.compare.png").exists());
    let _ = std::fs::remove_dir_all(&out);
}
