# SQL Performance Audit Report

**Date:** 2026-02-14
**Status:** Audit Complete

## Summary
The audit identified **2 Critical**, **3 General**, and **Several Negligible** issues.
The most significant finding is the recurring synchronous database query inside the high-frequency clipboard monitoring loop, which introduces unnecessary latency to every copy operation.

## Findings Table

| ID | Severity | Location | Description | Frequency | Recommendation |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **SQL-01** | **Critical** | `src-tauri/src/clipboard.rs:190` | Fetching `ignore_ghost_clips` setting from DB on every clipboard event. | High (Every Copy) | Cache this setting in memory (e.g., `Arc<AtomicBool>`) and update it only when settings change. |
| **SQL-02** | **Critical** | `src-tauri/src/commands.rs:655-802` | `save_settings` executes ~20 separate `INSERT OR REPLACE` queries sequentially. | Medium (On Save) | Use a single transaction or batch the writes into fewer queries. |
| **SQL-03** | **General** | `src-tauri/src/commands.rs:532-622` | `get_settings` executes ~20 separate `SELECT` queries to fetch keys one by one. | Medium (On Load) | Replace with `SELECT key, value FROM settings` and parse into a Map/Struct in Rust. |
| **SQL-04** | **General** | `src-tauri/src/commands.rs:425` | `search_clips` uses `LIKE %...%` on `content` and `text_preview`. | User Driven | Optimization: Ensure `text_preview` is indexed or consider FTS5 if full-text performance degrades. |
| **SQL-05** | **General** | `src-tauri/src/commands.rs:105` | `get_clips` fetches `SELECT *` including potentially large `content` blobs for the list view. | High (UI Refresh) | Select only necessary columns (`uuid`, `text_preview`, `created_at`) for the list, fetch full content only on detail view. |
| **SQL-06** | **Negligible** | `src-tauri/src/database.rs` | Initialization queries (`CREATE TABLE`, etc.). | Once (Startup) | None. |

## Detailed Analysis

### SQL-01: Hot Path Database Access (Clipboard)
The clipboard monitor blocks on an async SQL query (`SELECT value FROM settings...`) every time the system clipboard changes. This adds I/O latency to the clipboard chain, which can cause "stutter" in other applications or missing events if the DB is busy.
**Fix:** Introduce a `SettingsState` struct managed by Tauri, load values on startup, and read from memory in the loop.

### SQL-02/03: N+1 Query Problem (Settings)
The settings logic treats SQLite like a Key-Value store but without the benefits of caching or batching. Loading settings involves 20+ roundtrips. Saving does the same.
**Fix:**
-   **Read:** `SELECT key, value FROM settings` (1 query).
-   **Write:** `BEGIN TRANSACTION; ... updates ...; COMMIT;` (1 transaction, multiple rapid writes).

### SQL-05: Over-fetching
The main list view fetches the full `content` (which might be a large image or long text) but likely only displays `text_preview` or `source_icon`.
**Fix:** Create a `ClipSummary` struct that excludes the heavy `content` field and select only those columns.
