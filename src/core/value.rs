
/// The quantifier component of a DP.
#[derive(Debug, Clone)]
pub enum DpQuant {
    /// always [x]: restriction(x) -> body(x)
    ForAll { restriction: crate::core::logic::Expr },
    /// exists [x]: restriction(x) ∧ body(x)
    Exists { restriction: crate::core::logic::Expr },
    /// the [x]: restriction(x) -> body(x)
    The { restriction: crate::core::logic::Expr },
    /// Bare entity — no quantifier shell, variable is a constant name.
    Bare,
}

/// Semantic value produced by a phrase during bottom-up parsing.
#[derive(Debug, Clone)]
pub enum SemValue {
    /// A complete proposition (output of S-level rules).
    Prop(crate::core::logic::Expr),
    /// A determiner phrase that introduces an entity variable.
    Dp {
        var: String,
        var_type: String,
        quant: DpQuant,
    },
}

/// Generates fresh variable names.
#[derive(Debug)]
pub struct VarGen {
    counter: u32,
}

impl VarGen {
    pub fn new() -> Self {
        VarGen { counter: 0 }
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