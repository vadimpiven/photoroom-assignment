// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Addon as AddonDef } from "./addon-def.ts";
import packageJson from "../package.json" with { type: "json" };

const nodeFileUrl: string = import.meta.url;
const nodeDirname: string = dirname(fileURLToPath(nodeFileUrl));
const nodeRequire: NodeJS.Require = createRequire(nodeFileUrl);

// Resolve path to native addon (NAPI 8, requires Node.js 20+)
// Require calls dlopen under the hood
// DLOpen searches for napi_register_module_v1 in addon export table
// And neon exports napi_register_module_v1 from #[neon::export]
const addonPath = join(nodeDirname, "..", packageJson.addon.path);
const Addon: AddonDef = nodeRequire(addonPath);

export { Addon };
