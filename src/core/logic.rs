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

/// Compare logical expressions modulo consistent renaming of bound variables.
///
/// Free variables, entities, binder kinds, binder types, predicate names,
/// role order, counts, and question labels must still match exactly.
pub fn alpha_equivalent(left: &Expr, right: &Expr) -> bool {
    fn bound_index(name: &str, environment: &[String]) -> Option<usize> {
        environment.iter().rposition(|bound| bound == name)
    }

    fn equivalent(
        left: &Expr,
        right: &Expr,
        left_environment: &mut Vec<String>,
        right_environment: &mut Vec<String>,
    ) -> bool {
        match (left, right) {
            (
                Expr::Pred {
                    name: left_name,
                    roles: left_roles,
                },
                Expr::Pred {
                    name: right_name,
                    roles: right_roles,
                },
            ) => {
                left_name == right_name
                    && left_roles.len() == right_roles.len()
                    && left_roles.iter().zip(right_roles).all(
                        |((left_role, left_value), (right_role, right_value))| {
                            left_role == right_role
                                && equivalent(
                                    left_value,
                                    right_value,
                                    left_environment,
                                    right_environment,
                                )
                        },
                    )
            }
            (Expr::Not(left_inner), Expr::Not(right_inner)) => {
                equivalent(left_inner, right_inner, left_environment, right_environment)
            }
            (Expr::And(left_left, left_right), Expr::And(right_left, right_right)) => {
                equivalent(left_left, right_left, left_environment, right_environment)
                    && equivalent(left_right, right_right, left_environment, right_environment)
            }
            (
                Expr::Implies {
                    ante: left_ante,
                    cons: left_cons,
                },
                Expr::Implies {
                    ante: right_ante,
                    cons: right_cons,
                },
            ) => {
                equivalent(left_ante, right_ante, left_environment, right_environment)
                    && equivalent(left_cons, right_cons, left_environment, right_environment)
            }
            (
                Expr::ForAll {
                    var: left_var,
                    var_type: left_type,
                    body: left_body,
                },
                Expr::ForAll {
                    var: right_var,
                    var_type: right_type,
                    body: right_body,
                },
            )
            | (
                Expr::The {
                    var: left_var,
                    var_type: left_type,
                    body: left_body,
                },
                Expr::The {
                    var: right_var,
                    var_type: right_type,
                    body: right_body,
                },
            )
            | (
                Expr::This {
                    var: left_var,
                    var_type: left_type,
                    body: left_body,
                },
                Expr::This {
                    var: right_var,
                    var_type: right_type,
                    body: right_body,
                },
            )
            | (
                Expr::That {
                    var: left_var,
                    var_type: left_type,
                    body: left_body,
                },
                Expr::That {
                    var: right_var,
                    var_type: right_type,
                    body: right_body,
                },
            ) => {
                if left_type != right_type {
                    return false;
                }

                left_environment.push(left_var.clone());
                right_environment.push(right_var.clone());

                let result = equivalent(left_body, right_body, left_environment, right_environment);

                left_environment.pop();
                right_environment.pop();
                result
            }
            (
                Expr::Exists {
                    var: left_var,
                    var_type: left_type,
                    body: left_body,
                    count: left_count,
                },
                Expr::Exists {
                    var: right_var,
                    var_type: right_type,
                    body: right_body,
                    count: right_count,
                },
            ) => {
                if left_type != right_type || left_count != right_count {
                    return false;
                }

                left_environment.push(left_var.clone());
                right_environment.push(right_var.clone());

                let result = equivalent(left_body, right_body, left_environment, right_environment);

                left_environment.pop();
                right_environment.pop();
                result
            }
            (
                Expr::ExistsMany {
                    var: left_var,
                    var_type: left_type,
                    count: left_count,
                    body: left_body,
                },
                Expr::ExistsMany {
                    var: right_var,
                    var_type: right_type,
                    count: right_count,
                    body: right_body,
                },
            ) => {
                if left_type != right_type || left_count != right_count {
                    return false;
                }

                left_environment.push(left_var.clone());
                right_environment.push(right_var.clone());

                let result = equivalent(left_body, right_body, left_environment, right_environment);

                left_environment.pop();
                right_environment.pop();
                result
            }
            (
                Expr::Var {
                    name: left_name,
                    typ: left_type,
                },
                Expr::Var {
                    name: right_name,
                    typ: right_type,
                },
            ) => {
                if left_type != right_type {
                    return false;
                }

                match (
                    bound_index(left_name, left_environment),
                    bound_index(right_name, right_environment),
                ) {
                    (Some(left_index), Some(right_index)) => left_index == right_index,
                    (None, None) => left_name == right_name,
                    _ => false,
                }
            }
            (Expr::Entity(left_name), Expr::Entity(right_name)) => left_name == right_name,
            (
                Expr::Question {
                    label: left_label,
                    body: left_body,
                },
                Expr::Question {
                    label: right_label,
                    body: right_body,
                },
            ) => {
                left_label == right_label
                    && equivalent(left_body, right_body, left_environment, right_environment)
            }
            _ => false,
        }
    }

    equivalent(left, right, &mut Vec::new(), &mut Vec::new())
}

