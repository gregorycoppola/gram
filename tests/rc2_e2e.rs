use std::path::Path;

use gram::core::fixture::Fixture;
use gram::core::grammar::compile_rules;
use gram::core::lexicon::Lexicon;
use gram::core::matcher::{evaluate_gold, parse_hinted_sentence};

fn assert_fixture_semantics(name: &str, expected_sentences: usize) {
    let path = Path::new("fixtures").join(format!("{}.json", name));
    let fixture = Fixture::from_path(&path)
        .unwrap_or_else(|error| panic!("failed to load {}: {}", name, error));
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)
        .unwrap_or_else(|error| panic!("failed to compile {}: {}", name, error));

    let mut checked = 0;

    for sentence in &fixture.sentences {
        checked += 1;
        let display = sentence.tokens.join(" ");
        let gold = sentence.gold.as_deref().unwrap_or_else(|| {
            panic!(
                "{} sentence is missing a gold logical form: {:?}",
                name, display
            )
        });

        let matches = parse_hinted_sentence(sentence, &lexicon, &rules).unwrap_or_else(|error| {
            panic!(
                "{} failed to parse\nsentence: {}\ngold: {}\nerror: {}",
                name, display, gold, error
            )
        });

        let evaluation = evaluate_gold(&matches, gold).unwrap_or_else(|error| {
            panic!(
                "{} has invalid gold semantics\nsentence: {}\ngold: {}\nerror: {}",
                name, display, gold, error
            )
        });

        if !evaluation.correct {
            let candidates = matches
                .iter()
                .enumerate()
                .map(|(index, matched)| format!("  {}. {}", index + 1, matched.output))
                .collect::<Vec<_>>()
                .join("\n");

            panic!(
                "{} semantic mismatch\n\
                 sentence: {}\n\
                 gold: {}\n\
                 parses: {}\n\
                 distinct semantics: {}\n\
                 candidates:\n{}",
                name,
                display,
                gold,
                evaluation.parse_count,
                evaluation.semantic_count,
                if candidates.is_empty() {
                    "  <none>"
                } else {
                    &candidates
                }
            );
        }

        assert!(
            evaluation.gold_match_count >= 1,
            "{} reported correct without a matching parse for {:?}",
            name,
            display
        );
    }

    assert_eq!(
        checked, expected_sentences,
        "{} sentence-count regression",
        name
    );
}

#[test]
fn transitive_semantics_match_gold() {
    assert_fixture_semantics("rc2_transitive", 10);
}

#[test]
fn nested_semantics_match_gold() {
    assert_fixture_semantics("rc2_nested", 6);
}

#[test]
fn relative_semantics_match_gold() {
    assert_fixture_semantics("rc2_relative", 5);
}

#[test]
fn wh_semantics_match_gold() {
    assert_fixture_semantics("rc2_wh", 1);
}

#[test]
fn adjective_semantics_match_gold() {
    assert_fixture_semantics("rc2_adjectives", 2);
}

#[test]
fn comparative_semantics_match_gold() {
    assert_fixture_semantics("rc2_comparatives", 2);
}

#[test]
fn ditransitive_semantics_match_gold() {
    assert_fixture_semantics("rc2_ditransitive", 2);
}

#[test]
fn of_semantics_match_gold() {
    assert_fixture_semantics("rc2_of", 2);
}

#[test]
fn extended_of_semantics_match_gold() {
    assert_fixture_semantics("rc2_of_extended", 2);
}

#[test]
fn pp_semantics_match_gold() {
    assert_fixture_semantics("rc2_pp", 2);
}

#[test]
fn temporal_semantics_match_gold() {
    assert_fixture_semantics("rc2_temporal", 2);
}
