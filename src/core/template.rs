use std::collections::BTreeMap;

/// Replace each `$name` in the template with the bound canonical value.
/// Longest-name-first to avoid `$x` matching inside `$xy`.
pub fn apply_template(template: &str, bindings: &BTreeMap<String, (String, String)>) -> String {
    let mut keys: Vec<&String> = bindings.keys().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    let mut result = template.to_string();
    for k in keys {
        let (canonical, _typ) = &bindings[k];
        result = result.replace(k, canonical);
    }
    result
}