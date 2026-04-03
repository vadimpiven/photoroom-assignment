// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Addon } from "./addon.ts";
import type { NativeHandle } from "./addon-def.ts";

/** Definition for a user-provided operation. */
export interface OpDef {
  /** Human-readable label shown in debug output. */
  label: string;
  /** Expected number of inputs (>= 1). */
  numInputs: number;
  /** Compute the result from the given inputs. */
  apply: (...args: number[]) => number;
}

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

  /** Register an operation from its definition. */
  registerOp(def: OpDef): OpHandle {
    return new OpHandle(Addon.contextRegisterOp(this.state, def.label, def.numInputs, def.apply));
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
