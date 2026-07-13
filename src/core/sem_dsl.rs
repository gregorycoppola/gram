/// Parsed semantic constructor specification from a fixture rule's `sem` field.
///
/// Format: `constructor_name(arg1, arg2, ...)`
/// - Each arg is either `$slot_name` (references a pattern binding) or a bare string (literal)
///
/// Examples:
/// - `the_dp($N, theme)`
/// - `s_copula($SUB, $P, theme)`
/// - `bare_dp($x)`

#[derive(Debug, Clone)]
pub struct SemSpec {
    pub constructor: String,
    pub args: Vec<SemArg>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemArg {
    /// Reference to a pattern slot binding: $N, $SUB, $SUB1, etc.
    Slot(String),
    /// A literal string argument
    Literal(String),
}

/// Parse a sem DSL string into a SemSpec.
pub fn parse_sem(input: &str) -> Result<SemSpec, String> {
    let input = input.trim();
    let paren_pos = input
        .find('(')
        .ok_or_else(|| format!("sem: expected '(' in {:?}", input))?;
    if paren_pos == 0 {
        return Err("sem: empty constructor name".into());
    }
    let constructor = input[..paren_pos].trim().to_string();
    let rest = &input[paren_pos + 1..];
    let rest = rest.trim();
    if !rest.ends_with(')') {
        return Err(format!("sem: expected ')' in {:?}", input));
    }
    let inner = rest[..rest.len() - 1].trim();
    let args = if inner.is_empty() {
        Vec::new()
    } else {
        inner
            .split(',')
            .map(|s| {
                let s = s.trim();
                if let Some(name) = s.strip_prefix('$') {
                    SemArg::Slot(name.to_string())
                } else {
                    SemArg::Literal(s.to_string())
                }
            })
            .collect()
    };
    Ok(SemSpec { constructor, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let spec = parse_sem("the_dp($N, theme)").unwrap();
        assert_eq!(spec.constructor, "the_dp");
        assert_eq!(spec.args.len(), 2);
        assert_eq!(spec.args[0], SemArg::Slot("N".into()));
        assert_eq!(spec.args[1], SemArg::Literal("theme".into()));
    }

    #[test]
    fn parse_with_sub() {
        let spec = parse_sem("s_copula($SUB, $P, theme)").unwrap();
        assert_eq!(spec.constructor, "s_copula");
        assert_eq!(spec.args[0], SemArg::Slot("SUB".into()));
        assert_eq!(spec.args[1], SemArg::Slot("P".into()));
        assert_eq!(spec.args[2], SemArg::Literal("theme".into()));
    }

    #[test]
    fn parse_no_args() {
        let spec = parse_sem("noop()").unwrap();
        assert_eq!(spec.constructor, "noop");
        assert_eq!(spec.args.len(), 0);
    }

    #[test]
    fn parse_transitive() {
        let spec = parse_sem("s_transitive($SUB, $SUB1, $V, agent, patient)").unwrap();
        assert_eq!(spec.constructor, "s_transitive");
        assert_eq!(spec.args.len(), 5);
        assert_eq!(spec.args[0], SemArg::Slot("SUB".into()));
        assert_eq!(spec.args[1], SemArg::Slot("SUB1".into()));
        assert_eq!(spec.args[2], SemArg::Slot("V".into()));
        assert_eq!(spec.args[3], SemArg::Literal("agent".into()));
        assert_eq!(spec.args[4], SemArg::Literal("patient".into()));
    }
}
