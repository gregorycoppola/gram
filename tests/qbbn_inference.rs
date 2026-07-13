use std::path::PathBuf;

use gram::core::qbbn::inference::{
    run_inference_fixture_debug, InferenceFixture,
};

fn run_fixture(name: &str) {
    let path = PathBuf::from("fixtures/qbbn").join(name);
    let raw = std::fs::read_to_string(&path).expect("read fixture");

    let fixture: InferenceFixture =
        serde_json::from_str(&raw).expect("parse fixture");

    let result =
        run_inference_fixture_debug(&fixture, false)
            .expect("run inference");

    println!("\n📊 {}", result.title);
    println!(
        "  Topology: {}  acyclic={}  components={}",
        result.topology.kind(),
        result.topology.acyclic,
        result.topology.connected_components
    );
    println!(
        "  Graph: {} propositions, {} groups, {} AND, {} OR, {} NEG, {} evidence",
        result.stats.0,
        result.stats.1,
        result.stats.2,
        result.stats.3,
        result.stats.4,
        result.stats.5
    );
    println!("  BP iterations: {}", result.iterations);

    for query in &result.query_results {
        println!(
            "  P({})  exact={:?}  BP={:.6}  delta={:?}",
            query.formula,
            query.exact_prob,
            query.prob,
            query.bp_exact_delta
        );

        if query.expected.is_some() {
            assert_eq!(
                query.expected_ok,
                Some(true),
                "exact result for {} did not match semantic expectation",
                query.formula
            );
        }

        if result.topology.bp_should_be_exact() {
            assert_eq!(
                query.bp_matches_exact,
                Some(true),
                "BP did not match exact inference on acyclic graph for {}",
                query.formula
            );
        } else {
            println!(
                "  loopy diagnostic: BP-exact delta for {} is {:?}",
                query.formula,
                query.bp_exact_delta
            );
        }

        assert!(
            query.ok,
            "topology-appropriate verification failed for {}",
            query.formula
        );
    }
}

#[test]
fn test_socrates_mortal() {
    run_fixture("socrates_mortal.json");
}

#[test]
fn test_socrates_chain() {
    run_fixture("socrates_chain.json");
}

#[test]
fn test_socrates_uncertain() {
    run_fixture("socrates_uncertain.json");
}

#[test]
fn test_rain_conflict() {
    run_fixture("rain_conflict.json");
}

#[test]
fn test_beach_rain() {
    run_fixture("beach_rain.json");
}

#[test]
fn test_beach_sunny() {
    run_fixture("beach_sunny.json");
}

#[test]
fn test_beach_conflict() {
    run_fixture("beach_conflict.json");
}

#[test]
fn test_traffic_cross() {
    run_fixture("traffic_cross.json");
}