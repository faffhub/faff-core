from . import faff_core
from .faff_core import *
from .faff_core import models, managers
import sys

__doc__ = faff_core.__doc__
if hasattr(faff_core, "__all__"):
    __all__ = list(faff_core.__all__)
else:
    __all__ = []

# Re-export submodules (avoid duplicates)
if "models" not in __all__:
    __all__.append("models")
if "managers" not in __all__:
    __all__.append("managers")

# Make submodules importable via from faff_core.models import ...
sys.modules['faff_core.models'] = models
sys.modules['faff_core.managers'] = managers
