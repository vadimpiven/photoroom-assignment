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

/// Wraps a [`Node`] for `JsBox`.
struct NodeHandle(Node);

impl Finalize for NodeHandle {}

/// Proxy operation stored in the core graph for JS callbacks.
/// `apply()` is never called — the neon eval loop invokes
/// the JS callback directly.
struct JsProxyOp {
    label: String,
    num_inputs: usize,
}

impl Operation for JsProxyOp {
    fn label(&self) -> &str {
        &self.label
    }

    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn apply(&self, _inputs: &[f32]) -> f32 {
        unreachable!("JsProxyOp must be evaluated via neon eval")
    }
}

/// Holds the `Arc<dyn Operation>` for graph construction.
/// For JS ops this is a [`JsProxyOp`]; the real callback
/// lives in `BindingState.js_callbacks`.
struct OpHandle(Arc<dyn Operation>);

impl Finalize for OpHandle {}

/// Per-context state: eval cache + JS callback lookup.
struct BindingState {
    eval_ctx: EvalContext,
    /// Maps `Arc` data pointer to `Root<JsFunction>`.
    js_callbacks: HashMap<usize, Root<JsFunction>>,
}

impl Finalize for BindingState {}

// ── Helpers ─────────────────────────────────────────────

/// Extract the data pointer from an `Arc<dyn Operation>`
/// as a `usize` key for the JS callback lookup table.
fn op_ptr_key(op: &Arc<dyn Operation>) -> usize {
    Arc::as_ptr(op).cast::<()>() as usize
}

// ── Neon eval (handles both Rust and JS ops) ────────────

/// Evaluate a graph, calling JS callbacks via `cx` when
/// encountered instead of going through `Operation::apply`.
fn neon_eval<'cx>(
    cx: &mut FunctionContext<'cx>,
    node: &Node,
    state: &mut BindingState,
) -> NeonResult<f32> {
    match node.kind() {
        NodeKind::Value(v) => Ok(*v),
        NodeKind::Op { op, inputs } => {
            let mut args = Vec::with_capacity(inputs.len());
            for input in inputs {
                args.push(neon_eval(cx, input, state)?);
            }
            if let Some(root) = state.js_callbacks.get(&op_ptr_key(op)) {
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
            if let Some(v) = state.eval_ctx.get(&id) {
                return Ok(v);
            }
            let v = neon_eval(cx, inner, state)?;
            state.eval_ctx.insert(id, v);
            Ok(v)
        },
    }
}

// ── Exported functions ──────────────────────────────────

/// Create a new binding context (holds eval cache and
/// JS callback registry).
#[neon::export(name = "contextNew")]
fn context_new() -> Result<RefCell<BindingState>, neon::types::extract::Error> {
    Ok(RefCell::new(BindingState {
        eval_ctx: EvalContext::new(),
        js_callbacks: HashMap::new(),
    }))
}

/// Register a JS callback as an operation.
#[neon::export(name = "contextRegisterOp", context)]
fn context_register_op<'cx>(
    cx: &mut FunctionContext<'cx>,
    state: Handle<'cx, JsBox<RefCell<BindingState>>>,
    label: String,
    num_inputs: f64,
    callback: Handle<'cx, JsFunction>,
) -> JsResult<'cx, JsBox<OpHandle>> {
    let proxy: Arc<dyn Operation> = Arc::new(JsProxyOp {
        label,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        num_inputs: num_inputs as usize,
    });
    let key = op_ptr_key(&proxy);
    let root = callback.root(cx);

    let mut bs = state.borrow_mut();
    bs.js_callbacks.insert(key, root);

    Ok(cx.boxed(OpHandle(proxy)))
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
    state: Handle<'cx, JsBox<RefCell<BindingState>>>,
    root: Handle<'cx, JsBox<NodeHandle>>,
) -> JsResult<'cx, JsNumber> {
    let mut bs = state.borrow_mut();
    let v = neon_eval(cx, &root.0, &mut bs)?;
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
