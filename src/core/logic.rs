/// Core logical form AST.
/// Represents the semantic output of the grammar.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// Predicate application: name(role: arg, role: arg, ...)
    Pred {
        name: String,
        roles: Vec<(String, Expr)>,
    },
    /// Negation: not expr
    Not(Box<Expr>),
    /// Conjunction: expr ∧ expr
    And(Box<Expr>, Box<Expr>),
    /// Implication: ante -> cons
    Implies {
        ante: Box<Expr>,
        cons: Box<Expr>,
    },
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
    /// Existential: exists [var:type]: body [, |var| = count]
    Exists {
        var: String,
        var_type: String,
        body: Box<Expr>,
        count: Option<String>,
    },
    /// Variable reference: name:type
    Var {
        name: String,
        typ: String,
    },
    /// Named entity: socrates, the_meeting
    Entity(String),
    /// Question: ? [label:] body
    Question {
        label: Option<String>,
        body: Box<Expr>,
    },
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Expr::Pred { name, roles } => {
                write!(f, "{}(", name)?;
                for (i, (r, a)) in roles.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", r, a)?;
                }
                write!(f, ")")
            }
            Expr::Not(e) => write!(f, "not {}", e),
            Expr::And(l, r) => write!(f, "{} ∧ {}", l, r),
            Expr::Implies { ante, cons } => write!(f, "{} -> {}", ante, cons),
            Expr::ForAll { var, var_type, body } => {
                write!(f, "always [{}:{}]: {}", var, var_type, body)
            }
            Expr::The { var, var_type, body } => {
                write!(f, "the [{}:{}]: {}", var, var_type, body)
            }
            Expr::Exists { var, var_type, body, count } => {
                write!(f, "exists [{}:{}]: {}", var, var_type, body)?;
                if let Some(c) = count {
                    write!(f, ", |{}| = {}", var, c)?;
                }
                Ok(())
            }
            Expr::Var { name, typ } => write!(f, "{}:{}", name, typ),
            Expr::Entity(s) => write!(f, "{}", s),
            Expr::Question { label, body } => {
                write!(f, "? ")?;
                if let Some(l) = label { write!(f, "{}: ", l)?; }
                write!(f, "{}", body)
            }
        }
    }
}