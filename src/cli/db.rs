use anyhow::Result;
use clap::{Args, Subcommand};

use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::tokenize;
use crate::db::client::Db;
use crate::db::store::{
    self, CreateEntity, CreatePredicate, CreateRule, CreateSentence,
};

// ── Top-level subcommand ──────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum DbCommand {
    /// Manage sentences in the DB.
    #[command(subcommand)]
    Sentence(SentenceCmd),
    /// Manage predicates in the DB.
    #[command(subcommand)]
    Predicate(PredicateCmd),
    /// Manage entities in the DB.
    #[command(subcommand)]
    Entity(EntityCmd),
    /// Manage rules in the DB.
    #[command(subcommand)]
    Rule(RuleCmd),
}

pub fn run_db(cmd: DbCommand) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_db_async(cmd))
}

async fn run_db_async(cmd: DbCommand) -> Result<()> {
    let db = Db::connect().await?;
    match cmd {
        DbCommand::Sentence(c) => run_sentence(&db, c).await,
        DbCommand::Predicate(c) => run_predicate(&db, c).await,
        DbCommand::Entity(c) => run_entity(&db, c).await,
        DbCommand::Rule(c) => run_rule(&db, c).await,
    }
}

// ── Sentence ──────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum SentenceCmd {
    /// List all sentences (shows parse status against current grammar).
    Ls,
    /// Add a sentence.
    Add(SentenceAddArgs),
    /// Remove a sentence by ID.
    Rm(IdArg),
    /// Parse one sentence by ID and show detailed match output.
    Parse(IdArg),
    /// Parse all sentences and print a summary.
    ParseAll,
}

#[derive(Args)]
pub struct SentenceAddArgs {
    /// The sentence text.
    pub text: String,
    /// Optional notes.
    #[arg(long, default_value = "")]
    pub notes: String,
}

async fn run_sentence(db: &Db, cmd: SentenceCmd) -> Result<()> {
    match cmd {
        SentenceCmd::Ls => {
            let fixture = store::build_fixture(db).await?;
            let lexicon = Lexicon::from_fixture(&fixture);
            let rules = compile_rules(&fixture.grammar)?;
            let rows = store::list_sentences(db).await?;
            if rows.is_empty() {
                println!("(no sentences)");
                return Ok(());
            }
            println!("{:>4}  {:5}  {}", "ID", "Parse", "Text");
            println!("{}", "─".repeat(60));
            for r in &rows {
                let tokens = tokenize(&r.text);
                let matches = parse_sentence(&tokens, &lexicon, &rules);
                let status = if matches.is_empty() {
                    "❌"
                } else if matches.len() == 1 {
                    "✅"
                } else {
                    "⚠️ "
                };
                println!("{:>4}  {}  {}", r.id, status, r.text);
            }
        }

        SentenceCmd::Add(args) => {
            let row = store::create_sentence(
                db,
                CreateSentence { text: args.text, notes: args.notes },
            )
            .await?;
            println!("✅ added sentence #{}: {}", row.id, row.text);
        }

        SentenceCmd::Rm(args) => {
            let deleted = store::delete_sentence(db, args.id).await?;
            if deleted {
                println!("🗑️  deleted sentence #{}", args.id);
            } else {
                println!("❌ no sentence with id {}", args.id);
            }
        }

        SentenceCmd::Parse(args) => {
            let row = store::get_sentence(db, args.id).await?
                .ok_or_else(|| anyhow::anyhow!("no sentence with id {}", args.id))?;
            let fixture = store::build_fixture(db).await?;
            let lexicon = Lexicon::from_fixture(&fixture);
            let rules = compile_rules(&fixture.grammar)?;
            let tokens = tokenize(&row.text);
            let matches = parse_sentence(&tokens, &lexicon, &rules);

            println!("Sentence #{}: \"{}\"", row.id, row.text);
            println!("Tokens: {}", tokens.join(" | "));
            println!();
            if matches.is_empty() {
                println!("❌ no matching rule");
            } else {
                for m in &matches {
                    println!("✅ rule: {}  kind: {}", m.rule_name, m.kind);
                    println!("   output: {}", m.output);
                    println!("   bindings:");
                    for (var, (canonical, typ)) in &m.bindings {
                        println!("     ${} = {} ({})", var, canonical, typ);
                    }
                    println!("   token annotations:");
                    for (i, (tok, ann)) in tokens.iter().zip(m.token_annotations.iter()).enumerate() {
                        let tag = format!("{:?}", ann.kind).to_lowercase();
                        let extra = match &ann.canonical {
                            Some(c) => format!(" → {}", c),
                            None => String::new(),
                        };
                        println!("     [{:>2}] {:20} {}{}", i, tok, tag, extra);
                    }
                    println!();
                }
            }
        }

        SentenceCmd::ParseAll => {
            let fixture = store::build_fixture(db).await?;
            let lexicon = Lexicon::from_fixture(&fixture);
            let rules = compile_rules(&fixture.grammar)?;
            let rows = store::list_sentences(db).await?;
            let mut ok = 0;
            let mut ambig = 0;
            let mut fail = 0;
            for r in &rows {
                let tokens = tokenize(&r.text);
                let matches = parse_sentence(&tokens, &lexicon, &rules);
                match matches.len() {
                    0 => { fail += 1; println!("❌ #{} \"{}\"", r.id, r.text); }
                    1 => { ok += 1; println!("✅ #{} \"{}\"  → {}", r.id, r.text, matches[0].output); }
                    n => { ambig += 1; println!("⚠️  #{} \"{}\"  ({} parses)", r.id, r.text, n); }
                }
            }
            println!("\n✅ {}  ⚠️  {}  ❌ {}  total {}", ok, ambig, fail, rows.len());
        }
    }
    Ok(())
}

