
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