use std::path::Path;

use gram::core::fixture::{Fixture, InputSentence, SentenceInput, Span};
use gram::core::grammar::{compile_rules, Rule};
use gram::core::lexicon::Lexicon;
use gram::core::matcher::{parse_hinted_sentence, Match};

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
    };

    let error = parse_hinted_sentence(&sentence, &lexicon, &rules).unwrap_err();

    assert!(error.contains("no matching rule"), "{error}");
}
