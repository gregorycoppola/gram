/// Parser for semantic output strings into the logic AST.
///
/// Grammar (informal):
///   expr       = question | impl_expr
///   question   = '?' [label ':'] [binding] impl_expr
///   impl_expr  = and_expr ['->' and_expr]
///   and_expr   = primary ('∧' primary)*
///   primary    = 'not' primary
///              | 'always' binding impl_expr
///              | 'the' binding impl_expr
///              | 'this' binding impl_expr
///              | 'that' binding impl_expr
///              | 'exists' binding and_expr [count]
///              | 'exists_many' counted_binding and_expr
///              | ident '(' role_list ')'       -- predicate
///              | ident ':' ident               -- variable ref
///              | ident                         -- entity
///   binding    = '[' ident ':' ident ']' ':'
///   counted_binding = '[' ident ':' ident ',' count ']' ':'
///   count      = num | ident           -- "3", "many", "few", ...
///   count (legacy, on exists) = ',' '|' ident '|' '=' ident

use crate::core::lexicon::Lexicon;
use crate::core::logic::Expr;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    And,
    Implies,
    Question,
    Pipe,
    Ident(String),
    Num(String),
}

fn tokenize(input: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < n {
        match chars[i] {
            ' ' | '\t' | '\n' => i += 1,
            '(' => { tokens.push(Tok::LParen); i += 1; }
            ')' => { tokens.push(Tok::RParen); i += 1; }
            '[' => { tokens.push(Tok::LBracket); i += 1; }
            ']' => { tokens.push(Tok::RBracket); i += 1; }
            ':' => { tokens.push(Tok::Colon); i += 1; }
            ',' => { tokens.push(Tok::Comma); i += 1; }
            '∧' => { tokens.push(Tok::And); i += 1; }
            '?' => { tokens.push(Tok::Question); i += 1; }
            '|' => { tokens.push(Tok::Pipe); i += 1; }
            '-' if i + 1 < n && chars[i + 1] == '>' => {
                tokens.push(Tok::Implies);
                i += 2;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num: String = chars[start..i].iter().collect();
                tokens.push(Tok::Num(num));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < n {
                    if chars[i].is_alphanumeric() || chars[i] == '_' {
                        i += 1;
                    } else if chars[i] == '-' && i + 1 < n && chars[i + 1] != '>' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let word: String = chars[start..i].iter().collect();
                tokens.push(Tok::Ident(word));
            }
            c => return Err(format!("unexpected character '{}'", c)),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Tok>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek() == Some(t)
    }

    fn at_ident(&self) -> bool {
        matches!(self.peek(), Some(Tok::Ident(_)))
    }

    fn at_ident_val(&self, val: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == val)
    }

    fn expect(&mut self, t: &Tok) -> Result<(), String> {
        if self.at(t) {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", t, self.peek()))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.advance() {
            Some(Tok::Ident(s)) => Ok(s),
            other => Err(format!("expected identifier, got {:?}", other)),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        if self.at(&Tok::Question) {
            self.parse_question()
        } else {
            self.parse_impl()
        }
    }

    fn parse_question(&mut self) -> Result<Expr, String> {
        self.expect(&Tok::Question)?;

        // Optional label: ident ':'
        let label = if self.at_ident() {
            if let Some(Tok::Colon) = self.tokens.get(self.pos + 1) {
                let l = self.expect_ident()?;
                self.expect(&Tok::Colon)?;
                Some(l)
            } else {
                None
            }
        } else {
            None
        };

        // Optional bracket binding (implicit existential): [var:type]:
        let body = if self.at(&Tok::LBracket) {
            let (var, var_type, inner) = self.parse_binding_and()?;
            Expr::Exists {
                var,
                var_type,
                body: Box::new(inner),
                count: None,
            }
        } else {
            self.parse_impl()?
        };

        Ok(Expr::Question {
            label,
            body: Box::new(body),
        })
    }

    /// Parse [var:type]: and_expr
    fn parse_binding_and(&mut self) -> Result<(String, String, Expr), String> {
        self.expect(&Tok::LBracket)?;
        let var = self.expect_ident()?;
        self.expect(&Tok::Colon)?;
        let var_type = self.expect_ident()?;
        self.expect(&Tok::RBracket)?;
        self.expect(&Tok::Colon)?;
        let body = self.parse_and()?;
        Ok((var, var_type, body))
    }

    /// Parse [var:type]: impl_expr
    fn parse_binding_impl(&mut self) -> Result<(String, String, Expr), String> {
        self.expect(&Tok::LBracket)?;
        let var = self.expect_ident()?;
        self.expect(&Tok::Colon)?;
        let var_type = self.expect_ident()?;
        self.expect(&Tok::RBracket)?;
        self.expect(&Tok::Colon)?;
        let body = self.parse_impl()?;
        Ok((var, var_type, body))
    }

    fn parse_impl(&mut self) -> Result<Expr, String> {
        let left = self.parse_and()?;
        if self.at(&Tok::Implies) {
            self.advance();
            let right = self.parse_and()?;
            Ok(Expr::Implies {
                ante: Box::new(left),
                cons: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while self.at(&Tok::And) {
            self.advance();
            let right = self.parse_primary()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Ident(ref s)) if s == "not" => {
                self.advance();
                let inner = self.parse_primary()?;
                Ok(Expr::Not(Box::new(inner)))
            }
            Some(Tok::Ident(ref s)) if s == "always" => {
                self.advance();
                let (var, var_type, body) = self.parse_binding_impl()?;
                Ok(Expr::ForAll {
                    var,
                    var_type,
                    body: Box::new(body),
                })
            }
            Some(Tok::Ident(ref s)) if s == "the" => {
                self.advance();
                let (var, var_type, body) = self.parse_binding_impl()?;
                Ok(Expr::The {
                    var,
                    var_type,
                    body: Box::new(body),
                })
            }
            Some(Tok::Ident(ref s)) if s == "this" => {
                self.advance();
                let (var, var_type, body) = self.parse_binding_impl()?;
                Ok(Expr::This {
                    var,
                    var_type,
                    body: Box::new(body),
                })
            }
            Some(Tok::Ident(ref s)) if s == "that" => {
                self.advance();
                let (var, var_type, body) = self.parse_binding_impl()?;
                Ok(Expr::That {
                    var,
                    var_type,
                    body: Box::new(body),
                })
            }
            Some(Tok::Ident(ref s)) if s == "exists" => {
                self.advance();
                let (var, var_type, body) = self.parse_binding_and()?;
                // Check for count: , |var| = count_value
                let count = if self.at(&Tok::Comma) {
                    self.advance();
                    self.expect(&Tok::Pipe)?;
                    let _count_var = self.expect_ident()?;
                    self.expect(&Tok::Pipe)?;
                    // '=' is not a token anymore, so it appears as Ident("=")
                    if self.at_ident_val("=") {
                        self.advance();
                    }
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                Ok(Expr::Exists {
                    var,
                    var_type,
                    body: Box::new(body),
                    count,
                })
            }
            Some(Tok::Ident(ref s)) if s == "exists_many" => {
                self.advance();
                // [var:type, count]:
                self.expect(&Tok::LBracket)?;
                let var = self.expect_ident()?;
                self.expect(&Tok::Colon)?;
                let var_type = self.expect_ident()?;
                self.expect(&Tok::Comma)?;
                let count = match self.advance() {
                    Some(Tok::Num(n)) => n,
                    Some(Tok::Ident(s)) => s,  // vague: many, few, several
                    other => return Err(format!("expected count, got {:?}", other)),
                };
                self.expect(&Tok::RBracket)?;
                self.expect(&Tok::Colon)?;
                let body = self.parse_and()?;
                Ok(Expr::ExistsMany {
                    var,
                    var_type,
                    count,
                    body: Box::new(body),
                })
            }
            Some(Tok::Ident(s)) => {
                self.advance();
                if self.at(&Tok::LParen) {
                    // Predicate: name(role: expr, ...)
                    self.advance();
                    let mut roles = Vec::new();
                    while !self.at(&Tok::RParen) {
                        if !roles.is_empty() {
                            self.expect(&Tok::Comma)?;
                        }
                        let role_name = self.expect_ident()?;
                        self.expect(&Tok::Colon)?;
                        let role_val = self.parse_expr()?;
                        roles.push((role_name, role_val));
                    }
                    self.expect(&Tok::RParen)?;
                    Ok(Expr::Pred { name: s, roles })
                } else if self.at(&Tok::Colon) {
                    // Variable reference: name:type
                    self.advance();
                    let typ = self.expect_ident()?;
                    Ok(Expr::Var { name: s, typ })
                } else {
                    // Entity
                    Ok(Expr::Entity(s))
                }
            }
            other => Err(format!("unexpected token: {:?}", other)),
        }
    }
}

/// Parse a semantic output string into the logic AST.
pub fn parse(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        Err(format!(
            "unexpected token after expression: {:?}",
            parser.peek()
        ))
    } else {
        Ok(expr)
    }
}

// === Type checking ===
//
// Predicates declare role types in the lexicon. The proposition type `s`
// marks roles that accept a nested predicate as their value (sentential
// arguments). Entity-typed roles (e.g. `e`, `n`) must not receive a
// proposition. Predicates not declared in the lexicon are passed through
// without checking, so ad-hoc outputs don't fail.

/// True iff `arg` is an AST node that counts as a proposition (a truth-valued
/// expression rather than an entity).
fn is_proposition_value(arg: &Expr) -> bool {
    matches!(
        arg,
        Expr::Pred { .. }
            | Expr::Not(_)
            | Expr::And(_, _)
            | Expr::Implies { .. }
            | Expr::ForAll { .. }
            | Expr::The { .. }
            | Expr::This { .. }
            | Expr::That { .. }
            | Expr::Exists { .. }
            | Expr::ExistsMany { .. }
            | Expr::Question { .. }
    )
}

fn check_expr_types(expr: &Expr, lexicon: &Lexicon) -> Result<(), String> {
    match expr {
        Expr::Pred { name, roles } => {
            // If the predicate is declared, enforce role types. Otherwise skip.
            if let Some(pred) = lexicon.predicates.get(name) {
                for (role_name, role_val) in roles {
                    let declared = pred.roles.get(role_name);
                    let val_is_proposition = is_proposition_value(role_val);
                    match declared {
                        Some(t) if t == "s" => {
                            if !val_is_proposition {
                                return Err(format!(
                                    "role '{}' of '{}' expects s, got a non-proposition value",
                                    role_name, name
                                ));
                            }
                        }
                        Some(t) => {
                            if val_is_proposition {
                                return Err(format!(
                                    "role '{}' of '{}' expects {}, got a proposition",
                                    role_name, name, t
                                ));
                            }
                        }
                        None => {
                            // Role not declared. Be lenient for now — undeclared
                            // roles on declared predicates are a separate lint.
                        }
                    }
                    // Recurse into the value (propositions get their own checks).
                    check_expr_types(role_val, lexicon)?;
                }
            } else {
                // Undeclared predicate: recurse into role values but don't
                // enforce type discipline (we have no schema to enforce).
                for (_, role_val) in roles {
                    check_expr_types(role_val, lexicon)?;
                }
            }
            Ok(())
        }
        Expr::Not(e) => check_expr_types(e, lexicon),
        Expr::And(l, r) => {
            check_expr_types(l, lexicon)?;
            check_expr_types(r, lexicon)
        }
        Expr::Implies { ante, cons } => {
            check_expr_types(ante, lexicon)?;
            check_expr_types(cons, lexicon)
        }
        Expr::ForAll { body, .. }
        | Expr::The { body, .. }
        | Expr::This { body, .. }
        | Expr::That { body, .. }
        | Expr::Exists { body, .. }
        | Expr::ExistsMany { body, .. } => check_expr_types(body, lexicon),
        Expr::Question { body, .. } => check_expr_types(body, lexicon),
        Expr::Var { .. } | Expr::Entity(_) => Ok(()),
    }
}

/// Check that an `Expr` is well-typed against the lexicon's role declarations.
///
/// Returns `Ok(())` if every predicate's role values match their declared
/// types: roles declared `s` receive propositions, roles declared with any
/// other type do not. Predicates absent from the lexicon are passed through
/// without checking.
pub fn check_types(expr: &Expr, lexicon: &Lexicon) -> Result<(), String> {
    check_expr_types(expr, lexicon)
}

/// Parse a semantic output string into the logic AST, then type-check it
/// against the supplied lexicon. Returns the parsed `Expr` on success, or
/// the first parse/type error encountered.
pub fn parse_with_types(input: &str, lexicon: &Lexicon) -> Result<Expr, String> {
    let expr = parse(input)?;
    check_types(&expr, lexicon)?;
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_counted_existential_bare_number() {
        let s = "exists_many [x:e, 3]: man(theme: x) ∧ tall(theme: x)";
        let e = parse(s).expect("parse");
        match e {
            Expr::ExistsMany { var, var_type, count, body } => {
                assert_eq!(var, "x");
                assert_eq!(var_type, "e");
                assert_eq!(count, "3");
                assert!(matches!(*body, Expr::And(_, _)));
            }
            other => panic!("expected ExistsMany, got {:?}", other),
        }
    }

    #[test]
    fn parse_counted_existential_vague() {
        let s = "exists_many [x:e, many]: man(theme: x)";
        let e = parse(s).expect("parse");
        match e {
            Expr::ExistsMany { count, .. } => assert_eq!(count, "many"),
            other => panic!("expected ExistsMany, got {:?}", other),
        }
    }

    #[test]
    fn display_counted_existential_round_trips() {
        let s = "exists_many [x:e, 3]: man(theme: x) ∧ tall(theme: x)";
        let e = parse(s).expect("parse");
        assert_eq!(s, format!("{}", e));
    }
}

#[cfg(test)]
mod type_tests {
    use super::*;
    use crate::core::fixture::Fixture;

    fn test_lexicon() -> Lexicon {
        let json = r#"{
            "lexicon": {
                "predicates": [
                    {"name": "think", "roles": {"agent": "e", "content": "s"}, "forms": ["think"]},
                    {"name": "know", "roles": {"agent": "e", "content": "s"}, "forms": ["know"]},
                    {"name": "man", "roles": {"theme": "e"}, "forms": ["man"]},
                    {"name": "mortal", "roles": {"theme": "e"}, "forms": ["mortal"]}
                ],
                "entities": [
                    {"name": "socrates", "type": "e", "forms": ["Socrates"]}
                ]
            },
            "grammar": [],
            "sentences": []
        }"#;
        let fixture: Fixture = serde_json::from_str(json).unwrap();
        Lexicon::from_fixture(&fixture)
    }

    #[test]
    fn type_s_accepts_predicate() {
        let lex = test_lexicon();
        let e = parse("think(agent: socrates, content: mortal(theme: socrates))").unwrap();
        assert!(check_types(&e, &lex).is_ok());
    }

    #[test]
    fn type_e_rejects_predicate() {
        let lex = test_lexicon();
        let e = parse("mortal(theme: think(agent: socrates, content: mortal(theme: socrates)))").unwrap();
        let err = check_types(&e, &lex).unwrap_err();
        assert!(err.contains("theme"), "got: {}", err);
        assert!(err.contains("mortal"), "got: {}", err);
    }

    #[test]
    fn nested_propositions_checked() {
        let lex = test_lexicon();
        let e = parse("know(agent: socrates, content: think(agent: socrates, content: mortal(theme: socrates)))").unwrap();
        assert!(check_types(&e, &lex).is_ok());
    }

    #[test]
    fn nested_proposition_rejects_violation() {
        let lex = test_lexicon();
        // Outer is fine (content: s), but inner think has agent as a pred (agent: e).
        let e = parse("know(agent: socrates, content: think(agent: mortal(theme: socrates), content: mortal(theme: socrates)))").unwrap();
        assert!(check_types(&e, &lex).is_err());
    }

    #[test]
    fn undeclared_predicate_passes() {
        let lex = test_lexicon();
        let e = parse("unknown_pred(theme: socrates)").unwrap();
        assert!(check_types(&e, &lex).is_ok());
    }

    #[test]
    fn undeclared_predicate_with_nested_passes() {
        let lex = test_lexicon();
        // Unknown predicate containing a nested pred — passes since we can't check.
        let e = parse("unknown_pred(theme: mortal(theme: socrates))").unwrap();
        assert!(check_types(&e, &lex).is_ok());
    }

    #[test]
    fn parse_with_types_accepts() {
        let lex = test_lexicon();
        let s = "think(agent: socrates, content: mortal(theme: socrates))";
        assert!(parse_with_types(s, &lex).is_ok());
    }

    #[test]
    fn parse_with_types_rejects() {
        let lex = test_lexicon();
        let s = "mortal(theme: think(agent: socrates, content: mortal(theme: socrates)))";
        assert!(parse_with_types(s, &lex).is_err());
    }
}