# Release Process

This document explains the automated release process for faff-core.

## Dev Builds (Automatic)

Every push to `main` that passes CI automatically publishes a dev build to PyPI.

**Versioning:**
- Base version in `Cargo.toml`: `0.1.0`
- Published dev versions: `0.1.0.dev1`, `0.1.0.dev2`, `0.1.0.dev3`, etc.
- Dev number increments with each build using GitHub Actions run number

**Installation:**
```bash
# Get latest dev build
pip install --pre faff-core

# Or specific dev version
pip install faff-core==0.1.0.dev5
```

**What happens:**
1. Push to `main`
2. CI workflow runs (tests, formatting, clippy)
3. If CI passes → Publish Dev Build workflow triggers
4. Builds wheels for all platforms (Linux, macOS x86/ARM, Windows)
5. Publishes to PyPI with dev version number

## Stable Releases (Manual)

When you're ready to release a stable version, create and push a git tag:

```bash
# Release version 0.1.0
git tag v0.1.0
git push origin v0.1.0
```

**What happens:**
1. Release workflow triggers on tag
2. Extracts version from tag (strips `v` prefix)
3. Updates `Cargo.toml` with release version `0.1.0`
4. Commits version update to `main`
5. Builds wheels for all platforms
6. Publishes to PyPI (no `.dev` suffix)
7. Creates GitHub Release with artifacts
8. Bumps `Cargo.toml` to next patch version `0.1.1` for future dev builds

**Installation:**
```bash
# Get latest stable release (no --pre flag needed)
pip install faff-core

# Or specific version
pip install faff-core==0.1.0
```

## Version Flow Example

1. Start: `Cargo.toml` has `version = "0.1.0"`
2. Dev builds publish: `0.1.0.dev1`, `0.1.0.dev2`, `0.1.0.dev3`
3. Tag `v0.1.0`: publishes `0.1.0`, bumps `Cargo.toml` to `0.1.1`
4. Dev builds publish: `0.1.1.dev1`, `0.1.1.dev2`
5. Tag `v0.2.0`: publishes `0.2.0`, bumps `Cargo.toml` to `0.2.1`
6. Dev builds publish: `0.2.1.dev1`, `0.2.1.dev2`

## Requirements

The following GitHub secrets must be set:
- `PYPI_API_TOKEN`: PyPI API token with permission to publish `faff-core`

## Notes

- Dev builds are hidden from normal `pip install` (need `--pre` flag)
- Stable releases are what users get by default
- All builds are published to **real PyPI** (not TestPyPI)
- Version numbers follow PEP 440
