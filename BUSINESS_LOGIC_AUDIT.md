# Business Logic Audit

Audit date: 2025-11-21

This document tracks business logic that needs to be moved from faff-cli (Python) and the Python bindings into the Rust core.

---

## Architectural Note: Functional Core Pattern

The current architecture has managers that hold storage references and do both IO and logic. The bindings layer ends up doing orchestration (gathering context from workspace before calling manager methods).

A cleaner pattern would be **functional core, imperative shell**:

- **Pure functions** (`log_ops`, `plan_ops`, etc.) - Take data, return data. No IO, no async. All validation and transformation logic lives here. Trivially testable.
- **Workspace** - Does all IO and orchestration. Reads state, calls pure functions, writes state.
- **Bindings** - Thin wrappers that just call `workspace.method()`.

Example:
```rust
// Pure function - no IO
fn start_intent(log: Log, intent: Intent, start: DateTime, now: DateTime, ...) -> Result<Log>

// Workspace does IO + orchestration
impl Workspace {
    async fn start_intent(&self, intent: Intent, start_time: Option<DateTime>, ...) -> Result<()> {
        let log = self.read_log_or_empty(date).await?;
        let trackers = self.read_trackers(date).await?;
        let updated = log_ops::start_intent(log, intent, start, now, &trackers)?;
        self.write_log(&updated, &trackers).await
    }
}
```

This is a significant refactor tracked separately from the items below.

---

## Priority Legend

- **CRITICAL** - Data integrity risk, must fix
- **HIGH** - Significant logic duplication, should fix soon
- **MEDIUM** - Would improve consistency, fix when convenient
- **LOW** - Minor improvement, nice to have

---

## faff-cli (Python CLI)

### CRITICAL

#### [x] Timeline Validation (`start.py:114-155`) - DONE
The `--since` flag implementation contains overlap detection, future-time checks, and conflict validation. This is critical for data integrity - if validation only exists in CLI, other tools could corrupt the timeline.

**Moved to Rust:** `LogManager::start_intent()` now validates:
- Start time not in future
- No conflict with active session (must be after its start)
- No conflict with completed session (must be after its end)
- Auto-stops active session when starting new one

**Note:** Bindings still gather context (now, trackers) from workspace - see architectural note above.

---

### HIGH

#### [x] Duration Aggregations (`log.py:276-332`) - DONE
The `faff log summary` command calculates:
- Total recorded time
- Time aggregated by intent
- Time aggregated by tracker and tracker source
- Weighted mean reflection score

**Moved to Rust:** Added `Log::summary()` method that returns `LogSummary`:
- `total_minutes: i64` - Total time in minutes
- `by_intent: HashMap<String, i64>` - Minutes per intent alias
- `by_tracker: HashMap<String, i64>` - Minutes per tracker
- `by_tracker_source: HashMap<String, i64>` - Minutes per tracker source prefix
- `mean_reflection_score: Option<f64>` - Weighted mean (by duration)

CLI now just formats the summary dict from Rust.

---

#### [ ] Session Statistics (`intent.py:269-294`, `field.py:76-127`)
Intent and field list commands parse raw TOML files to build usage statistics:
- Counting sessions per intent ID
- Counting unique logs per intent
- Building field data with usage counts

**Target:** Add `LogManager::get_intent_usage_stats()` and enhance `get_field_usage_stats()` in Rust

---

#### [x] Active Session Duration (`session.py:106-111`, `main.py:334-342`) - DONE
Multiple places calculate active session duration by checking if end_time is None and using current time.

**Moved to Rust:** Added `Session::elapsed(now)` method for open sessions.
- `duration` property: for closed sessions (raises error if open)
- `elapsed(now)` method: for open sessions (raises error if closed)

CLI now uses the appropriate method based on session state.

---

#### [x] Stale Timesheet Detection (`main.py:385-404`) - ALREADY IN RUST
The status command finds timesheets where logs changed after compilation.

**Status:** Already implemented in Rust (`TimesheetManager::find_stale_timesheets`) and exposed in Python/WASM bindings. CLI already uses it.

---

#### [ ] Filter Operators (`filtering.py:222-249`)
Filter matching logic for session queries:
- Exact match (`=`) with string comparison
- Contains match (`~`) with case-insensitive substring
- Not equal (`!=`) operator

**Target:** Ensure `query::Filter` in Rust handles all operators. CLI should only parse filter strings, not implement matching.

---

### MEDIUM

#### [ ] Compilation Validation (`main.py:201-244`)
Before compiling timesheets, CLI checks:
- Whether log has unclosed sessions
- Which logs need compilation (batch mode)
- Building timesheet existence tuples

**Target:** Add `LogManager::can_compile()` or validation in `TimesheetManager::compile()` in Rust

---

