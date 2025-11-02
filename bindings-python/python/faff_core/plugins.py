"""
Plugin base classes for faff-core.

These classes define the interface for plugins that extend faff functionality.
"""

from __future__ import annotations

import datetime
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Dict, Any

from slugify import slugify

from faff_core.models import Log, Plan, Timesheet


class Plugin(ABC):
    """Base class for all faff plugins."""

    def __init__(
        self,
        plugin: str,
        name: str,
        config: Dict[str, Any],
        defaults: Dict[str, Any],
        state_path: Path
    ):
        """
        Initialize a plugin instance.

        Args:
            plugin: The plugin type/class name
            name: The instance name for this plugin
            config: Configuration specific to this plugin instance
            defaults: Default values for the plugin
            state_path: Path where the plugin can store persistent state
        """
        self.plugin = plugin
        self.name = name
        self.id = slugify(self.name)
        self.state_path = state_path
        self.state_path.mkdir(parents=False, exist_ok=True)

        self.slug = slugify(self.name)
        self.config = config
        self.defaults = defaults


class PlanSource(Plugin):
    """
    Plugin that provides plans from an external source.

    Examples: Jira, Linear, GitHub Issues, etc.
    """

    @abstractmethod
    def pull_plan(self, date: datetime.date) -> Plan:
        """
        Fetch a plan for the given date from the external source.

        Args:
            date: The date for which to fetch the plan

        Returns:
            A Plan object containing roles, objectives, actions, subjects, and trackers
        """
        pass


class Audience(Plugin):
    """
    Plugin that compiles and submits timesheets to an external system.

    Examples: Harvest, Clockify, Toggl, etc.
    """

    @abstractmethod
    def compile_time_sheet(self, log: Log) -> Timesheet:
        """
        Compile a timesheet from a log.

        Args:
            log: The log to compile into a timesheet

        Returns:
            A compiled Timesheet (may be empty if there are no relevant sessions for this audience)
        """
        pass

    @abstractmethod
    def submit_timesheet(self, timesheet: Timesheet) -> None:
        """
        Submit a compiled timesheet to the external system.

        Args:
            timesheet: The timesheet to submit (not SubmittableTimesheet,
                      as we need access to unsubmitted meta to update after submission)
        """
        pass
