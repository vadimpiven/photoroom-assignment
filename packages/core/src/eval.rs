// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph evaluation with memoization. Use
//! [`EvalContext::evaluate`] to compute the result of a
//! graph; cached nodes are computed once and reused.

use std::collections::HashMap;

use crate::node::Node;
use crate::node::NodeId;
use crate::node::NodeKind;

/// Evaluation state. Holds memoized results for nodes
/// marked with [`.cached()`](crate::Node::cached).
///
/// Immutable graphs mean cached values never invalidate
/// and remain valid across repeated evaluations.
#[derive(Default)]
pub struct EvalContext {
    cache: HashMap<NodeId, f32>,
}

/// Walk the graph recursively, returning the computed f32.
///
/// Non-cached nodes always recompute. Cached nodes store
/// their result on first evaluation and return the stored
/// value on subsequent encounters.
pub fn eval(node: &Node, ctx: &mut EvalContext) -> f32 {
    match node.kind() {
        NodeKind::Value(v) => *v,
        NodeKind::Op { op, inputs } => {
            let args: Vec<f32> = inputs.iter().map(|n| eval(n, ctx)).collect();
            op.apply(&args)
        },
        NodeKind::Cached(inner) => {
            let id = inner.id();
            if let Some(&v) = ctx.cache.get(&id) {
                return v;
            }
            let v = eval(inner, ctx);
            ctx.cache.insert(id, v);
            v
        },
    }
}

impl EvalContext {
    /// Create a fresh evaluation context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a memoized result by node identity.
    #[must_use]
    pub fn get_cached(&self, id: &NodeId) -> Option<f32> {
        self.cache.get(id).copied()
    }

    /// Store a memoized result for a node identity.
    pub fn cache(&mut self, id: NodeId, result: f32) {
        self.cache.insert(id, result);
    }

    /// Evaluate a graph and return the result.
    pub fn evaluate(&mut self, node: &Node) -> f32 {
        eval(node, self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::node;
    use crate::op;
    use crate::value;

    fn add() -> Arc<dyn crate::Operation> {
        op("x, y -> x + y", 2, |a| a[0] + a[1])
    }

    fn mul() -> Arc<dyn crate::Operation> {
        op("x, y -> x * y", 2, |a| a[0] * a[1])
    }

    fn neg() -> Arc<dyn crate::Operation> {
        op("x -> -x", 1, |a| -a[0])
    }

    fn sum3() -> Arc<dyn crate::Operation> {
        op("a, b, c -> a+b+c", 3, |a| a[0] + a[1] + a[2])
    }

    fn counting(counter: Arc<AtomicUsize>) -> Arc<dyn crate::Operation> {
        op("x -> x", 1, move |a| {
            counter.fetch_add(1, Ordering::Relaxed);
            a[0]
        })
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_single_value() {
        let mut ctx = EvalContext::new();
        assert_eq!(ctx.evaluate(&value(42.0)), 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_unary_op() {
        let mut ctx = EvalContext::new();
        let graph = node(&neg(), &[value(5.0)]);
        assert_eq!(ctx.evaluate(&graph), -5.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_binary_op() {
        let mut ctx = EvalContext::new();
        let graph = node(&add(), &[value(3.0), value(4.0)]);
        assert_eq!(ctx.evaluate(&graph), 7.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_multi_input_op() {
        let mut ctx = EvalContext::new();
        let graph = node(&sum3(), &[value(1.0), value(2.0), value(3.0)]);
        assert_eq!(ctx.evaluate(&graph), 6.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_nested_graph() {
        let mut ctx = EvalContext::new();
        let inner = node(&mul(), &[value(2.0), value(3.0)]);
        let graph = node(&add(), &[inner, value(4.0)]);
        assert_eq!(ctx.evaluate(&graph), 10.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_dag_shared_node() {
        let mut ctx = EvalContext::new();
        let shared = value(5.0);
        let graph = node(&add(), &[shared.clone(), shared]);
        assert_eq!(ctx.evaluate(&graph), 10.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_custom_op() {
        let mut ctx = EvalContext::new();
        let pow = op("x, y -> x^y", 2, |a| a[0].powf(a[1]));
        let graph = node(&pow, &[value(2.0), value(10.0)]);
        assert_eq!(ctx.evaluate(&graph), 1024.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn cache_hit() {
        let counter = Arc::new(AtomicUsize::new(0));
        let cnt = counting(Arc::clone(&counter));
        let cached = node(&cnt, &[value(7.0)]).cached();
        let graph = node(&add(), &[cached.clone(), cached]);
        let mut ctx = EvalContext::new();
        assert_eq!(ctx.evaluate(&graph), 14.0);
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "cached node must evaluate only once",
        );
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn cache_persists_across_evals() {
        let counter = Arc::new(AtomicUsize::new(0));
        let cnt = counting(Arc::clone(&counter));
        let graph = node(&cnt, &[value(3.0)]).cached();
        let mut ctx = EvalContext::new();
        assert_eq!(ctx.evaluate(&graph), 3.0);
        assert_eq!(ctx.evaluate(&graph), 3.0);
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "second eval must hit cache",
        );
    }

    #[test]
    fn uncached_recomputes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let cnt = counting(Arc::clone(&counter));
        let uncached = node(&cnt, &[value(1.0)]);
        let graph = node(&add(), &[uncached.clone(), uncached]);
        let mut ctx = EvalContext::new();
        let _ = ctx.evaluate(&graph);
        assert_eq!(
            counter.load(Ordering::Relaxed),
            2,
            "uncached node must evaluate each time",
        );
    }
}
