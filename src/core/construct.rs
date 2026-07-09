
use crate::core::logic::Expr;
use crate::core::value::{DpQuant, GapProp, SemValue, VarGen};

/// Argument to a semantic constructor.
#[derive(Debug, Clone)]
pub enum Arg {
    /// A lexical binding from a $var slot: (canonical_name, type_string)
    Lexical(String, String),
    /// A sub-phrase result from a SUB: slot
    Sub(SemValue),
    /// A literal string argument (hardcoded in the constructor spec)
    Literal(String),
}

/// Apply a named semantic constructor to the given arguments.
pub fn apply_constructor(
    name: &str,
    args: &[Arg],
    var_gen: &mut VarGen,
) -> Result<SemValue, String> {
    match name {
        // DP constructors
        "the_dp" => construct_quant_dp(DpQuantKind::The, args, var_gen),
        "exists_dp" => construct_quant_dp(DpQuantKind::Exists, args, var_gen),
        "forall_dp" => construct_quant_dp(DpQuantKind::ForAll, args, var_gen),
        "bare_dp" => construct_bare_dp(args),

        // S constructors
        "s_copula" => construct_s_copula(args),
        "s_transitive" => construct_s_transitive(args),
        "s_complement" => construct_s_complement(args),

        // Gap clause constructors
        "s_gap_agent" => construct_s_gap("agent", "patient", args, var_gen),
        "s_gap_patient" => construct_s_gap("patient", "agent", args, var_gen),
        "s_gap_theme" => construct_s_gap_theme(args, var_gen),

        // Relative DP constructors
        "rel_dp_agent" => construct_rel_dp("agent", args, var_gen),
        "rel_dp_patient" => construct_rel_dp("patient", args, var_gen),
        "rel_dp_theme" => construct_rel_dp("theme", args, var_gen),

        _ => Err(format!("unknown constructor: {}", name)),
    }
}

/// Internal tag for the three quantifier flavors, used only during DP construction.
enum DpQuantKind {
    The,
    Exists,
    ForAll,
}

/// DP constructor for quantified determiners (the, a, every).
/// Args: [Lexical(pred_name, pred_type), Literal(role_name)]
fn construct_quant_dp(
    kind: DpQuantKind,
    args: &[Arg],
    var_gen: &mut VarGen,
) -> Result<SemValue, String> {
    if args.len() != 2 {
        return Err(format!("quant_dp expects 2 args, got {}", args.len()));
    }
    let pred_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("quant_dp: first arg must be a lexical binding".into()),
    };
    let role = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("quant_dp: second arg must be a literal role name".into()),
    };

    let var: String = var_gen.fresh();
    let var_type: String = "e".to_string();
    let restriction = Expr::Pred {
        name: pred_name,
        roles: vec![(role, Expr::Var {
            name: var.clone(),
            typ: var_type.clone(),
        })],
    };

    let quant = match kind {
        DpQuantKind::The => DpQuant::The { restriction },
        DpQuantKind::Exists => DpQuant::Exists { restriction },
        DpQuantKind::ForAll => DpQuant::ForAll { restriction },
    };

    Ok(SemValue::Dp { var, var_type, quant })
}

/// DP constructor for bare entities (no determiner).
/// Args: [Lexical(entity_name, entity_type)]
fn construct_bare_dp(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 1 {
        return Err(format!("bare_dp expects 1 arg, got {}", args.len()));
    }
    let (name, typ) = match &args[0] {
        Arg::Lexical(name, typ) => (name.clone(), typ.clone()),
        _ => return Err("bare_dp: arg must be a lexical binding".into()),
    };

    Ok(SemValue::Dp {
        var: name,
        var_type: typ,
        quant: DpQuant::Bare,
    })
}

