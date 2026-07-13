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
