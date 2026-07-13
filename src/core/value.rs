use crate::core::logic::Expr;

/// The quantifier component of a DP.
#[derive(Debug, Clone)]
pub enum DpQuant {
    /// always [x]: restriction(x) -> body(x)
    ForAll { restriction: Expr },
    /// exists [x]: restriction(x) ∧ body(x)
    Exists { restriction: Expr },
    /// the [x]: restriction(x) -> body(x)
    The { restriction: Expr },
    /// Bare entity — no quantifier shell, variable is a constant name.
    Bare,
}

/// A proposition with exactly one free variable and a designated gap role.
/// Used for relative clause gaps like "loves sue" (agent gap).
#[derive(Debug, Clone)]
pub struct GapProp {
    pub var: String,
    pub var_type: String,
    pub gap_role: String,
    pub body: Expr,
}

/// Semantic value produced by a phrase during bottom-up parsing.
#[derive(Debug, Clone)]
pub enum SemValue {
    /// A complete proposition (output of S-level rules).
    Prop(Expr),
    /// A determiner phrase that introduces an entity variable.
    Dp {
        var: String,
        var_type: String,
        quant: DpQuant,
    },
    /// A noun phrase: a predicate restriction with a free variable.
    /// The determiner will bind this variable.
    N { var: String, restriction: Expr },
    /// A gapped proposition: a predicate with one free variable and a designated hole.
    GapProp(GapProp),
}

impl std::fmt::Display for SemValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SemValue::Prop(expr) => write!(f, "{}", expr),
            SemValue::Dp {
                var,
                var_type,
                quant,
            } => match quant {
                DpQuant::ForAll { restriction } => {
                    write!(f, "∀[{}:{}] ", var, var_type)?;
                    write!(f, "{}", restriction)
                }
                DpQuant::Exists { restriction } => {
                    write!(f, "∃[{}:{}] ", var, var_type)?;
                    write!(f, "{}", restriction)
                }
                DpQuant::The { restriction } => {
                    write!(f, "ι[{}:{}] ", var, var_type)?;
                    write!(f, "{}", restriction)
                }
                DpQuant::Bare => {
                    write!(f, "{}:{}", var, var_type)
                }
            },
            SemValue::N { var, restriction } => {
                write!(f, "N[{}] {}", var, restriction)
            }
            SemValue::GapProp(g) => {
                write!(f, "_[{}] {}", g.gap_role, g.body)
            }
        }
    }
}

/// Generates fresh variable names.
#[derive(Debug, Clone)]
pub struct VarGen {
    counter: u32,
}

impl VarGen {
    pub fn new() -> Self {
        VarGen { counter: 0 }
    }

    pub(crate) fn position(&self) -> u32 {
        self.counter
    }

    pub(crate) fn advance_to(&mut self, position: u32) {
        self.counter = self.counter.max(position);
    }

    /// Generate a fresh variable name: x, x1, x2, ...
    pub fn fresh(&mut self) -> String {
        let name = if self.counter == 0 {
            "x".to_string()
        } else {
            format!("x{}", self.counter)
        };
        self.counter += 1;
        name
    }
}

impl Default for VarGen {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GapProp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "_[{}] {}", self.gap_role, self.body)
    }
}
