// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from "vitest";
import { Context } from "../../export/index.ts";

describe("Context", () => {
  it("full usage example — evaluates to 2190", () => {
    const ctx = new Context();

    const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
    const sqrt = ctx.registerOp("x -> sqrt(x)", 1, (x) => Math.sqrt(x));
    const pow = ctx.registerOp("x, y -> x^y", 2, (a, b) => a ** b);

    const seven = ctx.value(7);
    const nine = ctx.value(9);

    const sqrtNine = ctx.node(sqrt, [nine]).cached();

    const graph = ctx.node(add, [sqrtNine, ctx.node(pow, [sqrtNine, seven])]);

    expect(ctx.evaluate(graph)).toBeCloseTo(2190, 0);
  });

  it("custom JS callback as operation", () => {
    const ctx = new Context();
    const clamp = ctx.registerOp("x -> clamp(x, 0, 1)", 1, (x) =>
      Math.min(1, Math.max(0, x)),
    );
    expect(ctx.evaluate(ctx.node(clamp, [ctx.value(5)]))).toBeCloseTo(1, 5);
    expect(ctx.evaluate(ctx.node(clamp, [ctx.value(-3)]))).toBeCloseTo(0, 5);
    expect(ctx.evaluate(ctx.node(clamp, [ctx.value(0.5)]))).toBeCloseTo(0.5, 5);
  });

  it("cache works across evaluations", () => {
    const ctx = new Context();
    let callCount = 0;
    const counting = ctx.registerOp("x -> x (counting)", 1, (x) => {
      callCount++;
      return x;
    });
    const cached = ctx.node(counting, [ctx.value(42)]).cached();
    const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
    const graph = ctx.node(add, [cached, cached]);

    const r1 = ctx.evaluate(graph);
    expect(r1).toBeCloseTo(84, 0);
    // Cached node should only have been called once
    expect(callCount).toBe(1);

    const r2 = ctx.evaluate(graph);
    expect(r2).toBeCloseTo(84, 0);
    // Still only one call — cache persists
    expect(callCount).toBe(1);
  });

  it("debug tree output with box-drawing characters", () => {
    const ctx = new Context();
    const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
    const sqrt = ctx.registerOp("x -> sqrt(x)", 1, (x) => Math.sqrt(x));
    const pow = ctx.registerOp("x, y -> x^y", 2, (a, b) => a ** b);

    const sqrtNine = ctx.node(sqrt, [ctx.value(9)]).cached();
    const graph = ctx.node(add, [sqrtNine, ctx.node(pow, [sqrtNine, ctx.value(7)])]);

    const expected = [
      "x, y -> x + y",
      "├── [cached] x -> sqrt(x)",
      "│   └── 9",
      "└── x, y -> x^y",
      "    ├── [cached] x -> sqrt(x)",
      "    │   └── 9",
      "    └── 7",
    ].join("\n");

    expect(ctx.debugTree(graph)).toBe(expected);
  });

  it("arity mismatch throws", () => {
    const ctx = new Context();
    const add = ctx.registerOp("x, y -> x + y", 2, (a, b) => a + b);
    expect(() => ctx.node(add, [ctx.value(1)])).toThrow();
  });
});
