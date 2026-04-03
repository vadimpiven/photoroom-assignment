// SPDX-License-Identifier: Apache-2.0 OR MIT

import process from "node:process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const thisDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(thisDir, "..", "..", "..");

function loadAddon(): NeonAddon {
  const ext =
    process.platform === "win32"
      ? "dll"
      : process.platform === "darwin"
        ? "dylib"
        : "so";
  const prefix = process.platform === "win32" ? "" : "lib";
  const libPath = join(
    projectRoot,
    "target",
    "debug",
    `${prefix}dag_ops_node.${ext}`,
  );
  const mod: { exports: NeonAddon } = { exports: {} as NeonAddon };
  process.dlopen(mod, libPath);
  return mod.exports;
}

/** Raw neon export signatures. */
export interface NeonAddon {
  contextNew(): unknown;
  contextRegisterOp(
    ctx: unknown,
    label: string,
    numInputs: number,
    callback: (...args: number[]) => number,
  ): unknown;
  contextValue(v: number): unknown;
  contextNode(op: unknown, inputs: unknown[]): unknown;
  nodeCached(node: unknown): unknown;
  contextEvaluate(ctx: unknown, root: unknown): number;
  contextDebugTree(root: unknown): string;
}

export const addon: NeonAddon = loadAddon();
