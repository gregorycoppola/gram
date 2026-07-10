use crate::core::logic::Expr;
use crate::core::semantics;
use crate::core::proof::{ProofFile, ProofResult, ProofStep, StepResult};

/// Check a proof file against the inference rules.
pub fn check_proof(file: &ProofFile) -> Result<ProofResult, String> {
    let mut checked_steps: Vec<(ProofStep, StepResult)> = Vec::new();
    let mut formulas: Vec<Expr> = Vec::new();

    for step in &file.proof {
        let result = match check_step(step, &file.premises, &formulas) {
            Ok(expr) => {
                formulas.push(expr);
                StepResult::Ok
            }
            Err(e) => StepResult::Err(e),
        };
        checked_steps.push((step.clone(), result));
    }

    let conclusion = parse_formula(&file.conclusion)?;
    let conclusion_reached = formulas.last().map(|e| expr_eq(e, &conclusion)).unwrap_or(false);

    Ok(ProofResult {
        title: file.title.clone(),
        conclusion: file.conclusion.clone(),
        steps: checked_steps,
        conclusion_reached,
    })
}

fn check_step(step: &ProofStep, premises: &[String], prior: &[Expr]) -> Result<Expr, String> {
    let formula = parse_formula(&step.formula)?;

    match step.justification.as_str() {
        "premise" => check_premise(&formula, premises),
        "universal_elim" => check_universal_elim(&formula, step, prior),
        "modus_ponens" => check_modus_ponens(&formula, step, prior),
        "existential_intro" => check_existential_intro(&formula, step, prior),
        "and_intro" => check_and_intro(&formula, step, prior),
        "and_elim_l" => check_and_elim_l(&formula, step, prior),
        "and_elim_r" => check_and_elim_r(&formula, step, prior),
        "belief_elim" => check_belief_elim(&formula, step, prior),
        "of_elim" => check_of_elim(&formula, step, prior),
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

fn check_universal_elim(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("universal_elim requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var, body) = match source {
        Expr::ForAll { var, body, .. } => (var, body),
        _ => return Err(format!("universal_elim source must be ForAll, got: {}", source)),
    };

    let subst = step.substitution.as_ref()
        .ok_or("universal_elim requires substitution")?;
    let term = subst.get(var)
        .ok_or(format!("substitution must map variable '{}'", var))?;

    let substituted = substitute_var_to_entity(*body.clone(), var, term);
    if expr_eq(formula, &substituted) {
        Ok(formula.clone())
    } else {
        Err(format!("universal_elim: expected {}, got {}", substituted, formula))
    }
}

fn check_modus_ponens(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("modus_ponens requires exactly 2 source steps".into());
    }
    let a = prior.get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let implication = prior.get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let (ante, cons) = match implication {
        Expr::Implies { ante, cons } => (ante, cons),
        _ => return Err(format!("modus_ponens: second source must be implication, got: {}", implication)),
    };

    if !expr_eq(a, ante) {
        return Err(format!("modus_ponens: antecedent mismatch. expected: {}, got: {}", ante, a));
    }

    if expr_eq(formula, cons) {
        Ok(formula.clone())
    } else {
        Err(format!("modus_ponens: expected conclusion {}, got {}", cons, formula))
    }
}

fn check_existential_intro(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("existential_intro requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (var, body) = match formula {
        Expr::Exists { var, body, .. } => (var, body),
        _ => return Err(format!("existential_intro target must be Exists, got: {}", formula)),
    };

    let subst = step.substitution.as_ref()
        .ok_or("existential_intro requires substitution")?;
    let term = subst.get(var)
        .ok_or(format!("substitution must map variable '{}'", var))?;

    let substituted = substitute_var_to_entity(*body.clone(), var, term);
    if expr_eq(source, &substituted) {
        Ok(formula.clone())
    } else {
        Err(format!("existential_intro: source {} does not match {} with {}->{}", source, substituted, var, term))
    }
}

fn check_and_intro(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("and_intro requires exactly 2 source steps".into());
    }
    let a = prior.get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let b = prior.get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let expected = Expr::And(Box::new(a.clone()), Box::new(b.clone()));
    if expr_eq(formula, &expected) {
        Ok(formula.clone())
    } else {
        Err(format!("and_intro: expected {} ∧ {} = {}, got {}", a, b, expected, formula))
    }
}

fn check_and_elim_l(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("and_elim_l requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(left, _) => {
            if expr_eq(formula, left) {
                Ok(formula.clone())
            } else {
                Err(format!("and_elim_l: expected left conjunct {}, got {}", left, formula))
            }
        }
        _ => Err(format!("and_elim_l source must be And, got: {}", source)),
    }
}

fn check_and_elim_r(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("and_elim_r requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(_, right) => {
            if expr_eq(formula, right) {
                Ok(formula.clone())
            } else {
                Err(format!("and_elim_r: expected right conjunct {}, got {}", right, formula))
            }
        }
        _ => Err(format!("and_elim_r source must be And, got: {}", source)),
    }
}

