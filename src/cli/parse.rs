
use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::{compile_rules, Rule};
use crate::core::lexicon::Lexicon;
use crate::core::matcher::{parse_hinted_sentence, parse_sentence, Match};
use crate::core::tokenize::{split_sentences, tokenize};

#[derive(Args)]
pub struct ParseArgs {
    #[arg(long)]
    pub fixture: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ParseOneArgs {
    #[arg(long)]
    pub fixture: PathBuf,
    #[arg(long)]
    pub sentence: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct ParsedSentence {
    sentence: String,
    tokens: Vec<String>,
    matches: Vec<Match>,
}

pub fn run_parse(args: ParseArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)
        .with_context(|| format!("loading fixture {}", args.fixture.display()))?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();
    for sent in &fixture.sentences {
        let parsed = match sent {
            SentenceInput::Plain(s) => parse_plain(s, &lexicon, &rules),
            SentenceInput::Hinted(s) => parse_hinted(s, &lexicon, &rules),
        };
        results.extend(parsed);
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
    let lexicon = Lexicon::from_path(&args.fixture)?;
    let rules = compile_rules(&fixture.grammar)?;

    let sentences = split_sentences(&args.sentence);
    let mut results = Vec::new();
    for sent in &sentences {
        results.extend(parse_plain(sent, &lexicon, &rules));
    }

    if args.json {
        emit_json(&results)?;
    } else {
        emit_pretty(&results);
    }
    Ok(())
}

fn parse_plain(s: &str, lexicon: &Lexicon, rules: &[Rule]) -> Vec<ParsedSentence> {
    let mut out = Vec::new();
    for split in split_sentences(s) {
        let tokens = tokenize(&split);
        let matches = parse_sentence(&tokens, lexicon, rules);
        out.push(ParsedSentence {
            sentence: split,
            tokens,
            matches,
        });
    }
    out
}

fn parse_hinted(s: &crate::core::fixture::InputSentence, lexicon: &Lexicon, rules: &[Rule]) -> Vec<ParsedSentence> {
    let display = s.tokens.join(" ");
    let matches = parse_hinted_sentence(s, lexicon, rules);
    vec![ParsedSentence {
        sentence: display,
        tokens: s.tokens.clone(),
        matches,
    }]
}

fn emit_pretty(results: &[ParsedSentence]) {
    println!("📄 {} sentences\n", results.len());
    let mut parsed = 0;
    let mut ambiguous = 0;
    let mut failed = 0;

    for r in results {
        if r.matches.is_empty() {
            failed += 1;
            println!("  ❌ \"{}\"", r.sentence);
            println!("     no matching rule\n");
        } else if r.matches.len() == 1 {
            parsed += 1;
            let m = &r.matches[0];
            println!("  ✅ \"{}\"", r.sentence);
            if let Some(syn) = &m.syntax {
                println!("     {}", syn);
            }
            emit_stages(m);
            println!("     → {}  [{}]", m.output, m.rule_name);
            if let Some(e) = &m.semantics_check {
                println!("     ⚠️  semantics: {}", e);
            }
            println!();
        } else {
            ambiguous += 1;
            println!("  ⚠️  \"{}\"  ({} parses)", r.sentence, r.matches.len());
            for m in &r.matches {
                if let Some(syn) = &m.syntax {
                    println!("     {}", syn);
                }
                emit_stages(m);
                println!("     → {}  [{}]", m.output, m.rule_name);
                if let Some(e) = &m.semantics_check {
                    println!("       ⚠️  semantics: {}", e);
                }
            }
            println!();
        }
    }

    println!("Parsed: {}/{}  Ambiguous: {}  Failed: {}",
        parsed, results.len(), ambiguous, failed);
}

fn emit_stages(m: &Match) {
    for (i, c) in m.constituents.iter().enumerate() {
        let span_str = match c.span {
            Some((s, e)) if e > s => format!("tokens[{}..{}]", s, e),
            _ => String::new(),
        };
        println!("     stage {} [{}]: {}", i + 1, c.label, c.semantics);
        if !span_str.is_empty() {
            println!("                  {}", span_str);
        }
    }
}

fn emit_json(results: &[ParsedSentence]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(results)?);
    Ok(())
}