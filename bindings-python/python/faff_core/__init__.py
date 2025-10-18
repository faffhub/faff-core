from .faff_core import *
from .faff_core import models, managers
import sys

__doc__ = faff_core.__doc__
if hasattr(faff_core, "__all__"):
    __all__ = faff_core.__all__

# Re-export submodules
__all__ += ["models", "managers"]

# Make submodules importable via from faff_core.models import ...
sys.modules['faff_core.models'] = models
sys.modules['faff_core.managers'] = managers