// ── Predicate ─────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum PredicateCmd {
    /// List all predicates.
    Ls,
    /// Add a predicate.
    Add(PredicateAddArgs),
    /// Remove a predicate by ID.
    Rm(IdArg),
}

#[derive(Args)]
pub struct PredicateAddArgs {
    /// Canonical predicate name (e.g. "chair").
    pub name: String,
    /// Comma-separated surface forms (e.g. "chairs,chaired,chairing").
    #[arg(long, default_value = "")]
    pub forms: String,
    /// Short gloss / description.
    #[arg(long, default_value = "")]
    pub gloss: String,
    /// Comma-separated role:type pairs (e.g. "subj:e,obj:e").
    #[arg(long, default_value = "")]
    pub roles: String,
}

async fn run_predicate(db: &Db, cmd: PredicateCmd) -> Result<()> {
    match cmd {
        PredicateCmd::Ls => {
            let rows = store::list_predicates(db).await?;
            if rows.is_empty() { println!("(no predicates)"); return Ok(()); }
            println!("{:>4}  {:20}  {:30}  {}", "ID", "Name", "Forms", "Gloss");
            println!("{}", "─".repeat(72));
            for r in &rows {
                let forms: Vec<String> = serde_json::from_str(&r.forms_json).unwrap_or_default();
                println!("{:>4}  {:20}  {:30}  {}", r.id, r.name, forms.join(", "), r.gloss);
            }
        }
        PredicateCmd::Add(args) => {
            let forms: Vec<String> = if args.forms.is_empty() {
                vec![]
            } else {
                args.forms.split(',').map(|s| s.trim().to_string()).collect()
            };
            let roles = if args.roles.is_empty() {
                std::collections::BTreeMap::new()
            } else {
                args.roles.split(',')
                    .filter_map(|pair| {
                        let mut it = pair.splitn(2, ':');
                        Some((it.next()?.trim().to_string(), it.next()?.trim().to_string()))
                    })
                    .collect()
            };
            let row = store::create_predicate(db, CreatePredicate {
                name: args.name, forms, gloss: args.gloss, roles,
            }).await?;
            println!("✅ added predicate #{}: {}", row.id, row.name);
        }
        PredicateCmd::Rm(args) => {
            let deleted = store::delete_predicate(db, args.id).await?;
            if deleted { println!("🗑️  deleted predicate #{}", args.id); }
            else { println!("❌ no predicate with id {}", args.id); }
        }
    }
    Ok(())
}

