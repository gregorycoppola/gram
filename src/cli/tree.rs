
use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::{split_sentences, tokenize};

#[derive(Args)]
pub struct TreeArgs {
    #[root]
    pub fixture: PathBuf,
}

pub fn run_tree(args: TreeArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)
        .with_context(|| format!("loading fixture {}", args.fixture.display()))?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    for input in &fixture.sentences {
        match input {
            SentenceInput::Plain(raw) => {
                for sent in split_sentences(raw) {
                    let tokens = tokenize(&sent);
                    let matches = parse_sentence(&tokens, &lexicon, &rules);

                    if matches.is_empty() {
                        println!("❌ \"{}\"", sent);
                        println!("   no matching rule\n");
                        continue;
                    }

                    for m in &matches {
                        println!("\"{}\"", sent);
                        if let Some(ref tree) = m.syntax_tree {
                            println!("{}", tree);
                        }
                        println!("→ {}  [{}]\n", m.output, m.rule_name);
                    }
                }
            }
            SentenceInput::Hinted(s) => {
                let tokens = &s.tokens;
                println!("\"{}\" (hinted)", tokens.join(" "));
                // hinted sentences don't go through parse_sentence yet in tree view
                println!("   (hinted parse — use `gram parse` to see results)\n");
            }
        }
    }

    Ok(())
}