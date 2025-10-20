"""
Type stubs for faff_core - Python bindings to Rust core library.

This file provides type hints for IDE autocomplete and static type checking.
"""

from __future__ import annotations
from typing import Optional, List, Dict
from zoneinfo import ZoneInfo
import datetime

# Models submodule
class models:
    """Core data models for time tracking."""

    class Intent:
        """
        Intent represents what you're doing, classified semantically.

        Most fields are optional except trackers which defaults to empty list.
        If alias is not provided, it's auto-generated.
        """
        alias: Optional[str]
        role: Optional[str]
        objective: Optional[str]
        action: Optional[str]
        subject: Optional[str]
        trackers: List[str]

        def __init__(
            self,
            alias: Optional[str] = None,
            role: Optional[str] = None,
            objective: Optional[str] = None,
            action: Optional[str] = None,
            subject: Optional[str] = None,
            trackers: List[str] = []
        ) -> None: ...

        def as_dict(self) -> Dict: ...
        def __hash__(self) -> int: ...
        def __eq__(self, other: object) -> bool: ...
        def __ne__(self, other: object) -> bool: ...
        def __repr__(self) -> str: ...
        def __str__(self) -> str: ...

    class Session:
        """
        A work session with start/end times and intent classification.

        Sessions are immutable - operations return new instances.
        """
        intent: models.Intent
        start: datetime.datetime
        end: Optional[datetime.datetime]
        note: Optional[str]

        def __init__(
            self,
            intent: models.Intent,
            start: datetime.datetime,
            end: Optional[datetime.datetime] = None,
            note: Optional[str] = None
        ) -> None: ...

        @property
        def duration(self) -> datetime.timedelta:
            """
            Calculate duration of this session.

            Raises:
                ValueError: If session has no end time or end is before start.
            """
            ...

        @classmethod
        def from_dict_with_tz(
            cls,
            data: Dict,
            date: datetime.date,
            timezone: ZoneInfo
        ) -> models.Session:
            """
            Create a session from a dictionary with timezone context.

            Args:
                data: Dictionary with session fields
                date: The date this session occurred
                timezone: Timezone for interpreting times

            Returns:
                New Session instance
            """
            ...

        def with_end(self, end: datetime.datetime) -> models.Session:
            """Return a new session with the specified end time."""
            ...

        def as_dict(self) -> Dict:
            """Convert to dictionary representation."""
            ...

        def __eq__(self, other: object) -> bool: ...
        def __ne__(self, other: object) -> bool: ...
        def __repr__(self) -> str: ...
        def __str__(self) -> str: ...

    class Log:
        """
        A log represents one day of work with multiple sessions.

        Logs are immutable - operations return new instances.
        """
        date: datetime.date
        timezone: ZoneInfo
        timeline: List[models.Session]

        def __init__(
            self,
            date: datetime.date,
            timezone: ZoneInfo,
            timeline: Optional[List[models.Session]] = None
        ) -> None: ...

        @classmethod
        def from_dict(cls, data: Dict) -> models.Log:
            """Parse a log from dictionary (e.g., from JSON)."""
            ...

        def append_session(self, session: models.Session) -> models.Log:
            """
            Append a session, automatically stopping any active session.

            Returns:
                New Log instance with the session added.
            """
            ...

        def active_session(self) -> Optional[models.Session]:
            """Return the currently active (open) session, if any."""
            ...

        def stop_active_session(self, stop_time: datetime.datetime) -> models.Log:
            """
            Stop the active session at the given time.

            Raises:
                ValueError: If no active session exists.

            Returns:
                New Log instance with the session stopped.
            """
            ...

        def is_closed(self) -> bool:
            """Check if all sessions in this log are closed (have end times)."""
            ...

        def total_recorded_time(self) -> datetime.timedelta:
            """
            Calculate total recorded time across all sessions.

            For open sessions on today, uses current time.
            For open sessions on past dates, uses end of day.
            """
            ...

        def __repr__(self) -> str: ...
        def __str__(self) -> str: ...

    class Plan:
        """
        A plan defines vocabulary and templates for work tracking.

        Plans can be local files or fetched from remote sources.
        """
        source: str
        valid_from: datetime.date
        valid_until: Optional[datetime.date]
        roles: List[str]
        actions: List[str]
        objectives: List[str]
        subjects: List[str]
        trackers: Dict[str, str]
        intents: List[models.Intent]

        def __init__(
            self,
            source: str,
            valid_from: datetime.date,
            valid_until: Optional[datetime.date] = None,
            roles: Optional[List[str]] = None,
            actions: Optional[List[str]] = None,
            objectives: Optional[List[str]] = None,
            subjects: Optional[List[str]] = None,
            trackers: Optional[Dict[str, str]] = None,
            intents: Optional[List[models.Intent]] = None
        ) -> None: ...

        @classmethod
        def from_dict(cls, data: Dict) -> models.Plan:
            """Parse a plan from dictionary (e.g., from TOML/JSON)."""
            ...

        def id(self) -> str:
            """Generate a slug ID from the source."""
            ...

        def add_intent(self, intent: models.Intent) -> models.Plan:
            """
            Add an intent to the plan (deduplicating if already present).

            Returns:
                New Plan instance with the intent added.
            """
            ...

        def as_dict(self) -> Dict:
            """Convert to dictionary representation."""
            ...

        def __repr__(self) -> str: ...
        def __str__(self) -> str: ...

    class TimesheetMeta:
        """
        Metadata about a timesheet (not included in signed content).

        This is stored separately to preserve cryptographic integrity.
        """
        audience_id: str
        submitted_at: Optional[datetime.datetime]
        submitted_by: Optional[str]

        def __init__(
            self,
            audience_id: str,
            submitted_at: Optional[datetime.datetime] = None,
            submitted_by: Optional[str] = None
        ) -> None: ...

        @classmethod
        def from_dict(cls, data: Dict) -> models.TimesheetMeta: ...

    class Timesheet:
        """
        A cryptographically signed, immutable record of work for external submission.

        Timesheets are compiled from logs by Audience plugins.
        """
        actor: Dict[str, str]
        date: datetime.date
        compiled: datetime.datetime
        timezone: ZoneInfo
        timeline: List[models.Session]
        signatures: Dict[str, Dict[str, str]]
        meta: models.TimesheetMeta

        def __init__(
            self,
            actor: Optional[Dict[str, str]],
            date: datetime.date,
            compiled: datetime.datetime,
            timezone: ZoneInfo,
            timeline: List[models.Session],
            signatures: Optional[Dict[str, Dict[str, str]]],
            meta: models.TimesheetMeta
        ) -> None: ...

        def sign(self, id: str, signing_key: bytes) -> models.Timesheet:
            """
            Sign the timesheet with an Ed25519 key.

            Args:
                id: Signer identifier (e.g., email address)
                signing_key: 32-byte Ed25519 private key

            Returns:
                New Timesheet instance with signature added.
            """
            ...

        def update_meta(
            self,
            audience_id: str,
            submitted_at: Optional[datetime.datetime] = None,
            submitted_by: Optional[str] = None
        ) -> models.Timesheet:
            """
            Update metadata (returns new instance).

            Note: Metadata is not part of signed content.
            """
            ...

        def submittable_timesheet(self) -> models.SubmittableTimesheet:
            """Convert to submittable format (without metadata)."""
            ...

        @classmethod
        def from_dict(cls, data: Dict) -> models.Timesheet:
            """Parse a timesheet from dictionary (e.g., from JSON)."""
            ...

    class SubmittableTimesheet:
        """
        Timesheet in canonical form for submission/verification.

        Contains all signed fields but no metadata.
        """
        actor: Dict[str, str]
        date: datetime.date
        compiled: datetime.datetime
        timezone: ZoneInfo
        timeline: List[models.Session]
        signatures: Dict[str, Dict[str, str]]

        def canonical_form(self) -> bytes:
            """
            Serialize to canonical JSON for signature verification.

            Returns:
                Canonical JSON bytes (sorted keys, no whitespace).
            """
            ...

    class Config:
        """Application configuration."""
        timezone: ZoneInfo
        plan_remotes: List[models.PlanRemote]
        audiences: List[models.TimesheetAudience]
        roles: List[models.Role]

        @classmethod
        def from_dict(cls, data: Dict) -> models.Config: ...

        def __repr__(self) -> str: ...

    class PlanRemote:
        """Configuration for a remote plan source."""
        name: str
        plugin: str
        config: Dict
        defaults: Dict

        @property
        def name(self) -> str: ...
        @property
        def plugin(self) -> str: ...
        @property
        def config(self) -> Dict: ...
        @property
        def defaults(self) -> Dict: ...

        def __repr__(self) -> str: ...

    class TimesheetAudience:
        """Configuration for a timesheet audience."""
        name: str
        plugin: str
        config: Dict
        signing_ids: List[str]

        @property
        def name(self) -> str: ...
        @property
        def plugin(self) -> str: ...
        @property
        def config(self) -> Dict: ...
        @property
        def signing_ids(self) -> List[str]: ...

        def __repr__(self) -> str: ...

    class Role:
        """Configuration for a user role."""
        name: str
        config: Dict

        @property
        def name(self) -> str: ...
        @property
        def config(self) -> Dict: ...

        def __repr__(self) -> str: ...

