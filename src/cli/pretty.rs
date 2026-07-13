use crate::core::matcher::{Constituent, Match, SyntaxNode, TokenAnnotation};
use crate::server::{ParseResult, ParseStatus};

pub fn print_pretty(results: &[ParseResult]) {
    for (i, r) in results.iter().enumerate() {
        println!("\n════════════════════════════════════════════════════════");
        println!("  [{}] \"{}\"", i + 1, r.sentence);
        println!("  status: {:?}", r.status);
        println!("════════════════════════════════════════════════════════\n");

        match r.status {
            ParseStatus::Failed => {
                println!("  ❌ no matching rule\n");
            }
            ParseStatus::Error(ref e) => {
                println!("  💥 error: {}\n", e);
            }
            _ => {
                for (mi, m) in r.matches.iter().enumerate() {
                    if r.matches.len() > 1 {
                        println!("  ── match {} of {} ──", mi + 1, r.matches.len());
                    }
                    print_match(m, &r.tokens);
                }
            }
        }
    }
}

fn print_match(m: &Match, tokens: &[String]) {
    println!("  🏷️  tokens:");
    print_token_chips(tokens, &m.token_annotations);
    println!();

    println!("  📤 {}  [{}: {}]", m.output, m.rule_name, m.pattern);
    if let Some(ref syn) = m.syntax {
        println!("  📝 {}", syn);
    }
    if let Some(ref check) = m.semantics_check {
        println!("  ⚠️  semantics: {}", check);
    }
    println!();

    if !m.constituents.is_empty() {
        println!("  📋 constituents");
        print_constituent_table(&m.constituents, &m.output);
        println!();
    }

    if let Some(ref tree) = m.syntax_tree {
        println!("  🌳 bracket tree");
        print_bracket_tree(tree, 0, true, "");
        println!();
    }

    println!("  🔍 rule derivation");
    print_rule_trace(m, tokens);
    println!();
}

fn print_token_chips(tokens: &[String], annotations: &[TokenAnnotation]) {
    if annotations.len() != tokens.len() {
        for t in tokens {
            print!("  [{}] ", t);
        }
        println!();
        return;
    }

    let mut i = 0;
    while i < tokens.len() {
        let a = &annotations[i];
        if a.canonical.is_some() && a.variable.is_some() {
            let mut j = i + 1;
            while j < tokens.len()
                && annotations[j].canonical == a.canonical
                && annotations[j].variable == a.variable
            {
                j += 1;
            }
            let _text = tokens[i..j].join(" ");
            let var = a.variable.as_deref().unwrap_or("");
            let kind = format!("{:?}", a.kind).to_lowercase();
            print!("  ┌─{}─{}─┐ ", kind, var);
            i = j;
        } else {
            let kind = format!("{:?}", a.kind).to_lowercase();
            let label = a.keyword_class.as_deref().unwrap_or(&kind);
            print!("  ┌─{}─┐ ", label);
            i += 1;
        }
    }
    println!();

    i = 0;
    while i < tokens.len() {
        let a = &annotations[i];
        if a.canonical.is_some() && a.variable.is_some() {
            let mut j = i + 1;
            while j < tokens.len()
                && annotations[j].canonical == a.canonical
                && annotations[j].variable == a.variable
            {
                j += 1;
            }
            let text = tokens[i..j].join(" ");
            print!("  │ {:^6} │ ", text);
            i = j;
        } else {
            print!("  │ {:^6} │ ", tokens[i]);
            i += 1;
        }
    }
    println!();
}

fn print_constituent_table(constituents: &[Constituent], output: &str) {
    println!("    {:<16} {:<12} {}", "span", "category", "semantics");
    println!("    {}", "─".repeat(60));

    fn print_rows(constituents: &[Constituent], depth: usize) {
        for c in constituents {
            let span = match c.span {
                Some((s, e)) => format!("tokens[{}..{}]", s, e),
                None => String::new(),
            };
            let indent = "  ".repeat(depth);
            println!(
                "    {:<16} {:<12} {}{}",
                span,
                format!("[{}]", c.label),
                indent,
                c.semantics
            );
            if !c.children.is_empty() {
                print_rows(&c.children, depth + 1);
            }
        }
    }

    print_rows(constituents, 0);
    println!("    {:<16} {:<12} {}", "", "[S]", output);
}

fn print_bracket_tree(node: &SyntaxNode, depth: usize, is_last: bool, prefix: &str) {
    let children = &node.children;
    let is_leaf = children.is_empty();

    let connector = if depth == 0 {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };

    let child_prefix = if depth == 0 {
        ""
    } else if is_last {
        "   "
    } else {
        "│  "
    };

    let full_prefix = format!("{}{}", prefix, connector);

    if is_leaf {
        println!(
            "    {}{} → \"{}\"",
            full_prefix,
            node.label,
            node.terminal.as_deref().unwrap_or("")
        );
    } else {
        println!("    {}{}", full_prefix, node.label);
        for (i, child) in children.iter().enumerate() {
            let child_is_last = i == children.len() - 1;
            print_bracket_tree(
                child,
                depth + 1,
                child_is_last,
                &format!("{}{}", prefix, child_prefix),
            );
        }
    }
}

fn print_rule_trace(m: &Match, tokens: &[String]) {
    let span_str = match m.span {
        Some((s, e)) => format!("tokens[{}..{}] = \"{}\"", s, e, tokens[s..e].join(" ")),
        None => String::new(),
    };
    println!("    [{}]  {}  {}", m.kind, m.rule_name, m.pattern);
    if !span_str.is_empty() {
        println!("    {}", span_str);
    }
    println!("    → {}", m.output);

    for c in &m.constituents {
        print_derivation_node(c, tokens, 0);
    }
}

fn print_derivation_node(c: &Constituent, tokens: &[String], depth: usize) {
    let pad = "  ".repeat(depth + 2);
    let span_str = match c.span {
        Some((s, e)) => format!("tokens[{}..{}] = \"{}\"", s, e, tokens[s..e].join(" ")),
        None => String::new(),
    };
    println!("{}[{}]  {}  {}", pad, c.label, c.rule_name, c.pattern);
    if !span_str.is_empty() {
        println!("{}{}", pad, span_str);
    }
    println!("{}→ {}", pad, c.semantics);

    for child in &c.children {
        print_derivation_node(child, tokens, depth + 1);
    }
}
