// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph evaluation with optional caching.

use std::collections::HashMap;

use crate::node::Node;
use crate::node::NodeId;
use crate::node::NodeKind;

/// Holds cached results for nodes marked with `.cached()`.
///
/// Immutable graphs mean the cache never invalidates and
/// remains valid across `eval()` calls.
#[derive(Default)]
pub struct EvalContext {
    cache: HashMap<NodeId, f32>,
}

impl EvalContext {
    /// Create a new, empty evaluation context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached result by node identity.
    #[must_use]
    pub fn get_cached(&self, id: &NodeId) -> Option<f32> {
        self.cache.get(id).copied()
    }

    /// Store a computed result for a node identity.
    pub fn cache(&mut self, id: NodeId, result: f32) {
        self.cache.insert(id, result);
    }
}

/// Evaluate a graph node, using `ctx` for caching.
///
/// Non-cached nodes always recompute. Cached nodes store
/// their result in `ctx` on first evaluation and return
/// the stored value on subsequent encounters.
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::CustomOp;
    use crate::node;
    use crate::value;

    fn add_op() -> Arc<dyn crate::Operation> {
        Arc::new(CustomOp::new("x, y -> x + y", 2, |a| a[0] + a[1]))
    }

    fn mul_op() -> Arc<dyn crate::Operation> {
        Arc::new(CustomOp::new("x, y -> x * y", 2, |a| a[0] * a[1]))
    }

    fn neg_op() -> Arc<dyn crate::Operation> {
        Arc::new(CustomOp::new("x -> -x", 1, |a| -a[0]))
    }

    fn sum3_op() -> Arc<dyn crate::Operation> {
        Arc::new(CustomOp::new("a, b, c -> a+b+c", 3, |a| a[0] + a[1] + a[2]))
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_single_value() {
        let mut ctx = EvalContext::new();
        assert_eq!(eval(&value(42.0), &mut ctx), 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_unary_op() {
        let mut ctx = EvalContext::new();
        let graph = node(neg_op(), vec![value(5.0)]);
        assert_eq!(eval(&graph, &mut ctx), -5.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_binary_op() {
        let mut ctx = EvalContext::new();
        let graph = node(add_op(), vec![value(3.0), value(4.0)]);
        assert_eq!(eval(&graph, &mut ctx), 7.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_multi_input_op() {
        let mut ctx = EvalContext::new();
        let graph = node(sum3_op(), vec![value(1.0), value(2.0), value(3.0)]);
        assert_eq!(eval(&graph, &mut ctx), 6.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_nested_graph() {
        // add(mul(2, 3), 4) = 10
        let mut ctx = EvalContext::new();
        let inner = node(mul_op(), vec![value(2.0), value(3.0)]);
        let graph = node(add_op(), vec![inner, value(4.0)]);
        assert_eq!(eval(&graph, &mut ctx), 10.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_dag_shared_node() {
        // shared = 5, add(shared, shared) = 10
        let mut ctx = EvalContext::new();
        let shared = value(5.0);
        let graph = node(add_op(), vec![shared.clone(), shared]);
        assert_eq!(eval(&graph, &mut ctx), 10.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn eval_custom_op() {
        let mut ctx = EvalContext::new();
        let pow = Arc::new(CustomOp::new("x, y -> x^y", 2, |a| a[0].powf(a[1])));
        let graph = node(pow, vec![value(2.0), value(10.0)]);
        assert_eq!(eval(&graph, &mut ctx), 1024.0);
    }

    /// Helper: an operation that counts how many times
    /// `apply` is called using an atomic counter.
    fn counting_op(counter: Arc<AtomicUsize>) -> Arc<dyn crate::Operation> {
        Arc::new(CustomOp::new("x -> x", 1, move |a| {
            counter.fetch_add(1, Ordering::Relaxed);
            a[0]
        }))
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn cache_hit() {
        let counter = Arc::new(AtomicUsize::new(0));
        let op = counting_op(Arc::clone(&counter));
        let cached_node = node(op, vec![value(7.0)]).cached();
        // Two parents reference the same cached node
        let graph = node(add_op(), vec![cached_node.clone(), cached_node]);
        let mut ctx = EvalContext::new();
        assert_eq!(eval(&graph, &mut ctx), 14.0);
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
        let op = counting_op(Arc::clone(&counter));
        let graph = node(op, vec![value(3.0)]).cached();
        let mut ctx = EvalContext::new();
        assert_eq!(eval(&graph, &mut ctx), 3.0);
        assert_eq!(eval(&graph, &mut ctx), 3.0);
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "second eval must hit cache",
        );
    }

    #[test]
    fn uncached_recomputes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let op = counting_op(Arc::clone(&counter));
        let uncached = node(op, vec![value(1.0)]);
        let graph = node(add_op(), vec![uncached.clone(), uncached]);
        let mut ctx = EvalContext::new();
        let _ = eval(&graph, &mut ctx);
        assert_eq!(
            counter.load(Ordering::Relaxed),
            2,
            "uncached node must evaluate each time",
        );
    }
}
