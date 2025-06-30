from __future__ import annotations
from typing import Optional, List

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