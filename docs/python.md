# faff-core Python SDK

The Python SDK is a compiled Rust extension (`faff_core`) that gives Python code full access to faff's data and operations. It is the primary way to build faff plugins and tooling.

## Installation

```bash
pip install faff-core
```

## Quick start

```python
import faff_core

ws = faff_core.Workspace()
today = ws.today()
log = ws.logs.get_log(today)

print(log)  # Log(date=2026-03-21, timezone=Europe/London, timeline=[3 sessions])
```

---

## `Workspace`

The entry point for all SDK operations.

```python
ws = faff_core.Workspace()
ws = faff_core.Workspace(storage=my_storage)  # custom storage backend
```

`Workspace()` reads the faff repository from `~/.faff` (or `$FAFF_DIR`). Raises `UninitializedLedgerError` if no faff directory is found.

### Properties

| Property | Type | Description |
|---|---|---|
| `ws.plans` | `PlanManager` | Access to plans |
| `ws.logs` | `LogManager` | Access to daily logs |
| `ws.timesheets` | `TimesheetManager` | Access to timesheets |
| `ws.identities` | `IdentityManager` | Access to signing identities |
| `ws.plugins` | `PluginManager` | Access to loaded plugins |

### Methods

```python
ws.now()       # -> datetime  current time in configured timezone
ws.today()     # -> date      today's date
ws.timezone()  # -> ZoneInfo  configured timezone

ws.config()    # -> Config    workspace configuration

# Natural language date/time parsing
ws.parse_natural_date("yesterday")       # -> date
ws.parse_natural_date("last monday")     # -> date
ws.parse_natural_date("2026-01-15")      # -> date
ws.parse_natural_date(None)              # -> today

ws.parse_natural_datetime("09:30")       # -> datetime (must be today)
ws.parse_natural_datetime("2 hours ago") # -> datetime
ws.parse_natural_datetime(None)          # -> now
# Raises ValueError if the parsed time is not today
```

---

## Models

### `Session`

A single tracked activity.

```python
from faff_core.models import Session
import datetime

session = Session(
    start=some_datetime,
    title="Sprint planning",   # optional
    role="engineer",           # optional
    impact="delivery",         # optional
    mode="sync",               # optional
    subject="project-x",       # optional
    trackers=["PROJ-123"],     # optional, list of tracker IDs
    end=some_other_datetime,   # optional, None = session still active
    note="went well",          # optional
)
```

**Attributes** (all read-only):

| Attribute | Type | Description |
|---|---|---|
| `title` | `str \| None` | |
| `role` | `str \| None` | |
| `impact` | `str \| None` | |
| `mode` | `str \| None` | |
| `subject` | `str \| None` | |
| `trackers` | `list[str]` | Tracker IDs |
| `start` | `datetime` | Timezone-aware |
| `end` | `datetime \| None` | None if session is still active |
| `note` | `str \| None` | |
| `reflection_score` | `int \| None` | |
| `reflection` | `str \| None` | |
| `duration` | `timedelta` | Raises `ValueError` if session has no end |

**Methods**:

```python
session.elapsed(now)            # -> timedelta  time since start (open sessions only)
session.with_end(dt)            # -> Session    new Session with end set
session.with_reflection(score, text)  # -> Session  new Session with reflection fields
session.as_dict()               # -> dict
Session.from_dict_with_tz(d, date, tz)  # classmethod
```

---

### `Log`

A day's worth of sessions.

```python
from faff_core.models import Log

log = Log(date=some_date, timezone=some_zoneinfo)
log = Log(date=some_date, timezone=some_zoneinfo, timeline=[session1, session2])
```

**Attributes**:

| Attribute | Type | Description |
|---|---|---|
| `date` | `date` | |
| `timezone` | `ZoneInfo` | |
| `timeline` | `list[Session]` | Ordered list of sessions, oldest first |

**Methods**:

```python
log.active_session()              # -> Session | None   the open session, if any
log.is_closed()                   # -> bool             True if all sessions have an end
log.total_recorded_time()         # -> timedelta
log.append_session(session)       # -> Log   new Log with session appended
log.stop_active_session(end_time) # -> Log   new Log with active session closed
log.summary(now)                  # -> dict  see below
log.hash(trackers)                # -> str   content hash (trackers: dict[str, str])
log.to_log_file(trackers)         # -> str   TOML serialisation

Log.from_dict(d)        # classmethod
Log.calculate_hash(toml_content)  # staticmethod
```

**`log.summary(now)` returns**:

```python
{
    "total_minutes": float,
    "by_title":  dict[str, float],   # title -> minutes
    "by_tracker": dict[str, float],  # tracker_id -> minutes
    "by_tracker_source": dict[str, float],
    "mean_reflection_score": float | None,
}
```

---

### `Plan`

The vocabulary and trackers available for a date range.

