use std::collections::BTreeMap;

use crate::core::value::SemValue;

// --- Debug trace ---

pub struct DebugTrace {
    lines: Vec<String>,
    indent: usize,
}

impl DebugTrace {
    pub fn new() -> Self {
        Self { lines: Vec::new(), indent: 0 }
    }

    pub fn push(&mut self, msg: &str) {
        self.lines.push(format!("{}{}", "  ".repeat(self.indent), msg));
    }

    pub fn enter(&mut self, msg: &str) {
        self.push(msg);
        self.indent += 1;
    }

    pub fn leave(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }

    pub fn emit(&self) {
        for line in &self.lines {
            eprintln!("{}", line);
        }
    }
}

pub fn filter_label(f: &KindFilter) -> String {
    match f {
        KindFilter::Any => "any".to_string(),
        KindFilter::Only(k) => format!("only({})", k),
    }
}

// --- Public types ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct Constituent {
    pub label: String,
    pub semantics: String,
    pub free_vars: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_tree: Option<SyntaxNode>,
    #[serde(skip)]
    pub sem_value: Option<SemValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Constituent>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Match {
    pub rule_name: String,
    pub kind: String,
    pub output: String,
    pub bindings: BTreeMap<String, (String, String)>,
    pub token_annotations: Vec<TokenAnnotation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constituents: Vec<Constituent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_tree: Option<SyntaxNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics_check: Option<String>,
    #[serde(skip)]
    pub sem_value: Option<SemValue>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyntaxNode {
    pub label: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SyntaxNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics: Option<String>,
}

impl std::fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        render_tree(f, self, 0)
    }
}

fn render_tree(f: &mut std::fmt::Formatter, node: &SyntaxNode, depth: usize) -> std::fmt::Result {
    let indent = "  ".repeat(depth);
    if let Some(ref term) = node.terminal {
        writeln!(f, "{}{} -> \"{}\"", indent, node.label, term)?;
    } else {
        writeln!(f, "{}{}", indent, node.label)?;
        for child in &node.children {
            render_tree(f, child, depth + 1)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenAnnotation {
    pub kind: TokenKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub typ: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_class: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Entity,
    Predicate,
    Keyword,
    Literal,
    Ignored,
    Unmatched,
    SubClause,
}

// --- Internal types ---

#[derive(Debug, Clone)]
pub enum KindFilter {
    Any,
    Only(String),
}

#[derive(Debug, Clone)]
pub enum Consumption {
    Var { variable: String, canonical: String, typ: String, start: usize, end: usize },
    Keyword { class: String, position: usize },
    Literal { position: usize },
    Skipped { position: usize },
    SubClause { start: usize, end: usize },
}

// --- Span cache types ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpanKey {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

pub struct SpanResult {
    pub sem_value: SemValue,
    pub output: String,
    pub rule_name: String,
    pub kind: String,
    pub bindings: BTreeMap<String, (String, String)>,
    pub token_annotations: Vec<TokenAnnotation>,
    pub constituents: Vec<Constituent>,
}

// --- Helpers ---

pub fn kind_to_syntax_label(kind: &str) -> String {
    match kind {
        "dp" => "DP".to_string(),
        "s" => "S".to_string(),
        "s\\agent" => "S\\agent".to_string(),
        "s\\patient" => "S\\patient".to_string(),
        "s\\theme" => "S\\theme".to_string(),
        _ => kind.to_uppercase(),
    }
}

pub fn parse_sub_index(key: &str) -> Option<usize> {
    if key == "SUB" {
        Some(0)
    } else if let Some(rest) = key.strip_prefix("SUB") {
        rest.parse::<usize>().ok()
    } else {
        None
    }
}