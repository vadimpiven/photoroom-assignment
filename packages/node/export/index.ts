// SPDX-License-Identifier: Apache-2.0 OR MIT

import { addon } from "./addon.ts";

/** Opaque handle to a registered operation. */
export class OpHandle {
  /** @internal */
  readonly _inner: unknown;

  /** @internal */
  constructor(inner: unknown) {
    this._inner = inner;
  }
}

/** Opaque handle to a graph node. */
export class NodeHandle {
  /** @internal */
  readonly _inner: unknown;

  /** @internal */
  constructor(inner: unknown) {
    this._inner = inner;
  }

  /**
   * Returns a new handle wrapping this node with
   * caching enabled.
   */
  cached(): NodeHandle {
    return new NodeHandle(addon.nodeCached(this._inner));
  }
}

/**
 * Graph builder and evaluator.
 *
 * Holds the eval cache and JS callback registry.
 */
export class Context {
  private readonly state: unknown;

  constructor() {
    this.state = addon.contextNew();
  }

  /**
   * Register an operation with a label (for debug),
   * expected input count, and a JS callback.
   */
  registerOp(
    label: string,
    numInputs: number,
    apply: (...args: number[]) => number,
  ): OpHandle {
    return new OpHandle(
      addon.contextRegisterOp(this.state, label, numInputs, apply),
    );
  }

  /** Create a leaf node holding a constant f32 value. */
  value(v: number): NodeHandle {
    return new NodeHandle(addon.contextValue(v));
  }

  /** Create an inner node applying `op` to `inputs`. */
  node(op: OpHandle, inputs: NodeHandle[]): NodeHandle {
    return new NodeHandle(
      addon.contextNode(op._inner, inputs.map((n) => n._inner)),
    );
  }

  /** Evaluate a graph, returning the numeric result. */
  evaluate(root: NodeHandle): number {
    return addon.contextEvaluate(this.state, root._inner);
  }

  /** Return a debug tree string for a graph. */
  debugTree(root: NodeHandle): string {
    return addon.contextDebugTree(root._inner);
  }
}
