use crate::core::logic::Expr;
use crate::core::value::{DpQuant, GapProp, SemValue, VarGen};

#[derive(Debug, Clone)]
pub enum Arg {
    Lexical(String, String),
    Sub(SemValue),
    Literal(String),
}

pub fn apply_constructor(
    name: &str,
    args: &[Arg],
    var_gen: &mut VarGen,
) -> Result<SemValue, String> {
    match name {
        "the_dp" => construct_quant_dp(DpQuantKind::The, args, var_gen),
        "exists_dp" => construct_quant_dp(DpQuantKind::Exists, args, var_gen),
        "forall_dp" => construct_quant_dp(DpQuantKind::ForAll, args, var_gen),
        "bare_dp" => construct_bare_dp(args),
        "bare_n" => construct_bare_n(args, var_gen),
        "adj_n" => construct_adj_n(args, var_gen),
        "the_n_dp" => construct_the_n_dp(args),
        "a_n_dp" => construct_a_n_dp(args),
        "the_of_dp" => construct_the_of_dp(args, var_gen),
        "adj_of_n" => construct_adj_of_n(args, var_gen),
        "s_copula" => construct_s_copula(args),
        "s_copula_adj_n" => construct_s_copula_adj_n(args),
        "s_equative" => construct_s_equative(args),
        "s_transitive" => construct_s_transitive(args),
        "s_ditransitive" => construct_s_ditransitive(args),
        "s_complement" => construct_s_complement(args),
        "s_gap_agent" => construct_s_gap("agent", "patient", args, var_gen),
        "s_gap_patient" => construct_s_gap("patient", "agent", args, var_gen),
        "s_gap_theme" => construct_s_gap_theme(args, var_gen),
        "rel_dp_agent" => construct_rel_dp("agent", args, var_gen),
        "rel_dp_patient" => construct_rel_dp("patient", args, var_gen),
        "rel_dp_theme" => construct_rel_dp("theme", args, var_gen),
        _ => Err(format!("unknown constructor: {}", name)),
    }
}

enum DpQuantKind { The, Exists, ForAll }

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

fn construct_bare_n(args: &[Arg], var_gen: &mut VarGen) -> Result<SemValue, String> {
    if args.len() != 2 {
        return Err(format!("bare_n expects 2 args, got {}", args.len()));
    }
    let pred_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("bare_n: first arg must be a lexical binding".into()),
    };
    let role = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("bare_n: second arg must be a literal role name".into()),
    };

    let var = var_gen.fresh();
    let restriction = Expr::Pred {
        name: pred_name,
        roles: vec![(role, Expr::Var {
            name: var.clone(),
            typ: "e".to_string(),
        })],
    };

    Ok(SemValue::N { var, restriction })
}

fn construct_adj_n(args: &[Arg], _var_gen: &mut VarGen) -> Result<SemValue, String> {
    if args.len() != 3 {
        return Err(format!("adj_n expects 3 args, got {}", args.len()));
    }
    let adj_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("adj_n: first arg must be a lexical binding".into()),
    };
    let role = match &args[1] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("adj_n: second arg must be a literal role name".into()),
    };
    let (n_var, n_restriction) = match &args[2] {
        Arg::Sub(SemValue::N { var, restriction }) => {
            (var.clone(), restriction.clone())
        }
        _ => return Err("adj_n: third arg must be an N".into()),
    };

    let adj_restriction = Expr::Pred {
        name: adj_name,
        roles: vec![(role, Expr::Var {
            name: n_var.clone(),
            typ: "e".to_string(),
        })],
    };

    let restriction = Expr::And(
        Box::new(adj_restriction),
        Box::new(n_restriction),
    );

    Ok(SemValue::N { var: n_var, restriction })
}

