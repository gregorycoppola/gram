use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::Fixture;
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::{split_sentences, tokenize};

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

    for raw in &fixture.sentences {
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

    Ok(())
}