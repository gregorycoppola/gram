/// Core logical form AST.
/// Represents the semantic output of the grammar.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// Predicate application: name(role: arg, ...)
    Pred {
        name: String,
        roles: Vec<(String, Expr)>,
    },
    /// Negation: not expr
    Not(Box<Expr>),
    /// Conjunction: expr ∧ expr
    And(Box<Expr>, Box<Expr>),
    /// Implication: ante -> cons
    Implies { ante: Box<Expr>, cons: Box<Expr> },
    /// Universal: always [var:type]: body
    ForAll {
        var: String,
        var_type: String,
        body: Box<Expr>,
    },
    /// Definite description: the [var:type]: body
    The {
        var: String,
        var_type: String,
        body: Box<Expr>,
    },
    /// Demonstrative: this [var:type]: body
    This {
        var: String,
        var_type: String,
        body: Box<Expr>,
    },
    /// Demonstrative: that [var:type]: body
    That {
        var: String,
        var_type: String,
        body: Box<Expr>,
    },
    /// Existential: exists [var:type]: body [, |var| = count]
    Exists {
        var: String,
        var_type: String,
        body: Box<Expr>,
        count: Option<String>,
    },
    /// Counted existential: exists_many [var:type, count]: body
    /// count is a string to allow bare numbers ("3"), vague atoms ("many"),
    /// and (in future) inequality specs (">=3") or ranges ("3..5").
    ExistsMany {
        var: String,
        var_type: String,
        count: String,
        body: Box<Expr>,
    },
    /// Variable reference: name:type
    Var { name: String, typ: String },
    /// Named entity: socrates, the_meeting
    Entity(String),
    /// Question: ? [label:] body
    Question {
        label: Option<String>,
        body: Box<Expr>,
    },
}

/// True if this expression needs parentheses when nested inside another
/// quantifier body or as the consequent of an implication.
fn is_quantifier(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::ForAll { .. }
            | Expr::The { .. }
            | Expr::This { .. }
            | Expr::That { .. }
            | Expr::Exists { .. }
            | Expr::ExistsMany { .. }
    )
}

/// True if this expression needs parentheses as the consequent of an implication.
fn cons_needs_parens(expr: &Expr) -> bool {
    is_quantifier(expr) || matches!(expr, Expr::Implies { .. })
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Expr::Pred { name, roles } => {
                write!(f, "{}(", name)?;
                for (i, (r, a)) in roles.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", r, a)?;
                }
                write!(f, ")")
            }
            Expr::Not(e) => write!(f, "not {}", e),
            Expr::And(l, r) => write!(f, "{} ∧ {}", l, r),
            Expr::Implies { ante, cons } => {
                write!(f, "{} -> ", ante)?;
                if cons_needs_parens(cons) {
                    write!(f, "({})", cons)
                } else {
                    write!(f, "{}", cons)
                }
            }
            Expr::ForAll {
                var,
                var_type,
                body,
            } => {
                write!(f, "always [{}:{}]: ", var, var_type)?;
                if is_quantifier(body) {
                    write!(f, "({})", body)
                } else {
                    write!(f, "{}", body)
                }
            }
            Expr::The {
                var,
                var_type,
                body,
            } => {
                write!(f, "the [{}:{}]: ", var, var_type)?;
                if is_quantifier(body) {
                    write!(f, "({})", body)
                } else {
                    write!(f, "{}", body)
                }
            }
            Expr::This {
                var,
                var_type,
                body,
            } => {
                write!(f, "this [{}:{}]: ", var, var_type)?;
                if is_quantifier(body) {
                    write!(f, "({})", body)
                } else {
                    write!(f, "{}", body)
                }
            }
            Expr::That {
                var,
                var_type,
                body,
            } => {
                write!(f, "that [{}:{}]: ", var, var_type)?;
                if is_quantifier(body) {
                    write!(f, "({})", body)
                } else {
                    write!(f, "{}", body)
                }
            }
            Expr::Exists {
                var,
                var_type,
                body,
                count,
            } => {
                write!(f, "exists [{}:{}]: {}", var, var_type, body)?;
                if let Some(c) = count {
                    write!(f, ", |{}| = {}", var, c)?;
                }
                Ok(())
            }
            Expr::ExistsMany {
                var,
                var_type,
                count,
                body,
            } => {
                write!(f, "exists_many [{}:{}, {}]: ", var, var_type, count)?;
                if is_quantifier(body) {
                    write!(f, "({})", body)
                } else {
                    write!(f, "{}", body)
                }
            }
            Expr::Var { name, .. } => write!(f, "{}", name),
            Expr::Entity(s) => write!(f, "{}", s),
            Expr::Question { label, body } => {
                write!(f, "? ")?;
                if let Some(l) = label {
                    write!(f, "{}: ", l)?;
                }
                write!(f, "{}", body)
            }
        }
    }
}
