
use crate::core::logic::Expr;
use crate::core::value::{DpQuant, SemValue, VarGen};

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

    let var = var_gen.fresh();
    let var_type = "e".to_string();
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

    // Expand quantifiers: subject outer, object inner (surface order)
    let expr = expand_quants(
        &[(subj_var, subj_type, subj_quant), (obj_var, obj_type, obj_quant)],
        body,
    )?;
    Ok(SemValue::Prop(expr))
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
            // Substitute Var nodes for this variable with Entity nodes.
            // Bare DPs introduce a constant name, not a bound variable.
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
    // Wrap from inside out: reverse so first DP is outermost
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
            var,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::The { var, var_type, body } => Expr::The {
            var,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::This { var, var_type, body } => Expr::This {
            var,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::That { var, var_type, body } => Expr::That {
            var,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::Exists { var, var_type, body, count } => Expr::Exists {
            var,
            var_type,
            body: Box::new(substitute_var_to_entity(*body, target)),
            count,
        },
        Expr::ExistsMany { var, var_type, count, body } => Expr::ExistsMany {
            var,
            var_type,
            count,
            body: Box::new(substitute_var_to_entity(*body, target)),
        },
        Expr::Question { label, body } => Expr::Question {
            label,
            body: Box::new(substitute_var_to_entity(*body, target)),
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
}