"""
Type stubs for faff_core - Python bindings to Rust core library.

This file provides type hints for IDE autocomplete and static type checking.
"""

from __future__ import annotations
from typing import Optional, List, Dict
from zoneinfo import ZoneInfo
import datetime

def version() -> str:
    """Get the version of the faff-core library."""
    ...

# Models submodule
class models:
    """Core data models for time tracking."""

    class Intent:
        """
        Intent represents what you're doing, classified semantically.

        Most fields are optional except trackers which defaults to empty list.
        If alias is not provided, it's auto-generated.
        If intent_id is not provided, it's auto-generated with the current date.
        """
        intent_id: str
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
            trackers: List[str] = [],
            intent_id: Optional[str] = None
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
        reflection_score: Optional[int]
        reflection: Optional[str]

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

        def to_log_file(self, trackers: Dict[str, str]) -> str:
            """
            Format the log as a TOML string for saving to file.

            Args:
                trackers: Dictionary mapping tracker IDs to human-readable names

            Returns:
                Formatted TOML string
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
        log_hash: Optional[str]
        submission_status: Optional[str]  # "success", "failed", or "partial"
        submission_error: Optional[str]

        def __init__(
            self,
            audience_id: str,
            log_hash: str,
            submitted_at: Optional[datetime.datetime] = None
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
            submitted_at: Optional[datetime.datetime] = None
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
        
        @property
        def name(self) -> str: ...
        @property
        def config(self) -> Dict: ...

        def __repr__(self) -> str: ...

# Manager classes
class LogManager:
    """Manager for log file operations."""

    def log_exists(self, date: datetime.date) -> bool:
        """Check if a log exists for the given date."""
        ...

    def read_log_raw(self, date: datetime.date) -> str:
        """Read raw log file contents."""
        ...

    def write_log_raw(self, date: datetime.date, contents: str) -> None:
        """Write raw log file contents."""
        ...

    def list_log_dates(self) -> List[datetime.date]:
        """List all log dates."""
        ...

    def list(self) -> List[models.Log]:
        """List all logs (returns Log objects)."""
        ...

    def delete_log(self, date: datetime.date) -> None:
        """Delete a log for a given date."""
        ...

    def timezone(self) -> ZoneInfo:
        """Get the timezone for this log manager."""
        ...

    def get_log(self, date: datetime.date) -> Optional[models.Log]:
        """Get a log for a given date (returns None if file doesn't exist)."""
        ...

    def get_log_or_create(self, date: datetime.date) -> models.Log:
        """Get a log for a given date (creates empty log if file doesn't exist)."""
        ...

    def write_log(self, log: models.Log, trackers: Dict[str, str]) -> None:
        """Write a log to storage."""
        ...

    def start_intent_now(self, intent: models.Intent, note: Optional[str] = None) -> None:
        """
        Start a new session with the given intent.

        Auto-fills current_date, current_time, and trackers from workspace.
        """
        ...

    def start_intent_at(self, intent: models.Intent, start_time: datetime.datetime, note: Optional[str] = None) -> None:
        """
        Start a new session with the given intent at a specific time.

        Args:
            intent: The intent to start
            start_time: When to start the session
            note: Optional note for the session
        """
        ...

    def stop_current_session(self) -> None:
        """
        Stop the currently active session.

        Auto-fills current_date, current_time, and trackers from workspace.
        """
        ...

class PlanManager:
    """Manager for plan loading, caching, and querying."""

    def get_plans(self, date: datetime.date) -> Dict[str, models.Plan]:
        """
        Get all plans valid for a given date.

        Returns:
            Dictionary mapping source names to Plans.
        """
        ...

    def get_intents(self, date: datetime.date) -> List[models.Intent]:
        """Get all intents from plans valid for a given date."""
        ...

    def get_roles(self, date: datetime.date) -> List[str]:
        """
        Get all roles from plans valid for a given date.

        Returns roles prefixed with their source (e.g., "element:engineer").
        """
        ...

    def get_objectives(self, date: datetime.date) -> List[str]:
        """Get all objectives from plans valid for a given date."""
        ...

    def get_actions(self, date: datetime.date) -> List[str]:
        """Get all actions from plans valid for a given date."""
        ...

    def get_subjects(self, date: datetime.date) -> List[str]:
        """Get all subjects from plans valid for a given date."""
        ...

    def get_trackers(self, date: datetime.date) -> Dict[str, str]:
        """
        Get all trackers from plans valid for a given date.

        Returns:
            Dictionary mapping tracker IDs (prefixed with source) to human-readable names.
            Example: {"element:12345": "Fix critical bug"}
        """
        ...

    def get_plan_by_tracker_id(
        self,
        tracker_id: str,
        date: datetime.date
    ) -> Optional[models.Plan]:
        """
        Get the plan containing a specific tracker ID.

        Returns None if the tracker is not found in any plan for the given date.
        """
        ...

    def get_local_plan(self, date: datetime.date) -> Optional[models.Plan]:
        """
        Get the local plan for a given date.

        Returns None if the local plan doesn't exist.
        """
        ...

    def get_local_plan_or_create(self, date: datetime.date) -> models.Plan:
        """
        Get the local plan for a given date, creating an empty one if it doesn't exist.
        """
        ...

    def write_plan(self, plan: models.Plan) -> None:
        """Write a plan to storage."""
        ...

    def clear_cache(self) -> None:
        """Clear the plan cache."""
        ...

    def remotes(self) -> List:
        """Get plan remote plugin instances (delegates to workspace.plugins.plan_remotes())."""
        ...

class IdentityManager:
    """Manager for Ed25519 identity keypairs for signing timesheets."""

    def create_identity(self, name: str, overwrite: bool = False) -> bytes:
        """
        Create a new Ed25519 identity keypair.

        Keys are stored as base64-encoded strings in ~/.faff/identities/

        Args:
            name: Identity name
            overwrite: Whether to overwrite if identity already exists

        Returns:
            The private signing key as bytes (32 bytes)
        """
        ...

    def get_identity(self, name: str) -> Optional[bytes]:
        """
        Get a specific identity by name.

        Args:
            name: Identity name

        Returns:
            The private signing key as bytes, or None if not found
        """
        ...

    def list_identities(self) -> Dict[str, bytes]:
        """
        List all identities.

        Returns:
            Dictionary mapping identity names to signing keys (as bytes)
        """
        ...

    def identity_exists(self, name: str) -> bool:
        """
        Check if an identity exists.

        Args:
            name: Identity name

        Returns:
            True if the identity exists, False otherwise
        """
        ...

    def delete_identity(self, name: str) -> None:
        """
        Delete an identity.

        Removes both the private and public key files.

        Args:
            name: Identity name
        """
        ...

class PluginManager:
    """Manager for loading and executing Python plugins."""

    def load_plugins(self) -> Dict[str, object]:
        """
        Load all available plugins from the plugins directory.

        Returns:
            Dictionary of plugin_name -> plugin_class
        """
        ...

    def instantiate_plugin(
        self,
        plugin_name: str,
        instance_name: str,
        config: Dict,
        defaults: Dict
    ) -> object:
        """
        Instantiate a plugin with the given config.

        Args:
            plugin_name: Name of the plugin to instantiate
            instance_name: Name for this instance
            config: Plugin-specific configuration
            defaults: Default configuration values

        Returns:
            The instantiated plugin object
        """
        ...

    def plan_remotes(self) -> List:
        """
        Get instantiated plan remote plugins based on config.

        Returns:
            List of plan remote plugin instances
        """
        ...

    def audiences(self) -> List:
        """
        Get instantiated audience plugins based on config.

        Returns:
            List of audience plugin instances
        """
        ...

    def get_audience_by_id(self, audience_id: str) -> Optional:
        """
        Get a specific audience plugin by ID.

        Args:
            audience_id: The ID of the audience to find

        Returns:
            The audience plugin instance, or None if not found
        """
        ...

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

    def audiences(self) -> List[models.TimesheetAudience]:
        """Get all audience plugin instances."""
        ...

    def get_audience(self, audience_id: str) -> Optional[models.TimesheetAudience]:
        """Get a specific audience plugin by ID."""
        ...

    def submit(self, timesheet: models.Timesheet) -> None:
        """Submit a timesheet via its audience plugin."""
        ...

class Workspace:
    """
    Workspace manager for accessing logs, plans, and configuration.

    This is the main entry point for interacting with a Faff workspace.
    Provides coordinated access to all managers through properties.
    """

    # Manager properties
    logs: LogManager
    """Log manager for reading/writing daily work logs."""

    plans: PlanManager
    """Plan manager for accessing plans, intents, and vocabulary."""

    timesheets: TimesheetManager
    """Timesheet manager for compiled, signed work records."""

    identities: IdentityManager
    """Identity manager for Ed25519 keypairs used in signing."""

    plugins: PluginManager
    """Plugin manager for loading and executing Python plugins."""

    def __init__(self, storage: Optional[object] = None) -> None:
        """
        Initialize workspace.

        Args:
            storage: Optional custom storage implementation. If None, uses
                    FileSystemStorage and searches for .faff directory from cwd.
        """
        ...

    def now(self) -> datetime.datetime:
        """
        Get the current time in the configured timezone.

        Returns:
            Current datetime with timezone applied.
        """
        ...

    def today(self) -> datetime.date:
        """
        Get today's date in the configured timezone.

        Returns:
            Current date in workspace timezone.
        """
        ...

    def timezone(self) -> ZoneInfo:
        """
        Get the configured timezone.

        Returns:
            ZoneInfo for the workspace timezone.
        """
        ...

    def config(self) -> models.Config:
        """
        Get the workspace configuration.

        Returns:
            Config object with timezone, plan remotes, audiences, and roles.
        """
        ...

    def __repr__(self) -> str: ...
