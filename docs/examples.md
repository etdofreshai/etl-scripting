# ETL Canonical Examples

These examples are the first shared source of truth for the ETL v0 surface language.
They serve four roles:
- human-readable language examples
- parser fixtures
- type-check smoke tests
- future codegen and runtime smoke tests

## Style rules
- 4-space indentation
- one module per file
- lowercase dotted module paths
- lowercase snake_case names
- no alternative syntax variants

## Example index
1. `hello_world.etl` — minimal module and output
2. `simple_math.etl` — pure computation
3. `counter_loop.etl` — mutable state and repetition
4. `health_check.etl` — boolean logic and if/else
5. `damage_resolution.etl` — game-rule flavored calculation
6. `entity_state.etl` — record declarations and field access
7. `tick_update.etl` — deterministic step logic
8. `seeded_random.etl` — explicit deterministic RNG usage shape
9. `inventory_model.etl` — collections and state modeling
10. `event_dispatch.etl` — library-level event flow
