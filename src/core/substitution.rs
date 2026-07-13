use std::collections::HashSet;

use crate::core::logic::Expr;

#[derive(Debug, Clone, Copy)]
enum OccurrencePolicy {
    VariableOnly,
    VariableOrEntityPlaceholder,
}

impl OccurrencePolicy {
    fn matches_entity(self) -> bool {
        matches!(self, Self::VariableOrEntityPlaceholder)
    }
}

#[derive(Debug, Clone, Copy)]
enum Replacement<'a> {
    Expression(&'a Expr),
    Rename(&'a str),
    Entity(&'a str),
}

impl Replacement<'_> {
    fn free_variable_names(self) -> HashSet<String> {
        match self {
            Self::Expression(expr) => free_variable_names(expr),
            Self::Rename(name) => HashSet::from([name.to_string()]),
            Self::Entity(_) => HashSet::new(),
        }
    }

    fn all_variable_names(self) -> HashSet<String> {
        match self {
            Self::Expression(expr) => all_variable_names(expr),
            Self::Rename(name) => HashSet::from([name.to_string()]),
            Self::Entity(_) => HashSet::new(),
        }
    }

    fn replace_variable(self, original_type: &str) -> Expr {
        match self {
            Self::Expression(expr) => expr.clone(),
            Self::Rename(name) => Expr::Var {
                name: name.to_string(),
                typ: original_type.to_string(),
            },
            Self::Entity(name) => Expr::Entity(name.to_string()),
        }
    }

    fn replace_entity_placeholder(self) -> Expr {
        match self {
            Self::Expression(expr) => expr.clone(),
            Self::Rename(name) | Self::Entity(name) => Expr::Entity(name.to_string()),
        }
    }
}

struct Substituter<'a> {
    target: &'a str,
    replacement: Replacement<'a>,
    policy: OccurrencePolicy,
    replacement_free: HashSet<String>,
    used_names: HashSet<String>,
}

impl<'a> Substituter<'a> {
    fn new(
        expr: &Expr,
        target: &'a str,
        replacement: Replacement<'a>,
        policy: OccurrencePolicy,
    ) -> Self {
        let replacement_free = replacement.free_variable_names();
        let mut used_names = all_variable_names(expr);
        used_names.extend(replacement.all_variable_names());

        Self {
            target,
            replacement,
            policy,
            replacement_free,
            used_names,
        }
    }

    fn apply(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::Var { name, typ } if name == self.target => {
                self.replacement.replace_variable(typ)
            }
            Expr::Var { name, typ } => Expr::Var {
                name: name.clone(),
                typ: typ.clone(),
            },
            Expr::Entity(name) if self.policy.matches_entity() && name == self.target => {
                self.replacement.replace_entity_placeholder()
            }
            Expr::Entity(name) => Expr::Entity(name.clone()),
            Expr::Pred { name, roles } => {
                let mut replaced_roles = Vec::with_capacity(roles.len());

                for (role, value) in roles {
                    replaced_roles.push((role.clone(), self.apply(value)));
                }

                Expr::Pred {
                    name: name.clone(),
                    roles: replaced_roles,
                }
            }
            Expr::Not(inner) => Expr::Not(Box::new(self.apply(inner))),
            Expr::And(left, right) => {
                Expr::And(Box::new(self.apply(left)), Box::new(self.apply(right)))
            }
            Expr::Implies { ante, cons } => Expr::Implies {
                ante: Box::new(self.apply(ante)),
                cons: Box::new(self.apply(cons)),
            },
            Expr::ForAll {
                var,
                var_type,
                body,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::ForAll {
                    var,
                    var_type: var_type.clone(),
                    body,
                }
            }
            Expr::The {
                var,
                var_type,
                body,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::The {
                    var,
                    var_type: var_type.clone(),
                    body,
                }
            }
            Expr::This {
                var,
                var_type,
                body,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::This {
                    var,
                    var_type: var_type.clone(),
                    body,
                }
            }
            Expr::That {
                var,
                var_type,
                body,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::That {
                    var,
                    var_type: var_type.clone(),
                    body,
                }
            }
            Expr::Exists {
                var,
                var_type,
                body,
                count,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::Exists {
                    var,
                    var_type: var_type.clone(),
                    body,
                    count: count.clone(),
                }
            }
            Expr::ExistsMany {
                var,
                var_type,
                count,
                body,
            } => {
                let (var, body) = self.apply_binder(var, body);

                Expr::ExistsMany {
                    var,
                    var_type: var_type.clone(),
                    count: count.clone(),
                    body,
                }
            }
            Expr::Question { label, body } => Expr::Question {
                label: label.clone(),
                body: Box::new(self.apply(body)),
            },
        }
    }

    fn apply_binder(&mut self, binder: &str, body: &Expr) -> (String, Box<Expr>) {
        if binder == self.target || !contains_free_occurrence(body, self.target, self.policy) {
            return (binder.to_string(), Box::new(body.clone()));
        }

        let (binder, body) = if self.replacement_free.contains(binder) {
            let fresh = self.fresh_name(binder);
            let renamed = alpha_rename_bound_variable(body, binder, &fresh);
            (fresh, renamed)
        } else {
            (binder.to_string(), body.clone())
        };

        (binder, Box::new(self.apply(&body)))
    }

    fn fresh_name(&mut self, base: &str) -> String {
        let mut suffix = 1usize;

        loop {
            let candidate = format!("{}{}", base, suffix);

            if self.used_names.insert(candidate.clone()) {
                return candidate;
            }

            suffix += 1;
        }
    }
}

