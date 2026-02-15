# Implementation Plan: SQL Performance Audit

## Phase 1: Discovery & Analysis
- [x] Task: Identify all SQL query locations in the codebase (`sqlx::query`, `query_as`, `execute`, etc.).
- [x] Task: Analyze each query for frequency and redundancy.
- [x] Task: Generate a detailed Markdown report.

## Phase 2: Implementation (Critical Path)
- [ ] Task: **SQL-01 Fix:** Cache `ignore_ghost_clips` in memory to eliminate DB query in hot clipboard loop.
  -   *Strategy:* Use a dedicated `SettingsCache` struct (AtomicBool) managed by Tauri state.
  -   *Scope:* `lib.rs` (init), `clipboard.rs` (read), `commands.rs` (update).

## Phase 3: Implementation (General Optimization)
- [ ] Task: **SQL-02/03 Fix:** Refactor Settings Storage to use JSON file (`SettingsManager`).
  -   *Goal:* Replace N+1 SQL queries with single file read/write.
  -   *Strict Constraint:* Do NOT modify `get_clips` or other unrelated commands. Only replace `get_settings`, `save_settings`, and ignored app commands.
- [ ] Task: Migration logic (SQLite -> JSON).
