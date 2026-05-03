use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::Fixture;
use crate::core::lexicon::Lexicon;
use crate::core::grammar::compile_rules;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::{split_sentences, tokenize};

#[derive(Args)]
pub struct ParseArgs {
    /// Path to the fixture JSON file.
    #[arg(long)]
    pub fixture: PathBuf,

    /// Emit JSON instead of pretty output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ParseOneArgs {
    /// Path to the fixture JSON file (used only for lexicon and grammar).
    #[arg(long)]
    pub fixture: PathBuf,

    /// The sentence to parse.
    #[arg(long)]
    pub sentence: String,

    #[arg(long)]
    pub json: bool,
}

pub fn run_parse(args: ParseArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)
        .with_context(|| format!("loading fixture {}", args.fixture.display()))?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    // Collect every sentence — fixture.sentences may contain multi-sentence strings.
    let mut all_sentences: Vec<String> = Vec::new();
    for s in &fixture.sentences {
        for split in split_sentences(s) {
            all_sentences.push(split);
        }
    }

    let mut results = Vec::new();
    for sent in &all_sentences {
        let tokens = tokenize(sent);
        let matches = parse_sentence(&tokens, &lexicon, &rules);
        results.push((sent.clone(), tokens, matches));
    }

    if args.json {
        emit_json(&results)?;
    } else {
        emit_pretty(&results);
    }
    Ok(())
}

pub fn run_parse_one(args: ParseOneArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let sentences = split_sentences(&args.sentence);
    let mut results = Vec::new();
    for sent in &sentences {
        let tokens = tokenize(sent);
        let matches = parse_sentence(&tokens, &lexicon, &rules);
        results.push((sent.clone(), tokens, matches));
    }

    if args.json {
        emit_json(&results)?;
    } else {
        emit_pretty(&results);
    }
    Ok(())
}

fn emit_pretty(results: &[(String, Vec<String>, Vec<crate::core::matcher::Match>)]) {
    println!("📄 {} sentences\n", results.len());
    let mut parsed = 0;
    let mut ambiguous = 0;
    let mut failed = 0;

    for (sent, _tokens, matches) in results {
        if matches.is_empty() {
            failed += 1;
            println!("  ❌ \"{}\"", sent);
            println!("     no matching rule\n");
        } else if matches.len() == 1 {
            parsed += 1;
            let m = &matches[0];
            println!("  ✅ \"{}\"", sent);
            println!("     → {}  [{}]\n", m.output, m.rule_name);
        } else {
            ambiguous += 1;
            println!("  ⚠️  \"{}\"  ({} parses)", sent, matches.len());
            for m in matches {
                println!("     → {}  [{}]", m.output, m.rule_name);
            }
            println!();
        }
    }

    println!("Parsed: {}/{}  Ambiguous: {}  Failed: {}",
        parsed, results.len(), ambiguous, failed);
}

fn emit_json(results: &[(String, Vec<String>, Vec<crate::core::matcher::Match>)]) -> Result<()> {
    let payload: Vec<_> = results.iter().map(|(sent, tokens, matches)| {
        serde_json::json!({
            "sentence": sent,
            "tokens": tokens,
            "matches": matches.iter().map(|m| serde_json::json!({
                "rule": m.rule_name,
                "output": m.output,
                "kind": m.kind,
                "bindings": m.bindings,
            })).collect::<Vec<_>>(),
        })
    }).collect();
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}