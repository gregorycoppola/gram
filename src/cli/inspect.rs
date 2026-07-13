use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::core::fixture::Fixture;
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;

#[derive(Args)]
pub struct InspectArgs {
    #[arg(long)]
    pub fixture: PathBuf,
}

pub fn run_inspect(args: InspectArgs) -> Result<()> {
    let fixture = Fixture::from_path(&args.fixture)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    println!("📚 Lexicon");
    println!("  predicates ({}):", lexicon.predicates.len());
    for (name, p) in &lexicon.predicates {
        println!("    {} {}", name, p.role_signature());
    }
    println!("  entities ({}):", lexicon.entities.len());
    for name in lexicon.entities.keys() {
        println!("    {}", name);
    }
    println!(
        "  form index ({} entries, max_form_len={}):",
        lexicon.form_index_len(),
        lexicon.max_form_len()
    );
    for (form, (canonical, cat)) in lexicon.form_index_entries() {
        println!("    {:?} -> ({}, {:?})", form, canonical, cat);
    }
    println!();

    println!("📐 Grammar ({} rules)", rules.len());
    for r in &rules {
        println!("  {} [{}]", r.name, r.kind);
        println!("    pattern: {:?}", r.pattern);
        println!(
            "    sem: {}({})",
            r.sem.constructor,
            r.sem
                .args
                .iter()
                .map(|argument| format!("{:?}", argument))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!();

    println!("✂️  Sentences");
    for sentence in &fixture.sentences {
        println!("  {:?} (hinted)", sentence.tokens.join(" "));
        println!("    tokens: {:?}", sentence.tokens);
        println!("    spans: {:?}", sentence.spans);
    }

    Ok(())
}