/// Replace free logical-variable occurrences of `target`.
///
/// Named entities remain constants and are not rewritten.
pub fn substitute_free_var(expr: &Expr, target: &str, replacement: &Expr) -> Expr {
    Substituter::new(
        expr,
        target,
        Replacement::Expression(replacement),
        OccurrencePolicy::VariableOnly,
    )
    .apply(expr)
}

/// Rename free logical-variable occurrences while preserving their types.
pub fn rename_free_var(expr: &Expr, target: &str, replacement_name: &str) -> Expr {
    Substituter::new(
        expr,
        target,
        Replacement::Rename(replacement_name),
        OccurrencePolicy::VariableOnly,
    )
    .apply(expr)
}

/// Replace free logical-variable occurrences with a named entity.
pub fn substitute_var_with_entity(expr: &Expr, target: &str, entity: &str) -> Expr {
    Substituter::new(
        expr,
        target,
        Replacement::Entity(entity),
        OccurrencePolicy::VariableOnly,
    )
    .apply(expr)
}

/// Replace a proof-level formula placeholder.
///
/// This additionally recognizes placeholders represented as
/// `Expr::Entity(target)`.
pub fn substitute_formula_placeholder(expr: &Expr, target: &str, replacement: &Expr) -> Expr {
    Substituter::new(
        expr,
        target,
        Replacement::Expression(replacement),
        OccurrencePolicy::VariableOrEntityPlaceholder,
    )
    .apply(expr)
}

fn contains_free_occurrence(expr: &Expr, target: &str, policy: OccurrencePolicy) -> bool {
    match expr {
        Expr::Var { name, .. } => name == target,
        Expr::Entity(name) => policy.matches_entity() && name == target,
        Expr::Pred { roles, .. } => roles
            .iter()
            .any(|(_, value)| contains_free_occurrence(value, target, policy)),
        Expr::Not(inner) => contains_free_occurrence(inner, target, policy),
        Expr::And(left, right) => {
            contains_free_occurrence(left, target, policy)
                || contains_free_occurrence(right, target, policy)
        }
        Expr::Implies { ante, cons } => {
            contains_free_occurrence(ante, target, policy)
                || contains_free_occurrence(cons, target, policy)
        }
        Expr::ForAll { var, body, .. }
        | Expr::The { var, body, .. }
        | Expr::This { var, body, .. }
        | Expr::That { var, body, .. }
        | Expr::Exists { var, body, .. }
        | Expr::ExistsMany { var, body, .. } => {
            var != target && contains_free_occurrence(body, target, policy)
        }
        Expr::Question { body, .. } => contains_free_occurrence(body, target, policy),
    }
}

fn free_variable_names(expr: &Expr) -> HashSet<String> {
    match expr {
        Expr::Var { name, .. } => HashSet::from([name.clone()]),
        Expr::Entity(_) => HashSet::new(),
        Expr::Pred { roles, .. } => roles
            .iter()
            .flat_map(|(_, value)| free_variable_names(value))
            .collect(),
        Expr::Not(inner) => free_variable_names(inner),
        Expr::And(left, right) => {
            let mut names = free_variable_names(left);
            names.extend(free_variable_names(right));
            names
        }
        Expr::Implies { ante, cons } => {
            let mut names = free_variable_names(ante);
            names.extend(free_variable_names(cons));
            names
        }
        Expr::ForAll { var, body, .. }
        | Expr::The { var, body, .. }
        | Expr::This { var, body, .. }
        | Expr::That { var, body, .. }
        | Expr::Exists { var, body, .. }
        | Expr::ExistsMany { var, body, .. } => {
            let mut names = free_variable_names(body);
            names.remove(var);
            names
        }
        Expr::Question { body, .. } => free_variable_names(body),
    }
}

