// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Node.js bindings for dag-ops via neon.
//!
//! Exposes `Context`, `NodeHandle`, and `OpHandle` to JS.
//! JS-defined operations are called directly through
//! neon's `FunctionContext` — no `Channel` or async needed.

use core::EvalContext;
use core::Node;
use core::NodeKind;
use core::Operation;
use core::debug_tree;
use core::node;
use core::value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use neon::prelude::*;

// ── Wrapper types (Finalize for JsBox) ──────────────────

/// Wraps a [`Node`] for `JsBox` (orphan rule).
struct NodeHandle(Node);

impl Finalize for NodeHandle {}

/// Wraps an `Arc<dyn Operation>` for `JsBox` (orphan rule).
struct OpHandle(Arc<dyn Operation>);

impl Finalize for OpHandle {}

/// Placeholder operation stored in the core graph when the
/// real logic lives in a JS callback. `apply()` is never
/// called — [`eval_with_js`] invokes the callback directly.
struct PlaceholderOp {
    label: String,
    num_inputs: usize,
}

impl Operation for PlaceholderOp {
    fn label(&self) -> &str {
        &self.label
    }

    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn apply(&self, _inputs: &[f32]) -> f32 {
        unreachable!("PlaceholderOp must be evaluated via eval_with_js")
    }
}

/// Per-`Context` state: eval cache + JS callback registry.
struct ContextState {
    eval_ctx: EvalContext,
    /// Maps operation identity to the JS callback.
    callbacks: HashMap<usize, Root<JsFunction>>,
}

impl Finalize for ContextState {}

// ── Helpers ─────────────────────────────────────────────

/// Stable identity for an `Arc<dyn Operation>`, used as
/// the lookup key into the JS callback registry.
fn op_identity(op: &Arc<dyn Operation>) -> usize {
    Arc::as_ptr(op).cast::<()>() as usize
}

// ── Evaluation with JS callback dispatch ────────────────

/// Evaluate a graph, dispatching JS callbacks via `cx`
/// when a [`PlaceholderOp`] is encountered.
fn eval_with_js<'cx>(
    cx: &mut FunctionContext<'cx>,
    node: &Node,
    state: &mut ContextState,
) -> NeonResult<f32> {
    match node.kind() {
        NodeKind::Value(v) => Ok(*v),
        NodeKind::Op { op, inputs } => {
            let mut args = Vec::with_capacity(inputs.len());
            for input in inputs {
                args.push(eval_with_js(cx, input, state)?);
            }
            if let Some(root) = state.callbacks.get(&op_identity(op)) {
                let func = root.to_inner(cx);
                let this = cx.undefined();
                let js_args: Vec<Handle<'cx, JsValue>> = args
                    .iter()
                    .map(|&v| cx.number(f64::from(v)).upcast())
                    .collect();
                let result = func.call(cx, this, &js_args)?;
                let num: Handle<'cx, JsNumber> = result.downcast_or_throw(cx)?;
                #[allow(clippy::cast_possible_truncation)]
                Ok(num.value(cx) as f32)
            } else {
                Ok(op.apply(&args))
            }
        },
        NodeKind::Cached(inner) => {
            let id = inner.id();
            if let Some(v) = state.eval_ctx.get_cached(&id) {
                return Ok(v);
            }
            let v = eval_with_js(cx, inner, state)?;
            state.eval_ctx.cache(id, v);
            Ok(v)
        },
    }
}

// ── Exported functions ──────────────────────────────────

/// Create a new context (holds eval cache and
/// JS callback registry).
#[neon::export(name = "contextNew")]
fn context_new() -> Result<RefCell<ContextState>, neon::types::extract::Error> {
    Ok(RefCell::new(ContextState {
        eval_ctx: EvalContext::new(),
        callbacks: HashMap::new(),
    }))
}

/// Register a JS callback as an operation.
#[neon::export(name = "contextRegisterOp", context)]
fn context_register_op<'cx>(
    cx: &mut FunctionContext<'cx>,
    state: Handle<'cx, JsBox<RefCell<ContextState>>>,
    label: String,
    num_inputs: f64,
    callback: Handle<'cx, JsFunction>,
) -> JsResult<'cx, JsBox<OpHandle>> {
    let placeholder: Arc<dyn Operation> = Arc::new(PlaceholderOp {
        label,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        num_inputs: num_inputs as usize,
    });
    let key = op_identity(&placeholder);
    let root = callback.root(cx);

    let mut cs = state.borrow_mut();
    cs.callbacks.insert(key, root);

    Ok(cx.boxed(OpHandle(placeholder)))
}

/// Create a leaf node holding a constant f32 value.
#[neon::export(name = "contextValue", context)]
fn context_value<'cx>(cx: &mut FunctionContext<'cx>, v: f64) -> JsResult<'cx, JsBox<NodeHandle>> {
    #[allow(clippy::cast_possible_truncation)]
    let v = v as f32;
    Ok(cx.boxed(NodeHandle(value(v))))
}

/// Create an inner node applying an operation to inputs.
#[neon::export(name = "contextNode", context)]
fn context_node<'cx>(
    cx: &mut FunctionContext<'cx>,
    op: Handle<'cx, JsBox<OpHandle>>,
    inputs: Handle<'cx, JsArray>,
) -> JsResult<'cx, JsBox<NodeHandle>> {
    let arc_op = Arc::clone(&op.0);

    let len = inputs.len(cx);
    let mut input_nodes = Vec::with_capacity(len as usize);
    for i in 0..len {
        let handle: Handle<'cx, JsBox<NodeHandle>> =
            inputs.get::<JsBox<NodeHandle>, _, _>(cx, i)?;
        input_nodes.push(handle.0.clone());
    }

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| node(arc_op, input_nodes)));

    match result {
        Ok(n) => Ok(cx.boxed(NodeHandle(n))),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| e.downcast_ref::<&str>().copied())
                .unwrap_or("arity mismatch");
            cx.throw_error(msg)
        },
    }
}

/// Mark a node for caching. Returns a new handle.
#[neon::export(name = "nodeCached", context)]
fn node_cached<'cx>(
    cx: &mut FunctionContext<'cx>,
    node_handle: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsBox<NodeHandle>> {
    Ok(cx.boxed(NodeHandle(node_handle.0.clone().cached())))
}

/// Evaluate a graph, returning the f32 result.
#[neon::export(name = "contextEvaluate", context)]
fn context_evaluate<'cx>(
    cx: &mut FunctionContext<'cx>,
    state: Handle<'cx, JsBox<RefCell<ContextState>>>,
    root: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsNumber> {
    let mut cs = state.borrow_mut();
    let v = eval_with_js(cx, &root.0, &mut cs)?;
    Ok(cx.number(f64::from(v)))
}

/// Return a debug tree string for a graph.
#[neon::export(name = "contextDebugTree", context)]
fn context_debug_tree<'cx>(
    cx: &mut FunctionContext<'cx>,
    root: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsString> {
    Ok(cx.string(debug_tree(&root.0)))
}

#[cfg(test)]
mod tests {
    use core::CustomOp;
    use core::EvalContext;
    use core::eval;
    use core::node;
    use core::value;
    use std::sync::Arc;

    #[test]
    #[allow(clippy::float_cmp)]
    fn core_reexported_correctly() {
        let add = Arc::new(CustomOp::new("x, y -> x + y", 2, |a| a[0] + a[1]));
        let graph = node(add, vec![value(1.0), value(2.0)]);
        let mut ctx = EvalContext::new();
        assert_eq!(eval(&graph, &mut ctx), 3.0);
    }
}