fn construct_the_n_dp(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 1 {
        return Err(format!("the_n_dp expects 1 arg, got {}", args.len()));
    }
    let (n_var, n_restriction) = match &args[0] {
        Arg::Sub(SemValue::N { var, restriction }) => {
            (var.clone(), restriction.clone())
        }
        _ => return Err("the_n_dp: arg must be an N".into()),
    };

    Ok(SemValue::Dp {
        var: n_var,
        var_type: "e".to_string(),
        quant: DpQuant::The { restriction: n_restriction },
    })
}

fn construct_a_n_dp(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 1 {
        return Err(format!("a_n_dp expects 1 arg, got {}", args.len()));
    }
    let (n_var, n_restriction) = match &args[0] {
        Arg::Sub(SemValue::N { var, restriction }) => {
            (var.clone(), restriction.clone())
        }
        _ => return Err("a_n_dp: arg must be an N".into()),
    };

    Ok(SemValue::Dp {
        var: n_var,
        var_type: "e".to_string(),
        quant: DpQuant::Exists { restriction: n_restriction },
    })
}

fn construct_the_of_dp(args: &[Arg], var_gen: &mut VarGen) -> Result<SemValue, String> {
    if args.len() != 4 {
        return Err(format!("the_of_dp expects 4 args, got {}", args.len()));
    }
    let pred_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("the_of_dp: first arg must be lexical".into()),
    };
    let theme_role = match &args[2] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("the_of_dp: third arg must be literal".into()),
    };
    let loc_role = match &args[3] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("the_of_dp: fourth arg must be literal".into()),
    };
    let (obj_var, obj_type) = match &args[1] {
        Arg::Sub(SemValue::Dp { var, var_type, quant: DpQuant::Bare }) => {
            (var.clone(), var_type.clone())
        }
        _ => return Err("the_of_dp: second arg must be a bare DP".into()),
    };

    let var = var_gen.fresh();
    let restriction = Expr::Pred {
        name: pred_name,
        roles: vec![
            (theme_role, Expr::Var { name: var.clone(), typ: "e".to_string() }),
            (loc_role, Expr::Var { name: obj_var, typ: obj_type }),
        ],
    };

    Ok(SemValue::Dp {
        var,
        var_type: "e".to_string(),
        quant: DpQuant::The { restriction },
    })
}

fn construct_adj_of_n(args: &[Arg], var_gen: &mut VarGen) -> Result<SemValue, String> {
    if args.len() != 5 {
        return Err(format!("adj_of_n expects 5 args, got {}", args.len()));
    }
    let adj_name = match &args[0] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("adj_of_n: first arg must be lexical".into()),
    };
    let pred_name = match &args[1] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("adj_of_n: second arg must be lexical".into()),
    };
    let (obj_var, obj_type) = match &args[2] {
        Arg::Sub(SemValue::Dp { var, var_type, .. }) => {
            (var.clone(), var_type.clone())
        }
        _ => return Err("adj_of_n: third arg must be DP".into()),
    };
    let theme_role = match &args[3] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("adj_of_n: fourth arg must be literal".into()),
    };
    let loc_role = match &args[4] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("adj_of_n: fifth arg must be literal".into()),
    };

    let var = var_gen.fresh();
    let noun_restriction = Expr::Pred {
        name: pred_name,
        roles: vec![
            (theme_role.clone(), Expr::Var { name: var.clone(), typ: "e".to_string() }),
            (loc_role, Expr::Var { name: obj_var, typ: obj_type }),
        ],
    };
    let adj_restriction = Expr::Pred {
        name: adj_name,
        roles: vec![(theme_role, Expr::Var { name: var.clone(), typ: "e".to_string() })],
    };

    let restriction = Expr::And(
        Box::new(adj_restriction),
        Box::new(noun_restriction),
    );

    Ok(SemValue::N { var, restriction })
}

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

