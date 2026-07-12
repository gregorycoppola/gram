use std::path::PathBuf;

use gram::core::qbbn::inference::{run_inference_fixture, InferenceFixture};

fn run_fixture(name: &str) {
    let path = PathBuf::from("fixtures/qbbn").join(name);
    let raw = std::fs::read_to_string(&path).expect("read fixture");
    let fixture: InferenceFixture = serde_json::from_str(&raw).expect("parse fixture");
    let result = run_inference_fixture(&fixture).expect("run inference");

    println!("\n📊 {}", result.title);
    println!("  Graph: {} propositions, {} groups, {} AND, {} OR, {} NEG, {} evidence",
        result.stats.0, result.stats.1, result.stats.2, result.stats.3, result.stats.4, result.stats.5);
    println!("  Converged in {} iterations", result.iterations);

    for qr in &result.query_results {
        match qr.expected {
            Some(expected) => {
                println!("  P({}) = {:.4}  (expected {:.4} ± {:.4})",
                    qr.formula, qr.prob, expected, qr.tolerance);
                assert!(qr.ok, "query {} failed: got {:.4}, expected {:.4} ± {:.4}",
                    qr.formula, qr.prob, expected, qr.tolerance);
            }
            None => {
                println!("  P({}) = {:.4}", qr.formula, qr.prob);
            }
        }
    }
}

#[test]
fn test_socrates_mortal() {
    run_fixture("socrates_mortal.json");
}

#[test]
fn test_rain_conflict() {
    run_fixture("rain_conflict.json");
}