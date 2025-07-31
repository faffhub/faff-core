from __future__ import annotations
from typing import Optional, List

import pendulum

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
        start: pendulum.DateTime
        end: Optional[pendulum.DateTime]
        note: Optional[str]

        def __init__(
            self,
            intent: models.Intent,
            start: pendulum.DateTime,
            end: Optional[pendulum.DateTime] = None,
            note: Optional[str] = None
        ) -> None: ...

        @staticmethod
        def from_dict_with_tz(data: dict, date: pendulum.Date, timezone: pendulum.Timezone | pendulum.FixedTimezone) -> models.Session: ...

        def with_end(self, end: pendulum.DateTime) -> models.Session: ...

    class Toy:
        word: str

        def __init__(self, word: str) -> None: ...

        def hello(self) -> str: ...