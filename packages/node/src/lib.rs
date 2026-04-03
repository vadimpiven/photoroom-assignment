// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Node.js bindings for dag-ops. Exposes the core graph
//! API to JavaScript through neon, with JS callbacks as
//! operations evaluated on the main thread.

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

/// Node handle for JS interop.
struct NodeHandle(Node);

impl Finalize for NodeHandle {}

/// Operation handle for JS interop.
struct OpHandle(Arc<dyn Operation>);

impl Finalize for OpHandle {}

/// Stands in for a JS-defined operation in the core graph.
/// Evaluation is handled by [`eval_with_js`], which calls
/// the real JS callback instead of `apply`.
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

/// State behind a JS `Context` instance.
struct ContextState {
    eval_ctx: EvalContext,
    /// Maps operation identity to its JS callback.
    callbacks: HashMap<usize, Root<JsFunction>>,
}

impl Finalize for ContextState {}

/// Stable identity for an operation, used to look up
/// JS callbacks during evaluation.
fn op_identity(op: &Arc<dyn Operation>) -> usize {
    Arc::as_ptr(op).cast::<()>() as usize
}

/// Evaluate a graph, calling JS callbacks for operations
/// registered from JavaScript.
fn eval_with_js<'cx>(
    cx: &mut FunctionContext<'cx>,
    graph_node: &Node,
    state: &mut ContextState,
) -> NeonResult<f32> {
    match graph_node.kind() {
        NodeKind::Value(v) => Ok(*v),
        NodeKind::Op { op, inputs } => {
            let mut args = Vec::with_capacity(inputs.len());
            for input in inputs {
                args.push(eval_with_js(cx, input, state)?);
            }
            if let Some(callback) = state.callbacks.get(&op_identity(op)) {
                let func = callback.to_inner(cx);
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

/// Create a new context.
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
    let rooted = callback.root(cx);

    let mut cs = state.borrow_mut();
    cs.callbacks.insert(key, rooted);

    Ok(cx.boxed(OpHandle(placeholder)))
}

/// Create a constant-value leaf node.
#[neon::export(name = "contextValue", context)]
fn context_value<'cx>(cx: &mut FunctionContext<'cx>, v: f64) -> JsResult<'cx, JsBox<NodeHandle>> {
    #[allow(clippy::cast_possible_truncation)]
    let v = v as f32;
    Ok(cx.boxed(NodeHandle(value(v))))
}

/// Create an operation node with the given inputs.
#[neon::export(name = "contextNode", context)]
fn context_node<'cx>(
    cx: &mut FunctionContext<'cx>,
    op: Handle<'cx, JsBox<OpHandle>>,
    inputs: Handle<'cx, JsArray>,
) -> JsResult<'cx, JsBox<NodeHandle>> {
    let len = inputs.len(cx);
    let mut input_nodes = Vec::with_capacity(len as usize);
    for i in 0..len {
        let handle: Handle<'cx, JsBox<NodeHandle>> =
            inputs.get::<JsBox<NodeHandle>, _, _>(cx, i)?;
        input_nodes.push(handle.0.clone());
    }

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| node(&op.0, &input_nodes)));

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

/// Mark a node for memoization. Returns a new handle.
#[neon::export(name = "nodeCached", context)]
fn node_cached<'cx>(
    cx: &mut FunctionContext<'cx>,
    node_handle: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsBox<NodeHandle>> {
    Ok(cx.boxed(NodeHandle(node_handle.0.clone().cached())))
}

/// Evaluate a graph and return the result.
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

/// Render a graph as a debug tree string.
#[neon::export(name = "contextDebugTree", context)]
fn context_debug_tree<'cx>(
    cx: &mut FunctionContext<'cx>,
    root: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsString> {
    Ok(cx.string(debug_tree(&root.0)))
}

#[cfg(test)]
mod tests {
    use core::EvalContext;
    use core::node;
    use core::op;
    use core::value;

    #[test]
    #[allow(clippy::float_cmp)]
    fn core_reexported_correctly() {
        let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
        let graph = node(&add, &[value(1.0), value(2.0)]);
        let mut ctx = EvalContext::new();
        assert_eq!(ctx.evaluate(&graph), 3.0);
    }
}