fn construct_s_copula_adj_n(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 3 {
        return Err(format!("s_copula_adj_n expects 3 args, got {}", args.len()));
    }
    let (subj_var, subj_type, subj_quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_copula_adj_n: first arg must be a DP".into()),
    };
    let (n_var, n_restriction) = match &args[1] {
        Arg::Sub(SemValue::N { var, restriction }) => {
            (var.clone(), restriction.clone())
        }
        _ => return Err("s_copula_adj_n: second arg must be an N".into()),
    };
    let _role = match &args[2] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_copula_adj_n: third arg must be a literal role name".into()),
    };

    let body = substitute_var_name(n_restriction, &n_var, &subj_var);
    let expr = expand_quant(subj_var, subj_type, &subj_quant, body)?;
    Ok(SemValue::Prop(expr))
}

fn construct_s_equative(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 2 {
        return Err(format!("s_equative expects 2 args, got {}", args.len()));
    }
    let (subj_var, subj_type, subj_quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_equative: first arg must be a DP".into()),
    };
    let (obj_var, obj_restriction) = match &args[1] {
        Arg::Sub(SemValue::Dp { var, quant: DpQuant::The { restriction }, .. }) |
        Arg::Sub(SemValue::Dp { var, quant: DpQuant::Exists { restriction }, .. }) => {
            (var.clone(), restriction.clone())
        }
        _ => return Err("s_equative: second arg must be a quantified DP".into()),
    };

    let body = substitute_var_name(obj_restriction, &obj_var, &subj_var);
    let expr = expand_quant(subj_var, subj_type, &subj_quant, body)?;
    Ok(SemValue::Prop(expr))
}

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

fn construct_s_ditransitive(args: &[Arg]) -> Result<SemValue, String> {
    if args.len() != 7 {
        return Err(format!("s_ditransitive expects 7 args, got {}", args.len()));
    }
    let (subj_var, subj_type, subj_quant) = match &args[0] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_ditransitive: first arg must be a DP".into()),
    };
    let (obj1_var, obj1_type, obj1_quant) = match &args[1] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_ditransitive: second arg must be a DP".into()),
    };
    let (obj2_var, obj2_type, obj2_quant) = match &args[2] {
        Arg::Sub(SemValue::Dp { var, var_type, quant }) => {
            (var.clone(), var_type.clone(), quant.clone())
        }
        _ => return Err("s_ditransitive: third arg must be a DP".into()),
    };
    let verb_name = match &args[3] {
        Arg::Lexical(name, _) => name.clone(),
        _ => return Err("s_ditransitive: fourth arg must be a lexical binding".into()),
    };
    let role1 = match &args[4] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_ditransitive: fifth arg must be a literal role name".into()),
    };
    let role2 = match &args[5] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_ditransitive: sixth arg must be a literal role name".into()),
    };
    let role3 = match &args[6] {
        Arg::Literal(s) => s.clone(),
        _ => return Err("s_ditransitive: seventh arg must be a literal role name".into()),
    };

    let body = Expr::Pred {
        name: verb_name,
        roles: vec![
            (role1, Expr::Var { name: subj_var.clone(), typ: subj_type.clone() }),
            (role2, Expr::Var { name: obj1_var.clone(), typ: obj1_type.clone() }),
            (role3, Expr::Var { name: obj2_var.clone(), typ: obj2_type.clone() }),
        ],
    };

    let dps: Vec<(String, String, DpQuant)> = vec![
        (subj_var, subj_type, subj_quant),
        (obj1_var, obj1_type, obj1_quant),
        (obj2_var, obj2_type, obj2_quant),
    ];
    let expr = expand_quants(&dps, body)?;
    Ok(SemValue::Prop(expr))
}

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

fn construct_s_gap(
    gap_role: &str,
    _other_role: &str,
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
        roles: vec![(role.clone(), Expr::Var { name: gap_var.clone(), typ: "e".to_string() })],
    };

    Ok(SemValue::GapProp(GapProp {
        var: gap_var,
        var_type: "e".to_string(),
        gap_role: role,
        body,
    }))
}

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

pub fn substitute_var_name(expr: Expr, old_name: &str, new_name: &str) -> Expr {
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
