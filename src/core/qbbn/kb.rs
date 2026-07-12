use std::collections::HashMap;

use crate::core::logic::Expr;

/// A Horn clause: premises → conclusion with optional variables and weight.
#[derive(Debug, Clone, PartialEq)]
pub struct HornClause {
    pub premises: Vec<Expr>,
    pub conclusion: Expr,
    pub variables: Vec<(String, String)>, // (name, type)
    pub weight: f64,
}

impl HornClause {
    pub fn is_fact(&self) -> bool {
        self.premises.is_empty()
    }

    pub fn is_grounded(&self) -> bool {
        self.variables.is_empty()
            && self.premises.iter().all(is_grounded)
            && is_grounded(&self.conclusion)
    }

    pub fn ground(&self, bindings: &HashMap<String, String>) -> Self {
        Self {
            premises: self.premises.iter().map(|p| substitute(p, bindings)).collect(),
            conclusion: substitute(&self.conclusion, bindings),
            variables: Vec::new(),
            weight: self.weight,
        }
    }
}

/// True if an expression contains no free variables.
fn is_grounded(expr: &Expr) -> bool {
    match expr {
        Expr::Var { .. } => false,
        Expr::Pred { roles, .. } => roles.iter().all(|(_, arg)| is_grounded(arg)),
        Expr::Not(e) => is_grounded(e),
        Expr::And(l, r) => is_grounded(l) && is_grounded(r),
        Expr::Implies { ante, cons } => is_grounded(ante) && is_grounded(cons),
        Expr::ForAll { body, .. } => is_grounded(body),
        Expr::The { body, .. } => is_grounded(body),
        Expr::This { body, .. } => is_grounded(body),
        Expr::That { body, .. } => is_grounded(body),
        Expr::Exists { body, .. } => is_grounded(body),
        Expr::ExistsMany { body, .. } => is_grounded(body),
        Expr::Entity(_) => true,
        Expr::Question { body, .. } => is_grounded(body),
    }
}

/// Replace variables with entities according to bindings.
fn substitute(expr: &Expr, bindings: &HashMap<String, String>) -> Expr {
    match expr {
        Expr::Var { name, typ } => {
            if let Some(entity) = bindings.get(name) {
                Expr::Entity(entity.clone())
            } else {
                Expr::Var {
                    name: name.clone(),
                    typ: typ.clone(),
                }
            }
        }
        Expr::Pred { name, roles } => Expr::Pred {
            name: name.clone(),
            roles: roles
                .iter()
                .map(|(r, a)| (r.clone(), substitute(a, bindings)))
                .collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(substitute(e, bindings))),
        Expr::And(l, r) => Expr::And(
            Box::new(substitute(l, bindings)),
            Box::new(substitute(r, bindings)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(substitute(ante, bindings)),
            cons: Box::new(substitute(cons, bindings)),
        },
        Expr::ForAll { var, var_type, body } => Expr::ForAll {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(substitute(body, bindings)),
        },
        Expr::The { var, var_type, body } => Expr::The {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(substitute(body, bindings)),
        },
        Expr::This { var, var_type, body } => Expr::This {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(substitute(body, bindings)),
        },
        Expr::That { var, var_type, body } => Expr::That {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(substitute(body, bindings)),
        },
        Expr::Exists { var, var_type, body, count } => Expr::Exists {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(substitute(body, bindings)),
            count: count.clone(),
        },
        Expr::ExistsMany { var, var_type, count, body } => Expr::ExistsMany {
            var: var.clone(),
            var_type: var_type.clone(),
            count: count.clone(),
            body: Box::new(substitute(body, bindings)),
        },
        Expr::Entity(s) => Expr::Entity(s.clone()),
        Expr::Question { label, body } => Expr::Question {
            label: label.clone(),
            body: Box::new(substitute(body, bindings)),
        },
    }
}

/// Knowledge base: entities, types, and Horn clauses.
#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    pub entities: HashMap<String, String>, // name -> type
    pub types: HashMap<String, Vec<String>>, // type -> [entity names]
    pub clauses: Vec<HornClause>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            types: HashMap::new(),
            clauses: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, name: String, typ: String) {
        self.entities.insert(name.clone(), typ.clone());
        self.types.entry(typ).or_default().push(name);
    }

    pub fn add_fact(&mut self, conclusion: Expr) {
        self.clauses.push(HornClause {
            premises: Vec::new(),
            conclusion,
            variables: Vec::new(),
            weight: 1.0,
        });
    }

    pub fn add_rule(
        &mut self,
        premises: Vec<Expr>,
        conclusion: Expr,
        variables: Vec<(String, String)>,
        weight: f64,
    ) {
        self.clauses.push(HornClause {
            premises,
            conclusion,
            variables,
            weight,
        });
    }

    pub fn entities_of_type(&self, type_name: &str) -> Vec<String> {
        self.types.get(type_name).cloned().unwrap_or_default()
    }

    /// Ground all clauses by instantiating variables with all entities of matching type.
    pub fn ground_all(&self) -> Vec<HornClause> {
        let mut grounded = Vec::new();
        for clause in &self.clauses {
            if clause.is_fact() {
                grounded.push(clause.clone());
            } else if clause.variables.is_empty() {
                grounded.push(clause.clone());
            } else {
                for binding in self.all_bindings(&clause.variables) {
                    grounded.push(clause.ground(&binding));
                }
            }
        }
        grounded
    }

    fn all_bindings(&self, variables: &[(String, String)]) -> Vec<HashMap<String, String>> {
        if variables.is_empty() {
            return vec![HashMap::new()];
        }
        let mut domains: Vec<Vec<String>> = Vec::new();
        for (_, typ) in variables {
            let ents = self.entities_of_type(typ);
            if ents.is_empty() {
                return Vec::new();
            }
            domains.push(ents);
        }
        cartesian_product(&domains, variables)
    }
}

fn cartesian_product(
    domains: &[Vec<String>],
    vars: &[(String, String)],
) -> Vec<HashMap<String, String>> {
    let mut bindings = Vec::new();
    let mut current = Vec::new();
    backtrack(domains, vars, 0, &mut current, &mut bindings);
    bindings
}

fn backtrack(
    domains: &[Vec<String>],
    vars: &[(String, String)],
    idx: usize,
    current: &mut Vec<String>,
    out: &mut Vec<HashMap<String, String>>,
) {
    if idx == domains.len() {
        let mut map = HashMap::new();
        for (i, (name, _)) in vars.iter().enumerate() {
            map.insert(name.clone(), current[i].clone());
        }
        out.push(map);
        return;
    }
    for entity in &domains[idx] {
        current.push(entity.clone());
        backtrack(domains, vars, idx + 1, current, out);
        current.pop();
    }
}