// ── Entity ────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum EntityCmd {
    /// List all entities.
    Ls,
    /// Add an entity.
    Add(EntityAddArgs),
    /// Remove an entity by ID.
    Rm(IdArg),
}

#[derive(Args)]
pub struct EntityAddArgs {
    /// Canonical entity name (e.g. "anna_wintour").
    pub name: String,
    /// Comma-separated surface forms (e.g. "Anna Wintour,Wintour").
    #[arg(long, default_value = "")]
    pub forms: String,
    /// Short gloss / description.
    #[arg(long, default_value = "")]
    pub gloss: String,
    /// Type tag (default: e).
    #[arg(long, default_value = "e")]
    pub typ: String,
}

async fn run_entity(db: &Db, cmd: EntityCmd) -> Result<()> {
    match cmd {
        EntityCmd::Ls => {
            let rows = store::list_entities(db).await?;
            if rows.is_empty() { println!("(no entities)"); return Ok(()); }
            println!("{:>4}  {:4}  {:20}  {:35}  {}", "ID", "Typ", "Name", "Forms", "Gloss");
            println!("{}", "─".repeat(80));
            for r in &rows {
                let forms: Vec<String> = serde_json::from_str(&r.forms_json).unwrap_or_default();
                println!("{:>4}  {:4}  {:20}  {:35}  {}", r.id, r.typ, r.name, forms.join(", "), r.gloss);
            }
        }
        EntityCmd::Add(args) => {
            let forms: Vec<String> = if args.forms.is_empty() {
                vec![]
            } else {
                args.forms.split(',').map(|s| s.trim().to_string()).collect()
            };
            let row = store::create_entity(db, CreateEntity {
                name: args.name, forms, gloss: args.gloss, typ: args.typ,
            }).await?;
            println!("✅ added entity #{}: {}", row.id, row.name);
        }
        EntityCmd::Rm(args) => {
            let deleted = store::delete_entity(db, args.id).await?;
            if deleted { println!("🗑️  deleted entity #{}", args.id); }
            else { println!("❌ no entity with id {}", args.id); }
        }
    }
    Ok(())
}

// ── Rule ──────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum RuleCmd {
    /// List all rules.
    Ls,
    /// Add a rule.
    Add(RuleAddArgs),
    /// Remove a rule by ID.
    Rm(IdArg),
}

#[derive(Args)]
pub struct RuleAddArgs {
    /// Rule name (unique identifier).
    pub name: String,
    /// Pattern string (e.g. "$subj:e is $pred").
    #[arg(long)]
    pub pattern: String,
    /// Template string (e.g. "{pred}(subj={subj})").
    #[arg(long)]
    pub template: String,
    /// Kind: fact | question | command (default: fact).
    #[arg(long, default_value = "fact")]
    pub kind: String,
}

async fn run_rule(db: &Db, cmd: RuleCmd) -> Result<()> {
    match cmd {
        RuleCmd::Ls => {
            let rows = store::list_rules(db).await?;
            if rows.is_empty() { println!("(no rules)"); return Ok(()); }
            println!("{:>4}  {:4}  {:25}  {:35}  {}", "ID", "Kind", "Name", "Pattern", "Template");
            println!("{}", "─".repeat(90));
            for r in &rows {
                println!("{:>4}  {:4}  {:25}  {:35}  {}", r.id, r.kind, r.name, r.pattern, r.template);
            }
        }
        RuleCmd::Add(args) => {
            let row = store::create_rule(db, CreateRule {
                name: args.name, pattern: args.pattern,
                template: args.template, kind: args.kind,
            }).await?;
            println!("✅ added rule #{}: {}", row.id, row.name);
        }
        RuleCmd::Rm(args) => {
            let deleted = store::delete_rule(db, args.id).await?;
            if deleted { println!("🗑️  deleted rule #{}", args.id); }
            else { println!("❌ no rule with id {}", args.id); }
        }
    }
    Ok(())
}

// ── Shared ────────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct IdArg {
    /// Row ID.
    pub id: i64,
}