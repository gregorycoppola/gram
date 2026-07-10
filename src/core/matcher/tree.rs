use crate::core::grammar::is_punctuation;
use crate::core::lexicon::clean_token;
use super::types::*;

pub fn build_syntax(tokens: &[String], constituents: &[(usize, usize, String)], top_label: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut ci = 0;
    let mut skip_until = 0;
    for i in 0..tokens.len() {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&tokens[i]);
        if is_punctuation(&cleaned) {
            continue;
        }
        if ci < constituents.len() && i == constituents[ci].0 {
            parts.push(constituents[ci].2.clone());
            skip_until = constituents[ci].1;
            ci += 1;
            continue;
        }
        parts.push(cleaned);
    }
    if parts.is_empty() {
        format!("[{}]", top_label)
    } else {
        format!("[{} {}]", top_label, parts.join(" "))
    }
}

pub fn build_syntax_tree(
    tokens: &[String],
    annotations: &[TokenAnnotation],
    constituents: &[(usize, usize, SyntaxNode)],
    top_label: &str,
) -> SyntaxNode {
    let mut children = Vec::new();
    let mut ci = 0;
    let mut skip_until = 0;
    for i in 0..tokens.len() {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&tokens[i]);
        if is_punctuation(&cleaned) {
            continue;
        }
        if ci < constituents.len() && i == constituents[ci].0 {
            children.push(constituents[ci].2.clone());
            skip_until = constituents[ci].1;
            ci += 1;
            continue;
        }
        let ann = &annotations[i];
        let node = match ann.kind {
            TokenKind::Keyword => {
                let label = ann.keyword_class.as_deref().unwrap_or("KW");
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Entity => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Predicate => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Literal => {
                SyntaxNode { label: "LIT".to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::SubClause => {
                SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            _ => {
                SyntaxNode { label: "UNK".to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
        };
        children.push(node);
    }
    SyntaxNode { label: top_label.to_string(), children, terminal: None, semantics: None }
}

pub fn offset_constituents(constituents: &[Constituent], offset: usize) -> Vec<Constituent> {
    constituents.iter().map(|c| {
        let mut c = c.clone();
        c.span = c.span.map(|(s, e)| (s + offset, e + offset));
        c.children = offset_constituents(&c.children, offset);
        c
    }).collect()
}

pub fn build_syntax_tree_from_constituent(
    c: &Constituent,
    all_tokens: &[String],
    global_offset: usize,
) -> SyntaxNode {
    let (s, e) = match c.span {
        Some((s, e)) => (s, e),
        None => {
            return SyntaxNode {
                label: kind_to_syntax_label(&c.label),
                children: vec![],
                terminal: None,
                semantics: Some(c.semantics.clone()),
            };
        }
    };
    let local_s = s.saturating_sub(global_offset);
    let local_e = e.saturating_sub(global_offset);
    if local_s >= all_tokens.len() {
        return SyntaxNode {
            label: kind_to_syntax_label(&c.label),
            children: vec![],
            terminal: None,
            semantics: Some(c.semantics.clone()),
        };
    }
    let local_e = local_e.min(all_tokens.len());

    let mut children = Vec::new();
    let mut ci = 0;
    let mut skip_until = local_s;
    for i in local_s..local_e {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&all_tokens[i]);
        if is_punctuation(&cleaned) {
            continue;
        }
        if ci < c.children.len() {
            let child = &c.children[ci];
            if let Some((child_s, _)) = child.span {
                if child_s == i + global_offset {
                    children.push(build_syntax_tree_from_constituent(child, all_tokens, global_offset));
                    if let Some((_, child_e)) = child.span {
                        skip_until = child_e.saturating_sub(global_offset);
                    }
                    ci += 1;
                    continue;
                }
            }
        }
        children.push(SyntaxNode {
            label: cleaned.clone(),
            children: vec![],
            terminal: Some(cleaned),
            semantics: None,
        });
    }
    SyntaxNode {
        label: kind_to_syntax_label(&c.label),
        children,
        terminal: None,
        semantics: Some(c.semantics.clone()),
    }
}

pub fn build_syntax_tree_for_result(
    result: &SpanResult,
    tokens: &[String],
    offset: usize,
) -> SyntaxNode {
    let mut annotations: Vec<TokenAnnotation> = tokens.iter()
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for (i, ann) in result.token_annotations.iter().enumerate() {
        if i < annotations.len() {
            annotations[i] = ann.clone();
        }
    }

    let constituent_trees: Vec<(usize, usize, SyntaxNode)> = result.constituents.iter()
        .filter_map(|c| {
            let (s, e) = c.span?;
            let local_s = s.saturating_sub(offset);
            let local_e = e.saturating_sub(offset);
            if local_s >= tokens.len() {
                return None;
            }
            let local_e = local_e.min(tokens.len());

            let tree = c.syntax_tree.clone().unwrap_or_else(|| {
                if c.children.is_empty() {
                    SyntaxNode {
                        label: kind_to_syntax_label(&c.label),
                        children: tokens[local_s..local_e].iter().filter_map(|t| {
                            let cleaned = clean_token(t);
                            if is_punctuation(&cleaned) {
                                None
                            } else {
                                Some(SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None })
                            }
                        }).collect(),
                        terminal: None,
                        semantics: None,
                    }
                } else {
                    build_syntax_tree_from_constituent(c, tokens, offset)
                }
            });
            Some((local_s, local_e, tree))
        })
        .collect();

    let mut tree = build_syntax_tree(tokens, &annotations, &constituent_trees, &kind_to_syntax_label(&result.kind));
    tree.semantics = Some(result.output.clone());
    tree
}

pub fn span_result_to_match(result: &SpanResult, span: &crate::core::fixture::Span, all_tokens: &[String]) -> Match {
    let mut full_annotations: Vec<TokenAnnotation> = all_tokens.iter()
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for (i, ann) in result.token_annotations.iter().enumerate() {
        let global_i = span.start + i;
        if global_i < full_annotations.len() {
            full_annotations[global_i] = ann.clone();
        }
    }

    let constituent_spans: Vec<(usize, usize, String)> = result.constituents.iter()
        .filter_map(|c| c.span.map(|(s, e)| (s, e, format!("[{}]", c.label))))
        .collect();
    let syntax = build_syntax(all_tokens, &constituent_spans, &kind_to_syntax_label(&result.kind));

    let constituent_trees: Vec<(usize, usize, SyntaxNode)> = result.constituents.iter()
        .filter_map(|c| {
            let (s, e) = c.span?;
            let tree = c.syntax_tree.clone().unwrap_or_else(|| {
                if c.children.is_empty() {
                    SyntaxNode {
                        label: kind_to_syntax_label(&c.label),
                        children: all_tokens[s..e].iter().filter_map(|t| {
                            let cleaned = clean_token(t);
                            if is_punctuation(&cleaned) { None } else {
                                Some(SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None })
                            }
                        }).collect(),
                        terminal: None,
                        semantics: None,
                    }
                } else {
                    build_syntax_tree_from_constituent(c, all_tokens, 0)
                }
            });
            Some((s, e, tree))
        })
        .collect();
    let mut syntax_tree = build_syntax_tree(all_tokens, &full_annotations, &constituent_trees, &kind_to_syntax_label(&result.kind));
    syntax_tree.semantics = Some(result.output.clone());

    Match {
        rule_name: result.rule_name.clone(),
        pattern: result.pattern.clone(),
        kind: result.kind.clone(),
        output: result.output.clone(),
        bindings: result.bindings.clone(),
        token_annotations: full_annotations,
        constituents: result.constituents.clone(),
        syntax: Some(syntax),
        syntax_tree: Some(syntax_tree),
        semantics_check: None,
        sem_value: Some(result.sem_value.clone()),
        span: Some((span.start, span.end)),
    }
}