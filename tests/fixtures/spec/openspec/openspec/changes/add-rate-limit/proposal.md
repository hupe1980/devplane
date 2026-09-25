## Why

Requests are unbounded, and one client can starve the rest.

## What Changes

- Add a per-client limiter to the API.

## Impact

- Affected specs: api
- Affected code: src/api/limit.rs
