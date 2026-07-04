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
///              | 'exists' binding and_expr [count]
///              | ident '(' role_list ')'       -- predicate
///              | ident ':' ident               -- variable ref
///              | ident                         -- entity
///   binding    = '[' ident ':' ident ']' ':'
///   count      = ',' '|' ident '|' '=' ident

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