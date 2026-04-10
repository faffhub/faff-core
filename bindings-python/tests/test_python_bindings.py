"""
Tests for Python bindings (PyO3 FFI boundary)

These tests verify that the Rust-Python interface works correctly,
including type conversions, error handling, and storage bridge functionality.
"""

import pytest
import datetime
from zoneinfo import ZoneInfo
from pathlib import Path
import tempfile

import faff_core
from faff_core import Workspace, UninitializedLedgerError
from faff_core.models import Log, Session, Plan, Timesheet


class TestDateTimeConversions:
    """Test datetime/date type conversions between Python and Rust"""

    def test_log_with_utc_timezone(self):
        """Test creating a log with UTC timezone"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")

        log = Log(date, tz, [])

        assert log.date == date
        assert str(log.timezone) == "UTC"
        assert len(log.timeline) == 0

    def test_log_with_london_timezone(self):
        """Test creating a log with Europe/London timezone"""
        date = datetime.date(2025, 6, 15)
        tz = ZoneInfo("Europe/London")

        log = Log(date, tz, [])

        assert log.date == date
        assert str(log.timezone) == "Europe/London"

    def test_session_datetime_roundtrip(self):
        """Test that datetime values survive Python->Rust->Python roundtrip"""
        tz = ZoneInfo("UTC")
        start_dt = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)
        end_dt = datetime.datetime(2025, 3, 15, 10, 30, 0, tzinfo=tz)

        session = Session(
            start=start_dt,
            end=end_dt,
            title="work",
            role="engineer",
            impact="development",
            mode="coding",
            subject="tests",
            trackers=[],
        )

        assert session.start == start_dt
        assert session.end == end_dt
        assert session.title == "work"

    def test_session_with_microseconds(self):
        """Test that microseconds are preserved in datetime conversion"""
        tz = ZoneInfo("UTC")
        start_dt = datetime.datetime(2025, 3, 15, 9, 0, 0, 123456, tzinfo=tz)

        session = Session(start=start_dt, title="test")

        assert session.start.microsecond == 123456

    def test_naive_datetime_raises_error(self):
        """Test that naive (timezone-unaware) datetime raises an error"""
        naive_dt = datetime.datetime(2025, 3, 15, 9, 0, 0)  # No tzinfo

        with pytest.raises(ValueError, match="timezone"):
            Session(naive_dt, title="test")


class TestExceptionMapping:
    """Test that Rust errors are correctly mapped to Python exceptions"""

    def test_invalid_timezone_raises_value_error(self):
        """Test that invalid timezone string raises ValueError"""
        date = datetime.date(2025, 3, 15)

        # This should fail because "INVALID/TIMEZONE" doesn't exist
        with pytest.raises(ValueError, match="timezone"):
            # We need to construct this through a dict since direct construction
            # validates the timezone in Python before it reaches Rust
            log_dict = {
                "date": "2025-03-15",
                "timezone": "INVALID/TIMEZONE",
                "timeline": []
            }
            Log.from_dict(log_dict)

    def test_log_stop_empty_timeline_raises_error(self):
        """Test that stopping a log with no sessions raises an error"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")
        log = Log(date, tz, [])

        stop_time = datetime.datetime(2025, 3, 15, 10, 0, 0, tzinfo=tz)

        with pytest.raises(ValueError, match="No active session to stop"):
            log.stop_all_active_sessions(stop_time)

    def test_ambiguous_datetime_during_dst(self):
        """Test handling of ambiguous times during DST transitions"""
        # During DST transition, 1:30 AM can occur twice
        # This tests that the error is properly propagated

        # Note: This test might be skipped if we always use .single() which
        # returns None for ambiguous times, but we should still test the error path
        pass  # TODO: Create a scenario that triggers ambiguous datetime error