#[cfg(test)]
mod alpha_equivalence_tests {
    use super::*;

    fn variable(name: &str, typ: &str) -> Expr {
        Expr::Var {
            name: name.to_string(),
            typ: typ.to_string(),
        }
    }

    fn relation(agent: Expr, patient: Expr) -> Expr {
        Expr::Pred {
            name: "loves".to_string(),
            roles: vec![
                ("agent".to_string(), agent),
                ("patient".to_string(), patient),
            ],
        }
    }

    #[test]
    fn consistently_renamed_binders_are_equivalent() {
        let left = Expr::ForAll {
            var: "x".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::Exists {
                var: "y".to_string(),
                var_type: "e".to_string(),
                body: Box::new(relation(variable("x", "e"), variable("y", "e"))),
                count: None,
            }),
        };

        let right = Expr::ForAll {
            var: "a".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::Exists {
                var: "b".to_string(),
                var_type: "e".to_string(),
                body: Box::new(relation(variable("a", "e"), variable("b", "e"))),
                count: None,
            }),
        };

        assert!(alpha_equivalent(&left, &right));
    }

    #[test]
    fn nested_shadowing_uses_the_nearest_binder() {
        let left = Expr::ForAll {
            var: "x".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::Exists {
                var: "x".to_string(),
                var_type: "e".to_string(),
                body: Box::new(relation(variable("x", "e"), variable("x", "e"))),
                count: None,
            }),
        };

        let equivalent = Expr::ForAll {
            var: "a".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::Exists {
                var: "b".to_string(),
                var_type: "e".to_string(),
                body: Box::new(relation(variable("b", "e"), variable("b", "e"))),
                count: None,
            }),
        };

        let not_equivalent = Expr::ForAll {
            var: "a".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::Exists {
                var: "b".to_string(),
                var_type: "e".to_string(),
                body: Box::new(relation(variable("a", "e"), variable("b", "e"))),
                count: None,
            }),
        };

        assert!(alpha_equivalent(&left, &equivalent));
        assert!(!alpha_equivalent(&left, &not_equivalent));
    }

    #[test]
    fn free_variables_must_keep_their_names_and_types() {
        assert!(alpha_equivalent(&variable("x", "e"), &variable("x", "e"),));
        assert!(!alpha_equivalent(&variable("x", "e"), &variable("y", "e"),));
        assert!(!alpha_equivalent(&variable("x", "e"), &variable("x", "s"),));
    }

    #[test]
    fn binder_kind_type_and_count_remain_significant() {
        let body = Box::new(variable("x", "e"));

        let universal = Expr::ForAll {
            var: "x".to_string(),
            var_type: "e".to_string(),
            body: body.clone(),
        };
        let definite = Expr::The {
            var: "y".to_string(),
            var_type: "e".to_string(),
            body: Box::new(variable("y", "e")),
        };
        let counted_three = Expr::ExistsMany {
            var: "x".to_string(),
            var_type: "e".to_string(),
            count: "3".to_string(),
            body: body.clone(),
        };
        let counted_two = Expr::ExistsMany {
            var: "y".to_string(),
            var_type: "e".to_string(),
            count: "2".to_string(),
            body: Box::new(variable("y", "e")),
        };

        assert!(!alpha_equivalent(&universal, &definite));
        assert!(!alpha_equivalent(&counted_three, &counted_two));
    }
}
