use std::process::Command;

#[test]
fn demo_table_discloses_estimate_scope_and_synthetic_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_burnrate"))
        .args(["--demo", "report"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("known subtotal (API-rate est.)"));
    assert!(text.contains("full-session totals, not period-only spend"));
    assert!(text.contains("actual bill and provider quota not observed"));
    assert!(text.contains("Synthetic demo — no local logs scanned"));
    assert!(text.contains("not proof of waste"));
    assert!(!text.contains("top waste"));
}

#[test]
fn demo_json_remains_machine_readable_with_additive_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_burnrate"))
        .args(["--demo", "report", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data_source"], "synthetic");
    assert_eq!(report["billing_status"], "not_observed");
    assert_eq!(report["quota_status"], "not_observed");
    assert!(report["scan"].is_null());
    assert_eq!(report["summary"]["session_count"], 5);
    assert_eq!(report["summary"]["unpriced_tokens"], 0);
    assert_eq!(report["summary"]["unpriced_models"], serde_json::json!([]));
    assert!(report["summary"]["total_cost_usd"].as_f64().unwrap() > 0.0);
}