```python
from faff_core.models import Plan
import datetime

plan = Plan(
    source="local",
    valid_from=datetime.date(2026, 1, 1),
    valid_until=None,           # optional, open-ended if None
    roles=["engineer", "lead"],
    impacts=["delivery", "quality"],
    modes=["sync", "async"],
    subjects=["project-x"],
    trackers={"PROJ-123": "Add login page"},  # id -> description
)
```

**Attributes** (all read-only):

| Attribute | Type |
|---|---|
| `source` | `str` |
| `valid_from` | `date` |
| `valid_until` | `date \| None` |
| `roles` | `list[str]` |
| `impacts` | `list[str]` |
| `modes` | `list[str]` |
| `subjects` | `list[str]` |
| `trackers` | `dict[str, str]` |
| `hints` | `list[dict]` |

**Methods**:

```python
plan.id()           # -> str   unique identifier for this plan
plan.to_toml()      # -> str
plan.as_dict()      # -> dict
plan.with_hints(hints)   # -> Plan  new Plan with hints attached
Plan.from_dict(d)   # classmethod
```

Each hint dict has: `title` (str), `role`, `subject`, `impact`, `mode` (all `str | None`), `trackers` (list[str]).

---

### `Timesheet`

A compiled, signed record of a day's sessions for submission to an audience.

Timesheets are produced by `TimesheetManager.compile()` and should not be constructed directly.

**Attributes**:

| Attribute | Type |
|---|---|
| `date` | `date` |
| `timeline` | `list[Session]` |
| `actor` | `dict[str, str]` |
| `version` | `str` |

---

## Managers

Managers are accessed via `Workspace` properties, not instantiated directly.

---

### `LogManager`

`ws.logs`

```python
# Check existence
ws.logs.log_exists(date)            # -> bool
ws.logs.log_file_path(date)         # -> str  absolute path

# Read
ws.logs.get_log(date)               # -> Log  (empty Log if file doesn't exist)
ws.logs.list_log_dates()            # -> list[date]
ws.logs.list_logs()                 # -> list[Log]
ws.logs.list_logs_recent(n)         # -> list[Log]  n most recent, oldest-first

# Write
ws.logs.write_log(log, trackers)    # trackers: dict[str, str]
ws.logs.delete_log(date)

# Raw file access
ws.logs.read_log_raw(date)          # -> str
ws.logs.write_log_raw(date, text)

# Session lifecycle
ws.logs.start_session(
    title=None, role=None, impact=None, mode=None, subject=None,
    trackers=[],
    start_time=None,   # defaults to now
    note=None,
)
ws.logs.stop_current_session()

# Bulk operations
ws.logs.replace_field_in_all_logs(field, old_value, new_value, trackers)
# -> (logs_updated: int, sessions_updated: int)

ws.logs.get_field_usage_stats(field)
# -> (dict[str, int], dict[str, list[date]])
# (value -> session count, value -> list of log dates)

ws.logs.timezone()  # -> ZoneInfo
```

`start_session` stops any currently active session before starting the new one. Raises `ValueError` if `start_time` is in the future or conflicts with existing sessions.

---

### `PlanManager`

`ws.plans`

```python
# Get plans for a date
ws.plans.get_plans(date)              # -> dict[str, Plan]  source -> Plan
ws.plans.get_local_plan(date)         # -> Plan | None
ws.plans.get_local_plan_or_create(date)  # -> Plan  (creates empty if missing)
ws.plans.get_plan_by_tracker_id(tracker_id, date)  # -> Plan | None

# Get vocabulary lists for a date (merged across all valid plans)
ws.plans.get_roles(date)      # -> list[str]
ws.plans.get_impacts(date)    # -> list[str]
ws.plans.get_modes(date)      # -> list[str]
ws.plans.get_subjects(date)   # -> list[str]
ws.plans.get_trackers(date)   # -> dict[str, str]  id -> description

# Hints and mappings
ws.plans.get_session_hints(date)     # -> list[dict]
ws.plans.get_tracker_mappings(date)  # -> list[dict]

# All plan data needed at session-start time in a single call
ws.plans.get_start_data(date)
# -> dict with keys: roles, impacts, modes, subjects, trackers, hints, tracker_mappings

# Write
ws.plans.write_plan(plan)

# Bulk operations
ws.plans.replace_field_in_all_plans(field, old_value, new_value)  # -> int (plans updated)
ws.plans.get_field_usage_stats(field)  # -> dict[str, int]  value -> count

# Remote plugin instances
ws.plans.remotes()  # -> list[PlanSource plugin instances]
```

`get_session_hints` returns `list[dict]` where each dict has: `title`, `role`, `subject`, `impact`, `mode` (`str | None`), `trackers` (`list[str]`).

