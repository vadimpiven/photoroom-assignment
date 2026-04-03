// SPDX-License-Identifier: Apache-2.0 OR MIT

import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { runScript } from "./helpers/run-script.ts";

const projectRoot: string = join(dirname(fileURLToPath(import.meta.url)), "..");

runScript("Copy addon", async () => {
  const ext = process.platform === "win32" ? "dll" : process.platform === "darwin" ? "dylib" : "so";
  const prefix = process.platform === "win32" ? "" : "lib";
  const src = join(projectRoot, "target", "debug", `${prefix}dag_ops_node.${ext}`);
  const dest = join(projectRoot, "packages", "node", "dist", "dag_ops_node.node");

  await mkdir(dirname(dest), { recursive: true });
  await copyFile(src, dest);
});
