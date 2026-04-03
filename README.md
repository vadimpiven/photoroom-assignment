# Photoroom Assignment

## Quick start

The only prerequisite is
[mise](https://mise.jdx.dev/getting-started.html).
It manages Node.js, Rust, Python, pnpm, and all other tooling
automatically.

```bash
git clone https://github.com/vadimpiven/node_reqwest.git
cd node_reqwest
mise trust
mise install
mise run test
```

Rerun tests without cache: `mise run -f t`

## Packages

| Package | Purpose                                                    |
| ------- | ---------------------------------------------------------- |
| `core`  | DAG types, evaluation, debug display — pure Rust, no FFI   |
| `meta`  | Build-time metadata and helpers used by `build.rs` scripts |
| `node`  | Neon-based Node.js native addon that wraps `core`          |

## Build requirements

- [mise](https://mise.jdx.dev/getting-started.html) for tool
  version management
- C++ development toolchain (required by Rust)
  - Windows: [Build Tools for Visual Studio][vs-build-tools]
  - macOS: `xcode-select --install`
  - Linux: preinstalled `g++`

[vs-build-tools]: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2026

## Environment setup

```bash
# GitHub token avoids rate limits during mise tool installation
# https://github.com/settings/personal-access-tokens/new
[ -f .env ] || cp .env.example .env
# Edit .env and set GITHUB_TOKEN
```
