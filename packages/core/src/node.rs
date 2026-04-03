// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph nodes, operations, and builder functions.

use std::sync::Arc;

type ApplyFn = Box<dyn Fn(&[f32]) -> f32 + Send + Sync>;

/// Opaque identity for cache lookups.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub struct NodeId(usize);

/// A graph node. Clone is cheap (shared ownership).
#[derive(Clone)]
pub struct Node(Arc<NodeKind>);

/// What kind of node this is.
pub enum NodeKind {
    /// Leaf node holding a constant f32 value.
    Value(f32),
    /// Inner node applying an operation to inputs.
    Op {
        /// The operation to apply.
        op: Arc<dyn Operation>,
        /// Child nodes (>= 1).
        inputs: Vec<Node>,
    },
    /// Wrapper that memoizes the inner node's result.
    Cached(Node),
}

/// An operation that takes one or more f32 inputs and
/// produces a single f32 output.
pub trait Operation: Send + Sync {
    /// Human-readable label used in debug output.
    fn label(&self) -> &str;
    /// Expected number of inputs (>= 1).
    fn num_inputs(&self) -> usize;
    /// Compute the result from the given inputs.
    fn apply(&self, inputs: &[f32]) -> f32;
}

/// User-provided closure-based operation.
pub struct CustomOp {
    label: String,
    num_inputs: usize,
    apply: ApplyFn,
}

impl CustomOp {
    /// Create a new closure-based operation.
    pub fn new(
        label: impl Into<String>,
        num_inputs: usize,
        apply: impl Fn(&[f32]) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            num_inputs,
            apply: Box::new(apply),
        }
    }
}

impl Operation for CustomOp {
    fn label(&self) -> &str {
        &self.label
    }

    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn apply(&self, inputs: &[f32]) -> f32 {
        (self.apply)(inputs)
    }
}

impl Node {
    /// Returns a new node wrapping self with caching enabled.
    #[must_use]
    pub fn cached(self) -> Self {
        Self(Arc::new(NodeKind::Cached(self)))
    }

    /// Opaque identity for cache lookups.
    #[must_use]
    pub fn id(&self) -> NodeId {
        NodeId(Arc::as_ptr(&self.0) as usize)
    }

    /// Access the underlying node kind.
    #[must_use]
    pub fn kind(&self) -> &NodeKind {
        &self.0
    }
}

/// Create a leaf node holding a constant value.
#[must_use]
pub fn value(v: f32) -> Node {
    Node(Arc::new(NodeKind::Value(v)))
}

/// Create a closure-based operation (convenience for
/// `Arc::new(CustomOp::new(...))`).
#[must_use]
pub fn op(
    label: impl Into<String>,
    num_inputs: usize,
    apply: impl Fn(&[f32]) -> f32 + Send + Sync + 'static,
) -> Arc<dyn Operation> {
    Arc::new(CustomOp::new(label, num_inputs, apply))
}

/// Create an inner node applying `op` to `inputs`.
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