/// S constructor for copular sentences: "the man is mortal"
/// Args: [Sub(Dp), Lexical(pred_name, pred_type), Literal(role_name)]
fn construct_s_copula(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 3 {
        return Err(format!("s_copula expects 3 args, got {}", args.len()));
    }
    let (var, var_type, quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_copula: first arg must be a DP".into()),
    };
    let pred_name = match &args[1] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_copula: second arg must be a lexical binding".into()),
    };
    let role = match &args[2] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_copula: third arg must be a literal role name".into()),
    };

    let body = Expr::Pred {
        name: pred_name,
        roles: vec![(role, Expr::Var {
            name: var.clone(),
            typ: var_type.clone(),
        })],
    };

    let expr = expand_quant(var, var_type, &quant, body)?;
    Ok(SemValue::Prop(expr))
}

/// S constructor for transitive sentences: "the man loves sue"
/// Args: [Sub(Dp_subject), Sub(Dp_object), Lexical(verb_name, verb_type),
///        Literal(agent_role), Literal(patient_role)]
fn construct_s_transitive(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 5 {
        return Err(format!("s_transitive expects 5 args, got {}", args.len()));
    }
    let (subj_var, subj_type, subj_quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_transitive: first arg must be a DP".into()),
    };
    let (obj_var, obj_type, obj_quant) = match &args[1] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_transitive: second arg must be a DP".into()),
    };
    let verb_name = match &args[2] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_transitive: third arg must be a lexical binding".into()),
    };
    let agent_role = match &args[3] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_transitive: fourth arg must be a literal role name".into()),
    };
    let patient_role = match &args[4] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_transitive: fifth arg must be a literal role name".into()),
    };

    let body = Expr::Pred {
        name: verb_name,
        roles: vec![
            (agent_role, Expr::Var { name: subj_var.clone(), typ: subj_type.clone() }),
            (patient_role, Expr::Var { name: obj_var.clone(), typ: obj_type.clone() }),
        ],
    };

    let dps: Vec<(String, String, DpQuant)> = vec![
        (subj_var, subj_type, subj_quant),
        (obj_var, obj_type, obj_quant),
    ];
    let expr = expand_quants(&dps, body)?;
    Ok(SemValue::Prop(expr))
}

/// S constructor for complement clauses: "John said that Sue is happy"
/// Args: [Sub(Dp_subject), Sub(Prop_complement), Lexical(verb_name, verb_type),
///        Literal(agent_role), Literal(theme_role)]
fn construct_s_complement(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 5 {
        return Err(format!("s_complement expects 5 args, got {}", args.len()));
    }
    let (subj_var, subj_type, subj_quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_complement: first arg must be a DP".into()),
    };
    let complement = match &args[1] {
        Arg::Sub(SemValue::Prop(expr)) => expr.clone(),
        _ => return Err("s_complement: second arg must be a Prop (embedded S)".into()),
    };
    let verb_name = match &args[2] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_complement: third arg must be a lexical binding".into()),
    };
    let agent_role = match &args[3] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_complement: fourth arg must be a literal role name".into()),
    };
    let theme_role = match &args[4] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_complement: fifth arg must be a literal role name".into()),
    };

    let body = Expr::Pred {
        name: verb_name,
        roles: vec![
            (agent_role, Expr::Var { name: subj_var.clone(), typ: subj_type.clone() }),
            (theme_role, complement),
        ],
    };

    let expr = expand_quant(subj_var, subj_type, &subj_quant, body)?;
    Ok(SemValue::Prop(expr))
}

// --- Gap clause constructors ---

