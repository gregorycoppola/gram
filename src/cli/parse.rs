use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::{
    evaluate_gold, parse_hinted_sentence, parse_hinted_sentence_traced, semantic_count, DebugTrace,
    GoldEvaluation, Match,
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
    semantic_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    gold_evaluation: Option<GoldEvaluation>,
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

fn parse_hinted(
    sentence: &crate::core::fixture::InputSentence,
    lexicon: &Lexicon,
    rules: &[crate::core::grammar::Rule],
) -> ParsedSentence {
    parsed_sentence_from_result(sentence, parse_hinted_sentence(sentence, lexicon, rules))
}

fn parse_hinted_debug(
    sentence: &crate::core::fixture::InputSentence,
    lexicon: &Lexicon,
    rules: &[crate::core::grammar::Rule],
) -> ParsedSentence {
    let mut trace = DebugTrace::new();
    let result = parse_hinted_sentence_traced(sentence, lexicon, rules, &mut trace);

    trace.emit();
    eprintln!();

    parsed_sentence_from_result(sentence, result)
}

fn parsed_sentence_from_result(
    sentence: &crate::core::fixture::InputSentence,
    result: Result<Vec<Match>, String>,
) -> ParsedSentence {
    let display = sentence.tokens.join(" ");

    match result {
        Ok(matches) => {
            let distinct_semantics = semantic_count(&matches);

            let gold_evaluation = match sentence.gold.as_deref() {
                Some(gold) => match evaluate_gold(&matches, gold) {
                    Ok(evaluation) => Some(evaluation),
                    Err(error) => {
                        return ParsedSentence {
                            sentence: display,
                            tokens: sentence.tokens.clone(),
                            matches,
                            semantic_count: distinct_semantics,
                            gold_evaluation: None,
                            error: Some(error),
                        };
                    }
                },
                None => None,
            };

            ParsedSentence {
                sentence: display,
                tokens: sentence.tokens.clone(),
                matches,
                semantic_count: distinct_semantics,
                gold_evaluation,
                error: None,
            }
        }
        Err(error) => ParsedSentence {
            sentence: display,
            tokens: sentence.tokens.clone(),
            matches: Vec::new(),
            semantic_count: 0,
            gold_evaluation: None,
            error: Some(error),
        },
    }
}

fn emit_gold_evaluation(evaluation: &GoldEvaluation) {
    println!("     gold: {}", evaluation.gold);
    println!(
        "     gold match: {}  ({}/{})",
        if evaluation.correct { "✓" } else { "✗" },
        evaluation.gold_match_count,
        evaluation.parse_count
    );

    if !evaluation.matching_parse_indices.is_empty() {
        let indices = evaluation
            .matching_parse_indices
            .iter()
            .map(|index| (index + 1).to_string())
            .collect::<Vec<_>>()
            .join(", ");

        println!("     matching parses: {}", indices);
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
            println!("     → {}  [{}: {}]", m.output, m.rule_name, m.pattern);
            if let Some(e) = &m.semantics_check {
                println!("     ⚠️  semantics: {}", e);
            }
            if let Some(evaluation) = &r.gold_evaluation {
                emit_gold_evaluation(evaluation);
            }
            println!();
        } else {
            ambiguous += 1;
            println!("  ⚠️  \"{}\"  ({} parses)", r.sentence, r.matches.len());
            println!("     distinct semantics: {}", r.semantic_count);
            for m in &r.matches {
                if let Some(syn) = &m.syntax {
                    println!("     {}", syn);
                }
                emit_constituents(&m.constituents, 2);
                println!("     → {}  [{}: {}]", m.output, m.rule_name, m.pattern);
                if let Some(e) = &m.semantics_check {
                    println!("       ⚠️  semantics: {}", e);
                }
            }
            if let Some(evaluation) = &r.gold_evaluation {
                emit_gold_evaluation(evaluation);
            }
            println!();
        }
    }

    println!(
        "Parsed: {}/{}  Ambiguous: {}  Failed: {}  Errored: {}",
        parsed,
        results.len(),
        ambiguous,
        failed,
        errored
    );
}

fn emit_constituents(constituents: &[crate::core::matcher::Constituent], indent: usize) {
    let pad = " ".repeat(indent);
    for c in constituents {
        let span_str = match c.span {
            Some((s, e)) if e > s => format!("tokens[{}..{}]", s, e),
            _ => String::new(),
        };
        println!("{}[{}] {}", pad, c.label, c.semantics);
        if !c.rule_name.is_empty() {
            println!("{}     [{}: {}]", pad, c.rule_name, c.pattern);
        }
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
