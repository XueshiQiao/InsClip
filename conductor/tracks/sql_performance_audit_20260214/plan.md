# Implementation Plan: SQL Performance Audit

## Phase 1: Discovery & Analysis
- [x] Task: Identify all SQL query locations in the codebase (`sqlx::query`, `query_as`, `execute`, etc.).
- [x] Task: Analyze each query for:
    -   Frequency (is it in a loop or hot path?)
    -   Redundancy (is it fetching data we already have?)
    -   Efficiency (are we fetching too much? indices?)
- [x] Task: Generate a detailed Markdown report categorized by severity (Critical, General, Negligible).

## Phase 2: Implementation (Later)
- [ ] Task: Refactor critical queries based on audit findings.
- [ ] Task: Implement caching where appropriate.
- [ ] Task: Add database indices if missing.
