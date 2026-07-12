use std::collections::HashMap;

use super::kb::KnowledgeBase;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Proposition,
    Group,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FactorType {
    And,
    Or,
}

#[derive(Debug, Clone)]
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
    pub evidence_value: Option<bool>,
}

impl Variable {
    pub fn prob(&self) -> f64 {
        self.belief[1]
    }

    pub fn set_evidence(&mut self, value: bool) {
        self.is_evidence = true;
        self.evidence_value = Some(value);
        self.belief = if value { [0.0, 1.0] } else { [1.0, 0.0] };
    }
}

#[derive(Debug, Clone)]
pub struct Factor {
    pub id: String,
    pub factor_type: FactorType,
    pub input_ids: Vec<String>,
    pub input_negated: Vec<bool>,
    pub output_id: String,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub premise_patterns: Vec<String>,
    pub conclusion_pattern: String,
    pub variables: Vec<(String, String)>,
    pub weight: f64,
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
            evidence_value: None,
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
            evidence_value: None,
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

    pub fn set_evidence(&mut self, formula: &str, value: bool) -> bool {
        let id = if let Some(id) = self.formula_to_id.get(formula).cloned() {
            id
        } else {
            self.add_proposition(formula)
        };
        if let Some(var) = self.variables.get_mut(&id) {
            var.set_evidence(value);
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
                graph.set_evidence(&formula, true);
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