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

    class Toy:
        word: str

        def __init__(self, word: str) -> None: ...

        def hello(self) -> str: ...

        def add_days(self, datetime: datetime.datetime, days: int) -> datetime.datetime: ...