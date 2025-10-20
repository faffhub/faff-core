from . import faff_core
import sys
import types

# Explicitly import public names from faff_core to avoid wildcard imports.
if hasattr(faff_core, "__all__"):
    for _name in faff_core.__all__:
        globals()[_name] = getattr(faff_core, _name)
else:
    # If __all__ is not defined, import all non-private attributes.
    for _name, _value in vars(faff_core).items():
        if not _name.startswith("_"):
            globals()[_name] = _value

# Expose submodules as variables if present
models = getattr(faff_core, "models", None)
managers = getattr(faff_core, "managers", None)

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
if isinstance(models, types.ModuleType):
    sys.modules['faff_core.models'] = models
if isinstance(managers, types.ModuleType):
    sys.modules['faff_core.managers'] = managers
