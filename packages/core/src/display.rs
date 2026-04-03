// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Textual debug display for graphs.

use crate::node::Node;
use crate::node::NodeKind;

/// Return a tree-formatted debug string for a graph.
///
/// Uses box-drawing characters (`├──`, `└──`, `│`) for
/// tree structure. Cached nodes show a `[cached]` prefix.
#[must_use]
pub fn debug_tree(node: &Node) -> String {
    let mut buf = String::new();
    fmt_node(node, &mut buf, &[], true);
    // Remove trailing newline for clean output
    if buf.ends_with('\n') {
        buf.pop();
    }
    buf
}

/// Format a single node and recurse into children.
///
/// `ancestors` tracks whether each ancestor level is a
/// "last child" (true) or not (false), so we know whether
/// to draw `│` or blank in the prefix column.
///
/// `is_root` suppresses the branch prefix on the top node.
fn fmt_node(node: &Node, buf: &mut String, ancestors: &[bool], is_root: bool) {
    // Draw prefix for non-root nodes
    if !is_root {
        for &is_last in &ancestors[..ancestors.len() - 1] {
            if is_last {
                buf.push_str("    ");
            } else {
                buf.push_str("│   ");
            }
        }
        if let Some(&is_last) = ancestors.last() {
            if is_last {
                buf.push_str("└── ");
            } else {
                buf.push_str("├── ");
            }
        }
    }

    match node.kind() {
        NodeKind::Value(v) => {
            push_f32(buf, *v);
            buf.push('\n');
        },
        NodeKind::Op { op, inputs } => {
            buf.push_str(op.label());
            buf.push('\n');
            for (i, child) in inputs.iter().enumerate() {
                let is_last = i == inputs.len() - 1;
                let mut next = ancestors.to_vec();
                next.push(is_last);
                fmt_node(child, buf, &next, false);
            }
        },
        NodeKind::Cached(inner) => {
            buf.push_str("[cached] ");
            // Inline the inner node on the same line
            fmt_cached_inner(inner, buf, ancestors);
        },
    }
}

/// Format the inner content of a cached node, continuing
/// on the same line as the `[cached]` prefix.
fn fmt_cached_inner(node: &Node, buf: &mut String, ancestors: &[bool]) {
    match node.kind() {
        NodeKind::Value(v) => {
            push_f32(buf, *v);
            buf.push('\n');
        },
        NodeKind::Op { op, inputs } => {
            buf.push_str(op.label());
            buf.push('\n');
            for (i, child) in inputs.iter().enumerate() {
                let is_last = i == inputs.len() - 1;
                let mut next = ancestors.to_vec();
                next.push(is_last);
                fmt_node(child, buf, &next, false);
            }
        },
        NodeKind::Cached(inner) => {
            buf.push_str("[cached] ");
            fmt_cached_inner(inner, buf, ancestors);
        },
    }
}

/// Push an f32 value, using integer formatting when possible.
fn push_f32(buf: &mut String, v: f32) {
    use std::fmt::Write as _;
    #[allow(clippy::float_cmp)]
    if v == v.trunc() && v.is_finite() {
        // Print as integer for clean output (42 not 42.0)
        #[allow(clippy::cast_possible_truncation)]
        let _ = write!(buf, "{}", v as i64);
    } else {
        let _ = write!(buf, "{v}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node;
    use crate::op;
    use crate::value;

    #[test]
    fn debug_single_value() {
        assert_eq!(debug_tree(&value(42.0)), "42");
    }

    #[test]
    fn debug_single_value_fractional() {
        assert_eq!(debug_tree(&value(1.5)), "1.5");
    }

    #[test]
    fn debug_simple_op() {
        let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
        let graph = node(&add, &[value(1.0), value(2.0)]);
        let expected = "\
x, y -> x + y
├── 1
└── 2";
        assert_eq!(debug_tree(&graph), expected);
    }

    #[test]
    fn debug_nested_graph() {
        let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
        let sqrt = op("x -> sqrt(x)", 1, |a| a[0].sqrt());
        let pow = op("x, y -> x^y", 2, |a| a[0].powf(a[1]));
        let graph = node(
            &add,
            &[
                node(&sqrt, &[value(9.0)]),
                node(&pow, &[value(2.0), value(3.0)]),
            ],
        );
        let expected = "\
x, y -> x + y
├── x -> sqrt(x)
│   └── 9
└── x, y -> x^y
    ├── 2
    └── 3";
        assert_eq!(debug_tree(&graph), expected);
    }

    #[test]
    fn debug_cached_node() {
        let sqrt = op("x -> sqrt(x)", 1, |a| a[0].sqrt());
        let graph = node(&sqrt, &[value(9.0)]).cached();
        let expected = "\
[cached] x -> sqrt(x)
└── 9";
        assert_eq!(debug_tree(&graph), expected);
    }

    #[test]
    fn debug_dag_shared_cached() {
        let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
        let sqrt = op("x -> sqrt(x)", 1, |a| a[0].sqrt());
        let pow = op("x, y -> x^y", 2, |a| a[0].powf(a[1]));
        let cached_sqrt = node(&sqrt, &[value(9.0)]).cached();
        let graph = node(
            &add,
            &[cached_sqrt.clone(), node(&pow, &[cached_sqrt, value(7.0)])],
        );
        let expected = "\
x, y -> x + y
├── [cached] x -> sqrt(x)
│   └── 9
└── x, y -> x^y
    ├── [cached] x -> sqrt(x)
    │   └── 9
    └── 7";
        assert_eq!(debug_tree(&graph), expected);
    }
}
