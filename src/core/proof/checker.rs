use crate::core::logic::Expr;
use crate::core::proof::{ProofFile, ProofResult, ProofStep, StepResult};
use crate::core::semantics;
use crate::core::substitution::{
    substitute_formula_placeholder as shared_substitute_formula_placeholder,
    substitute_var_with_entity as shared_substitute_var_with_entity,
};

#[derive(Default)]
struct PriorSteps {
    formulas: Vec<Option<Expr>>,
}

impl PriorSteps {
    fn push(&mut self, formula: Option<Expr>) {
        self.formulas.push(formula);
    }

    fn get(&self, index: usize) -> Option<&Expr> {
        self.formulas.get(index).and_then(Option::as_ref)
    }

    fn iter(&self) -> impl Iterator<Item = &Expr> {
        self.formulas.iter().filter_map(Option::as_ref)
    }

    fn last(&self) -> Option<&Expr> {
        self.formulas.last().and_then(Option::as_ref)
    }
}

fn validate_proof_structure(steps: &[ProofStep]) -> Result<(), String> {
    for (index, step) in steps.iter().enumerate() {
        let expected = index + 1;
        if step.step != expected {
            return Err(format!(
                "proof steps must be sequential and 1-based: expected step {}, got {}",
                expected, step.step
            ));
        }

        for source in &step.from {
            if *source == 0 {
                return Err(format!("step {} references invalid step 0", step.step));
            }
            if *source >= step.step {
                return Err(format!(
                    "step {} references step {}, which is not an earlier step",
                    step.step, source
                ));
            }
        }
    }

    Ok(())
}

/// Check a proof file against the inference rules.
pub fn check_proof(file: &ProofFile) -> Result<ProofResult, String> {
    validate_proof_structure(&file.proof)?;

    let mut checked_steps: Vec<(ProofStep, StepResult)> = Vec::new();
    let mut formulas = PriorSteps::default();

    for step in &file.proof {
        let result = match check_step(step, &file.premises, &formulas) {
            Ok(expr) => {
                formulas.push(Some(expr));
                StepResult::Ok
            }
            Err(error) => {
                formulas.push(None);
                StepResult::Err(error)
            }
        };
        checked_steps.push((step.clone(), result));
    }

    let conclusion = parse_formula(&file.conclusion)?;
    let all_steps_valid = checked_steps
        .iter()
        .all(|(_, status)| matches!(status, StepResult::Ok));
    let conclusion_derived = formulas.iter().any(|expr| expr_eq(expr, &conclusion));
    let final_step_is_conclusion = formulas
        .last()
        .map(|expr| expr_eq(expr, &conclusion))
        .unwrap_or(false);
    let proof_valid = all_steps_valid && final_step_is_conclusion;

    Ok(ProofResult {
        title: file.title.clone(),
        conclusion: file.conclusion.clone(),
        steps: checked_steps,
        all_steps_valid,
        conclusion_derived,
        final_step_is_conclusion,
        proof_valid,
    })
}

fn check_step(step: &ProofStep, premises: &[String], prior: &PriorSteps) -> Result<Expr, String> {
    let formula = parse_formula(&step.formula)?;

    match step.justification.as_str() {
        "premise" => check_premise(&formula, premises),
        "universal_elim" => check_universal_elim(&formula, step, prior),
        "modus_ponens" => check_modus_ponens(&formula, step, prior),
        "existential_intro" => check_existential_intro(&formula, step, prior),
        "and_intro" => check_and_intro(&formula, step, prior),
        "and_elim" => check_and_elim(&formula, step, prior),
        "and_elim_l" => check_and_elim_l(&formula, step, prior),
        "and_elim_r" => check_and_elim_r(&formula, step, prior),
        "belief_elim" => check_belief_elim(&formula, step, prior),
        "of_elim" => check_of_elim(&formula, step, prior),
        "apply_elim" => check_apply_elim(&formula, step, prior),
        "exists_many_weaken" => check_exists_many_weaken(&formula, step, prior),
        "quantifier_weaken" => check_quantifier_weaken(&formula, step, prior),
        other => Err(format!("unknown justification: {}", other)),
    }
}

fn parse_formula(s: &str) -> Result<Expr, String> {
    semantics::parse(s)
}