/// Gap clause constructor for binary predicates (agent or patient gap).
/// Args: [Lexical(verb, type), Literal(gap_role), Literal(other_role), Sub(Dp)]
fn construct_s_gap(
    gap_role: &str,
    other_role: &str,
    args: &[Arg],
    var_gen: &mut VarGen,
) -> Result<SemValue, String> {
    if args.len() != 4 {
        return Err(format!("s_gap_{} expects 4 args, got {}", gap_role, args.len()));
    }
    let verb_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_gap: first arg must be a lexical binding".into()),
    };
    let role1 = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_gap: second arg must be a literal role name".into()),
    };
    let role2 = match &args[2] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_gap: third arg must be a literal role name".into()),
    };
    let (other_var, other_type) = match &args[3] {
        Arg::Sub(SemValue::Dp { var, var_type, .. }) => (var.clone(), var_type.clone()),
        _ => return Err("s_gap: fourth arg must be a DP".into()),
    };

    let gap_var = var_gen.fresh();

    let (gap_role_expr, other_role_expr) = if gap_role == role1 {
        (Expr::Var { name: gap_var.clone(), typ: "e".to_string() }, Expr::Var { name: other_var, typ: other_type })
    } else {
        (Expr::Var { name: other_var, typ: other_type }, Expr::Var { name: gap_var.clone(), typ: "e".to_string() })
    };

    let body = Expr::Pred {
        name: verb_name,
        roles: vec![(role1, gap_role_expr), (role2, other_role_expr)],
    };

    Ok(SemValue::GapProp(GapProp {
        var: gap_var,
        var_type: "e".to_string(),
        gap_role: gap_role.to_string(),
        body,
    }))
}

/// Gap clause constructor for copular (theme gap).
/// Args: [Lexical(pred, type), Literal("theme")]
fn construct_s_gap_theme(args: &[Arg], var_gen: &mut VarGen) -> Result<SemValue, String> {
    if args.len() != 2 {
        return Err(format!("s_gap_theme expects 2 args, got {}", args.len()));
    }
    let pred_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_gap_theme: first arg must be a lexical binding".into()),
    };
    let role = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_gap_theme: second arg must be a literal role name".into()),
    };

    let gap_var = var_gen.fresh();

    let body = Expr::Pred {
        name: pred_name,
        roles: vec![(role, Expr::Var { name: gap_var, typ: "e".to_string() })],
    };

    Ok(SemValue::GapProp(GapProp {
        var: gap_var,
        var_type: "e".to_string(),
        gap_role: role,
        body,
    }))
}

// --- Relative DP constructors ---

/// Relative DP constructor.
/// Args: [Lexical(head_pred, head_type), Literal(head_role), Sub(GapProp)]
/// Substitutes gap_var → head_var, conjoins head restriction with gap body.
fn construct_rel_dp(
    gap_role: &str,
    args: &[Arg],
    var_gen: &mut VarGen,
) -> Result<SemValue, String> {
    if args.len() != 3 {
        return Err(format!("rel_dp_{} expects 3 args, got {}", gap_role, args.len()));
    }
    let head_pred = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("rel_dp: first arg must be a lexical binding".into()),
    };
    let head_role = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("rel_dp: second arg must be a literal role name".into()),
    };
    let (gap_var, gap_body) = match &args[2] {
        Arg::Sub(SemValue::GapProp(GapProp { var, body, .. })) => (var.clone(), body.clone()),
        _ => return Err("rel_dp: third arg must be a GapProp".into()),
    };

    let head_var = var_gen.fresh();
    let head_var_type = "e".to_string();

    let substituted_body = substitute_var_name(gap_body, &gap_var, &head_var);

    let head_restriction = Expr::Pred {
        name: head_pred,
        roles: vec![(head_role, Expr::Var { name: head_var.clone(), typ: head_var_type.clone() })],
    };

    let restriction = Expr::And(
        Box::new(head_restriction),
        Box::new(substituted_body),
    );

    Ok(SemValue::Dp {
        var: head_var,
        var_type: head_var_type,
        quant: DpQuant::The { restriction },
    })
}

// --- Helpers ---

/// Expand a single quantifier around a body.
fn expand_quant(
    var: String,
    var_type: String,
    quant: &DpQuant,
    body: Expr,
) -> Result<Expr, String> {
    match quant {
        DpQuant::ForAll { restriction } => Ok(Expr::ForAll {
            var,
            var_type,
            body: Box::new(Expr::Implies {
                ante: Box::new(restriction.clone()),
                cons: Box::new(body),
            }),
        }),
        DpQuant::Exists { restriction } => Ok(Expr::Exists {
            var,
            var_type,
            body: Box::new(Expr::And(
                Box::new(restriction.clone()),
                Box::new(body),
            )),
            count: None,
        }),
        DpQuant::The { restriction } => Ok(Expr::The {
            var,
            var_type,
            body: Box::new(Expr::Implies {
                ante: Box::new(restriction.clone()),
                cons: Box::new(body),
            }),
        }),
        DpQuant::Bare => {
            Ok(substitute_var_to_entity(body, &var))
        }
    }
}

