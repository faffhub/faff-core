# Concurrent Sessions

## Background

Sessions were previously assumed to be strictly sequential — only one session could be active at
a time, and starting a new session would automatically close the previous one. This design was
fine for simple time-tracking but prevented legitimate concurrent work (e.g. a meeting happening
during a coding block, or two tracked projects genuinely running in parallel).

## Design Goals

- The core library removes the sequential assumption entirely
- Sequential use remains the common case and stays simple at the CLI level
- Compilation plugins receive enough context to apply their own strategy for overlapping time
  (count both in full, split evenly, take the primary session, etc.)
- Log files stay human-readable and hand-editable

## Session IDs

Sessions gain an optional `id` field — a SHA256 hex string written by the software, omitted or
hand-deletable by the user.

**Computation:** SHA256 of a newline-delimited string of the session's creation-time fields in
fixed order: `start` (UTC unix timestamp), `title`, `role`, `impact`, `mode`, `subject`.
Absent optional fields contribute an empty string. Trackers and note are excluded (can change
post-creation).

**Semi-optional:** always written on save, never required on read. If absent, `session.id()`
falls back to computing from current fields. If present, the stored value is authoritative (stable
through edits to excluded fields).

**Prefix addressing:** the full 64-char hex is stored; the CLI and any UI can address sessions
using a unique prefix, exactly like `git` and `docker`.

**Position in TOML:** `id` is written as the first field of each `[[session]]` block.

## `Log` API Changes

### Removed
- `active_session() -> Option<&Session>` — replaced by `active_sessions()`
- `append_session()` — replaced by `start_session()` (no auto-close behaviour)
- `stop_active_session()` — removed entirely; inference about which session to stop was surprising

### Added / Changed
- `active_sessions() -> Vec<&Session>` — all sessions with no end time
- `start_session(session) -> Log` — appends a session, no inference, no auto-closing; caller
  decides what to close beforehand
- `stop_session(id_prefix, stop_time) -> Result<Log, LogError>` — closes the open session
  whose id matches the given prefix
- `stop_all_active_sessions(stop_time) -> Result<Log, LogError>` — closes every open session;
  used by `faff stop` (no args) and midnight continuation
- `session_overlaps() -> Vec<(&Session, &Session)>` — all pairwise combinations of sessions
  whose time ranges overlap; A/B/C all overlapping yields AB, AC, BC
- `has_concurrent_sessions() -> bool` — convenience wrapper around `session_overlaps()`
- `LogSummary.has_concurrent_sessions: bool` — set in `summary()`; `total_minutes` remains the
  raw sum of all individual session durations (double-counting overlaps); plugins resolve as
  they see fit
- `LogError::NoActiveSession` — returned by `stop_session` when no matching open session found

## `LogManager` API Changes

- `start_session()` — drops auto-close logic; validates and appends only
- `stop_session(id_prefix)` — stops a specific session by id prefix
- `stop_all_active_sessions()` — stops all open sessions; what `faff stop` calls
- `materialize_continuation()` — no longer takes a session argument; calls
  `yesterday_log.active_sessions()` itself, closes all via `stop_all_active_sessions`, and
  creates a continuation session for each starting at 00:00 (today may start with concurrent
  sessions if yesterday ended that way)

## Python / WASM Bindings

All new and changed methods are exposed. `summary()` dict gains `has_concurrent_sessions`.
