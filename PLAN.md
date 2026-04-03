# Implementation Plan

DAG-based f32 operation library in Rust with Node.js
bindings via neon. Three Rust packages (`core`, `meta`,
`node`), one TypeScript export layer.

## JS API

```typescript
import { Context } from "dag-ops";

const ctx = new Context();

// Register operations: label (for debug), arity, function
const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
const sqrt = ctx.registerOp("x -> sqrt(x)", 1, (x) => Math.sqrt(x));
const pow = ctx.registerOp("x, y -> x^y", 2, (a, b) => a ** b);

// Build graph bottom-up (acyclic by construction)
const five = ctx.value(5);
const seven = ctx.value(7);
const nine = ctx.value(9);

// .cached() returns a new handle marked for memoization
const sqrtNine = ctx.node(sqrt, [nine]).cached();

// DAG: sqrtNine referenced by two parents
const graph = ctx.node(add, [sqrtNine, ctx.node(pow, [sqrtNine, seven])]);

// Evaluate — cached nodes computed once per eval context
const r1 = ctx.evaluate(graph);
// = add(3, pow(3, 7)) = 2190
const r2 = ctx.evaluate(graph);
// sqrt(9) is a cache hit

console.log(ctx.debugTree(graph));
// x, y -> x + y
// ├── [cached] x -> sqrt(x)
// │   └── 9
// └── x, y -> x^y
//     ├── [cached] x -> sqrt(x)
//     │   └── 9
//     └── 7
```

## TypeScript types

```typescript
type OpHandle = object; // opaque, identity only

export class NodeHandle {
    /** Returns a *new* handle wrapping this node
     *  with caching enabled. */
    cached(): NodeHandle;
}

export class Context {
    registerOp(
        name: string,
        numInputs: number,
        apply: (...args: number[]) => number,
    ): OpHandle;

    value(v: number): NodeHandle;
    node(op: OpHandle, inputs: NodeHandle[]): NodeHandle;
    evaluate(root: NodeHandle): number;
    debugTree(root: NodeHandle): string;
}
```

## Rust core (`packages/core/src/`)

### Operation trait

```rust
pub trait Operation: Send + Sync {
  fn label(&self) -> &str;
  fn num_inputs(&self) -> usize; // >= 1
  fn apply(&self, inputs: &[f32]) -> f32;
}

/// User-provided closure-based operation.
pub struct CustomOp {
  label: String,
  num_inputs: usize,
  apply: ApplyFn,
}
```

### Node

```rust
/// Graph node. Clone is cheap (shared ownership).
#[derive(Clone)]
pub struct Node(Arc<NodeKind>);

pub enum NodeKind {
  Value(f32),
  Op {
    op: Arc<dyn Operation>,
    inputs: Vec<Node>,
  },
  Cached(Node),
}

/// Opaque identity for cache lookups.
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct NodeId(usize);

impl Node {
  pub fn cached(self) -> Node;
  pub fn id(&self) -> NodeId;
  pub fn kind(&self) -> &NodeKind;
}
```

Bottom-up construction guarantees acyclicity: `node()`
and `cached()` consume existing `Node`s, no mutation
after creation.

### Builders

```rust
pub fn value(v: f32) -> Node;
pub fn node(
  op: Arc<dyn Operation>, inputs: Vec<Node>,
) -> Node;
// panics if inputs.len() != op.num_inputs()
```

Core `node()` panics on arity mismatch. The neon layer
catches this via `catch_unwind` and calls
`cx.throw_error()` instead, so JS callers see a proper
exception.

### Evaluation

```rust
pub struct EvalContext {
  cache: HashMap<NodeId, f32>,
}

impl EvalContext {
  pub fn new() -> Self;
  /// Look up a cached result by node identity.
  pub fn get_cached(&self, id: &NodeId) -> Option<f32>;
  /// Store a computed result for a node identity.
  pub fn cache(&mut self, id: NodeId, result: f32);
}

pub fn eval(
  node: &Node, ctx: &mut EvalContext,
) -> f32 {
  match node.kind() {
    NodeKind::Value(v) => *v,
    NodeKind::Op { op, inputs } => {
      let args: Vec<f32> =
        inputs.iter().map(|n| eval(n, ctx)).collect();
      op.apply(&args)
    }
    NodeKind::Cached(inner) => {
      let id = inner.id();
      if let Some(v) = ctx.get_cached(&id) {
        return v;
      }
      let v = eval(inner, ctx);
      ctx.cache(id, v);
      v
    }
  }
}
```

Immutable graph, so the cache never invalidates and
remains valid across `eval()` calls. Non-cached nodes
always recompute.

### Debug display

```rust
pub fn debug_tree(node: &Node) -> String;
```

`Value` prints the f32 literal (integer formatting
when possible: `42` not `42.0`). `Op` prints
`op.label()`. `Cached` adds a `[cached]` prefix.
Box-drawing characters (`├──`, `└──`, `│`) for tree
structure.

### File layout

```txt
packages/core/src/
├── lib.rs        # re-exports
├── node.rs       # NodeKind, Node, NodeId,
│                 # Operation, CustomOp,
│                 # value(), node()
├── eval.rs       # eval(), EvalContext
└── display.rs    # debug_tree()
```

## Neon layer (`packages/node/src/lib.rs`)

### Wrapper types (orphan rule)

Core types cannot implement neon's `Finalize` from
the `node` crate. Thin wrappers solve this:

```rust
struct NodeHandle(Node);
impl Finalize for NodeHandle {}

struct OpHandle(Arc<dyn Operation>);
impl Finalize for OpHandle {}
```

