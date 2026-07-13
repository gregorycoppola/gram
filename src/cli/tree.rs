use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::Fixture;
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

    for sentence in &fixture.sentences {
        println!("\"{}\" (hinted)", sentence.tokens.join(" "));

        match parse_hinted_sentence(sentence, &lexicon, &rules) {
            Ok(matches) => {
                if matches.is_empty() {
                    println!("   no matching rule\n");
                    continue;
                }

                for matched in &matches {
                    if let Some(ref tree) = matched.syntax_tree {
                        println!("{}", tree);
                    }
                    println!("→ {}  [{}]\n", matched.output, matched.rule_name);
                }
            }
            Err(error) => {
                println!("   error: {}\n", error);
            }
        }
    }

    Ok(())
}
