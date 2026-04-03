// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tree-formatted debug output for graphs. Use
//! [`debug_tree`] to get a string with box-drawing
//! characters (`├──`, `└──`, `│`).

use crate::node::Node;
use crate::node::NodeKind;

fn write_value(buf: &mut String, v: f32) {
    use std::fmt::Write as _;
    #[allow(clippy::float_cmp)]
    if v == v.trunc() && v.is_finite() {
        #[allow(clippy::cast_possible_truncation)]
        let _ = write!(buf, "{}", v as i64);
    } else {
        let _ = write!(buf, "{v}");
    }
}

fn write_cached_content(node: &Node, buf: &mut String, depth: &[bool]) {
    match node.kind() {
        NodeKind::Value(v) => {
            write_value(buf, *v);
            buf.push('\n');
        },
        NodeKind::Op { op, inputs } => {
            buf.push_str(op.label());
            buf.push('\n');
            write_children(inputs, buf, depth);
        },
        NodeKind::Cached(inner) => {
            buf.push_str("[cached] ");
            write_cached_content(inner, buf, depth);
        },
    }
}

fn write_children(inputs: &[Node], buf: &mut String, depth: &[bool]) {
    for (i, child) in inputs.iter().enumerate() {
        let is_last = i == inputs.len() - 1;
        let mut next_depth = depth.to_vec();
        next_depth.push(is_last);
        write_node(child, buf, &next_depth, false);
    }
}

fn write_tree_prefix(buf: &mut String, depth: &[bool]) {
    for &is_last in &depth[..depth.len() - 1] {
        if is_last {
            buf.push_str("    ");
        } else {
            buf.push_str("│   ");
        }
    }
    if let Some(&is_last) = depth.last() {
        if is_last {
            buf.push_str("└── ");
        } else {
            buf.push_str("├── ");
        }
    }
}

fn write_node(node: &Node, buf: &mut String, depth: &[bool], is_root: bool) {
    if !is_root {
        write_tree_prefix(buf, depth);
    }

    match node.kind() {
        NodeKind::Value(v) => {
            write_value(buf, *v);
            buf.push('\n');
        },
        NodeKind::Op { op, inputs } => {
            buf.push_str(op.label());
            buf.push('\n');
            write_children(inputs, buf, depth);
        },
        NodeKind::Cached(inner) => {
            buf.push_str("[cached] ");
            write_cached_content(inner, buf, depth);
        },
    }
}

/// Render a graph as a tree with box-drawing characters.
///
/// Cached nodes are prefixed with `[cached]`. Values are
/// printed as integers when possible (`42` not `42.0`).
#[must_use]
pub fn debug_tree(node: &Node) -> String {
    let mut buf = String::new();
    write_node(node, &mut buf, &[], true);
    if buf.ends_with('\n') {
        buf.pop();
    }
    buf
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
