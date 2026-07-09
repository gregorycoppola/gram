
use std::path::Path;

use gram::core::fixture::{Fixture, SentenceInput};
use gram::core::grammar::compile_rules;
use gram::core::lexicon::Lexicon;
use gram::core::matcher::parse_hinted_sentence;

fn parse_fixture(name: &str) -> Vec<(String, Result<Vec<gram::core::matcher::Match>, String>)> {
    let path = Path::new("fixtures").join(format!("{}.json", name));
    let fixture = Fixture::from_path(&path).unwrap();
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).unwrap();

    fixture.sentences.iter()
        .filter_map(|sent| match sent {
            SentenceInput::Hinted(s) => {
                let display = s.tokens.join(" ");
                Some((display, parse_hinted_sentence(s, &lexicon, &rules)))
            }
            SentenceInput::Plain(_) => None,
        })
        .collect()
}

#[test]
fn transitive_all_parse() {
    let results = parse_fixture("rc2_transitive");
    for (sentence, result) in &results {
        assert!(result.is_ok(), "transitive failed on \"{}\": {}", sentence, result.as_ref().unwrap_err());
    }
    assert_eq!(results.len(), 6);
}

#[test]
fn nested_all_parse() {
    let results = parse_fixture("rc2_nested");
    for (sentence, result) in &results {
        assert!(result.is_ok(), "nested failed on \"{}\": {}", sentence, result.as_ref().unwrap_err());
    }
    assert_eq!(results.len(), 5);
}