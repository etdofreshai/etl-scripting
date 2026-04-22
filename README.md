# ETL Scripting

ETL is a deterministic, data-centric, game-logic-first, general-purpose programming language.

This repository currently contains:
- v0 design and specification drafts
- canonical example ETL programs
- a bootstrap Rust workspace for the first Linux-native compiler/runtime path
- placeholder layout for future self-hosted ETL compiler components

## Current priorities
1. stabilize the ETL v0 spec
2. use the example programs as golden fixtures
3. build the bootstrap lexer and parser in Rust
4. grow toward a Linux-first CLI compiler/runtime
5. migrate compiler components into ETL over time

## Design principles
- minimal core language
- indentation-based syntax
- no explicit end blocks
- deterministic by default
- ECS-friendly through libraries/runtime, not syntax
- safe-first and bootstrap-friendly

## Repository layout
- `docs/spec/` — language and runtime specification drafts
- `docs/architecture/` — planning documents and implementation plans
- `examples/` — canonical ETL example programs
- `bootstrap/host/` — first Rust bootstrap compiler/runtime crate
- `compiler/` — future ETL-authored compiler implementation
- `runtime/` — runtime support code
- `tests/golden/` — future golden fixtures

## First targets
1. CLI/bootstrap workflow
2. Linux native
3. Windows native
4. broader native targets
5. WASM/web
6. mobile targets later
