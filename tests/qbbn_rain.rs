use gram::core::logic::Expr;
use gram::core::qbbn::{belief_propagation, KnowledgeBase, QBBNGraph};

fn make_pred(name: &str, role: &str, arg: &str) -> Expr {
    Expr::Pred {
        name: name.to_string(),
        roles: vec![(role.to_string(), Expr::Entity(arg.to_string()))],
    }
}

fn make_pred_var(name: &str, role: &str, var: &str, typ: &str) -> Expr {
    Expr::Pred {
        name: name.to_string(),
        roles: vec![(
            role.to_string(),
            Expr::Var {
                name: var.to_string(),
                typ: typ.to_string(),
            },
        )],
    }
}

#[test]
fn test_rain_inference() {
    let mut kb = KnowledgeBase::new();
    kb.add_entity("today".to_string(), "e".to_string());

    // weather_said(theme: rain) -> rain(theme: x) [weight=0.7]
    kb.add_rule(
        vec![make_pred("weather_said", "theme", "rain")],
        make_pred_var("rain", "theme", "x", "e"),
        vec![("x".to_string(), "e".to_string())],
        0.7,
    );

    // no_clouds() -> not rain(theme: x) [weight=0.8]
    kb.add_rule(
        vec![Expr::Pred {
            name: "no_clouds".to_string(),
            roles: vec![],
        }],
        Expr::Not(Box::new(make_pred_var("rain", "theme", "x", "e"))),
        vec![("x".to_string(), "e".to_string())],
        0.8,
    );

    let mut graph = QBBNGraph::from_kb(&kb);

    // Set evidence: weather said rain, no clouds
    graph.set_evidence("weather_said(theme: rain)", true);
    graph.set_evidence("no_clouds()", true);

    // Set query
    graph.set_query("rain(theme: today)");

    let trace = belief_propagation(&mut graph, 20, 0.5, 1e-6);

    let p_rain = graph.prob("rain(theme: today)");
    println!("P(rain) = {}", p_rain);
    println!("iterations = {}", trace.iterations.len());

    // With conflicting evidence, rain should be somewhere between 0 and 1
    assert!(p_rain >= 0.0 && p_rain <= 1.0);
    assert!(!trace.iterations.is_empty());
}