#### [ ] Intent Deduplication (`start.py:187-211`)
When presenting intents to user, CLI deduplicates by alias and adds ID suffix for disambiguation.

**Target:** This might be acceptable as display logic, but could add `PlanManager::get_intents_for_display()` that returns pre-deduplicated list with disambiguation info.

---

#### [ ] Session Data Transformation (`session.py:96-128`)
Converting sessions to display dictionaries, extracting metadata, handling active session end times.

**Target:** Most of this is display formatting (acceptable in CLI), but duration calculation should use Rust.

---

#### [ ] Log Status Determination (`log.py:116-141`)
Determining if log is closed, calculating stats for display.

**Target:** Add `Log::is_closed()`, `Log::total_duration()`, `Log::reflection_stats()` in Rust (some may exist).

---

### LOW

#### [ ] Plan Validity Display (`plan.py:34-45`)
Determining valid_until display (infinity symbol for None).

**Assessment:** This is display formatting, acceptable in CLI.

---

#### [ ] Query Duration Conversion (`query.py:33-52`)
Converting duration seconds to timedelta after calling Rust.

**Assessment:** Acceptable - Rust returns seconds, Python converts to its native type.

---

---

## bindings-python (Rust Bindings)

### HIGH

#### [ ] LogManager Workspace Context Injection

Three methods in `log_manager.rs` follow this pattern:
```rust
let workspace = self.workspace.as_ref()?;
let current_date = workspace.today();
let current_time = workspace.now();
let trackers = rt.block_on(workspace.plans().get_trackers(current_date))?;
rt.block_on(self.inner.method(...))
```

**Affected methods:**
- `start_intent_now()` (lines 182-214)
- `start_intent_at()` (lines 219-254)
- `stop_current_session()` (lines 257-282)

**Target:** Add workspace-aware methods to Rust `LogManager`:
- `start_intent_now_with_workspace(&self, workspace: &Workspace, intent: Intent, note: Option<String>)`
- `start_intent_at_with_workspace(&self, workspace: &Workspace, intent: Intent, start_time: DateTime<Tz>, note: Option<String>)`
- `stop_current_session_with_workspace(&self, workspace: &Workspace)`

These methods handle fetching today/now/trackers internally.

---

#### [ ] TimesheetManager Workspace Context Injection

Similar pattern in `timesheet_manager.rs`:

**Affected methods:**
- `compile()` (lines 114-137) - fetches log_manager
- `sign_timesheet()` (lines 201-224) - fetches identity_manager
- `find_stale_timesheets()` (lines 140-164) - fetches log_manager
- `submit()` (lines 226-241) - fetches plugin_manager
- `audiences()` (lines 81-95) - fetches plugin_manager
- `get_audience()` (lines 97-109) - fetches plugin_manager

**Target:** Add workspace-aware methods to Rust `TimesheetManager`:
- `compile_with_workspace(&self, workspace: &Workspace, log: &Log, plugin: &dyn Plugin)`
- `sign_with_workspace(&self, workspace: &Workspace, timesheet: &Timesheet, signing_ids: &[String])`
- `find_stale_with_workspace(&self, workspace: &Workspace, date: Option<NaiveDate>)`
- `submit_with_workspace(&self, workspace: &Workspace, timesheet: &Timesheet)`

---

### MEDIUM

#### [ ] PlanManager Plugin Access (`plan_manager.rs:268-279`)

`remotes()` method fetches plugin_manager from workspace.

**Target:** Either:
1. Add `remotes_with_workspace()` to Rust `PlanManager`, or
2. Accept `plugin_manager` parameter directly

---

---

## Implementation Notes

### Workspace-Aware Pattern

The recommended pattern for workspace-aware methods in Rust:

```rust
impl LogManager {
    /// Start intent using workspace context for date, time, and trackers
    pub async fn start_intent_now_with_workspace(
        &self,
        workspace: &Workspace,
        intent: Intent,
        note: Option<String>,
    ) -> Result<()> {
        let current_date = workspace.today();
        let current_time = workspace.now();
        let trackers = workspace.plans().get_trackers(current_date).await?;

        self.start_intent_now(intent, note, current_date, current_time, &trackers).await
    }
}
```

This keeps the low-level methods available for testing and flexibility, while providing convenience methods that handle orchestration.

### Testing Strategy

1. Rust unit tests for new methods
2. Integration tests using mock workspace
3. Python binding tests remain thin (just verify delegation works)

---

## Progress Tracking

| Item | Status | PR/Commit |
|------|--------|-----------|
| Timeline validation | Done | |
| Duration aggregations | Done | |
| Session statistics | Not started | |
| Active session duration | Done | |
| Stale timesheet detection | Already in Rust | |
| Filter operators | Not started | |
| Compilation validation | Not started | |
| LogManager workspace injection | Not started | |
| TimesheetManager workspace injection | Not started | |
| PlanManager plugin access | Not started | |
