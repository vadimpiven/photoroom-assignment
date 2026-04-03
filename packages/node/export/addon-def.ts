// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Opaque native handle — prevents accidental misuse. */
export interface NativeHandle {
  readonly _: unique symbol;
}

/** Raw signatures exported by the native addon. */
export interface Addon {
  contextNew(): NativeHandle;
  contextRegisterOp(
    ctx: NativeHandle,
    label: string,
    numInputs: number,
    callback: (...args: number[]) => number,
  ): NativeHandle;
  contextValue(v: number): NativeHandle;
  contextNode(op: NativeHandle, inputs: NativeHandle[]): NativeHandle;
  nodeCached(node: NativeHandle): NativeHandle;
  contextEvaluate(ctx: NativeHandle, root: NativeHandle): number;
  contextDebugTree(root: NativeHandle): string;
}
