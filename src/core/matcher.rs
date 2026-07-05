impl std::fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        render_tree(f, self, 0)
    }
}

fn render_tree(f: &mut std::fmt::Formatter, node: &SyntaxNode, depth: usize) -> std::fmt::Result {
    let indent = "  ".repeat(depth);
    if let Some(ref term) = node.terminal {
        writeln!(f, "{}{} → \"{}\"", indent, node.label, term)?;
    } else {
        writeln!(f, "{}{}", indent, node.label)?;
        for child in &node.children {
            render_tree(f, child, depth + 1)?;
        }
    }
    Ok(())
}