### JS callback design

All evaluation originates from JS via
`Context.evaluate()`. Since we hold a neon
`FunctionContext`, JS callbacks execute directly on
the main thread — no `Channel`, no async, no deadlock
risk.

```rust
/// Placeholder in the core graph. The real callback
/// lives in ContextState.callbacks.
struct PlaceholderOp {
  label: String,
  num_inputs: usize,
}
```

`PlaceholderOp` implements `Operation` with
`apply()` as `unreachable!()` — it exists only to
satisfy `core::node(Arc<dyn Operation>, ...)`.
The real JS callback is stored separately in
`ContextState.callbacks`, keyed by the `Arc` data
pointer (`op_identity()` helper).

During evaluation, `eval_with_js` checks each
operation's identity against the callback registry:

- **Match found** — invoke the JS callback via
  `root.to_inner(cx).call(cx, ...)`.
- **No match** — pure Rust op, call `op.apply()`.

`core::eval()` remains JS-agnostic.

### Type mapping

| JS type      | Rust type                      |
| ------------ | ------------------------------ |
| `Context`    | `JsBox<RefCell<ContextState>>` |
| `OpHandle`   | `JsBox<OpHandle>`              |
| `NodeHandle` | `JsBox<NodeHandle>`            |

```rust
/// Per-Context state: eval cache + callback registry.
struct ContextState {
  eval_ctx: EvalContext,
  callbacks: HashMap<usize, Root<JsFunction>>,
}
```

### Addon loading

The native addon path is stored in `package.json`
under `addon.path` (e.g., `./dist/dag_ops_node.node`).
`addon.ts` loads it via `createRequire`:

```typescript
import packageJson from "../package.json"
  with { type: "json" };
const addonPath = join(
  nodeDirname, "..", packageJson.addon.path,
);
const Addon = nodeRequire(addonPath);
```

`scripts/copy-addon.ts` copies the cargo build
output (`.dylib`/`.so`/`.dll`) to the `.node` path
after each build.

### Method mapping

- **`registerOp`** — creates `PlaceholderOp`, stores
  `Root<JsFunction>` in `ContextState.callbacks`
  keyed by `op_identity()`.
- **`value`** — `core::value(v)`, wraps in
  `NodeHandle`.
- **`node`** — `core::node(op, inputs)`, catches
  arity panics via `catch_unwind`.
- **`cached`** — `Node::cached()`, wraps in new
  `NodeHandle`.
- **`evaluate`** — `eval_with_js()` dispatches JS
  callbacks via `FunctionContext`.
- **`debugTree`** — `core::debug_tree(node)`.

## `meta` package

Build-time utility for neon. Pre-exists in the
workspace scaffold; no new code needed there.

## Stages

### Stage 1a — Core types

Files: `packages/core/src/{lib,node}.rs`

`Operation` trait, `CustomOp`, `NodeKind`, `Node`,
`NodeId`, builders `value()` and `node()`.

Rust tests: construction, arity-mismatch panic.

### Stage 1b — Evaluation

Files: `packages/core/src/eval.rs`

`EvalContext` (with `get_cached`/`cache` accessors),
`eval()`.

Rust tests:

1. `eval_single_value` — `eval(value(42.0))` returns
   `42.0`
2. `eval_unary_op` — negate on a single value
3. `eval_binary_op` — add two values
4. `eval_multi_input_op` — 3+ inputs
5. `eval_nested_graph` — `add(mul(2, 3), 4)` returns
   `10.0`
6. `eval_dag_shared_node` — same node in two parents
7. `eval_custom_op` — closure via `CustomOp`
8. `node_panics_on_arity_mismatch` — `#[should_panic]`
9. `cache_hit` — cached node evaluated once when
   referenced twice (verify via `AtomicUsize` counter)
10. `cache_persists_across_evals` — second `eval()`
    call hits cache
11. `uncached_recomputes` — without `.cached()`,
    counter increments each time

### Stage 2 — Debug display

Files: `packages/core/src/display.rs`

`debug_tree()` with integer formatting for whole
numbers.

Rust tests:

1. `debug_single_value` — prints `42`
2. `debug_single_value_fractional` — prints `1.5`
3. `debug_simple_op` — label + box-drawing children
4. `debug_nested_graph` — multi-level
   `├──`/`└──`/`│`
5. `debug_cached_node` — `[cached]` prefix
6. `debug_dag_shared_cached` — shared cached node in
   both branches

### Stage 3 — Node.js bindings

Files: `packages/node/src/lib.rs`,
`packages/node/export/{addon-def,addon,index}.ts`,
`scripts/copy-addon.ts`, JS test files.

Neon layer: `NodeHandle`/`OpHandle` wrappers,
`PlaceholderOp` + `op_identity()` callback lookup,
`ContextState`, `eval_with_js()`.

TypeScript layer: `addon-def.ts` (native interface
with opaque `NativeHandle`), `addon.ts` (loader via
`createRequire` + `package.json` addon path),
`index.ts` (`Context`/`NodeHandle`/`OpHandle`
classes).

JS integration tests:

1. `full_usage_example` — JS example above returns
   `2190`
2. `custom_op_from_js` — JS callback as operation
3. `cache_works_across_evals` — two evals, correct
   results
4. `debug_tree_output` — expected string with
   box-drawing characters
5. `arity_mismatch_throws` — wrong input count throws

## Out of scope

Serde, async, generics beyond f32, error recovery
(f32 produces NaN/Inf naturally).