fn check_premise(formula: &Expr, premises: &[String]) -> Result<Expr, String> {
    for p in premises {
        let parsed = parse_formula(p)?;
        if expr_eq(formula, &parsed) {
            return Ok(formula.clone());
        }
    }
    Err(format!("formula not found in premises: {}", formula))
}

fn check_universal_elim(
    formula: &Expr,
    step: &ProofStep,
    prior: &PriorSteps,
) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("universal_elim requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var, var_type, body) = match source {
        Expr::ForAll {
            var,
            var_type,
            body,
            ..
        } => (var, var_type, body),
        _ => {
            return Err(format!(
                "universal_elim source must be ForAll, got: {}",
                source
            ))
        }
    };

    let subst = step
        .substitution
        .as_ref()
        .ok_or("universal_elim requires substitution")?;
    let term = subst
        .get(var)
        .ok_or(format!("substitution must map variable '{}'", var))?;

    let substituted = if var_type == "s" {
        let parsed = parse_formula(term)?;
        shared_substitute_formula_placeholder(body, var, &parsed)
    } else {
        shared_substitute_var_with_entity(body, var, term)
    };

    if expr_eq(formula, &substituted) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "universal_elim: expected {}, got {}",
            substituted, formula
        ))
    }
}

fn check_modus_ponens(
    formula: &Expr,
    step: &ProofStep,
    prior: &PriorSteps,
) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("modus_ponens requires exactly 2 source steps".into());
    }
    let a = prior
        .get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let implication = prior
        .get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let (ante, cons) = match implication {
        Expr::Implies { ante, cons } => (ante, cons),
        _ => {
            return Err(format!(
                "modus_ponens: second source must be implication, got: {}",
                implication
            ))
        }
    };

    if !expr_eq(a, ante) {
        return Err(format!(
            "modus_ponens: antecedent mismatch. expected: {}, got: {}",
            ante, a
        ));
    }

    if expr_eq(formula, cons) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "modus_ponens: expected conclusion {}, got {}",
            cons, formula
        ))
    }
}

fn check_existential_intro(
    formula: &Expr,
    step: &ProofStep,
    prior: &PriorSteps,
) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("existential_intro requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var, body) = match formula {
        Expr::Exists { var, body, .. } => (var, body),
        _ => {
            return Err(format!(
                "existential_intro target must be Exists, got: {}",
                formula
            ))
        }
    };

    let subst = step
        .substitution
        .as_ref()
        .ok_or("existential_intro requires substitution")?;
    let term = subst
        .get(var)
        .ok_or(format!("substitution must map variable '{}'", var))?;

    let substituted =
        shared_substitute_var_with_entity(body, var, term);
    if expr_eq(source, &substituted) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "existential_intro: source {} does not match {} with {}->{}",
            source, substituted, var, term
        ))
    }
}

fn check_and_intro(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("and_intro requires exactly 2 source steps".into());
    }
    let a = prior
        .get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let b = prior
        .get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let expected = Expr::And(Box::new(a.clone()), Box::new(b.clone()));
    if expr_eq(formula, &expected) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "and_intro: expected {} ∧ {} = {}, got {}",
            a, b, expected, formula
        ))
    }
}

fn check_and_elim(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("and_elim requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(left, right) => {
            if expr_eq(formula, left) {
                Ok(formula.clone())
            } else if expr_eq(formula, right) {
                Ok(formula.clone())
            } else {
                Err(format!(
                    "and_elim: formula {} matches neither {} nor {}",
                    formula, left, right
                ))
            }
        }
        _ => Err(format!("and_elim source must be And, got: {}", source)),
    }
}

fn check_and_elim_l(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("and_elim_l requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(left, _) => {
            if expr_eq(formula, left) {
                Ok(formula.clone())
            } else {
                Err(format!(
                    "and_elim_l: expected left conjunct {}, got {}",
                    left, formula
                ))
            }
        }
        _ => Err(format!("and_elim_l source must be And, got: {}", source)),
    }
}

fn check_and_elim_r(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("and_elim_r requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(_, right) => {
            if expr_eq(formula, right) {
                Ok(formula.clone())
            } else {
                Err(format!(
                    "and_elim_r: expected right conjunct {}, got {}",
                    right, formula
                ))
            }
        }
        _ => Err(format!("and_elim_r source must be And, got: {}", source)),
    }
}

