// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Addon } from "./addon.ts";
import type { NativeHandle } from "./addon-def.ts";

/** Opaque handle to a registered operation. */
export class OpHandle {
  /** @internal */
  readonly _inner: NativeHandle;

  /** @internal */
  constructor(inner: NativeHandle) {
    this._inner = inner;
  }
}

/** Opaque handle to a graph node. */
export class NodeHandle {
  /** @internal */
  readonly _inner: NativeHandle;

  /** @internal */
  constructor(inner: NativeHandle) {
    this._inner = inner;
  }

  /**
   * Returns a new handle wrapping this node with
   * caching enabled.
   */
  cached(): NodeHandle {
    return new NodeHandle(Addon.nodeCached(this._inner));
  }
}

/**
 * Graph builder and evaluator.
 *
 * Holds the eval cache and JS callback registry.
 */
export class Context {
  private readonly state: NativeHandle;

  constructor() {
    this.state = Addon.contextNew();
  }

  /**
   * Register an operation with a label (for debug),
   * expected input count, and a JS callback.
   */
  registerOp(label: string, numInputs: number, apply: (...args: number[]) => number): OpHandle {
    return new OpHandle(Addon.contextRegisterOp(this.state, label, numInputs, apply));
  }

  /** Create a leaf node holding a constant f32 value. */
  value(v: number): NodeHandle {
    return new NodeHandle(Addon.contextValue(v));
  }

  /** Create an inner node applying `op` to `inputs`. */
  node(op: OpHandle, inputs: NodeHandle[]): NodeHandle {
    return new NodeHandle(
      Addon.contextNode(
        op._inner,
        inputs.map((n) => n._inner),
      ),
    );
  }

  /** Evaluate a graph, returning the numeric result. */
  evaluate(root: NodeHandle): number {
    return Addon.contextEvaluate(this.state, root._inner);
  }

  /** Return a debug tree string for a graph. */
  debugTree(root: NodeHandle): string {
    return Addon.contextDebugTree(root._inner);
  }
}
