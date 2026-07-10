// In check_step dispatch, add:
"and_elim" => check_and_elim(&formula, step, prior),

// Add the flexible rule:
fn check_and_elim(formula: &Expr, step: &ProofStep, prior: &[Expr]) -> Result<Expr, String> {
    let source_idx = step.from.get(0).copied()
        .ok_or("and_elim requires a source step")?;
    let source = prior.get(source_idx.saturating_sub(1))
        .ok_or(format!("step {} not yet derived", source_idx))?;

    match source {
        Expr::And(left, right) => {
            if expr_eq(formula, left) {
                Ok(formula.clone())
            } else if expr_eq(formula, right) {
                Ok(formula.clone())
            } else {
                Err(format!("and_elim: formula {} matches neither {} nor {}", formula, left, right))
            }
        }
        _ => Err(format!("and_elim source must be And, got: {}", source)),
    }
}