fn check_belief_elim(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("belief_elim requires exactly 2 source steps".into());
    }
    let say_form = prior
        .get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let believable_form = prior
        .get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let (agent, content) = match say_form {
        Expr::Pred { name, roles } if name == "say" => {
            let agent = roles
                .iter()
                .find(|(r, _)| r == "agent")
                .ok_or("belief_elim: say predicate missing agent role")?
                .1
                .clone();
            let content = roles
                .iter()
                .find(|(r, _)| r == "content")
                .ok_or("belief_elim: say predicate missing content role")?
                .1
                .clone();
            (agent, content)
        }
        _ => {
            return Err(format!(
                "belief_elim: first source must be say(...), got: {}",
                say_form
            ))
        }
    };

    let theme = match believable_form {
        Expr::Pred { name, roles } if name == "believable" => roles
            .iter()
            .find(|(r, _)| r == "theme")
            .ok_or("belief_elim: believable predicate missing theme role")?
            .1
            .clone(),
        _ => {
            return Err(format!(
                "belief_elim: second source must be believable(...), got: {}",
                believable_form
            ))
        }
    };

    if !expr_eq(&agent, &theme) {
        return Err(format!(
            "belief_elim: agent {} does not match believable theme {}",
            agent, theme
        ));
    }

    if expr_eq(formula, &content) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "belief_elim: expected content {}, got {}",
            content, formula
        ))
    }
}

fn check_of_elim(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("of_elim requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (name, roles) = match source {
        Expr::Pred { name, roles } => (name, roles),
        _ => {
            return Err(format!(
                "of_elim source must be a predicate, got: {}",
                source
            ))
        }
    };

    if !name.ends_with("_of") {
        return Err(format!(
            "of_elim: predicate name must end with '_of', got: {}",
            name
        ));
    }
    let base_name = name.strip_suffix("_of").unwrap();

    let theme = roles
        .iter()
        .find(|(r, _)| r == "theme")
        .ok_or("of_elim: predicate missing theme role")?
        .1
        .clone();
    let location = roles
        .iter()
        .find(|(r, _)| r == "location")
        .ok_or("of_elim: predicate missing location role")?
        .1
        .clone();

    let expected = Expr::And(
        Box::new(Expr::Pred {
            name: base_name.to_string(),
            roles: vec![("theme".to_string(), theme.clone())],
        }),
        Box::new(Expr::Pred {
            name: "in".to_string(),
            roles: vec![
                ("theme".to_string(), theme),
                ("location".to_string(), location),
            ],
        }),
    );

    if expr_eq(formula, &expected) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "of_elim: expected {} ∧ in(...), got {}",
            base_name, formula
        ))
    }
}

fn check_apply_elim(formula: &Expr, step: &ProofStep, prior: &PriorSteps) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("apply_elim requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (pred_name, entity) = match source {
        Expr::Pred { name, roles } if name == "apply" => {
            let pred_expr = roles
                .iter()
                .find(|(r, _)| r == "predicate")
                .ok_or("apply_elim: apply missing predicate role")?
                .1
                .clone();
            let ent_expr = roles
                .iter()
                .find(|(r, _)| r == "entity")
                .ok_or("apply_elim: apply missing entity role")?
                .1
                .clone();
            let pred_name = match pred_expr {
                Expr::Entity(s) => s,
                other => {
                    return Err(format!(
                        "apply_elim: predicate must be an entity name, got: {}",
                        other
                    ))
                }
            };
            (pred_name, ent_expr)
        }
        _ => {
            return Err(format!(
                "apply_elim source must be apply(...), got: {}",
                source
            ))
        }
    };

    let expected = Expr::Pred {
        name: pred_name,
        roles: vec![("theme".to_string(), entity)],
    };

    if expr_eq(formula, &expected) {
        Ok(formula.clone())
    } else {
        Err(format!(
            "apply_elim: expected {}, got {}",
            expected, formula
        ))
    }
}

