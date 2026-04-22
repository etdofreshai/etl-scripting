# ETL v0 Determinism Contract

ETL is deterministic by default.

## Default constraints
- no hidden randomness
- no hidden wall-clock access
- deterministic collection ordering
- no concurrency in v0
- float allowed, but gameplay-critical deterministic logic should prefer integer or future fixed-point helpers