fn check_belief_elim(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    if step.from.len() != 2 {
        return Err("belief_elim requires exactly 2 source steps".into());
    }
    let say_form = prior.get(step.from[0].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[0]))?;
    let believable_form = prior.get(step.from[1].saturating_sub(1))
        .ok_or(format!("step {} not yet derived", step.from[1]))?;

    let (agent, content) = match say_form {
        Expr::Pred { name, roles } if name == "say" => {
            let agent = roles.iter().find(|(r, _)| r == "agent")
                .ok_or("belief_elim: say predicate missing agent role")?.1.clone();
            let content = roles.iter().find(|(r, _)| r == "content")
                .ok_or("belief_elim: say predicate missing content role")?.1.clone();
            (agent, content)
        }
        _ => return Err(format!("belief_elim: first source must be say(...), got: {}", say_form)),
    };

    let theme = match believable_form {
        Expr::Pred { name, roles } if name == "believable" => {
            roles.iter().find(|(r, _)| r == "theme")
                .ok_or("belief_elim: believable predicate missing theme role")?.1.clone()
        }
        _ => return Err(format!("belief_elim: second source must be believable(...), got: {}", believable_form)),
    };

    if !expr_eq(&agent, &theme) {
        return Err(format!("belief_elim: agent {} does not match believable theme {}", agent, theme));
    }

    if expr_eq(formula, &content) {
        Ok(formula.clone())
    } else {
        Err(format!("belief_elim: expected content {}, got {}", content, formula))
    }
}

fn check_of_elim(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("of_elim requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    let (name, roles) = match source {
        Expr::Pred { name, roles } => (name, roles),
        _ => return Err(format!("of_elim source must be a predicate, got: {}", source)),
    };

    if !name.ends_with("_of") {
        return Err(format!("of_elim: predicate name must end with '_of', got: {}", name));
    }
    let base_name = name.strip_suffix("_of").unwrap();

    let theme = roles.iter().find(|(r, _)| r == "theme")
        .ok_or("of_elim: predicate missing theme role")?.1.clone();
    let location = roles.iter().find(|(r, _)| r == "location")
        .ok_or("of_elim: predicate missing location role")?.1.clone();

    let expected = Expr::And(
        Box::new(Expr::Pred {
            name: base_name.to_string(),
            roles: vec![("theme".to_string(), theme.clone())],
        }),
        Box::new(Expr::Pred {
            name: "in".to_string(),
            roles: vec![("theme".to_string(), theme), ("location".to_string(), location)],
        }),
    );

    if expr_eq(formula, &expected) {
        Ok(formula.clone())
    } else {
        Err(format!("of_elim: expected {} ∧ in(...), got {}", base_name, formula))
    }
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
            roles: roles.into_iter().map(|(r, e)| (r, substitute_var_to_entity(e, var, term))).collect(),
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
        Expr::ForAll { var: v, var_type, body } => Expr::ForAll {
            var: v, var_type, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::The { var: v, var_type, body } => Expr::The {
            var: v, var_type, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::This { var: v, var_type, body } => Expr::This {
            var: v, var_type, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::That { var: v, var_type, body } => Expr::That {
            var: v, var_type, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::Exists { var: v, var_type, body, count } => Expr::Exists {
            var: v, var_type, body: Box::new(substitute_var_to_entity(*body, var, term)), count,
        },
        Expr::ExistsMany { var: v, var_type, count, body } => Expr::ExistsMany {
            var: v, var_type, count, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
        Expr::Question { label, body } => Expr::Question {
            label, body: Box::new(substitute_var_to_entity(*body, var, term)),
        },
    }
}

/// Structural equality on Expr.
fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Pred { name: n1, roles: r1 }, Expr::Pred { name: n2, roles: r2 }) => {
            n1 == n2 && r1.len() == r2.len() && r1.iter().zip(r2.iter()).all(|((r1, e1), (r2, e2))| r1 == r2 && expr_eq(e1, e2))
        }
        (Expr::Not(e1), Expr::Not(e2)) => expr_eq(e1, e2),
        (Expr::And(l1, r1), Expr::And(l2, r2)) => expr_eq(l1, l2) && expr_eq(r1, r2),
        (Expr::Implies { ante: a1, cons: c1 }, Expr::Implies { ante: a2, cons: c2 }) => expr_eq(a1, a2) && expr_eq(c1, c2),
        (Expr::ForAll { var: v1, var_type: t1, body: b1 }, Expr::ForAll { var: v2, var_type: t2, body: b2 }) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (Expr::The { var: v1, var_type: t1, body: b1 }, Expr::The { var: v2, var_type: t2, body: b2 }) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (Expr::This { var: v1, var_type: t1, body: b1 }, Expr::This { var: v2, var_type: t2, body: b2 }) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (Expr::That { var: v1, var_type: t1, body: b1 }, Expr::That { var: v2, var_type: t2, body: b2 }) => v1 == v2 && t1 == t2 && expr_eq(b1, b2),
        (Expr::Exists { var: v1, var_type: t1, body: b1, count: c1 }, Expr::Exists { var: v2, var_type: t2, body: b2, count: c2 }) => v1 == v2 && t1 == t2 && expr_eq(b1, b2) && c1 == c2,
        (Expr::ExistsMany { var: v1, var_type: t1, count: c1, body: b1 }, Expr::ExistsMany { var: v2, var_type: t2, count: c2, body: b2 }) => v1 == v2 && t1 == t2 && c1 == c2 && expr_eq(b1, b2),
        (Expr::Var { name: n1, .. }, Expr::Var { name: n2, .. }) => n1 == n2,
        (Expr::Entity(s1), Expr::Entity(s2)) => s1 == s2,
        (Expr::Question { label: l1, body: b1 }, Expr::Question { label: l2, body: b2 }) => l1 == l2 && expr_eq(b1, b2),
        _ => false,
    }
}