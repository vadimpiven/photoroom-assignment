// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph nodes and builder functions. Build graphs
//! bottom-up with [`value`] and [`node`], then mark
//! subtrees for memoization with [`Node::cached`].

use std::sync::Arc;

use crate::operation::Operation;

/// Opaque node identity, stable across clones.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub struct NodeId(usize);

/// The variants a graph node can be.
pub enum NodeKind {
    /// Constant f32 value (leaf).
    Value(f32),
    /// Operation applied to child nodes (inner).
    Op {
        /// The operation to apply.
        op: Arc<dyn Operation>,
        /// Child nodes (>= 1).
        inputs: Vec<Node>,
    },
    /// Memoized wrapper — result is cached on first eval.
    Cached(Node),
}

/// A graph node. Cloning is cheap.
#[derive(Clone)]
pub struct Node(Arc<NodeKind>);

impl Node {
    /// Wrap this node for memoization. Returns a new node.
    #[must_use]
    pub fn cached(self) -> Self {
        Self(Arc::new(NodeKind::Cached(self)))
    }

    /// Stable identity for this node, suitable as a cache key.
    #[must_use]
    pub fn id(&self) -> NodeId {
        NodeId(Arc::as_ptr(&self.0) as usize)
    }

    /// Which variant this node is.
    #[must_use]
    pub fn kind(&self) -> &NodeKind {
        &self.0
    }
}

/// Create a constant-value leaf node.
#[must_use]
pub fn value(v: f32) -> Node {
    Node(Arc::new(NodeKind::Value(v)))
}

/// Create an operation node with the given inputs.
///
/// # Panics
///
/// Panics if `inputs.len() != op.num_inputs()`.
#[must_use]
pub fn node(op: &Arc<dyn Operation>, inputs: &[Node]) -> Node {
    assert!(
        inputs.len() == op.num_inputs(),
        "arity mismatch: operation '{}' expects {} inputs, got {}",
        op.label(),
        op.num_inputs(),
        inputs.len(),
    );
    Node(Arc::new(NodeKind::Op {
        op: Arc::clone(op),
        inputs: inputs.to_vec(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op;

    fn add_op() -> Arc<dyn Operation> {
        op("x, y -> x + y", 2, |a| a[0] + a[1])
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn create_value_node() {
        let n = value(42.0);
        assert!(matches!(n.kind(), NodeKind::Value(v) if *v == 42.0));
    }

    #[test]
    fn create_op_node() {
        let n = node(&add_op(), &[value(1.0), value(2.0)]);
        assert!(matches!(n.kind(), NodeKind::Op { .. }));
    }

    #[test]
    fn create_cached_node() {
        let n = value(5.0).cached();
        assert!(matches!(n.kind(), NodeKind::Cached(_)));
    }

    #[test]
    fn clone_preserves_identity() {
        let n = value(7.0);
        let n2 = n.clone();
        assert!(n.id() == n2.id(), "cloned nodes must share identity");
    }

    #[test]
    #[should_panic(expected = "arity mismatch")]
    fn node_panics_on_arity_mismatch() {
        let _ = node(&add_op(), &[value(1.0)]);
    }
}
