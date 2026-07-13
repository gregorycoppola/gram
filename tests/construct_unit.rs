use gram::core::construct::{apply_constructor, Arg};
use gram::core::logic::Expr;
use gram::core::substitution::rename_free_var;
use gram::core::value::{DpQuant, GapProp, SemValue, VarGen};

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
        SemValue::Dp {
            var,
            var_type,
            quant,
        } => {
            assert_eq!(var, "x");
            assert_eq!(var_type, "e");
            assert!(matches!(quant, DpQuant::The { .. }));
        }
        _ => panic!("expected Dp"),
    }
}

#[test]
fn test_bare_dp() {
    let args = vec![Arg::Lexical("socrates".into(), "e".into())];
    let result = apply_constructor("bare_dp", &args, &mut VarGen::new()).unwrap();
    match result {
        SemValue::Dp {
            var,
            var_type,
            quant,
        } => {
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(
                format!("{}", expr),
                "the [x:e]: man(theme: x) -> mortal(theme: x)"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(
                format!("{}", expr),
                "the [x:e]: man(theme: x) -> loves(agent: x, patient: sue)"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            },
        },
    };
    let dp_obj = SemValue::Dp {
        var: "y".into(),
        var_type: "e".into(),
        quant: DpQuant::The {
            restriction: Expr::Pred {
                name: "woman".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                )],
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
fn test_s_ditransitive() {
    let dp_subj = SemValue::Dp {
        var: "i".into(),
        var_type: "e".into(),
        quant: DpQuant::Bare,
    };
    let dp_theme = SemValue::Dp {
        var: "x".into(),
        var_type: "e".into(),
        quant: DpQuant::The {
            restriction: Expr::Pred {
                name: "letter".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            },
        },
    };
    let dp_recip = SemValue::Dp {
        var: "sue".into(),
        var_type: "e".into(),
        quant: DpQuant::Bare,
    };
    let args = vec![
        Arg::Sub(dp_subj),
        Arg::Sub(dp_theme),
        Arg::Sub(dp_recip),
        Arg::Lexical("sent".into(), "{agent:e,recipient:e,theme:e}".into()),
        Arg::Literal("agent".into()),
        Arg::Literal("theme".into()),
        Arg::Literal("recipient".into()),
    ];
    let result = apply_constructor("s_ditransitive", &args, &mut VarGen::new()).unwrap();
    match result {
        SemValue::Prop(expr) => {
            assert_eq!(
                format!("{}", expr),
                "the [x:e]: letter(theme: x) -> sent(agent: i, theme: x, recipient: sue)"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(
                format!("{}", expr),
                "always [x:e]: man(theme: x) -> mortal(theme: x)"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(
                format!("{}", expr),
                "exists [x:e]: man(theme: x) ∧ mortal(theme: x)"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            },
        },
    };
    let dp_obj = SemValue::Dp {
        var: "y".into(),
        var_type: "e".into(),
        quant: DpQuant::Exists {
            restriction: Expr::Pred {
                name: "woman".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(
                format!("{}", expr),
                "said(agent: john, theme: happy(theme: sue))"
            );
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
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            },
        },
    };
    let inner_prop = SemValue::Prop(Expr::The {
        var: "y".into(),
        var_type: "e".into(),
        body: Box::new(Expr::Implies {
            ante: Box::new(Expr::Pred {
                name: "woman".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                )],
            }),
            cons: Box::new(Expr::Pred {
                name: "happy".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                )],
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
            assert_eq!(g.var, "x");
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
                (
                    "agent".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                ),
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
        SemValue::Dp {
            var,
            var_type,
            quant,
        } => {
            assert_eq!(var, "x");
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
                (
                    "patient".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                ),
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
        SemValue::Dp {
            var,
            var_type,
            quant,
        } => {
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
            roles: vec![(
                "theme".into(),
                Expr::Var {
                    name: "y".into(),
                    typ: "e".into(),
                },
            )],
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
        SemValue::Dp {
            var,
            var_type,
            quant,
        } => {
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
    let expr = Expr::Pred {
        name: "loves".into(),
        roles: vec![
            (
                "agent".into(),
                Expr::Var {
                    name: "y".into(),
                    typ: "e".into(),
                },
            ),
            ("patient".into(), Expr::Entity("sue".into())),
        ],
    };
    let result = rename_free_var(&expr, "y", "x");
    assert_eq!(format!("{}", result), "loves(agent: x, patient: sue)");

    let expr2 = Expr::Pred {
        name: "loves".into(),
        roles: vec![
            ("agent".into(), Expr::Entity("john".into())),
            ("patient".into(), Expr::Entity("sue".into())),
        ],
    };
    let result2 = rename_free_var(&expr2, "y", "x");
    assert_eq!(format!("{}", result2), "loves(agent: john, patient: sue)");

    let expr3 = Expr::ForAll {
        var: "z".into(),
        var_type: "e".into(),
        body: Box::new(Expr::Implies {
            ante: Box::new(Expr::Pred {
                name: "man".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "y".into(),
                        typ: "e".into(),
                    },
                )],
            }),
            cons: Box::new(Expr::Pred {
                name: "loves".into(),
                roles: vec![
                    (
                        "agent".into(),
                        Expr::Var {
                            name: "y".into(),
                            typ: "e".into(),
                        },
                    ),
                    ("patient".into(), Expr::Entity("sue".into())),
                ],
            }),
        }),
    };
    let result3 = rename_free_var(&expr3, "y", "x");
    assert_eq!(
        format!("{}", result3),
        "always [z:e]: man(theme: x) -> loves(agent: x, patient: sue)"
    );
}

#[test]
fn test_s_gap_bare_other_dp_becomes_entity() {
    let args = vec![
        Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
        Arg::Literal("agent".into()),
        Arg::Literal("patient".into()),
        Arg::Sub(SemValue::Dp {
            var: "sue".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        }),
    ];

    let mut gen = VarGen::new();
    let result = apply_constructor("s_gap_agent", &args, &mut gen).unwrap();

    let SemValue::GapProp(gap) = result else {
        panic!("expected GapProp");
    };
    let Expr::Pred { roles, .. } = gap.body else {
        panic!("expected predicate body");
    };

    assert!(matches!(
        &roles[0].1,
        Expr::Var { name, typ } if name == "x" && typ == "e"
    ));
    assert!(matches!(
        &roles[1].1,
        Expr::Entity(name) if name == "sue"
    ));
}

#[test]
fn test_s_gap_preserves_quantified_other_dp() {
    let restriction = Expr::Pred {
        name: "woman".into(),
        roles: vec![(
            "theme".into(),
            Expr::Var {
                name: "y".into(),
                typ: "e".into(),
            },
        )],
    };

    let args = vec![
        Arg::Lexical("loves".into(), "{agent:e,patient:e}".into()),
        Arg::Literal("agent".into()),
        Arg::Literal("patient".into()),
        Arg::Sub(SemValue::Dp {
            var: "y".into(),
            var_type: "e".into(),
            quant: DpQuant::Exists { restriction },
        }),
    ];

    let mut gen = VarGen::new();
    let result = apply_constructor("s_gap_agent", &args, &mut gen).unwrap();

    let SemValue::GapProp(gap) = result else {
        panic!("expected GapProp");
    };

    assert_eq!(
        format!("{}", gap.body),
        "exists [y:e]: woman(theme: y) ∧ loves(agent: x, patient: y)"
    );
}

#[test]
fn test_the_of_dp_bare_complement_is_entity() {
    let args = vec![
        Arg::Lexical("mayor_of".into(), "{theme:e,location:e}".into()),
        Arg::Sub(SemValue::Dp {
            var: "london".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        }),
        Arg::Literal("theme".into()),
        Arg::Literal("location".into()),
    ];

    let mut gen = VarGen::new();
    let result = apply_constructor("the_of_dp", &args, &mut gen).unwrap();

    let SemValue::Dp {
        quant: DpQuant::The { restriction },
        ..
    } = result
    else {
        panic!("expected The DP");
    };

    let Expr::Pred { roles, .. } = restriction else {
        panic!("expected relational restriction");
    };

    assert!(matches!(
        &roles[0].1,
        Expr::Var { name, typ } if name == "x" && typ == "e"
    ));
    assert!(matches!(
        &roles[1].1,
        Expr::Entity(name) if name == "london"
    ));
}

#[test]
fn test_the_pp_dp_bare_complement_is_entity() {
    let args = vec![
        Arg::Lexical("man".into(), "{theme:e}".into()),
        Arg::Literal("from".into()),
        Arg::Sub(SemValue::Dp {
            var: "nantucket".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        }),
        Arg::Literal("theme".into()),
        Arg::Literal("complement".into()),
    ];

    let mut gen = VarGen::new();
    let result = apply_constructor("the_pp_dp", &args, &mut gen).unwrap();

    let SemValue::Dp {
        quant: DpQuant::The { restriction },
        ..
    } = result
    else {
        panic!("expected The DP");
    };

    let Expr::And(_, pp_restriction) = restriction else {
        panic!("expected noun and PP restriction");
    };
    let Expr::Pred { roles, .. } = *pp_restriction else {
        panic!("expected PP predicate");
    };

    assert!(matches!(
        &roles[1].1,
        Expr::Entity(name) if name == "nantucket"
    ));
}

#[test]
fn test_adj_of_n_bare_complement_is_entity() {
    let args = vec![
        Arg::Lexical("american".into(), "{theme:e}".into()),
        Arg::Lexical("edition_of".into(), "{theme:e,location:e}".into()),
        Arg::Sub(SemValue::Dp {
            var: "show".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        }),
        Arg::Literal("theme".into()),
        Arg::Literal("location".into()),
    ];

    let mut gen = VarGen::new();
    let result = apply_constructor("adj_of_n", &args, &mut gen).unwrap();

    let SemValue::N { restriction, .. } = result else {
        panic!("expected N");
    };
    let Expr::And(_, noun_restriction) = restriction else {
        panic!("expected adjective and noun restriction");
    };
    let Expr::Pred { roles, .. } = *noun_restriction else {
        panic!("expected relational noun predicate");
    };

    assert!(matches!(
        &roles[1].1,
        Expr::Entity(name) if name == "show"
    ));
}

#[test]
fn test_the_pp_dp_rejects_quantified_complement() {
    let complement_restriction = Expr::Pred {
        name: "island".into(),
        roles: vec![(
            "theme".into(),
            Expr::Var {
                name: "y".into(),
                typ: "e".into(),
            },
        )],
    };

    let args = vec![
        Arg::Lexical("man".into(), "{theme:e}".into()),
        Arg::Literal("from".into()),
        Arg::Sub(SemValue::Dp {
            var: "y".into(),
            var_type: "e".into(),
            quant: DpQuant::Exists {
                restriction: complement_restriction,
            },
        }),
        Arg::Literal("theme".into()),
        Arg::Literal("complement".into()),
    ];

    let mut gen = VarGen::new();
    let error = apply_constructor("the_pp_dp", &args, &mut gen).unwrap_err();

    assert!(error.contains("bare DP"), "got: {error}");
}


#[test]
fn test_possessive_the_n_dp_enriches_composed_head() {
    let noun = SemValue::N {
        var: "x".into(),
        restriction: Expr::And(
            Box::new(Expr::Pred {
                name: "first".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            }),
            Box::new(Expr::Pred {
                name: "nba_championship".into(),
                roles: vec![
                    (
                        "theme".into(),
                        Expr::Var {
                            name: "x".into(),
                            typ: "e".into(),
                        },
                    ),
                    ("interval".into(), Expr::Entity("years_53".into())),
                ],
            }),
        ),
    };

    let args = vec![
        Arg::Sub(SemValue::Dp {
            var: "new_york_knicks".into(),
            var_type: "e".into(),
            quant: DpQuant::Bare,
        }),
        Arg::Sub(noun),
        Arg::Literal("team".into()),
    ];

    let result =
        apply_constructor("possessive_the_n_dp", &args, &mut VarGen::new()).unwrap();

    let SemValue::Dp {
        var,
        quant: DpQuant::The { restriction },
        ..
    } = result
    else {
        panic!("expected definite DP");
    };

    assert_eq!(var, "x");
    assert_eq!(
        format!("{}", restriction),
        "first(theme: x) ∧ nba_championship(theme: x, interval: years_53, team: new_york_knicks)"
    );
}

#[test]
fn test_possessive_the_n_dp_rejects_quantified_possessor() {
    let args = vec![
        Arg::Sub(SemValue::Dp {
            var: "team".into(),
            var_type: "e".into(),
            quant: DpQuant::The {
                restriction: Expr::Pred {
                    name: "team".into(),
                    roles: vec![(
                        "theme".into(),
                        Expr::Var {
                            name: "team".into(),
                            typ: "e".into(),
                        },
                    )],
                },
            },
        }),
        Arg::Sub(SemValue::N {
            var: "x".into(),
            restriction: Expr::Pred {
                name: "championship".into(),
                roles: vec![(
                    "theme".into(),
                    Expr::Var {
                        name: "x".into(),
                        typ: "e".into(),
                    },
                )],
            },
        }),
        Arg::Literal("team".into()),
    ];

    let error =
        apply_constructor("possessive_the_n_dp", &args, &mut VarGen::new()).unwrap_err();

    assert!(error.contains("bare possessor DP"), "got: {error}");
}