`get_tracker_mappings` returns `list[dict]` where each dict has: `tracker_id`, `tracker_name`, `hint_title`, `role`, `subject`, `impact`, `mode` (last four are `str | None`).

---

### `TimesheetManager`

`ws.timesheets`

```python
# Compile and submit
ws.timesheets.compile(log, plugin)   # -> Timesheet
ws.timesheets.submit(timesheet)

# Sign
ws.timesheets.sign_timesheet(timesheet, signing_ids)  # -> Timesheet
# signing_ids: list[str] - identity names to sign with

# Persistence
ws.timesheets.write_timesheet(timesheet)
ws.timesheets.get_timesheet(audience_id, date)    # -> Timesheet | None
ws.timesheets.list_timesheets(date=None)          # -> list[Timesheet]
ws.timesheets.delete_timesheet(audience_id, date)

# Status checks
ws.timesheets.find_stale_timesheets(date=None)     # -> list[Timesheet]
ws.timesheets.find_failed_submissions(date=None)   # -> list[Timesheet]

# Audience plugins
ws.timesheets.audiences()                          # -> list[Audience plugin instances]
ws.timesheets.get_audience(audience_id)            # -> Audience | None
```

A stale timesheet is one where the source log has changed since the timesheet was compiled.

---

### `IdentityManager`

`ws.identities`

Ed25519 key pairs used for signing timesheets. Keys are stored in `~/.faff/identities/`.

```python
ws.identities.create_identity(name, overwrite=False)
ws.identities.load_identity(name)    # signing key
ws.identities.verify_identity(name)  # verification key
```

---

## Querying

`faff_core.Filter` and `faff_core.query_sessions` let you aggregate session time across logs.

```python
from faff_core import Filter, query_sessions

# Parse a filter from a string
f = Filter.parse("role=engineer")
f = Filter.parse("impact~delivery")   # contains (case-insensitive)
f = Filter.parse("mode!=async")

# Filter fields: title, role, impact, mode, subject, note
# Operators: = (equals), ~ (contains), != (not equals)

f.field()     # -> str  e.g. "role"
f.operator()  # -> str  e.g. "="
f.value()     # -> str  e.g. "engineer"

# Aggregate session durations across a list of logs
results = query_sessions(
    logs=ws.logs.list_logs(),
    filters=[Filter.parse("role=engineer")],
    from_date=None,   # optional date
    to_date=None,     # optional date
)
# results: dict[tuple[str, ...], int]
# keys are tuples of filter field values, values are durations in seconds
```

---

## Event watching

Watch the faff repository for file changes.

```python
from faff_core import start_watching

stream = start_watching("~/.faff")

for event in stream:          # blocks until events arrive
    print(event.event_type)   # "log_changed" or "plan_changed"
    print(event.path)         # absolute path of changed file
```

`EventStream` is a blocking iterator that releases the GIL between events and respects `KeyboardInterrupt`. If events are produced faster than they are consumed it raises `StopIteration` with a "lagged" message.

---

## Writing plugins

Plugins live in `~/.faff/plugins/<name>/plugin/plugin.py` and subclass one of the base classes from `faff_core.plugins`.

### `PlanSource`

Provides a plan from an external system (e.g. Jira, Linear).

```python
from faff_core.plugins import PlanSource
from faff_core.models import Plan
import datetime

class MyPlanSource(PlanSource):
    def pull_plan(self, date: datetime.date) -> Plan:
        # fetch from external system using self.config
        return Plan(
            source=self.name,
            valid_from=date,
            trackers={"PROJ-1": "Fix the bug"},
        )
```

### `Audience`

Compiles and submits timesheets to an external system (e.g. Harvest, Clockify).

```python
from faff_core.plugins import Audience
from faff_core.models import Log, Timesheet

class MyAudience(Audience):
    def compile_time_sheet(self, log: Log) -> Timesheet:
        # filter log sessions and build a Timesheet
        ...

    def submit_timesheet(self, timesheet: Timesheet) -> None:
        # send to external system using self.config
        ...
```

### `Plugin` base class

Both `PlanSource` and `Audience` inherit from `Plugin`, which provides:

| Attribute | Type | Description |
|---|---|---|
| `self.plugin` | `str` | Plugin type/class name |
| `self.name` | `str` | Instance name |
| `self.id` | `str` | Slugified instance name |
| `self.slug` | `str` | Same as `id` |
| `self.config` | `dict` | Plugin-specific configuration |
| `self.defaults` | `dict` | Default values |
| `self.state_path` | `Path` | Directory for persistent state |

---

## Errors

| Exception | When raised |
|---|---|
| `faff_core.UninitializedLedgerError` | No faff directory found at startup |
| `ValueError` | Invalid dates, times, or field values |
| `FileNotFoundError` | Log file does not exist |
| `RuntimeError` | Internal errors (plugin loading, etc.) |
