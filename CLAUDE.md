# photoroom-assignment

## Project

Rust library for DAG-based f32 operations with Node.js bindings
via neon. Three Rust packages (`core`, `meta`, `node`), and
TypeScript exports.

## Commands

- `mise run check` — full pre-commit: lint, format, build checks
- `mise run fix` — auto-fix lint and format issues
- `mise run test` — run all tests
- `mise run build` — build all packages

## Guiding Principles

1. **Tooling over documentation** — if a rule can be enforced
   by a tool (clippy, oxlint, ruff), configure the tool.
   Don't document what tooling already enforces.
2. **Lean instructions** — CLAUDE.md captures conventions that
   require judgment; mechanical checks belong in config files.
3. **Hooks for guardrails** — `mise run check` must pass before
   stopping after any coding task. Enforced by the Stop hook
   in `.claude/settings.json`.

## Dependency Management

- **Prefer updating over overriding** — when a transitive
  dependency has a vulnerability, update the parent dependency
  first. Only add overrides as a last resort.
- **Respect cooldown periods** — Python uses
  `exclude-newer = "1 day"` (pyproject.toml) and pnpm uses
  `minimumReleaseAge: 1440` minutes (pnpm-workspace.yaml).
  Never pin to a version published less than 1 day ago.
- **Exact pins in pnpm catalog** — all entries in
  `pnpm-workspace.yaml`'s `catalog:` section use exact
  versions (no `^`). This prevents version drift from
  semver ranges.

## Code Conventions

- License header on all source files:
  `// SPDX-License-Identifier: Apache-2.0 OR MIT`
- Use `node:` prefix for Node.js built-in imports:
  `import process from "node:process"`.
- Scripts in `scripts/` must use the helper patterns from
  `scripts/helpers/` (`runCommand`, `runScript`).
- Markdown lines must not exceed 100 characters
  (enforced by markdownlint `MD013`).
