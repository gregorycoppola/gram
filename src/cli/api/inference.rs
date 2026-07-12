println!("\n  ── flow chains ──");
for src in &source_props {
    let src_id = src.id.as_str();
    for edge in &graph.edges {
        if edge.edge_type != "and" {
            continue;
        }
        let input_idx = edge.source_ids.iter().position(|s| s == src_id);
        if input_idx.is_none() {
            continue;
        }
        let group_id = &edge.target_id;
        let group = groups.iter().find(|g| g.id == *group_id);
        if group.is_none() {
            continue;
        }
        let g = group.unwrap();
        let is_negated = edge.input_negated[input_idx.unwrap()];
        let neg = if is_negated { "¬" } else { "" };
        // For negated inputs, the probability is 1 - source belief
        let input_prob = if is_negated {
            1.0 - src.belief
        } else {
            src.belief
        };
        let conclusion = g.conclusion_id.as_deref().unwrap_or("?");
        let or_id = or_by_target.get(conclusion).unwrap_or(&"?");
        let rule_weight = g.rule_id.as_ref()
            .and_then(|rid| graph.rules.iter().find(|r| r.id == *rid))
            .map(|r| r.weight)
            .unwrap_or(0.0);
        println!(
            "  {}{}({:.2}) → {} → {}{}(w={:.1}) → {} → {}",
            neg,
            src_id,
            input_prob,
            edge.id,
            if g.negated { "¬" } else { "" },
            group_id,
            rule_weight,
            or_id,
            conclusion
        );
    }
}