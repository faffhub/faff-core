# Migration Plan: faff-core → faff-core-rust

## Goal
Migrate all functionality from Python faff-core to Rust faff-core-rust, then rename faff-core-rust to faff-core.

## Strategy
1. Reimplement functionality in Rust in faff-core-rust
2. Keep Python wrappers in faff-core as thin wrappers calling Rust
3. Gradually hollow out faff-core until it's nearly empty
4. Update faff-cli to depend directly on faff-core-rust
5. Rename faff-core-rust → faff-core

## Progress

### ✅ Already Implemented in Rust
- [x] Intent model
- [x] Session model
- [x] Log model
- [x] Plan model (basic structure)
- [x] Timesheet model (basic structure)
- [x] LogManager (read, write, start/stop sessions)
- [x] Storage trait + PyStorage wrapper
- [x] Python bindings via PyO3

### 🚧 Needs Rust Implementation

#### Core Managers
- [ ] **PlanManager** - High priority (LogManager already calls get_trackers())
  - [ ] Load plans from `.faff/plans/*.toml` files
  - [ ] Filter plans by date (valid_from/valid_until)
  - [ ] Extract roles, objectives, actions, subjects from plans
  - [ ] Extract trackers (used by LogManager)
  - [ ] Cache plan data (currently uses @cache decorator)
  - [ ] get_intents(), get_plan_by_tracker_id(), local_plan()
  - [ ] write_plan()

- [ ] **TimesheetManager**
  - [ ] Read/write timesheet files (JSON format)
  - [ ] Read/write timesheet metadata
  - [ ] List timesheets (with optional date filter)
  - [ ] get_timesheet(), write_timesheet()
  - [ ] Integration with plugin system for audiences

- [ ] **IdentityManager** - Use ed25519-dalek
  - [ ] Create Ed25519 key pairs
  - [ ] Store keys in `.faff/keys/id_*` format
  - [ ] Base64 encoding for key storage
  - [ ] Set proper file permissions (0o600 for private keys)
  - [ ] get_identity(), create_identity(), get() for listing all

#### Configuration & Workspace
- [ ] **Config**
  - [ ] Parse config.toml
  - [ ] Handle timezone (ZoneInfo/chrono-tz)
  - [ ] Handle plan_remotes, audiences, roles lists
  - [ ] from_dict() constructor

- [ ] **Workspace**
  - [ ] Coordinate all managers
  - [ ] Provide now() - current time in configured timezone
  - [ ] Provide today() - current date
  - [ ] Initialize all managers (logs, plans, timesheets, identities)
  - [ ] Hold FileSystem and Config instances

#### Utilities
- [x] **PrivateLogFormatter** - Already implemented as `Log::to_log_file()` in Rust
  - Not needed - functionality already in Log model

- [ ] **TomlSerializer**
  - [ ] Generic TOML serialization for Plan, Intent, etc.
  - [ ] Handle datetime serialization
  - [ ] Remove None values
  - [ ] Support both dataclasses and Rust types

### 🐍 Staying in Python
- [x] **PluginManager** - Dynamic plugin loading from `.faff/plugins/*.py`
- [x] **Plugin base classes** - PlanSource, Audience, RemoteRecordSource
- [x] **InstanceManager** - Plugin instantiation

## Next Steps
1. Implement PlanManager in Rust (highest priority - already used by LogManager)
2. Implement Config parsing
3. Implement IdentityManager with ed25519-dalek
4. Implement Workspace coordinator
5. Implement TimesheetManager
6. Implement utility formatters/serializers
7. Update faff-cli to depend on faff-core-rust directly
8. Archive old faff-core
9. Rename faff-core-rust → faff-core

## Notes
- Python wrappers in faff-core currently call Rust via PyO3
- LogManager already migrated and working
- Plugin system will remain in Python for dynamic loading flexibility
- Post-quantum crypto (Dilithium/ML-DSA) not adopted yet - stick with Ed25519 for now
