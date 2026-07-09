
use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_hinted_sentence;

#[derive(Args)]
pub struct TreeArgs {
    #[arg(long)]
    pub fixture: PathBuf,
}

pub fn run_tree(args: TreeArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)
        .with_context(|| format!("loading fixture {}", args.fixture.display()))?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    for input in &fixture.sentences {
        match input {
            SentenceInput::Plain(_) => {
                continue;
            }
            SentenceInput::Hinted(s) => {
                let tokens = &s.tokens;
                println!("\"{}\" (hinted)", tokens.join(" "));
                let matches = parse_hinted_sentence(s, &lexicon, &rules);
                if matches.is_empty() {
                    println!("   no matching rule\n");
                    continue;
                }
                for m in &matches {
                    if let Some(ref tree) = m.syntax_tree {
                        println!("{}", tree);
                    }
                    println!("→ {}  [{}]\n", m.output, m.rule_name);
                }
            }
        }
    }

    Ok(())
}