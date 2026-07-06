
use gram::core::fixture::Fixture;
use gram::core::grammar::compile_rules;
use gram::core::lexicon::Lexicon;
use gram::core::matcher::parse_sentence;
use gram::core::tokenize::tokenize;

#[test]
fn the_man_is_happy() {
    let fixture = Fixture::from_path("fixtures/rc2_happy.json").unwrap();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    let tokens = tokenize("the man is happy");
    let matches = parse_sentence(&tokens, &lexicon, &rules);

    assert_eq!(matches.len(), 1, "expected exactly 1 match, got {}: {:?}", matches.len(), matches.iter().map(|m| &m.rule_name).collect::<Vec<_>>());
    let m = &matches[0];
    assert_eq!(m.rule_name, "s_copula");
    assert_eq!(
        m.output,
        "the [x:e]: man(theme: x) -> happy(theme: x)"
    );
    assert!(m.sem_value.is_some(), "sem_value should be set on new-path matches");
    assert!(m.semantics_check.is_none(), "should have no type errors");
}