/// Expand multiple quantifiers around a body, outermost first (surface order).
fn expand_quants(
    dps: &[(String, String, DpQuant)],
    body: Expr,
) -> Result<Expr, String> {
    let mut result = body;
    for (var, var_type, quant) in dps.iter().rev() {
        result = expand_quant(var.clone(), var_type.clone(), quant, result)?;
    }
    Ok(result)
}

/// Replace Var { name: target, .. } with Entity(target) throughout an Expr.
/// Used for bare entity DPs where the variable is really a constant.
fn substitute_var_to_entity(expr: Expr, target: &str) -> Expr {
    match expr {
        Expr::Var { name, .. } if name == target => Expr::Entity(target.to_string()),
        Expr::Var { name, typ } => Expr::Var { name, typ },
        Expr::Pred { name, roles } => Expr::Pred {
            name,
            roles: roles
                .into_iter()
                .map(|(r, e)| (r, substitute_var_to_entity(e, target)))
                .collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(substitute_var_to_entity(*e, target))),
        Expr::And(l, r) => Expr::And(
            Box::new(substitute_var_to_entity(*l, target)),
            Box::new(substitute_var_to_entity(*r, target)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(substitute_var_to_entity(*ante, target)),
            cons: Box::new(substitute_var_to_entity(*cons, target)),
        },
        Expr::ForAll { var, var_type, body } => Expr::ForAll {
            var, var_type, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::The { var, var_type, body } => Expr::The {
            var, var_type, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::This { var, var_type, body } => Expr::This {
            var, var_type, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::That { var, var_type, body } => Expr::That {
            var, var_type, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::Exists { var, var_type, body, count } => Expr::Exists {
            var, var_type, body: Box::new(substitute_var_to_entity(*body, target)), count,
        },
        Expr::ExistsMany { var, var_type, count, body } => Expr::ExistsMany {
            var, var_type, count, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::Question { label, body } => Expr::Question {
            label, body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::Entity(s) => Expr::Entity(s),
    }
}

/// Replace one Var name with another Var name throughout an Expr.
/// Used by relative DP constructors to unify the gap variable with the head variable.
fn substitute_var_name(expr: Expr, old_name: &str, new_name: &str) -> Expr {
    match expr {
        Expr::Var { name, .. } if name == old_name => Expr::Var { name: new_name.to_string(), typ: "e".to_string() },
        Expr::Var { name, typ } => Expr::Var { name, typ },
        Expr::Pred { name, roles } => Expr::Pred {
            name,
            roles: roles
                .into_iter()
                .map(|(r, e)| (r, substitute_var_name(e, old_name, new_name)))
                .collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(substitute_var_name(*e, old_name, new_name))),
        Expr::And(l, r) => Expr::And(
            Box::new(substitute_var_name(*l, old_name, new_name)),
            Box::new(substitute_var_name(*r, old_name, new_name)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(substitute_var_name(*ante, old_name, new_name)),
            cons: Box::new(substitute_var_name(*cons, old_name, new_name)),
        },
        Expr::ForAll { var, var_type, body } => Expr::ForAll {
            var, var_type, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::The { var, var_type, body } => Expr::The {
            var, var_type, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::This { var, var_type, body } => Expr::This {
            var, var_type, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::That { var, var_type, body } => Expr::That {
            var, var_type, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::Exists { var, var_type, body, count } => Expr::Exists {
            var, var_type, body: Box::new(substitute_var_name(*body, old_name, new_name)), count,
        },
        Expr::ExistsMany { var, var_type, count, body } => Expr::ExistsMany {
            var, var_type, count, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::Question { label, body } => Expr::Question {
            label, body: Box::new(substitute_var_name(*body, old_name, new_name)),
        },
        Expr::Entity(s) => Expr::Entity(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_gen() {
        let mut gen = VarGen::new();
        assert_eq!(gen.fresh(), "x");
        assert_eq!(gen.fresh(), "x1");
        assert_eq!(gen.fresh(), "x2");
    }

    #[test]
    fn test_the_dp() {
        let mut gen = VarGen::new();
        let args = vec![
            Arg::Lexical("man".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("the_dp", &args, &mut gen).unwrap();
        match result {
            SemValue::Dp { var, var_type, quant } => {
                assert_eq!(var, "x");
                assert_eq!(var_type, "e");
                assert!(matches!(quant, DpQuant::The { .. }));
            }
            _ => panic!("expected Dp"),
        }
    }

    #[test]
    fn test_bare_dp() {
        let args = vec![
            Arg::Lexical("socrates".into(), "e".into()),
        ];
        let result = apply_constructor("bare_dp", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Dp { var, var_type, quant } => {
                assert_eq!(var, "socrates");
                assert_eq!(var_type, "e");
                assert!(matches!(quant, DpQuant::Bare));
            }
            _ => panic!("expected Dp"),
        }
    }

    #[test]
    fn test_s_copula_the() {
        let dp = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let args = vec![
            Arg::Sub(dp),
            Arg::Lexical("mortal".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_copula", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "the [x:e]: man(theme: x) -> mortal(theme: x)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_copula_bare() {
        let dp = SemValue::Dp {
            var: "socrates".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let args = vec![
            Arg::Sub(dp),
            Arg::Lexical("mortal".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_copula", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "mortal(theme: socrates)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_transitive_the_bare() {
        let dp_subj = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let dp_obj = SemValue::Dp {
            var: "sue".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(dp_obj),
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
        ];
        let result = apply_constructor("s_transitive", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "the [x:e]: man(theme: x) -> loves(agent: x, patient: sue)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_transitive_the_the() {
        let dp_subj = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let dp_obj = SemValue::Dp {
            var: "y".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "woman".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
                },
            },
        };
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(dp_obj),
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
        ];
        let result = apply_constructor("s_transitive", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(
                    format!("{}", expr),
                    "the [x:e]: man(theme: x) -> (the [y:e]: woman(theme: y) -> loves(agent: x, patient: y))"
                );
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_transitive_bare_bare() {
        let dp_subj = SemValue::Dp {
            var: "sue".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let dp_obj = SemValue::Dp {
            var: "tom".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(dp_obj),
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
        ];
        let result = apply_constructor("s_transitive", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "loves(agent: sue, patient: tom)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_copula_forall() {
        let dp = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::ForAll {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let args = vec![
            Arg::Sub(dp),
            Arg::Lexical("mortal".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_copula", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "always [x:e]: man(theme: x) -> mortal(theme: x)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_copula_exists() {
        let dp = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::Exists {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let args = vec![
            Arg::Sub(dp),
            Arg::Lexical("mortal".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_copula", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "exists [x:e]: man(theme: x) ∧ mortal(theme: x)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_transitive_forall_exists() {
        let dp_subj = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::ForAll {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let dp_obj = SemValue::Dp {
            var: "y".into(),
            var_type: "e".into(),
            quant: DpQuant::Exists {
                restriction: Expr::Pred {
                    name: "woman".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
                },
            },
        };
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(dp_obj),
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
        ];
        let result = apply_constructor("s_transitive", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(
                    format!("{}", expr),
                    "always [x:e]: man(theme: x) -> (exists [y:e]: woman(theme: y) ∧ loves(agent: x, patient: y))"
                );
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_complement_bare() {
        let dp_subj = SemValue::Dp {
            var: "john".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let inner_prop = SemValue::Prop(Expr::Pred {
            name: "happy".into(),
            roles: vec![("theme".into(), Expr::Entity("sue".into()))],
        });
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(inner_prop),
            Arg::Lexical("said".into(), "{agent:e,theme:s}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_complement", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(format!("{}", expr), "said(agent: john, theme: happy(theme: sue)");
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_complement_quantified() {
        let dp_subj = SemValue::Dp {
            var: "x".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "x".into(), typ: "e".into() })],
                },
            },
        };
        let inner_prop = SemValue::Prop(Expr::The {
            var: "y".into(),
            var_type: "e".into(),
            body: Box::new(Expr::Implies {
                ante: Box::new(Expr::Pred {
                    name: "woman".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
                }),
                cons: Box::new(Expr::Pred {
                    name: "happy".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
                }),
            }),
        });
        let args = vec![
            Arg::Sub(dp_subj),
            Arg::Sub(inner_prop),
            Arg::Lexical("said".into(), "{agent:e,theme:s}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("theme".into()),
        ];
        let result = apply_constructor("s_complement", &args, &mut VarGen::new()).unwrap();
        match result {
            SemValue::Prop(expr) => {
                assert_eq!(
                    format!("{}", expr),
                    "the [x:e]: man(theme: x) -> said(agent: x, theme: the [y:e]: woman(theme: y) -> happy(theme: y))"
                );
            }
            _ => panic!("expected Prop"),
        }
    }

    #[test]
    fn test_s_gap_agent() {
        let dp_obj = SemValue::Dp {
            var: "sue".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let args = vec![
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
            Arg::Sub(dp_obj),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("s_gap_agent", &args, &mut gen).unwrap();
        match result {
            SemValue::GapProp(g) => {
                assert_eq!(g.gap_role, "agent");
                assert_eq!(g.var, "x"); // first fresh var
                assert_eq!(format!("{}", g), "_[agent] loves(agent: x, patient: sue)");
            }
            _ => panic!("expected GapProp"),
        }
    }

    #[test]
    fn test_s_gap_patient() {
        let dp_subj = SemValue::Dp {
            var: "sue".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        };
        let args = vec![
            Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
            Arg::Literal("agent".into()),
            Arg::Literal("patient".into()),
            Arg::Sub(dp_subj),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("s_gap_patient", &args, &mut gen).unwrap();
        match result {
            SemValue::GapProp(g) => {
                assert_eq!(g.gap_role, "patient");
                assert_eq!(g.var, "x");
                assert_eq!(format!("{}", g), "_[patient] loves(agent: sue, patient: x)");
            }
            _ => panic!("expected GapProp"),
        }
    }

    #[test]
    fn test_s_gap_theme() {
        let args = vec![
            Arg::Lexical("happy".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("s_gap_theme", &args, &mut gen).unwrap();
        match result {
            SemValue::GapProp(g) => {
                assert_eq!(g.gap_role, "theme");
                assert_eq!(g.var, "x");
                assert_eq!(format!("{}", g), "_[theme] happy(theme: x)");
            }
            _ => panic!("expected GapProp"),
        }
    }

    #[test]
    fn test_rel_dp_agent() {
        let gap_prop = SemValue::GapProp(GapProp {
            var: "y".into(),
            var_type: "e".into(),
            gap_role: "agent".into(),
            body: Expr::Pred {
                name: "loves".into(),
                roles: vec![
                    ("agent".into(), Expr::Var { name: "y".into(), typ: "e".into() }),
                    ("patient".into(), Expr::Entity("sue".into())),
                ],
            },
        });
        let args = vec![
            Arg::Lexical("man".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
            Arg::Sub(gap_prop),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("rel_dp_agent", &args, &mut gen).unwrap();
        match result {
            SemValue::Dp { var, var_type, quant } => {
                assert_eq!(var, "x"); // head var
                assert_eq!(var_type, "e");
                assert!(matches!(quant, DpQuant::The { .. }));
                let restriction = match quant {
                    DpQuant::The { restriction } => restriction,
                    _ => panic!("expected The quantifier"),
                };
                assert_eq!(
                    format!("{}", restriction),
                    "man(theme: x) ∧ loves(agent: x, patient: sue)"
                );
            }
            _ => panic!("expected Dp"),
        }
    }

    #[test]
    fn test_rel_dp_patient() {
        let gap_prop = SemValue::GapProp(GapProp {
            var: "y".into(),
            var_type: "e".into(),
            gap_role: "patient".into(),
            body: Expr::Pred {
                name: "loves".into(),
                roles: vec![
                    ("agent".into(), Expr::Entity("sue".into())),
                    ("patient".into(), Expr::Var { name: "y".into(), typ: "e".into() }),
                ],
            },
        });
        let args = vec![
            Arg::Lexical("man".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
            Arg::Sub(gap_prop),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("rel_dp_patient", &args, &mut gen).unwrap();
        match result {
            SemValue::Dp { var, var_type, quant } => {
                assert_eq!(var, "x");
                assert_eq!(var_type, "e");
                assert!(matches!(quant, DpQuant::The { .. }));
                let restriction = match quant {
                    DpQuant::The { restriction } => restriction,
                    _ => panic!("expected The quantifier"),
                };
                assert_eq!(
                    format!("{}", restriction),
                    "man(theme: x) ∧ loves(agent: sue, patient: x)"
                );
            }
            _ => panic!("expected Dp"),
        }
    }

    #[test]
    fn test_rel_dp_theme() {
        let gap_prop = SemValue::GapProp(GapProp {
            var: "y".into(),
            var_type: "e".into(),
            gap_role: "theme".into(),
            body: Expr::Pred {
                name: "happy".into(),
                roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
            },
        });
        let args = vec![
            Arg::Lexical("woman".into(), "{theme:e}".into()),
            Arg::Literal("theme".into()),
            Arg::Sub(gap_prop),
        ];
        let mut gen = VarGen::new();
        let result = apply_constructor("rel_dp_theme", &args, &mut gen).unwrap();
        match result {
            SemValue::Dp { var, var_type, quant } => {
                assert_eq!(var, "x");
                assert_eq!(var_type, "e");
                assert!(matches!(quant, DpQuant::The { .. }));
                let restriction = match quant {
                    DpQuant::The { restriction } => restriction,
                    _ => panic!("expected The quantifier"),
                };
                assert_eq!(
                    format!("{}", restriction),
                    "woman(theme: x) ∧ happy(theme: x)"
                );
            }
            _ => panic!("expected Dp"),
        }
    }

    #[test]
    fn test_substitute_var_name() {
        // Replace y with x in a nested expression
        let expr = Expr::Pred {
            name: "loves".into(),
            roles: vec![
                ("agent".into(), Expr::Var { name: "y".into(), typ: "e".into() }),
                ("patient".into(), Expr::Entity("sue".into())),
            ],
        };
        let result = substitute_var_name(expr, "y", "x");
        assert_eq!(format!("{}", result), "loves(agent: x, patient: sue)");

        // No match — nothing to replace
        let expr2 = Expr::Pred {
            name: "loves".into(),
            roles: vec![
                ("agent".into(), Expr::Entity("john".into())),
                ("patient".into(), Expr::Entity("sue".into())),
            ],
        };
        let result2 = substitute_var_name(expr2, "y", "x");
        assert_eq!(format!("{}", result2), "loves(agent: john, patient: sue)");

        // Nested case: quantifier with the variable inside
        let expr3 = Expr::ForAll {
            var: "z".into(),
            var_type: "e".into(),
            body: Box::new(Expr::Implies {
                ante: Box::new(Expr::Pred {
                    name: "man".into(),
                    roles: vec![("theme".into(), Expr::Var { name: "y".into(), typ: "e".into() })],
                }),
                cons: Box::new(Expr::Pred {
                    name: "loves".into(),
                    roles: vec![
                        ("agent".into(), Expr::Var { name: "y".into(), typ: "e".into() }),
                        ("patient".into(), Expr::Entity("sue".into())),
                    ],
                }),
            }),
        };
        let result3 = substitute_var_name(expr3, "y", "x");
        assert_eq!(
            format!("{}", result3),
            "always [z:e]: man(theme: y) -> loves(agent: x, patient: sue)"
        );
    }
}