use std::path::Path;

use gram::core::fixture::{Fixture, InputSentence, SentenceInput, Span};
use gram::core::grammar::{compile_rules, Rule};
use gram::core::lexicon::Lexicon;
use gram::core::matcher::{evaluate_gold, parse_hinted_sentence, semantic_count, Match};

fn load_fixture() -> Fixture {
    Fixture::from_path(Path::new("fixtures/ambiguity_cartesian.json")).unwrap()
}

fn hinted_sentence(fixture: &Fixture) -> &InputSentence {
    match fixture.sentences.first().unwrap() {
        SentenceInput::Hinted(sentence) => sentence,
        SentenceInput::Plain(_) => panic!("expected hinted sentence"),
    }
}

fn parse_with_rules(sentence: &InputSentence, lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    parse_hinted_sentence(sentence, lexicon, rules).unwrap()
}

fn signature(matches: &[Match]) -> Vec<(String, String, Vec<String>)> {
    matches
        .iter()
        .map(|matched| {
            (
                matched.rule_name.clone(),
                matched.output.clone(),
                matched
                    .constituents
                    .iter()
                    .map(|constituent| constituent.rule_name.clone())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn ambiguous_children_form_the_full_cartesian_product() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let matches = parse_with_rules(hinted_sentence(&fixture), &lexicon, &rules);

    assert_eq!(matches.len(), 4);

    let mut child_rule_pairs = matches
        .iter()
        .map(|matched| {
            assert_eq!(matched.constituents.len(), 2);
            (
                matched.constituents[0].rule_name.clone(),
                matched.constituents[1].rule_name.clone(),
            )
        })
        .collect::<Vec<_>>();

    child_rule_pairs.sort();

    assert_eq!(
        child_rule_pairs,
        vec![
            ("bare_dp_a".to_string(), "bare_dp_a".to_string()),
            ("bare_dp_a".to_string(), "bare_dp_b".to_string()),
            ("bare_dp_b".to_string(), "bare_dp_a".to_string()),
            ("bare_dp_b".to_string(), "bare_dp_b".to_string()),
        ]
    );
}

#[test]
fn distinct_derivations_with_equal_semantics_remain_visible() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let matches = parse_with_rules(hinted_sentence(&fixture), &lexicon, &rules);

    assert_eq!(matches.len(), 4);

    let first_output = &matches[0].output;
    assert!(matches
        .iter()
        .all(|matched| matched.output == *first_output));
}

#[test]
fn grammar_order_does_not_change_results_or_ordering() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let forward = parse_with_rules(hinted_sentence(&fixture), &lexicon, &rules);

    let mut reversed_rules = rules.clone();
    reversed_rules.reverse();

    let reversed = parse_with_rules(hinted_sentence(&fixture), &lexicon, &reversed_rules);

    assert_eq!(signature(&forward), signature(&reversed));
}

#[test]
fn subslots_cannot_invent_unhinted_child_spans() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let sentence = InputSentence {
        tokens: vec!["sue".to_string(), "loves".to_string(), "tom".to_string()],
        spans: vec![Span {
            label: "s".to_string(),
            start: 0,
            end: 3,
        }],
        gold: None,
    };

    let error = parse_hinted_sentence(&sentence, &lexicon, &rules).unwrap_err();

    assert!(error.contains("no matching rule"), "{error}");
}

#[test]
fn gold_evaluation_counts_every_matching_derivation() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();
    let sentence = hinted_sentence(&fixture);

    let matches = parse_with_rules(sentence, &lexicon, &rules);
    let evaluation = evaluate_gold(&matches, sentence.gold.as_deref().unwrap()).unwrap();

    assert!(evaluation.correct);
    assert_eq!(evaluation.parse_count, 4);
    assert_eq!(evaluation.semantic_count, 1);
    assert_eq!(evaluation.gold_match_count, 4);
    assert_eq!(evaluation.matching_parse_indices, vec![0, 1, 2, 3]);
    assert_eq!(semantic_count(&matches), 1);
}

#[test]
fn gold_evaluation_uses_alpha_equivalence() {
    let fixture = Fixture::from_path(Path::new("fixtures/rc2_transitive.json")).unwrap();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let sentence = match &fixture.sentences[2] {
        SentenceInput::Hinted(sentence) => sentence,
        SentenceInput::Plain(_) => panic!("expected hinted sentence"),
    };

    let matches = parse_with_rules(sentence, &lexicon, &rules);
    let evaluation =
        evaluate_gold(&matches, "always [y:e]: man(theme: y) -> happy(theme: y)").unwrap();

    assert!(evaluation.correct);
    assert_eq!(evaluation.parse_count, 1);
    assert_eq!(evaluation.gold_match_count, 1);
    assert_eq!(evaluation.matching_parse_indices, vec![0]);
}

#[test]
fn gold_evaluation_reports_no_matching_candidate() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let matches = parse_with_rules(hinted_sentence(&fixture), &lexicon, &rules);
    let evaluation = evaluate_gold(&matches, "hates(agent: sue, patient: tom)").unwrap();

    assert!(!evaluation.correct);
    assert_eq!(evaluation.parse_count, 4);
    assert_eq!(evaluation.semantic_count, 1);
    assert_eq!(evaluation.gold_match_count, 0);
    assert!(evaluation.matching_parse_indices.is_empty());
}

#[test]
fn malformed_gold_is_a_fixture_error() {
    let fixture = load_fixture();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let matches = parse_with_rules(hinted_sentence(&fixture), &lexicon, &rules);
    let error = evaluate_gold(&matches, "always [").unwrap_err();

    assert!(error.contains("invalid gold logical form"), "{error}");
}

#[test]
fn sentence_is_correct_when_exactly_one_ambiguous_parse_matches_gold() {
    let fixture = Fixture::from_path(Path::new("fixtures/ambiguity_one_gold.json")).unwrap();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();
    let sentence = hinted_sentence(&fixture);

    let matches = parse_with_rules(sentence, &lexicon, &rules);
    let evaluation = evaluate_gold(&matches, sentence.gold.as_deref().unwrap()).unwrap();

    assert_eq!(matches.len(), 2);
    assert_eq!(evaluation.parse_count, 2);
    assert_eq!(evaluation.semantic_count, 2);
    assert_eq!(evaluation.gold_match_count, 1);
    assert_eq!(evaluation.matching_parse_indices.len(), 1);
    assert!(evaluation.correct);
}