fn all_variable_names(expr: &Expr) -> HashSet<String> {
    match expr {
        Expr::Var { name, .. } => HashSet::from([name.clone()]),
        Expr::Entity(_) => HashSet::new(),
        Expr::Pred { roles, .. } => roles
            .iter()
            .flat_map(|(_, value)| all_variable_names(value))
            .collect(),
        Expr::Not(inner) => all_variable_names(inner),
        Expr::And(left, right) => {
            let mut names = all_variable_names(left);
            names.extend(all_variable_names(right));
            names
        }
        Expr::Implies { ante, cons } => {
            let mut names = all_variable_names(ante);
            names.extend(all_variable_names(cons));
            names
        }
        Expr::ForAll { var, body, .. }
        | Expr::The { var, body, .. }
        | Expr::This { var, body, .. }
        | Expr::That { var, body, .. }
        | Expr::Exists { var, body, .. }
        | Expr::ExistsMany { var, body, .. } => {
            let mut names = all_variable_names(body);
            names.insert(var.clone());
            names
        }
        Expr::Question { body, .. } => all_variable_names(body),
    }
}

/// Rename occurrences controlled by one particular enclosing binder.
///
/// A nested binder with the same name begins a separate scope and remains
/// unchanged.
fn alpha_rename_bound_variable(expr: &Expr, old_name: &str, new_name: &str) -> Expr {
    match expr {
        Expr::Var { name, typ } if name == old_name => Expr::Var {
            name: new_name.to_string(),
            typ: typ.clone(),
        },
        Expr::Var { name, typ } => Expr::Var {
            name: name.clone(),
            typ: typ.clone(),
        },
        Expr::Entity(name) => Expr::Entity(name.clone()),
        Expr::Pred { name, roles } => Expr::Pred {
            name: name.clone(),
            roles: roles
                .iter()
                .map(|(role, value)| {
                    (
                        role.clone(),
                        alpha_rename_bound_variable(value, old_name, new_name),
                    )
                })
                .collect(),
        },
        Expr::Not(inner) => Expr::Not(Box::new(alpha_rename_bound_variable(
            inner, old_name, new_name,
        ))),
        Expr::And(left, right) => Expr::And(
            Box::new(alpha_rename_bound_variable(left, old_name, new_name)),
            Box::new(alpha_rename_bound_variable(right, old_name, new_name)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(alpha_rename_bound_variable(ante, old_name, new_name)),
            cons: Box::new(alpha_rename_bound_variable(cons, old_name, new_name)),
        },
        Expr::ForAll {
            var,
            var_type,
            body,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::ForAll {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                }
            }
        }
        Expr::The {
            var,
            var_type,
            body,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::The {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                }
            }
        }
        Expr::This {
            var,
            var_type,
            body,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::This {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                }
            }
        }
        Expr::That {
            var,
            var_type,
            body,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::That {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                }
            }
        }
        Expr::Exists {
            var,
            var_type,
            body,
            count,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::Exists {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                    count: count.clone(),
                }
            }
        }
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => {
            if var == old_name {
                expr.clone()
            } else {
                Expr::ExistsMany {
                    var: var.clone(),
                    var_type: var_type.clone(),
                    count: count.clone(),
                    body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
                }
            }
        }
        Expr::Question { label, body } => Expr::Question {
            label: label.clone(),
            body: Box::new(alpha_rename_bound_variable(body, old_name, new_name)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(name: &str, typ: &str) -> Expr {
        Expr::Var {
            name: name.to_string(),
            typ: typ.to_string(),
        }
    }

    fn relation(left: Expr, right: Expr) -> Expr {
        Expr::Pred {
            name: "related".to_string(),
            roles: vec![("left".to_string(), left), ("right".to_string(), right)],
        }
    }

    fn binder_variants(body: Expr, name: &str) -> Vec<Expr> {
        vec![
            Expr::ForAll {
                var: name.to_string(),
                var_type: "e".to_string(),
                body: Box::new(body.clone()),
            },
            Expr::The {
                var: name.to_string(),
                var_type: "e".to_string(),
                body: Box::new(body.clone()),
            },
            Expr::This {
                var: name.to_string(),
                var_type: "e".to_string(),
                body: Box::new(body.clone()),
            },
            Expr::That {
                var: name.to_string(),
                var_type: "e".to_string(),
                body: Box::new(body.clone()),
            },
            Expr::Exists {
                var: name.to_string(),
                var_type: "e".to_string(),
                body: Box::new(body.clone()),
                count: Some("3".to_string()),
            },
            Expr::ExistsMany {
                var: name.to_string(),
                var_type: "e".to_string(),
                count: "many".to_string(),
                body: Box::new(body),
            },
        ]
    }

    fn binder_parts(expr: &Expr) -> (&str, &str, &Expr) {
        match expr {
            Expr::ForAll {
                var,
                var_type,
                body,
            }
            | Expr::The {
                var,
                var_type,
                body,
            }
            | Expr::This {
                var,
                var_type,
                body,
            }
            | Expr::That {
                var,
                var_type,
                body,
            }
            | Expr::Exists {
                var,
                var_type,
                body,
                ..
            }
            | Expr::ExistsMany {
                var,
                var_type,
                body,
                ..
            } => (var, var_type, body),
            other => panic!("expected binder, got {:?}", other),
        }
    }

    #[test]
    fn rename_preserves_occurrence_type() {
        let result = rename_free_var(&variable("p", "s"), "p", "q");

        assert_eq!(result, variable("q", "s"));
    }

    #[test]
    fn every_binder_shadows_the_target() {
        for source in binder_variants(variable("x", "e"), "x") {
            let result = substitute_var_with_entity(&source, "x", "sue");

            assert_eq!(result, source);
        }
    }

    #[test]
    fn every_binder_avoids_capture() {
        let body = relation(variable("x", "e"), variable("y", "e"));
        let replacement = variable("y", "e");

        for source in binder_variants(body.clone(), "y") {
            let result = substitute_free_var(&source, "x", &replacement);
            let (binder, binder_type, result_body) = binder_parts(&result);

            assert_eq!(binder, "y1");
            assert_eq!(binder_type, "e");
            assert_eq!(
                result_body,
                &relation(variable("y", "e"), variable("y1", "e"))
            );
        }
    }

    #[test]
    fn alpha_renaming_stops_at_nested_shadowing() {
        let source = Expr::ForAll {
            var: "x".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::And(
                Box::new(relation(variable("y", "e"), variable("x", "e"))),
                Box::new(Expr::Exists {
                    var: "x".to_string(),
                    var_type: "e".to_string(),
                    body: Box::new(variable("x", "e")),
                    count: None,
                }),
            )),
        };

        let result = substitute_free_var(&source, "y", &variable("x", "e"));

        let expected = Expr::ForAll {
            var: "x1".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::And(
                Box::new(relation(variable("x", "e"), variable("x1", "e"))),
                Box::new(Expr::Exists {
                    var: "x".to_string(),
                    var_type: "e".to_string(),
                    body: Box::new(variable("x", "e")),
                    count: None,
                }),
            )),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn fresh_names_skip_existing_suffixes() {
        let source = Expr::ForAll {
            var: "y".to_string(),
            var_type: "e".to_string(),
            body: Box::new(Expr::And(
                Box::new(variable("x", "e")),
                Box::new(Expr::And(
                    Box::new(variable("y1", "e")),
                    Box::new(variable("y2", "e")),
                )),
            )),
        };

        let result = substitute_free_var(&source, "x", &variable("y", "e"));
        let (binder, _, _) = binder_parts(&result);

        assert_eq!(binder, "y3");
    }

    #[test]
    fn absent_target_does_not_rename_binder() {
        let source = Expr::ForAll {
            var: "y".to_string(),
            var_type: "e".to_string(),
            body: Box::new(variable("z", "e")),
        };

        let result = substitute_free_var(&source, "x", &variable("y", "e"));

        assert_eq!(result, source);
    }

    #[test]
    fn entity_constants_are_distinct_from_placeholders() {
        let source = Expr::Entity("P".to_string());
        let replacement = relation(
            Expr::Entity("john".to_string()),
            Expr::Entity("sue".to_string()),
        );

        assert_eq!(substitute_free_var(&source, "P", &replacement), source);
        assert_eq!(
            substitute_formula_placeholder(&source, "P", &replacement),
            replacement
        );
    }

    #[test]
    fn quantifier_counts_are_preserved() {
        let exists = Expr::Exists {
            var: "z".to_string(),
            var_type: "e".to_string(),
            body: Box::new(variable("x", "e")),
            count: Some("3".to_string()),
        };
        let many = Expr::ExistsMany {
            var: "z".to_string(),
            var_type: "e".to_string(),
            count: "many".to_string(),
            body: Box::new(variable("x", "e")),
        };

        let exists_result = substitute_var_with_entity(&exists, "x", "sue");
        let many_result = substitute_var_with_entity(&many, "x", "sue");

        assert!(matches!(
            exists_result,
            Expr::Exists {
                count: Some(ref count),
                ..
            } if count == "3"
        ));
        assert!(matches!(
            many_result,
            Expr::ExistsMany {
                ref count,
                ..
            } if count == "many"
        ));
    }
}
