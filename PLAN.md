# Implementation Plan

DAG-based f32 operation library in Rust with Node.js
bindings via neon. Three stages: core types + evaluation,
debug display, JS bindings.

## JS API

```typescript
import { Context } from "dag-ops";

const ctx = new Context();

// Register operations: name (for debug), arity, function
const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
const sqrt = ctx.registerOp("x -> sqrt(x)", 1, (x) => Math.sqrt(x));
const pow = ctx.registerOp("x, y -> x^y", 2, (a, b) => a ** b);

// Build graph bottom-up (acyclic by construction)
const five = ctx.value(5);
const seven = ctx.value(7);
const nine = ctx.value(9);

// .cached() marks a node for memoization
const sqrtNine = ctx.node(sqrt, [nine]).cached();

// DAG: sqrtNine referenced by two parents
const graph = ctx.node(add, [sqrtNine, ctx.node(pow, [sqrtNine, seven])]);

// Evaluate — cached nodes computed once per context
const r1 = ctx.evaluate(graph); // = add(3, pow(3, 7)) = 2190
const r2 = ctx.evaluate(graph); // sqrt(9) is a cache hit

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
type OpHandle = object;

export class NodeHandle {
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
    fn name(&self) -> &str;
    fn num_inputs(&self) -> usize; // >= 1
    fn apply(&self, inputs: &[f32]) -> f32;
}

// Closure wrapper
struct FnOperation {
    name: String,
    num_inputs: usize,
    apply: Box<dyn Fn(&[f32]) -> f32 + Send + Sync>,
}
```

### Node

```rust
// Newtype over Arc — enables methods, hides internals
#[derive(Clone)]
pub struct NodeRef(Arc<NodeInner>);

pub type OpRef = Arc<dyn Operation>;

enum NodeInner {
    Value(f32),
    Op { op: OpRef, inputs: Vec<NodeRef> },
    Cached(NodeRef),
}

impl NodeRef {
    pub fn cached(self) -> NodeRef {
        NodeRef(Arc::new(NodeInner::Cached(self)))
    }

    // Pointer-based identity for cache keys
    fn cache_key(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
```

Bottom-up construction guarantees acyclicity: `node()`
and `cached()` consume existing `NodeRef`s, no mutation
after creation.

### Builders

```rust
pub fn value(v: f32) -> NodeRef;
pub fn node(op: OpRef, inputs: Vec<NodeRef>) -> NodeRef;
// panics if inputs.len() != op.num_inputs()
```

### Evaluation

```rust
pub struct Context {
    cache: HashMap<usize, f32>, // key = NodeRef::cache_key()
}

pub fn eval(node: &NodeRef, ctx: &mut Context) -> f32 {
    match node.inner() {
        NodeInner::Value(v) => *v,
        NodeInner::Op { op, inputs } => {
            let args: Vec<f32> =
                inputs.iter().map(|n| eval(n, ctx)).collect();
            op.apply(&args)
        }
        NodeInner::Cached(inner) => {
            let key = inner.cache_key();
            if let Some(&v) = ctx.cache.get(&key) {
                return v;
            }
            let v = eval(inner, ctx);
            ctx.cache.insert(key, v);
            v
        }
    }
}
```

Immutable graph → cache never invalidates → valid across
`eval()` calls. Non-cached nodes always recompute.

### Debug display

```rust
pub fn debug_tree(node: &NodeRef) -> String;
```

`Value` → f32 literal. `Op` → `op.name()`.
`Cached` → `[cached]` prefix. Box-drawing characters
(`├──`, `└──`, `│`) for tree structure.

### File layout

```txt
packages/core/src/
├── lib.rs        # re-exports
├── node.rs       # NodeInner, NodeRef, OpRef,
│                 # Operation, FnOperation,
│                 # value(), node()
├── eval.rs       # eval(), Context
└── display.rs    # debug_tree()
```

## Neon layer (`packages/node/src/lib.rs`)

| JS type      | Rust type                 |
| ------------ | ------------------------- |
| `Context`    | `JsBox<RefCell<RustCtx>>` |
| `OpHandle`   | `JsBox<OpRef>`            |
| `NodeHandle` | `JsBox<NodeRef>`          |

Method mapping:

- **`NodeHandle.cached`** → `NodeRef::cached()`
- **`Context.registerOp`** → roots JS callback via
  `JsFunction::root(cx)`, wraps in `JsOperation`
  implementing `Operation`. Holds `Root<JsFunction>` +
  neon `Channel` to call back into JS during `apply()`.
- **`Context.value`** → `core::value(v)`
- **`Context.node`** → `core::node(op, inputs)`
- **`Context.evaluate`** → `core::eval(node, &mut ctx)`
- **`Context.debugTree`** → `core::debug_tree(node)`

## Stages

### Stage 1 — Core types and evaluation

Files: `packages/core/src/{lib,node,eval}.rs`

`Operation` trait, `FnOperation`, `NodeInner`, `NodeRef`,
`Context`, builders, `eval()`.
Covers assignment features 1–4.

**Rust tests:**

1. `eval_single_value` — `eval(value(42.0))` → `42.0`
2. `eval_unary_op` — negate on a single value
3. `eval_binary_op` — add two values
4. `eval_multi_input_op` — 3+ inputs
5. `eval_nested_graph` — `add(mul(2, 3), 4)` → `10.0`
6. `eval_dag_shared_node` — same node in two parents
7. `eval_custom_op` — closure via `FnOperation`
8. `node_panics_on_arity_mismatch` — `#[should_panic]`
9. `cache_hit` — cached node evaluated once when
   referenced twice (verify via `AtomicUsize` counter)
10. `cache_persists_across_evals` — second `eval()` call
    hits cache
11. `uncached_recomputes` — without `.cached()`, counter
    increments each time

### Stage 2 — Debug display

Files: `packages/core/src/display.rs`

`debug_tree()` implementation. Covers assignment
feature 6.

**Rust tests:**

1. `debug_single_value` — prints `42`
2. `debug_simple_op` — name + box-drawing children
3. `debug_nested_graph` — multi-level `├──`/`└──`/`│`
4. `debug_cached_node` — `[cached]` prefix
5. `debug_dag_shared_cached` — shared cached node in
   both branches

### Stage 3 — Node.js bindings

Files: `packages/node/src/lib.rs`, JS test files

Neon layer exposing `Context`, `NodeHandle`, `OpHandle`.
Covers assignment feature 5.

**JS integration tests:**

1. `full_usage_example` — JS example above → `2190`
2. `custom_op_from_js` — JS callback as operation
3. `cache_works_across_evals` — two evals, correct
   results
4. `debug_tree_output` — expected string with
   box-drawing characters
5. `arity_mismatch_throws` — wrong input count throws

## Out of scope

Serde, async, generics beyond f32, error recovery
(f32 produces NaN/Inf naturally).
