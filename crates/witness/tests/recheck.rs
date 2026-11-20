//! `witness recheck` (FEAT-045) against the committed evidence bundle
//! `tests/fixtures/recheck/suite` (regenerate with
//! `tests/fixtures/recheck/regen.sh`). The same tamperings run as plant
//! controls in CI (ci-controls.toml, recheck-*); here they run in-process.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use witness::recheck::{ExpectedKey, Outcome, Status, recheck, write_inventory};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/recheck")
}

fn expected_key() -> ExpectedKey {
    ExpectedKey::load(&fixture().join("expected-signer.pub")).unwrap()
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

fn copy_suite() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let suite = tmp.path().join("suite");
    copy_tree(&fixture().join("suite"), &suite);
    (tmp, suite)
}

fn status(o: &Outcome, id: &str) -> Status {
    o.checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id} in\n{}", o.to_text()))
        .status
}

fn classes(o: &Outcome) -> BTreeSet<&'static str> {
    o.failure_classes()
}

#[test]
fn committed_fixture_passes_every_recorded_link() {
    let key = expected_key();
    let o = recheck(&fixture().join("suite"), Some(&key)).unwrap();
    assert!(o.ok, "{}", o.to_text());
    assert_eq!(status(&o, "inventory"), Status::Pass);
    for v in ["leap_year", "state_machine"] {
        for check in [
            "rederive-report",
            "chain/source->manifest",
            "chain/module->run",
            "chain/manifest->run",
            "chain/module->predicate",
            "chain/source->predicate",
            "chain/run->predicate",
            "chain/predicate->envelope",
            "signature",
        ] {
            assert_eq!(
                status(&o, &format!("{v}/{check}")),
                Status::Pass,
                "{v}/{check}\n{}",
                o.to_text()
            );
        }
        // witness-coverage/v1 does not bind report.json: said, not passed.
        assert_eq!(
            status(&o, &format!("{v}/chain/report->predicate")),
            Status::NotChecked
        );
    }
    assert!(
        o.to_text()
            .contains("does NOT show that run.json came from a real execution")
    );
}

#[test]
fn without_expected_key_the_signature_is_not_checked() {
    let o = recheck(&fixture().join("suite"), None).unwrap();
    assert!(o.ok, "{}", o.to_text());
    assert_eq!(status(&o, "state_machine/signature"), Status::NotChecked);
}

#[test]
fn deleted_run_json_fails() {
    let (_t, suite) = copy_suite();
    std::fs::remove_file(suite.join("state_machine/run.json")).unwrap();
    let o = recheck(&suite, Some(&expected_key())).unwrap();
    assert!(!o.ok);
    let c = classes(&o);
    assert!(c.contains("required-file-missing"), "{c:?}");
    assert!(c.contains("inventory-missing-file"), "{c:?}");
}

#[test]
fn flipped_module_byte_fails_inventory_and_chain() {
    let (_t, suite) = copy_suite();
    let p = suite.join("state_machine/instrumented.wasm");
    let mut b = std::fs::read(&p).unwrap();
    let mid = b.len() / 2;
    b[mid] ^= 1;
    std::fs::write(&p, b).unwrap();
    let o = recheck(&suite, Some(&expected_key())).unwrap();
    let c = classes(&o);
    assert!(c.contains("inventory-digest-mismatch"), "{c:?}");
    assert!(c.contains("chain-mismatch"), "{c:?}");
    assert_eq!(status(&o, "state_machine/chain/module->run"), Status::Fail);
    assert_eq!(
        status(&o, "state_machine/chain/module->predicate"),
        Status::Fail
    );
}

#[test]
fn gap_edited_to_proved_with_inventory_recomputed_is_caught_only_by_rederivation() {
    let (_t, suite) = copy_suite();
    let p = suite.join("state_machine/report.json");
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(
        text.contains("\"status\": \"gap\""),
        "fixture needs a gap condition"
    );
    std::fs::write(
        &p,
        text.replacen("\"status\": \"gap\"", "\"status\": \"proved\"", 1),
    )
    .unwrap();
    write_inventory(&suite).unwrap();
    let o = recheck(&suite, Some(&expected_key())).unwrap();
    assert_eq!(
        classes(&o),
        BTreeSet::from(["rederive-mismatch"]),
        "{}",
        o.to_text()
    );
}

#[test]
fn inventory_line_for_absent_file_fails() {
    let (_t, suite) = copy_suite();
    let inv = suite.join("INVENTORY.sha256");
    let mut text = std::fs::read_to_string(&inv).unwrap();
    text.push_str(&format!(
        "{}  state_machine/coverage-addendum.json\n",
        "0".repeat(64)
    ));
    std::fs::write(&inv, text).unwrap();
    let o = recheck(&suite, Some(&expected_key())).unwrap();
    assert_eq!(
        classes(&o),
        BTreeSet::from(["inventory-missing-file"]),
        "{}",
        o.to_text()
    );
}

#[test]
fn resigned_with_fresh_key_fails_against_expected_key_only() {
    let (t, suite) = copy_suite();
    let sk = t.path().join("fresh.sk");
    witness_core::attest::generate_keypair_files(&sk, &suite.join("verifying-key.pub")).unwrap();
    witness_core::attest::sign_predicate_file(
        &suite.join("state_machine/predicate.json"),
        &sk,
        &suite.join("state_machine/signed.dsse.json"),
        Some("witness-recheck-fixture/state_machine"),
    )
    .unwrap();
    write_inventory(&suite).unwrap();
    let o = recheck(&suite, Some(&expected_key())).unwrap();
    assert_eq!(
        classes(&o),
        BTreeSet::from(["signer-mismatch"]),
        "{}",
        o.to_text()
    );
    // The in-bundle key is reported, never trusted.
    assert!(
        o.to_text()
            .contains("does verify against the in-bundle verifying-key.pub")
    );
    // Without a trust anchor from outside the bundle the same bundle passes:
    // the signature line is NOT-CHECKED, not PASS.
    let no_key = recheck(&suite, None).unwrap();
    assert!(no_key.ok);
    assert_eq!(
        status(&no_key, "state_machine/signature"),
        Status::NotChecked
    );
}

#[test]
fn single_verdict_directory_is_rechecked_on_its_own() {
    let o = recheck(&fixture().join("suite/leap_year"), Some(&expected_key())).unwrap();
    assert!(o.ok, "{}", o.to_text());
    assert_eq!(status(&o, "leap_year/rederive-report"), Status::Pass);
    // The inventory sits at the suite root, so a lone verdict has none.
    assert_eq!(status(&o, "inventory"), Status::NotChecked);
}
