// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration tests mirroring the JS test suite.
//! Compare with `packages/node/tests/vitest/index.test.ts`.

use core::EvalContext;
use core::debug_tree;
use core::node;
use core::op;
use core::value;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// Full usage example — evaluates to 2190.
///
/// ```js
/// const add  = ctx.registerOp({ label: "x, y -> x + y", numInputs: 2, apply: (a, b) => a + b });
/// const sqrt = ctx.registerOp({ label: "x -> sqrt(x)",  numInputs: 1, apply: (x) => Math.sqrt(x) });
/// const pow  = ctx.registerOp({ label: "x, y -> x^y",   numInputs: 2, apply: (a, b) => a ** b });
/// const sqrtNine = ctx.node(sqrt, [ctx.value(9)]).cached();
/// const graph = ctx.node(add, [sqrtNine, ctx.node(pow, [sqrtNine, ctx.value(7)])]);
/// expect(ctx.evaluate(graph)).toBeCloseTo(2190, 0);
/// ```
#[test]
#[allow(clippy::float_cmp)]
fn full_usage_example() {
    let mut ctx = EvalContext::new();

    let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
    let sqrt = op("x -> sqrt(x)", 1, |a| a[0].sqrt());
    let pow = op("x, y -> x^y", 2, |a| a[0].powf(a[1]));

    let seven = value(7.0);
    let nine = value(9.0);

    let sqrt_nine = node(&sqrt, &[nine]).cached();
    let graph = node(&add, &[sqrt_nine.clone(), node(&pow, &[sqrt_nine, seven])]);

    assert_eq!(ctx.evaluate(&graph), 2190.0);
}

/// Custom operation — clamp(x, 0, 1).
///
/// ```js
/// const clamp = ctx.registerOp({ label: "x -> clamp(x, 0, 1)", numInputs: 1,
///     apply: (x) => Math.min(1, Math.max(0, x)) });
/// expect(ctx.evaluate(ctx.node(clamp, [ctx.value(5)]))).toBeCloseTo(1, 5);
/// expect(ctx.evaluate(ctx.node(clamp, [ctx.value(-3)]))).toBeCloseTo(0, 5);
/// expect(ctx.evaluate(ctx.node(clamp, [ctx.value(0.5)]))).toBeCloseTo(0.5, 5);
/// ```
#[test]
#[allow(clippy::float_cmp)]
fn custom_op() {
    let mut ctx = EvalContext::new();

    let clamp = op("x -> clamp(x, 0, 1)", 1, |a| a[0].clamp(0.0, 1.0));

    assert_eq!(ctx.evaluate(&node(&clamp, &[value(5.0)])), 1.0);
    assert_eq!(ctx.evaluate(&node(&clamp, &[value(-3.0)])), 0.0);
    assert_eq!(ctx.evaluate(&node(&clamp, &[value(0.5)])), 0.5);
}

/// Cache works across evaluations.
///
/// ```js
/// let callCount = 0;
/// const counting = ctx.registerOp({ label: "x -> x (counting)", numInputs: 1,
///     apply: (x) => { callCount++; return x; } });
/// const cached = ctx.node(counting, [ctx.value(42)]).cached();
/// const add = ctx.registerOp({ ... });
/// const graph = ctx.node(add, [cached, cached]);
/// ctx.evaluate(graph);  // callCount === 1
/// ctx.evaluate(graph);  // still 1
/// ```
#[test]
#[allow(clippy::float_cmp)]
fn cache_works_across_evals() {
    let mut ctx = EvalContext::new();
    let call_count = Arc::new(AtomicUsize::new(0));

    let counting = {
        let counter = Arc::clone(&call_count);
        op("x -> x (counting)", 1, move |a| {
            counter.fetch_add(1, Ordering::Relaxed);
            a[0]
        })
    };
    let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);

    let cached = node(&counting, &[value(42.0)]).cached();
    let graph = node(&add, &[cached.clone(), cached]);

    assert_eq!(ctx.evaluate(&graph), 84.0);
    assert_eq!(
        call_count.load(Ordering::Relaxed),
        1,
        "cached node called once"
    );

    assert_eq!(ctx.evaluate(&graph), 84.0);
    assert_eq!(
        call_count.load(Ordering::Relaxed),
        1,
        "cache persists across evals"
    );
}

/// Debug tree output with box-drawing characters.
///
/// ```js
/// expect(ctx.debugTree(graph)).toBe([
///   "x, y -> x + y",
///   "├── [cached] x -> sqrt(x)",
///   "│   └── 9",
///   "└── x, y -> x^y",
///   "    ├── [cached] x -> sqrt(x)",
///   "    │   └── 9",
///   "    └── 7",
/// ].join("\n"));
/// ```
#[test]
fn debug_tree_output() {
    let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
    let sqrt = op("x -> sqrt(x)", 1, |a| a[0].sqrt());
    let pow = op("x, y -> x^y", 2, |a| a[0].powf(a[1]));

    let sqrt_nine = node(&sqrt, &[value(9.0)]).cached();
    let graph = node(
        &add,
        &[sqrt_nine.clone(), node(&pow, &[sqrt_nine, value(7.0)])],
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

/// Arity mismatch panics.
///
/// ```js
/// expect(() => ctx.node(add, [ctx.value(1)])).toThrow();
/// ```
#[test]
#[should_panic(expected = "arity mismatch")]
fn arity_mismatch_panics() {
    let add = op("x, y -> x + y", 2, |a| a[0] + a[1]);
    let _ = node(&add, &[value(1.0)]);
}