fn check_exists_many_weaken(
    formula: &Expr,
    step: &ProofStep,
    prior: &PriorSteps,
) -> Result<Expr, String> {
    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("exists_many_weaken requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var1, var_type1, count1, body1) = match source {
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => (var, var_type, count, body),
        _ => {
            return Err(format!(
                "exists_many_weaken source must be ExistsMany, got: {}",
                source
            ))
        }
    };

    let (var2, var_type2, count2, body2) = match formula {
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => (var, var_type, count, body),
        _ => {
            return Err(format!(
                "exists_many_weaken target must be ExistsMany, got: {}",
                formula
            ))
        }
    };

    if var1 != var2 {
        return Err(format!(
            "exists_many_weaken: variable mismatch: {} vs {}",
            var1, var2
        ));
    }
    if var_type1 != var_type2 {
        return Err(format!(
            "exists_many_weaken: type mismatch: {} vs {}",
            var_type1, var_type2
        ));
    }
    if !expr_eq(body1, body2) {
        return Err(format!("exists_many_weaken: body mismatch"));
    }

    let n1: i32 = count1.parse().map_err(|_| {
        format!(
            "exists_many_weaken: source count '{}' is not a number",
            count1
        )
    })?;
    let n2: i32 = count2.parse().map_err(|_| {
        format!(
            "exists_many_weaken: target count '{}' is not a number",
            count2
        )
    })?;

    if n2 >= n1 {
        return Err(format!(
            "exists_many_weaken: target count {} must be less than source count {}",
            n2, n1
        ));
    }

    Ok(formula.clone())
}

fn check_quantifier_weaken(
    formula: &Expr,
    step: &ProofStep,
    prior: &PriorSteps,
) -> Result<Expr, String> {
    let hierarchy = ["most", "many", "several", "some", "few"];

    let source_idx = step
        .from
        .get(0)
        .copied()
        .ok_or("quantifier_weaken requires a source step")?;
    let source = prior
        .get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var1, var_type1, count1, body1) = match source {
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => (var, var_type, count, body),
        _ => {
            return Err(format!(
                "quantifier_weaken source must be ExistsMany with a vague quantifier, got: {}",
                source
            ))
        }
    };

    let (var2, var_type2, count2, body2) = match formula {
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => (var, var_type, count, body),
        _ => {
            return Err(format!(
                "quantifier_weaken target must be ExistsMany with a vague quantifier, got: {}",
                formula
            ))
        }
    };

    if var1 != var2 || var_type1 != var_type2 || !expr_eq(body1, body2) {
        return Err("quantifier_weaken: variable, type, or body mismatch".into());
    }

    let pos1 = hierarchy
        .iter()
        .position(|&q| q == count1.as_str())
        .ok_or(format!(
            "quantifier_weaken: source count '{}' not in hierarchy",
            count1
        ))?;
    let pos2 = hierarchy
        .iter()
        .position(|&q| q == count2.as_str())
        .ok_or(format!(
            "quantifier_weaken: target count '{}' not in hierarchy",
            count2
        ))?;

    if pos2 <= pos1 {
        return Err(format!(
            "quantifier_weaken: target '{}' must be weaker than source '{}'",
            count2, count1
        ));
    }

    Ok(formula.clone())
}