# Manager classes
class TimesheetManager:
    """Manager for timesheet operations."""

    def write_timesheet(self, timesheet: models.Timesheet) -> None:
        """Write a timesheet to storage."""
        ...

    def get_timesheet(
        self,
        audience_id: str,
        date: datetime.date
    ) -> Optional[models.Timesheet]:
        """Get a timesheet for a specific audience and date."""
        ...

    def list_timesheets(
        self,
        date: Optional[datetime.date] = None
    ) -> List[models.Timesheet]:
        """
        List all timesheets, optionally filtered by date.

        Args:
            date: Optional date to filter by. If None, returns all timesheets.

        Returns:
            List of Timesheet instances.
        """
        ...

    def list(
        self,
        date: Optional[datetime.date] = None
    ) -> List[models.Timesheet]:
        """
        Alias for list_timesheets (backwards compatibility).

        Args:
            date: Optional date to filter by. If None, returns all timesheets.

        Returns:
            List of Timesheet instances.
        """
        ...

    def audiences(self) -> List: ...

    def get_audience(self, audience_id: str) -> Optional: ...

    def submit(self, timesheet: models.Timesheet) -> None:
        """Submit a timesheet via its audience plugin."""
        ...

class Workspace:
    """
    Workspace manager for accessing logs, plans, and configuration.

    This is the main entry point for interacting with a Faff workspace.
    """

    timesheets: TimesheetManager

    def __init__(self, root_path: str) -> None:
        """
        Initialize workspace at the given root path.

        Args:
            root_path: Path to workspace root (containing .faff/ directory)
        """
        ...

    def get_log(self, date: datetime.date) -> models.Log:
        """
        Load a log for the specified date.

        Args:
            date: Date to load log for

        Returns:
            Log instance (may be empty if file doesn't exist)
        """
        ...

    def save_log(self, log: models.Log) -> None:
        """
        Save a log to disk.

        Args:
            log: Log instance to save
        """
        ...

    def get_plan(self, plan_id: str) -> models.Plan:
        """
        Load a plan by ID.

        Args:
            plan_id: Plan identifier (slug)

        Returns:
            Plan instance

        Raises:
            FileNotFoundError: If plan doesn't exist
        """
        ...

    def save_plan(self, plan: models.Plan) -> None:
        """Save a plan to disk."""
        ...

    def list_plans(self) -> List[str]:
        """List all available plan IDs."""
        ...

    def get_config(self) -> models.Config:
        """Load workspace configuration."""
        ...

    def save_config(self, config: models.Config) -> None:
        """Save workspace configuration."""
        ...
