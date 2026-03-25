# soroban_sdk_minor

Gas-efficiency and readability improvements for the Soroban SDK minor version bump.

## Motivation

The Soroban SDK minor version bump changes how host-function costs are metered.
Several patterns in the original contract were spending extra gas unnecessarily:

- Two-step `has` + `get` storage reads (two host calls instead of one).
- Inline `checked_mul / checked_div` fee arithmetic duplicated across `withdraw`.
- Inline `DataKey::Contribution(addr)` construction repeated at every call site.
- Inline deadline comparison logic duplicated in `contribute`, `withdraw`, and `refund`.

This module centralises those patterns into small, well-tested helpers.

## Public API

### Storage helpers

| Function | Storage tier | Description |
|---|---|---|
| `instance_get_or(env, key, default)` | Instance | Returns stored value or `default` in one call |
| `persistent_get_or(env, key, default)` | Persistent | Same for persistent storage |

### Arithmetic helpers

| Function | Description |
|---|---|
| `progress_bps(total_raised, goal)` | Progress toward goal in basis points (0–10 000), division-by-zero safe |
| `compute_fee(total, fee_bps)` | Platform fee amount; panics on overflow (unreachable for realistic amounts) |

### Deadline helpers

| Function | Description |
|---|---|
| `is_past_deadline(env)` | `true` when `ledger.timestamp > deadline` |
| `is_active_window(env)` | Inverse of `is_past_deadline` |

### Contributor helpers

| Function | Description |
|---|---|
| `get_contribution(env, contributor)` | Reads persistent contribution, returns 0 if absent |
| `set_contribution(env, contributor, amount, ttl_ledgers)` | Writes contribution and refreshes TTL atomically |

## Security Notes

- No new trust assumptions are introduced.
- All arithmetic uses `checked_*` or `saturating_*` operations.
- Mutating helpers (`set_contribution`) do not enforce auth — callers in `lib.rs` are responsible for calling `require_auth` before invoking them.
- `compute_fee` will panic on overflow, which is unreachable for token amounts within the `i128` range divided by 10 000.

## Test Coverage

Tests live in `soroban_sdk_minor_test.rs` and cover:

- `progress_bps`: zero goal, half, exact, over-goal cap, zero raised, one bps.
- `compute_fee`: zero bps, 100 %, 2.5 %, rounding, large amounts.
- `is_past_deadline` / `is_active_window`: no deadline set, future deadline, past deadline.
- `get_contribution` / `set_contribution`: absent key, set and read, overwrite, zero.
- `instance_get_or` / `persistent_get_or`: absent key, stored value.
