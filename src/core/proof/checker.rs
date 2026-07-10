// Add to check_step dispatch:
"of_elim" => check_of_elim(&formula, step, prior),

// Add the rule:
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