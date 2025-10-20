# Development Guide

This guide covers local development workflows for faff-core.

## Project Structure

```
faff-core/
├── core/                   # Rust core library
│   ├── src/
│   │   ├── models/        # Data models (Intent, Session, Log, Plan, Timesheet)
│   │   ├── managers/      # Business logic managers
│   │   ├── storage/       # File I/O abstraction
│   │   └── plugins.rs     # Python plugin system
│   └── Cargo.toml
├── bindings-python/       # Python bindings (PyO3)
│   ├── python/
│   │   └── faff_core/
│   │       ├── __init__.py
│   │       ├── plugins.py      # Plugin base classes
│   │       └── faff_core.pyi   # Type stubs
│   ├── src/
│   │   └── python/        # PyO3 wrapper code
│   └── Cargo.toml
├── Cargo.toml             # Workspace root
└── docs/                  # Documentation
```

This is a **Cargo workspace** with two crates:
- `core`: Pure Rust library
- `bindings-python`: Python bindings that depend on `core`

## Prerequisites

- **Rust**: 1.70+ (install via [rustup](https://rustup.rs/))
- **Python**: 3.11+ (we target modern Python only)
- **Maturin**: `pip install maturin`

## Building for Development

### Quick Start

```bash
cd bindings-python
maturin develop
```

**This command will:**
1. Build the `core` Rust library (automatically, as it's a dependency)
2. Build the Python bindings (`faff-core-python`)
3. Install the Python package in your current virtualenv/environment

You can now import and use it:

```python
import faff_core
from faff_core.models import Intent, Session, Log, Plan
```

### Alternative: From Workspace Root

```bash
# From project root
maturin develop -m bindings-python/Cargo.toml
```

This does the same thing but requires specifying the manifest path.

### Release Build

For optimized builds:

```bash
cd bindings-python
maturin develop --release
```

Release builds are ~10x faster at runtime but slower to compile.

## Testing

### Rust Tests

```bash
# From workspace root - runs all workspace tests
cargo test --workspace

# Or run specific crate tests
cargo test -p faff-core
cargo test -p faff-core-python
```

### Python Tests

```bash
# First, install the package in development mode
cd bindings-python
maturin develop

# Then run Python tests
cd ..
pytest bindings-python/tests/ -v
```