class TestLogOperations:
    """Test Log model operations through Python bindings"""

    def test_create_empty_log(self):
        """Test creating an empty log"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")

        log = Log(date, tz, [])

        assert log.is_closed()
        assert log.active_sessions() == []

    def test_log_with_open_session(self):
        """Test log with an open (no end time) session"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")
        start = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)

        session = Session(start, title="work")
        log = Log(date, tz, [session])

        assert not log.is_closed()
        assert len(log.active_sessions()) == 1
        assert log.active_sessions()[0].title == "work"

    def test_append_session(self):
        """Test appending a session to a log"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")

        log = Log(date, tz, [])

        start = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)
        end = datetime.datetime(2025, 3, 15, 10, 0, 0, tzinfo=tz)
        session = Session(start, title="work", end=end)

        new_log = log.start_session(session)

        assert len(new_log.timeline) == 1
        assert new_log.timeline[0].title == "work"
        # Original log should be unchanged (immutability)
        assert len(log.timeline) == 0

    def test_stop_active_session(self):
        """Test stopping an active session"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")
        start = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)

        session = Session(start, title="work")
        log = Log(date, tz, [session])

        stop_time = datetime.datetime(2025, 3, 15, 10, 30, 0, tzinfo=tz)
        stopped_log = log.stop_all_active_sessions(stop_time)

        assert stopped_log.is_closed()
        assert stopped_log.timeline[0].end == stop_time
        # Original log should be unchanged
        assert log.timeline[0].end is None

    def test_total_recorded_time(self):
        """Test calculating total recorded time"""
        date = datetime.date(2025, 3, 15)
        tz = ZoneInfo("UTC")

        start1 = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)
        end1 = datetime.datetime(2025, 3, 15, 10, 0, 0, tzinfo=tz)

        start2 = datetime.datetime(2025, 3, 15, 14, 0, 0, tzinfo=tz)
        end2 = datetime.datetime(2025, 3, 15, 15, 30, 0, tzinfo=tz)

        session1 = Session(start1, title="work", end=end1)
        session2 = Session(start2, title="work", end=end2)

        log = Log(date, tz, [session1, session2])

        total = log.total_recorded_time()

        # 1 hour + 1.5 hours = 2.5 hours = 150 minutes
        assert total == datetime.timedelta(hours=2, minutes=30)


class TestSessionModel:
    """Test Session model through Python bindings"""

    def test_create_full_session(self):
        """Test creating a session with all fields"""
        tz = ZoneInfo("UTC")
        start = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)

        session = Session(
            start,
            title="work",
            role="engineer",
            impact="development",
            mode="coding",
            subject="features",
            trackers=["PROJ-123", "PROJ-456"],
        )

        assert session.title == "work"
        assert session.role == "engineer"
        assert session.impact == "development"
        assert session.mode == "coding"
        assert session.subject == "features"
        assert len(session.trackers) == 2
        assert "PROJ-123" in session.trackers

    def test_create_minimal_session(self):
        """Test creating a session with minimal fields"""
        tz = ZoneInfo("UTC")
        start = datetime.datetime(2025, 3, 15, 9, 0, 0, tzinfo=tz)

        session = Session(start, title="minimal")

        assert session.title == "minimal"
        assert session.role is None
        assert session.trackers == []


class TestPlanModel:
    """Test Plan model through Python bindings"""

    def test_create_plan(self):
        """Test creating a plan"""
        date = datetime.date(2025, 3, 15)

        plan = Plan(
            source="local",
            valid_from=date,
            valid_until=None,
            roles=["engineer"],
            impacts=["development"],
            modes=["coding"],
            subjects=["features"],
            trackers={"PROJ-123": "Implement feature X"},
        )

        assert plan.source == "local"
        assert plan.valid_from == date
        assert "engineer" in plan.roles
        assert plan.trackers["PROJ-123"] == "Implement feature X"


class TestStorageBridge:
    """Test Python storage bridge functionality"""

    def test_python_storage_implementation(self):
        """Test that Python storage implementation works with Rust managers"""
        # Note: Testing with actual Python storage class would require implementing
        # the storage protocol in Python first. For now, test default storage.
        # TODO: Implement and test custom Python storage class
        pass


class TestWorkspace:
    """Test Workspace functionality"""

    def test_workspace_default(self):
        """Test creating workspace with default (FileSystemStorage)"""
        # This will look for .faff in current directory or parents
        # Just verify it creates without error
        try:
            ws = Workspace()
            # If we're in a directory with .faff, this should work
            assert ws is not None
            assert ws.timezone is not None
        except UninitializedLedgerError:
            # If we're not in a .faff directory, that's expected
            pass

    def test_workspace_managers_accessible(self):
        """Test that all managers are accessible from workspace"""
        try:
            ws = Workspace()
            # All managers should be accessible
            assert ws.logs is not None
            assert ws.plans is not None
            assert ws.identities is not None
            assert ws.timesheets is not None
            assert ws.plugins is not None
        except UninitializedLedgerError:
            # If we're not in a .faff directory, skip this test
            pytest.skip("Not in a .faff directory")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
