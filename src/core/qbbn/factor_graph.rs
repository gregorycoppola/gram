use std::collections::HashMap;
use serde::Serialize;

use super::kb::KnowledgeBase;
use super::bp::cpt_prob_true;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Proposition,
    Group,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FactorType {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize)]
pub struct Variable {
    pub id: String,
    pub node_type: NodeType,
    pub formula: Option<String>,
    pub conjunct_ids: Vec<String>,
    pub conclusion_id: Option<String>,
    pub rule_id: Option<String>,
    pub negated: bool,
    pub belief: [f64; 2],
    pub is_evidence: bool,
    pub evidence_prob: Option<f64>,
}

impl Variable {
    pub fn prob(&self) -> f64 {
        self.belief[1]
    }

    pub fn set_evidence(&mut self, prob: f64) {
        self.is_evidence = true;
        self.evidence_prob = Some(prob);
        self.belief = [1.0 - prob, prob];
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Factor {
    pub id: String,
    pub factor_type: FactorType,
    pub input_ids: Vec<String>,
    pub input_negated: Vec<bool>,
    pub output_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rule {
    pub id: String,
    pub premise_patterns: Vec<String>,
    pub conclusion_pattern: String,
    pub variables: Vec<(String, String)>,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub rules: Vec<GraphRule>,
    pub formula_map: Vec<(String, String)>,
    pub cpt_tables: Vec<CPTTable>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub formula: Option<String>,
    pub negated: bool,
    pub is_evidence: bool,
    pub evidence_prob: Option<f64>,
    pub belief: f64,
    pub rule_id: Option<String>,
    pub conclusion_id: Option<String>,
    pub conjunct_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub edge_type: String,
    pub source_ids: Vec<String>,
    pub target_id: String,
    pub input_negated: Vec<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphRule {
    pub id: String,
    pub premise_patterns: Vec<String>,
    pub conclusion_pattern: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CPTTable {
    pub factor_id: String,
    pub factor_type: String,
    pub inputs: Vec<String>,
    pub output: String,
    pub rows: Vec<CPTRow>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CPTRow {
    pub assignment: Vec<bool>,
    pub prob_true: f64,
}

#[derive(Debug, Clone)]
pub struct QBBNGraph {
    pub variables: HashMap<String, Variable>,
    pub factors: HashMap<String, Factor>,
    pub rules: HashMap<String, Rule>,
    pub var_to_factors: HashMap<String, Vec<String>>,
    pub prop_to_groups: HashMap<String, Vec<String>>,
    pub formula_to_id: HashMap<String, String>,
    pub query_id: Option<String>,
    p_count: usize,
    g_count: usize,
    and_count: usize,
    or_count: usize,
    r_count: usize,
}

impl QBBNGraph {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            factors: HashMap::new(),
            rules: HashMap::new(),
            var_to_factors: HashMap::new(),
            prop_to_groups: HashMap::new(),
            formula_to_id: HashMap::new(),
            query_id: None,
            p_count: 0,
            g_count: 0,
            and_count: 0,
            or_count: 0,
            r_count: 0,
        }
    }

    pub fn add_proposition(&mut self, formula: &str) -> String {
        if let Some(id) = self.formula_to_id.get(formula) {
            return id.clone();
        }
        self.p_count += 1;
        let id = format!("p{}", self.p_count);
        let var = Variable {
            id: id.clone(),
            node_type: NodeType::Proposition,
            formula: Some(formula.to_string()),
            conjunct_ids: Vec::new(),
            conclusion_id: None,
            rule_id: None,
            negated: false,
            belief: [0.5, 0.5],
            is_evidence: false,
            evidence_prob: None,
        };
        self.variables.insert(id.clone(), var);
        self.formula_to_id.insert(formula.to_string(), id.clone());
        self.var_to_factors.insert(id.clone(), Vec::new());
        self.prop_to_groups.insert(id.clone(), Vec::new());
        id
    }

    pub fn add_group(
        &mut self,
        premise_ids: Vec<String>,
        conclusion_id: String,
        rule_id: String,
        negated: bool,
    ) -> String {
        self.g_count += 1;
        let id = format!("g{}", self.g_count);
        let var = Variable {
            id: id.clone(),
            node_type: NodeType::Group,
            formula: None,
            conjunct_ids: premise_ids.clone(),
            conclusion_id: Some(conclusion_id.clone()),
            rule_id: Some(rule_id),
            negated,
            belief: [0.5, 0.5],
            is_evidence: false,
            evidence_prob: None,
        };
        self.variables.insert(id.clone(), var);
        self.var_to_factors.insert(id.clone(), Vec::new());
        self.prop_to_groups
            .entry(conclusion_id)
            .or_default()
            .push(id.clone());
        id
    }

    pub fn add_and_factor(
        &mut self,
        premise_ids: Vec<String>,
        premise_negated: Vec<bool>,
        group_id: String,
    ) -> String {
        self.and_count += 1;
        let id = format!("and{}", self.and_count);
        let factor = Factor {
            id: id.clone(),
            factor_type: FactorType::And,
            input_ids: premise_ids.clone(),
            input_negated: premise_negated,
            output_id: group_id.clone(),
        };
        self.factors.insert(id.clone(), factor);
        for pid in &premise_ids {
            self.var_to_factors
                .entry(pid.clone())
                .or_default()
                .push(id.clone());
        }
        self.var_to_factors
            .entry(group_id)
            .or_default()
            .push(id.clone());
        id
    }

    pub fn add_or_factor(&mut self, group_ids: Vec<String>, conclusion_id: String) -> String {
        self.or_count += 1;
        let id = format!("or{}", self.or_count);
        let factor = Factor {
            id: id.clone(),
            factor_type: FactorType::Or,
            input_ids: group_ids.clone(),
            input_negated: vec![false; group_ids.len()],
            output_id: conclusion_id.clone(),
        };
        self.factors.insert(id.clone(), factor);
        for gid in &group_ids {
            self.var_to_factors
                .entry(gid.clone())
                .or_default()
                .push(id.clone());
        }
        self.var_to_factors
            .entry(conclusion_id)
            .or_default()
            .push(id.clone());
        id
    }

    pub fn add_rule(
        &mut self,
        premise_patterns: Vec<String>,
        conclusion_pattern: String,
        variables: Vec<(String, String)>,
        weight: f64,
    ) -> String {
        self.r_count += 1;
        let id = format!("r{}", self.r_count);
        let rule = Rule {
            id: id.clone(),
            premise_patterns,
            conclusion_pattern,
            variables,
            weight,
        };
        self.rules.insert(id.clone(), rule);
        id
    }

    pub fn add_grounded_rule(
        &mut self,
        premise_formulas: Vec<String>,
        conclusion_formula: String,
        rule_id: String,
    ) -> String {
        let is_negated = is_negated_formula(&conclusion_formula);
        let pos_formula = get_positive_formula(&conclusion_formula);
        
        let mut premise_ids = Vec::new();
        let mut premise_negated = Vec::new();
        for f in &premise_formulas {
            if is_negated_formula(f) {
                premise_ids.push(self.add_proposition(&get_positive_formula(f)));
                premise_negated.push(true);
            } else {
                premise_ids.push(self.add_proposition(f));
                premise_negated.push(false);
            }
        }
        
        let conc_id = self.add_proposition(&pos_formula);
        let group_id = self.add_group(premise_ids.clone(), conc_id.clone(), rule_id, is_negated);
        self.add_and_factor(premise_ids, premise_negated, group_id.clone());
        group_id
    }

    pub fn build_or_factors(&mut self) {
        let prop_groups: Vec<(String, Vec<String>)> = self
            .prop_to_groups
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (prop_id, group_ids) in prop_groups {
            if !group_ids.is_empty() {
                self.add_or_factor(group_ids, prop_id);
            }
        }
    }

    pub fn set_evidence(&mut self, formula: &str, prob: f64) -> bool {
        let id = if let Some(id) = self.formula_to_id.get(formula).cloned() {
            id
        } else {
            self.add_proposition(formula)
        };
        if let Some(var) = self.variables.get_mut(&id) {
            var.set_evidence(prob);
            true
        } else {
            false
        }
    }

    pub fn set_query(&mut self, formula: &str) {
        if let Some(id) = self.formula_to_id.get(formula).cloned() {
            self.query_id = Some(id);
        }
    }

    pub fn prob(&self, formula: &str) -> f64 {
        if is_negated_formula(formula) {
            let pos_formula = get_positive_formula(formula);
            self.formula_to_id
                .get(&pos_formula)
                .and_then(|id| self.variables.get(id))
                .map(|v| 1.0 - v.prob())
                .unwrap_or(0.0)
        } else {
            self.formula_to_id
                .get(formula)
                .and_then(|id| self.variables.get(id))
                .map(|v| v.prob())
                .unwrap_or(0.0)
        }
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize, usize) {
        let n_props = self
            .variables
            .values()
            .filter(|v| v.node_type == NodeType::Proposition)
            .count();
        let n_groups = self
            .variables
            .values()
            .filter(|v| v.node_type == NodeType::Group)
            .count();
        let n_and = self
            .factors
            .values()
            .filter(|f| f.factor_type == FactorType::And)
            .count();
        let n_or = self
            .factors
            .values()
            .filter(|f| f.factor_type == FactorType::Or)
            .count();
        let n_evidence = self.variables.values().filter(|v| v.is_evidence).count();
        (n_props, n_groups, n_and, n_or, 0, n_evidence)
    }

    pub fn from_kb(kb: &KnowledgeBase) -> Self {
        let mut graph = Self::new();
        let mut rule_map: HashMap<String, String> = HashMap::new();

        for clause in kb.ground_all() {
            if clause.is_fact() {
                let formula = clause.conclusion.to_string();
                graph.add_proposition(&formula);
                graph.set_evidence(&formula, 1.0);
            } else {
                let prem_patterns: Vec<String> =
                    clause.premises.iter().map(|p| p.to_string()).collect();
                let conc_pattern = clause.conclusion.to_string();
                let mut sig_parts = prem_patterns.clone();
                sig_parts.sort();
                let sig = format!("{}|{}", sig_parts.join(","), conc_pattern);

                let rule_id = if let Some(id) = rule_map.get(&sig) {
                    id.clone()
                } else {
                    let rid = graph.add_rule(
                        prem_patterns.clone(),
                        conc_pattern.clone(),
                        clause.variables.clone(),
                        clause.weight,
                    );
                    rule_map.insert(sig, rid.clone());
                    rid
                };

                let prem_formulas: Vec<String> =
                    clause.premises.iter().map(|p| p.to_string()).collect();
                let conc_formula = clause.conclusion.to_string();
                graph.add_grounded_rule(prem_formulas, conc_formula, rule_id);
            }
        }

        graph.build_or_factors();
        graph
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        let mut nodes = Vec::new();
        for (id, var) in &self.variables {
            nodes.push(GraphNode {
                id: id.clone(),
                node_type: match var.node_type {
                    NodeType::Proposition => "proposition".to_string(),
                    NodeType::Group => "group".to_string(),
                },
                formula: var.formula.clone(),
                negated: var.negated,
                is_evidence: var.is_evidence,
                evidence_prob: var.evidence_prob,
                belief: var.prob(),
                rule_id: var.rule_id.clone(),
                conclusion_id: var.conclusion_id.clone(),
                conjunct_ids: var.conjunct_ids.clone(),
            });
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        let mut edges = Vec::new();
        for (id, factor) in &self.factors {
            edges.push(GraphEdge {
                id: id.clone(),
                edge_type: match factor.factor_type {
                    FactorType::And => "and".to_string(),
                    FactorType::Or => "or".to_string(),
                },
                source_ids: factor.input_ids.clone(),
                target_id: factor.output_id.clone(),
                input_negated: factor.input_negated.clone(),
            });
        }
        edges.sort_by(|a, b| a.id.cmp(&b.id));

        let mut rules = Vec::new();
        for (id, rule) in &self.rules {
            rules.push(GraphRule {
                id: id.clone(),
                premise_patterns: rule.premise_patterns.clone(),
                conclusion_pattern: rule.conclusion_pattern.clone(),
                weight: rule.weight,
            });
        }
        rules.sort_by(|a, b| a.id.cmp(&b.id));

        let mut formula_map: Vec<(String, String)> = self.formula_to_id.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        formula_map.sort_by(|a, b| a.1.cmp(&b.1));

        let mut cpt_tables = Vec::new();
        for (id, factor) in &self.factors {
            let input_labels: Vec<String> = factor.input_ids.iter().map(|i| {
                self.variables.get(i).and_then(|v| v.formula.clone()).unwrap_or_else(|| i.clone())
            }).collect();
            let output_label = self.variables.get(&factor.output_id).and_then(|v| v.formula.clone()).unwrap_or_else(|| factor.output_id.clone());

            if factor.factor_type == FactorType::Or {
                let n = factor.input_ids.len();
                let max_rows = 6; // cap at 2^6 = 64 rows
                let truncated = n > max_rows;
                let limit = if truncated { 0 } else { 1 << n };
                let mut rows = Vec::new();
                for assignment in 0..limit {
                    let mut score_pos = 0.0;
                    let mut score_neg = 0.0;
                    let mut input_states = Vec::new();
                    for i in 0..n {
                        let g_id = &factor.input_ids[i];
                        let g = &self.variables[g_id];
                        let is_active = (assignment >> i) & 1 == 1;
                        input_states.push(is_active);
                        if is_active {
                            if let Some(rule_id) = &g.rule_id {
                                if let Some(rule) = self.rules.get(rule_id) {
                                    if g.negated {
                                        score_neg += rule.weight;
                                    } else {
                                        score_pos += rule.weight;
                                    }
                                }
                            }
                        }
                    }
                    let prob_true = cpt_prob_true(score_pos, score_neg);
                    rows.push(CPTRow {
                        assignment: input_states,
                        prob_true,
                    });
                }
                cpt_tables.push(CPTTable {
                    factor_id: id.clone(),
                    factor_type: "or".to_string(),
                    inputs: input_labels,
                    output: output_label,
                    rows,
                    truncated,
                });
            } else {
                cpt_tables.push(CPTTable {
                    factor_id: id.clone(),
                    factor_type: "and".to_string(),
                    inputs: input_labels,
                    output: output_label,
                    rows: Vec::new(),
                    truncated: false,
                });
            }
        }
        cpt_tables.sort_by(|a, b| a.factor_id.cmp(&b.factor_id));

        GraphSnapshot { nodes, edges, rules, formula_map, cpt_tables }
    }
}

fn is_negated_formula(formula: &str) -> bool {
    formula.starts_with("not ")
}

fn get_positive_formula(formula: &str) -> String {
    if formula.starts_with("not ") {
        formula[4..].to_string()
    } else {
        formula.to_string()
    }
}