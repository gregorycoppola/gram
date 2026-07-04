use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("fixtures")
}

fn run_fixture(path: &std::path::Path) -> (String, usize, usize, usize) {
    let fixture = gram::core::fixture::Fixture::from_path(path)
        .unwrap_or_else(|e| panic!("failed to load {}: {}", path.display(), e));
    let lexicon = gram::core::lexicon::Lexicon::from_fixture(&fixture);
    let rules = gram::core::grammar::compile_rules(&fixture.grammar).unwrap();

    let mut parsed = 0usize;
    let mut ambiguous = 0usize;
    let mut failed = 0usize;

    for sent in &fixture.sentences {
        let sentences = gram::core::tokenize::split_sentences(sent);
        for s in &sentences {
            let tokens = gram::core::tokenize::tokenize(s);
            let matches = gram::core::matcher::parse_sentence(&tokens, &lexicon, &rules);
            if matches.is_empty() {
                failed += 1;
            } else if matches.len() == 1 {
                parsed += 1;
            } else {
                ambiguous += 1;
            }
        }
    }

    let name = path.file_name().unwrap().to_string_lossy().to_string();
    (name, parsed, ambiguous, failed)
}

#[test]
fn all_fixtures_pass() {
    let dir = fixtures_dir();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut total_parsed = 0;
    let mut total_ambiguous = 0;
    let mut total_failed = 0;
    let mut failures = Vec::new();

    for entry in &entries {
        let (name, parsed, ambiguous, failed) = run_fixture(&entry.path());
        total_parsed += parsed;
        total_ambiguous += ambiguous;
        total_failed += failed;
        if failed > 0 {
            failures.push(format!(
                "  {} — parsed: {}, ambiguous: {}, failed: {}",
                name, parsed, ambiguous, failed
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "\nFixture failures:\n{}\n\nTotal: parsed={}, ambiguous={}, failed={}",
            failures.join("\n"),
            total_parsed, total_ambiguous, total_failed
        );
    }

    assert!(
        total_parsed > 0,
        "no sentences parsed — fixtures dir empty?"
    );
}