/// Substitute all occurrences of `Var { name: var }` or `Entity(var)` with `Entity(term)`.
fn substitute_var_to_entity(expr: Expr, var: &str, term: &str) -> Expr {
    match expr {
        Expr::Var { name, .. } if name == var => Expr::Entity(term.to_string()),
        Expr::Entity(name) if name == var => Expr::Entity(term.to_string()),
        Expr::Var { name, typ } => Expr::Var { name, typ },
        Expr::Entity(s) => Expr::Entity(s),
        Expr::Pred { name, roles } => Expr::Pred {
            name,
            roles: roles
                .into_iter()
                .map(|(r, e)| (r, substitute_var_to_entity(e, var, term)))
                .collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(substitute_var_to_entity(*e, var, term))),
        Expr::And(l, r) => Expr::And(
            Box::new(substitute_var_to_entity(*l, var, term)),
            Box::new(substitute_var_to_entity(*r, var, term)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(substitute_var_to_entity(*ante, var, term)),
            cons: Box::new(substitute_var_to_entity(*cons, var, term)),
        },
        Expr::ForAll {
            var: v,
            var_type,
            body,
        } => Expr::ForAll {
            var: v,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::The {
            var: v,
            var_type,
            body,
        } => Expr::The {
            var: v,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::This {
            var: v,
            var_type,
            body,
        } => Expr::This {
            var: v,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::That {
            var: v,
            var_type,
            body,
        } => Expr::That {
            var: v,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::Exists {
            var: v,
            var_type,
            body,
            count,
        } => Expr::Exists {
            var: v,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
            count,
        },
        Expr::ExistsMany {
            var: v,
            var_type,
            count,
            body,
        } => Expr::ExistsMany {
            var: v,
            var_type,
            count,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::Question { label, body } => Expr::Question {
            label,
            body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
    }
}

/// Substitute all occurrences of `Var { name: var }` or `Entity(var)` with `replacement` Expr.
/// Used for s-typed (sentence-level) universal elimination.
fn substitute_var(expr: Expr, var: &str, replacement: Expr) -> Expr {
    match expr {
        Expr::Var { name, .. } if name == var => replacement.clone(),
        Expr::Entity(name) if name == var => replacement,
        Expr::Var { name, typ } => Expr::Var { name, typ },
        Expr::Entity(s) => Expr::Entity(s),
        Expr::Pred { name, roles } => Expr::Pred {
            name,
            roles: roles
                .into_iter()
                .map(|(r, e)| (r, substitute_var(e, var, replacement.clone())))
                .collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(substitute_var(*e, var, replacement))),
        Expr::And(l, r) => Expr::And(
            Box::new(substitute_var(*l, var, replacement.clone())),
            Box::new(substitute_var(*r, var, replacement)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(substitute_var(*ante, var, replacement.clone())),
            cons: Box::new(substitute_var(*cons, var, replacement)),
        },
        Expr::ForAll {
            var: v,
            var_type,
            body,
        } => Expr::ForAll {
            var: v,
            var_type,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
        Expr::The {
            var: v,
            var_type,
            body,
        } => Expr::The {
            var: v,
            var_type,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
        Expr::This {
            var: v,
            var_type,
            body,
        } => Expr::This {
            var: v,
            var_type,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
        Expr::That {
            var: v,
            var_type,
            body,
        } => Expr::That {
            var: v,
            var_type,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
        Expr::Exists {
            var: v,
            var_type,
            body,
            count,
        } => Expr::Exists {
            var: v,
            var_type,
            body: Box::new(substitute_var(*body, var, replacement)),
            count,
        },
        Expr::ExistsMany {
            var: v,
            var_type,
            count,
            body,
        } => Expr::ExistsMany {
            var: v,
            var_type,
            count,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
        Expr::Question { label, body } => Expr::Question {
            label,
            body: Box::new(substitute_var(*body, var, replacement)),
        },
    }
}

/// Structural equality on Expr.
fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (
            Expr::Pred {
                name: n1,
                roles: r1,
            },
            Expr::Pred {
                name: n2,
                roles: r2,
            },
        ) => {
            n1 == n2
                && r1.len() == r2.len()
                && r1
                    .iter()
                    .zip(r2.iter())
                    .all(|((r1, e1), (r2, e2))| r1 == r2 && expr_eq(e1, e2))
        }
        (Expr::Not(e1), Expr::Not(e2)) => expr_eq(e1, e2),
        (Expr::And(l1, r1), Expr::And(l2, r2)) => expr_eq(l1, l2) && expr_eq(r1, r2),
        (Expr::Implies { ante: a1, cons: c1 }, Expr::Implies { ante: a2, cons: c2 }) => {
            expr_eq(a1, a2) && expr_eq(c1, c2)
        }
        (
            Expr::ForAll {
                var: v1,
                var_type: t1,
                body: b1,
            },
            Expr::ForAll {
                var: v2,
                var_type: t2,
                body: b2,
            },
        ) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (
            Expr::The {
                var: v1,
                var_type: t1,
                body: b1,
            },
            Expr::The {
                var: v2,
                var_type: t2,
                body: b2,
            },
        ) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (
            Expr::This {
                var: v1,
                var_type: t1,
                body: b1,
            },
            Expr::This {
                var: v2,
                var_type: t2,
                body: b2,
            },
        ) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (
            Expr::That {
                var: v1,
                var_type: t1,
                body: b1,
            },
            Expr::That {
                var: v2,
                var_type: t2,
                body: b2,
            },
        ) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (
            Expr::Exists {
                var: v1,
                var_type: t1,
                body: b1,
                count: c1,
            },
            Expr::Exists {
                var: v2,
                var_type: t2,
                body: b2,
                count: c2,
            },
        ) => v1 == v2 && t1 == t2 && expr_eq(b1, b2) && c1 == c2,
        (
            Expr::ExistsMany {
                var: v1,
                var_type: t1,
                count: c1,
                body: b1,
            },
            Expr::ExistsMany {
                var: v2,
                var_type: t2,
                count: c2,
                body: b2,
            },
        ) => v1 == v2 && t1 == t2 && c1 == c2 && expr_eq(b1, b2),
        (Expr::Var { name: n1, .. }, Expr::Var { name: n2, .. }) => n1 == n2,
        (Expr::Entity(s1), Expr::Entity(s2)) => s1 == s2,
        (
            Expr::Question {
                label: l1,
                body: b1,
            },
            Expr::Question {
                label: l2,
                body: b2,
            },
        ) => l1 == l2 && expr_eq(b1, b2),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof_step(step: usize, formula: &str, justification: &str, from: Vec<usize>) -> ProofStep {
        ProofStep {
            step,
            formula: formula.to_string(),
            justification: justification.to_string(),
            from,
            substitution: None,
        }
    }

    fn prior_steps(expressions: Vec<Expr>) -> PriorSteps {
        let mut prior = PriorSteps::default();

        for expression in expressions {
            prior.push(Some(expression));
        }

        prior
    }

    #[test]
    fn failed_step_does_not_shift_later_step_identity() {
        let file = ProofFile {
            title: "failed identity".to_string(),
            premises: vec![
                "man(theme: socrates)".to_string(),
                "mortal(theme: socrates)".to_string(),
            ],
            conclusion: "man(theme: socrates) ∧ mortal(theme: socrates)".to_string(),
            proof: vec![
                proof_step(1, "man(theme: socrates)", "premise", vec![]),
                proof_step(2, "happy(theme: socrates)", "premise", vec![]),
                proof_step(3, "mortal(theme: socrates)", "premise", vec![]),
                proof_step(
                    4,
                    "man(theme: socrates) ∧ mortal(theme: socrates)",
                    "and_intro",
                    vec![1, 2],
                ),
            ],
        };

        let result = check_proof(&file).unwrap();

        assert!(matches!(result.steps[1].1, StepResult::Err(_)));
        assert!(matches!(result.steps[3].1, StepResult::Err(_)));
        assert!(!result.all_steps_valid);
        assert!(!result.proof_valid);
    }

    #[test]
    fn proof_structure_requires_sequential_step_ids() {
        let file = ProofFile {
            title: "bad numbering".to_string(),
            premises: vec!["man(theme: socrates)".to_string()],
            conclusion: "man(theme: socrates)".to_string(),
            proof: vec![proof_step(2, "man(theme: socrates)", "premise", vec![])],
        };

        let error = check_proof(&file).unwrap_err();
        assert!(error.contains("expected step 1, got 2"));
    }

    #[test]
    fn conclusion_can_be_derived_without_a_valid_proof() {
        let file = ProofFile {
            title: "derived but invalid".to_string(),
            premises: vec!["man(theme: socrates)".to_string()],
            conclusion: "man(theme: socrates)".to_string(),
            proof: vec![
                proof_step(1, "man(theme: socrates)", "premise", vec![]),
                proof_step(2, "mortal(theme: socrates)", "premise", vec![]),
            ],
        };

        let result = check_proof(&file).unwrap();

        assert!(result.conclusion_derived);
        assert!(!result.final_step_is_conclusion);
        assert!(!result.all_steps_valid);
        assert!(!result.proof_valid);
    }

    #[test]
    fn valid_proof_sets_all_validity_flags() {
        let file = ProofFile {
            title: "valid".to_string(),
            premises: vec!["man(theme: socrates)".to_string()],
            conclusion: "man(theme: socrates)".to_string(),
            proof: vec![proof_step(1, "man(theme: socrates)", "premise", vec![])],
        };

        let result = check_proof(&file).unwrap();

        assert!(result.all_steps_valid);
        assert!(result.conclusion_derived);
        assert!(result.final_step_is_conclusion);
        assert!(result.proof_valid);
    }

    #[test]
    fn substitute_var_replaces_entity_with_expr() {
        let body = Expr::Pred {
            name: "certain".to_string(),
            roles: vec![("content".to_string(), Expr::Entity("P".to_string()))],
        };
        let replacement = Expr::Pred {
            name: "happy".to_string(),
            roles: vec![("theme".to_string(), Expr::Entity("john".to_string()))],
        };
        let result = substitute_var(body, "P", replacement);
        match result {
            Expr::Pred { name, roles } => {
                assert_eq!(name, "certain");
                assert_eq!(roles.len(), 1);
                match &roles[0].1 {
                    Expr::Pred { name, .. } => assert_eq!(name, "happy"),
                    other => panic!("expected Pred, got {:?}", other),
                }
            }
            other => panic!("expected Pred, got {:?}", other),
        }
    }

    #[test]
    fn substitute_var_replaces_in_not() {
        let body = Expr::Not(Box::new(Expr::Entity("P".to_string())));
        let replacement = Expr::Pred {
            name: "happy".to_string(),
            roles: vec![("theme".to_string(), Expr::Entity("john".to_string()))],
        };
        let result = substitute_var(body, "P", replacement);
        match result {
            Expr::Not(inner) => match *inner {
                Expr::Pred { name, .. } => assert_eq!(name, "happy"),
                other => panic!("expected Pred inside Not, got {:?}", other),
            },
            other => panic!("expected Not, got {:?}", other),
        }
    }

    #[test]
    fn exists_many_weaken_3_to_2() {
        let source = semantics::parse(
            "exists_many [x:e, 3]: man(theme: x) ∧ in(theme: x, location: the_house)",
        )
        .unwrap();
        let target = semantics::parse(
            "exists_many [x:e, 2]: man(theme: x) ∧ in(theme: x, location: the_house)",
        )
        .unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, 2]: man(theme: x) ∧ in(theme: x, location: the_house)"
                .to_string(),
            justification: "exists_many_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_exists_many_weaken(&target, &step, &prior);
        assert!(result.is_ok());
    }

    #[test]
    fn exists_many_weaken_rejects_equal() {
        let source = semantics::parse("exists_many [x:e, 3]: man(theme: x)").unwrap();
        let target = semantics::parse("exists_many [x:e, 3]: man(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, 3]: man(theme: x)".to_string(),
            justification: "exists_many_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_exists_many_weaken(&target, &step, &prior);
        assert!(result.is_err());
    }

    #[test]
    fn exists_many_weaken_rejects_greater() {
        let source = semantics::parse("exists_many [x:e, 2]: man(theme: x)").unwrap();
        let target = semantics::parse("exists_many [x:e, 3]: man(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, 3]: man(theme: x)".to_string(),
            justification: "exists_many_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_exists_many_weaken(&target, &step, &prior);
        assert!(result.is_err());
    }

    #[test]
    fn quantifier_weaken_most_to_many() {
        let source =
            semantics::parse("exists_many [x:e, most]: man(theme: x) ∧ mortal(theme: x)").unwrap();
        let target =
            semantics::parse("exists_many [x:e, many]: man(theme: x) ∧ mortal(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, many]: man(theme: x) ∧ mortal(theme: x)".to_string(),
            justification: "quantifier_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_quantifier_weaken(&target, &step, &prior);
        assert!(result.is_ok());
    }

    #[test]
    fn quantifier_weaken_most_to_some() {
        let source = semantics::parse("exists_many [x:e, most]: happy(theme: x)").unwrap();
        let target = semantics::parse("exists_many [x:e, some]: happy(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, some]: happy(theme: x)".to_string(),
            justification: "quantifier_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_quantifier_weaken(&target, &step, &prior);
        assert!(result.is_ok());
    }

    #[test]
    fn quantifier_weaken_rejects_same() {
        let source = semantics::parse("exists_many [x:e, many]: happy(theme: x)").unwrap();
        let target = semantics::parse("exists_many [x:e, many]: happy(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, many]: happy(theme: x)".to_string(),
            justification: "quantifier_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_quantifier_weaken(&target, &step, &prior);
        assert!(result.is_err());
    }

    #[test]
    fn quantifier_weaken_rejects_stronger() {
        let source = semantics::parse("exists_many [x:e, some]: happy(theme: x)").unwrap();
        let target = semantics::parse("exists_many [x:e, many]: happy(theme: x)").unwrap();
        let prior = prior_steps(vec![source]);
        let step = ProofStep {
            step: 2,
            formula: "exists_many [x:e, many]: happy(theme: x)".to_string(),
            justification: "quantifier_weaken".to_string(),
            from: vec![1],
            substitution: None,
        };
        let result = check_quantifier_weaken(&target, &step, &prior);
        assert!(result.is_err());
    }
}
