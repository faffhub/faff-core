from __future__ import annotations
from typing import Optional, List

from zoneinfo import ZoneInfo
import datetime

def hello_world() -> str: ...

class models:
    class Intent:
        alias: Optional[str]
        role: Optional[str]
        objective: Optional[str]
        action: Optional[str]
        subject: Optional[str]
        trackers: List[str]

        def __init__(
            self,
            alias: Optional[str] = ...,
            role: Optional[str] = ...,
            objective: Optional[str] = ...,
            action: Optional[str] = ...,
            subject: Optional[str] = ...,
            trackers: List[str] = ...
        ) -> None: ...

        @staticmethod
        def from_dict(data: dict) -> models.Intent: ...

        def as_dict(self) -> dict: ...

        def __hash__(self) -> int: ...
        def __eq__(self, other: object) -> bool: ...
        def __ne__(self, other: object) -> bool: ...

    class Session:
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
        def duration(self) -> datetime.timedelta: ...

        @staticmethod
        def from_dict_with_tz(data: dict, date: datetime.date, timezone: ZoneInfo) -> models.Session: ...

        def with_end(self, end: datetime.datetime) -> models.Session: ...

        def as_dict(self) -> dict: ...

        def __eq__(self, other: object) -> bool: ...
        def __ne__(self, other: object) -> bool: ...

    class Log:
        date: datetime.date
        timezone: ZoneInfo
        timeline: List[models.Session]

        def __init__(
            self,
            date: datetime.date,
            timezone: ZoneInfo,
            timeline: List[models.Session] = ...
        ) -> None: ...

        @staticmethod
        def from_dict(data: dict) -> models.Log: ...

        def append_session(self, session: models.Session) -> models.Log: ...
        def active_session(self) -> Optional[models.Session]: ...
        def stop_active_session(self, stop_time: datetime.datetime) -> models.Log: ...
        def is_closed(self) -> bool: ...
        def total_recorded_time(self) -> datetime.timedelta: ...

    class Plan:
        source: str
        valid_from: datetime.date
        valid_until: Optional[datetime.date]
        roles: List[str]
        actions: List[str]
        objectives: List[str]
        subjects: List[str]
        trackers: dict[str, str]
        intents: List[models.Intent]

        def __init__(
            self,
            source: str,
            valid_from: datetime.date,
            valid_until: Optional[datetime.date] = ...,
            roles: List[str] = ...,
            actions: List[str] = ...,
            objectives: List[str] = ...,
            subjects: List[str] = ...,
            trackers: Optional[dict[str, str]] = ...,
            intents: List[models.Intent] = ...
        ) -> None: ...

        @staticmethod
        def from_dict(data: dict) -> models.Plan: ...

        def id(self) -> str: ...
        def add_intent(self, intent: models.Intent) -> models.Plan: ...
        def as_dict(self) -> dict: ...

    class TimesheetMeta:
        audience_id: str
        submitted_at: Optional[datetime.datetime]
        submitted_by: Optional[str]

        def __init__(
            self,
            audience_id: str,
            submitted_at: Optional[datetime.datetime] = None,
            submitted_by: Optional[str] = None
        ) -> None: ...

        @staticmethod
        def from_dict(data: dict) -> models.TimesheetMeta: ...

    class Timesheet:
        actor: dict[str, str]
        date: datetime.date
        compiled: datetime.datetime
        timezone: ZoneInfo
        timeline: List[models.Session]
        signatures: dict[str, dict[str, str]]
        meta: models.TimesheetMeta

        def __init__(
            self,
            *,
            actor: Optional[dict[str, str]] = None,
            date: datetime.date,
            compiled: datetime.datetime,
            timezone: ZoneInfo,
            timeline: Optional[List[models.Session]] = None,
            signatures: Optional[dict[str, dict[str, str]]] = None,
            meta: models.TimesheetMeta
        ) -> None: ...

        def sign(self, id: str, signing_key: bytes) -> models.Timesheet: ...

        def update_meta(
            self,
            audience_id: str,
            submitted_at: Optional[datetime.datetime] = None,
            submitted_by: Optional[str] = None
        ) -> models.Timesheet: ...

        def submittable_timesheet(self) -> models.SubmittableTimesheet: ...

        @staticmethod
        def from_dict(data: dict) -> models.Timesheet: ...

    class SubmittableTimesheet:
        actor: dict[str, str]
        date: datetime.date
        compiled: datetime.datetime
        timezone: ZoneInfo
        timeline: List[models.Session]
        signatures: dict[str, dict[str, str]]

        def canonical_form(self) -> bytes: ...

    class Toy:
        word: str

        def __init__(self, word: str) -> None: ...

        def hello(self) -> str: ...

        def add_days(self, datetime: datetime.datetime, days: int) -> datetime.datetime: ...