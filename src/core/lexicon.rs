use std::collections::BTreeMap;

use crate::core::fixture::Fixture;

#[derive(Debug, Clone)]
pub struct Predicate {
    pub name: String,
    pub roles: BTreeMap<String, String>,
}

impl Predicate {
    /// Canonical role signature, e.g. "{theme:e}" or "{agent:e,patient:e}".
    pub fn role_signature(&self) -> String {
        let parts: Vec<String> = self.roles.iter()
            .map(|(r, t)| format!("{}:{}", r, t))
            .collect();
        format!("{{{}}}", parts.join(","))
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub typ: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    Predicate,
    Entity,
}

#[derive(Debug)]
pub struct Lexicon {
    pub predicates: BTreeMap<String, Predicate>,
    pub entities: BTreeMap<String, Entity>,
    /// Lowercased surface form -> (canonical name, category).
    form_index: BTreeMap<String, (String, Category)>,
    max_form_len: usize,
}

impl Lexicon {
    pub fn from_fixture(fixture: &Fixture) -> Self {
        let mut predicates = BTreeMap::new();
        let mut entities = BTreeMap::new();
        let mut form_index: BTreeMap<String, (String, Category)> = BTreeMap::new();
        let mut max_form_len = 1;

        for p in &fixture.lexicon.predicates {
            let pred = Predicate {
                name: p.name.clone(),
                roles: p.roles.clone(),
            };
            let forms: Vec<String> = if p.forms.is_empty() {
                vec![p.name.clone()]
            } else {
                p.forms.clone()
            };
            for form in &forms {
                let key = form.to_lowercase();
                let len = key.split_whitespace().count();
                if len > max_form_len { max_form_len = len; }
                form_index.insert(key, (p.name.clone(), Category::Predicate));
            }
            predicates.insert(p.name.clone(), pred);
        }

        for e in &fixture.lexicon.entities {
            let ent = Entity { name: e.name.clone(), typ: e.typ.clone() };
            let forms: Vec<String> = if e.forms.is_empty() {
                vec![e.name.clone()]
            } else {
                e.forms.clone()
            };
            for form in &forms {
                let key = form.to_lowercase();
                let len = key.split_whitespace().count();
                if len > max_form_len { max_form_len = len; }
                form_index.insert(key, (e.name.clone(), Category::Entity));
            }
            entities.insert(e.name.clone(), ent);
        }

        Lexicon { predicates, entities, form_index, max_form_len }
    }

    pub fn get_type(&self, canonical: &str) -> Option<String> {
        if let Some(p) = self.predicates.get(canonical) {
            return Some(p.role_signature());
        }
        if let Some(e) = self.entities.get(canonical) {
            return Some(e.typ.clone());
        }
        None
    }

    /// Try to match tokens starting at `position`, longest match first.
    /// Returns (canonical_name, category, num_tokens_consumed).
    pub fn lookup_at(&self, tokens: &[String], position: usize) -> Option<(String, Category, usize)> {
        let max_len = self.max_form_len.min(tokens.len() - position);
        for length in (1..=max_len).rev() {
            let phrase: String = tokens[position..position + length].iter()
                .map(|t| clean_token(t))
                .collect::<Vec<_>>()
                .join(" ");
            if let Some((canonical, cat)) = self.form_index.get(&phrase) {
                return Some((canonical.clone(), cat.clone(), length));
            }
        }
        None
    }

    pub fn form_index_len(&self) -> usize { self.form_index.len() }
    pub fn max_form_len(&self) -> usize { self.max_form_len }
    pub fn form_index_entries(&self) -> impl Iterator<Item = (&String, &(String, Category))> {
        self.form_index.iter()
    }
}

pub fn clean_token(token: &str) -> String {
    token.to_lowercase().trim_end_matches(|c: char| ".,!?;:".contains(c)).to_string()
}