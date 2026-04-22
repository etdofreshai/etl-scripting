# ETL Roadmap

## Phase 0 — Specs and examples
- finalize language overview
- finalize lexical rules
- finalize grammar
- finalize determinism contract
- finalize core vs stdlib boundary
- stabilize 10 canonical examples

## Phase 1 — Bootstrap frontend
- create Rust workspace and CLI
- implement lexer
- implement parser
- define AST and diagnostics
- add parser golden tests

## Phase 2 — Semantic checks
- name resolution
- symbol tables
- type checking
- valid/invalid fixtures

## Phase 3 — IR and code generation
- typed linear IR
- AST lowering
- Linux x86_64 first backend
- assembly/debug output

## Phase 4 — Self-hosting preparation
- define V-zero boundary
- add ETL-authored compiler module placeholders
- compare bootstrap output with ETL-authored passes over time

## Phase 5 — Broader targets
- Windows native
- WASM/web
- additional native targets
- mobile strategy
