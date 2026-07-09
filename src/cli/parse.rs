
use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::{
    parse_hinted_sentence, parse_hinted_sentence_traced, DebugTrace, Match,
};

#[derive(Args)]
pub struct ParseArgs {
    #[arg(long)]
    pub fixture: PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long)]
    pub focus: Option<usize>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn run_parse(args: ParseArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)
        .with_context(|| format!("loading fixture {}", args.fixture.display()))?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();
    for (i, sent) in fixture.sentences.iter().enumerate() {
        if let Some(focus) = args.focus {
            if i + 1 != focus {
                continue;
            }
        }
        match sent {
            SentenceInput::Plain(_) => {
                continue;
            }
            SentenceInput::Hinted(s) => {
                let parsed = if args.debug {
                    parse_hinted_debug(s, &lexicon, &rules)
                } else {
                    parse_hinted(s, &lexicon, &rules)
                };
                results.push(parsed);
            }
        }
    }

    if args.json {
        emit_json(&results)?;
    } else {
        emit_pretty(&results);
    }
    Ok(())
}

pub fn run_parse_one(_args: ParseOneArgs) -> Result<()> {
    anyhow::bail!("parse-one requires hinted sentences (tokens + spans); use --fixture with a hinted JSON file instead")
}

fn parse_hinted(s: &crate::core::fixture::InputSentence, lexicon: &Lexicon, rules: &[crate::core::grammar::Rule]) -> ParsedSentence {
    let display = s.tokens.join(" ");
    match parse_hinted_sentence(s, lexicon, rules) {
        Ok(matches) => ParsedSentence {
            sentence: display,
            tokens: s.tokens.clone(),
            matches,
            error: None,
        },
        Err(e) => ParsedSentence {
            sentence: display,
            tokens: s.tokens.clone(),
            matches: Vec::new(),
            error: Some(e),
        },
    }
}

fn parse_hinted_debug(s: &crate::core::fixture::InputSentence, lexicon: &Lexicon, rules: &[crate::core::grammar::Rule]) -> ParsedSentence {
    let display = s.tokens.join(" ");
    let mut trace = DebugTrace::new();
    let result = parse_hinted_sentence_traced(s, lexicon, rules, &mut trace);
    trace.emit();
    eprintln!();
    match result {
        Ok(matches) => ParsedSentence {
            sentence: display,
            tokens: s.tokens.clone(),
            matches,
            error: None,
        },
        Err(e) => ParsedSentence {
            sentence: display,
            tokens: s.tokens.clone(),
            matches: Vec::new(),
            error: Some(e),
        },
    }
}

fn emit_pretty(results: &[ParsedSentence]) {
    println!("📄 {} sentences\n", results.len());
    let mut parsed = 0;
    let mut ambiguous = 0;
    let mut failed = 0;
    let mut errored = 0;

    for r in results {
        if let Some(ref e) = r.error {
            errored += 1;
            println!("  💥 \"{}\"", r.sentence);
            println!("     {}", e);
            println!();
        } else if r.matches.is_empty() {
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
            emit_constituents(&m.constituents, 2);
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
                emit_constituents(&m.constituents, 2);
                println!("     → {}  [{}]", m.output, m.rule_name);
                if let Some(e) = &m.semantics_check {
                    println!("       ⚠️  semantics: {}", e);
                }
            }
            println!();
        }
    }

    println!("Parsed: {}/{}  Ambiguous: {}  Failed: {}  Errored: {}",
        parsed, results.len(), ambiguous, failed, errored);
}

fn emit_constituents(constituents: &[crate::core::matcher::Constituent], indent: usize) {
    let pad = " ".repeat(indent);
    for c in constituents {
        let span_str = match c.span {
            Some((s, e)) if e > s => format!("tokens[{}..{}]", s, e),
            _ => String::new(),
        };
        println!("{}[{}] {}", pad, c.label, c.semantics);
        if !span_str.is_empty() {
            println!("{}     {}", pad, span_str);
        }
        if !c.children.is_empty() {
            emit_constituents(&c.children, indent + 2);
        }
    }
}

fn emit_json(results: &[ParsedSentence]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(results)?);
    Ok(())
}