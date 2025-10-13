"""Manager classes exposed from Rust"""

from .faff_core import managers as _managers

LogManager = _managers.LogManager
