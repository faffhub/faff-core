# faff-core TODO

## Priority 1: Production Safety

### Replace .unwrap()/.expect() with proper error handling
**Impact:** HIGH - Runtime panics possible in production

Currently 232 instances of `.unwrap()` and `.expect()` throughout the codebase. Most problematic areas:

- [ ] `src/models/log.rs:96-102` - Multiple `.expect()` on timezone operations
- [ ] `src/models/session.rs:264-266` - Timezone handling unwraps
- [ ] `src/managers/plan_manager.rs:115` - Unwrap on regex compilation
- [ ] Review all remaining unwraps and convert to proper `Result` propagation with `?`
- [ ] Consider using `lazy_static` or `once_cell` for compile-time regex validation

### Add integration tests
**Impact:** HIGH - Cross-module interactions untested

- [ ] Test interactions between multiple managers
- [ ] Test workspace orchestration with real manager instances
- [ ] Test plugin loading with actual Python plugins
- [ ] Test end-to-end workflows (e.g., log creation → plan updates → timesheet generation)

### Test Python bindings
**Impact:** MEDIUM-HIGH - FFI boundary currently untested

- [ ] Add PyO3 mock testing for Python bindings
- [ ] Test type mapping utilities (datetime/date conversions)
- [ ] Test Python storage bridge
- [ ] Verify zoneinfo availability handling
- [ ] Test exception mapping (PyFileNotFoundError, PyValueError, etc.)

---

## Priority 2: Code Quality

### Extract MockStorage to shared test utilities
**Impact:** MEDIUM - ~500 lines of duplication

Nearly identical MockStorage implementations in:
- [ ] `src/managers/log_manager.rs:196-290`
- [ ] `src/managers/plan_manager.rs:358-444`
- [ ] `src/managers/identity_manager.rs:134-218`
- [ ] `src/managers/timesheet_manager.rs:130-212`

Create `src/test_utils/mock_storage.rs` and deduplicate.

### Resolve FIXMEs and technical debt comments
**Impact:** MEDIUM

- [ ] `src/managers/plan_manager.rs:14-16` - "FIXME: Currently takes just Storage, but may need access to other managers"
- [ ] `src/plugins.rs:260` - "FIXME: This is a temporary solution - we should properly serialize PlanDefaults"
- [ ] `src/bindings/python/models/intent.rs:120-121` - XXX comment about unknown functionality - document or fix
- [ ] Search for any other TODO/FIXME/XXX comments and address

### Add inline documentation for complex logic
**Impact:** MEDIUM

- [ ] Document complex timezone handling logic in Session and Timesheet
- [ ] Add comments explaining DST edge case handling
- [ ] Document the plugin loading introspection mechanism
- [ ] Explain the custom serialization strategies for datetime types

---

## Priority 3: Performance & Maintainability

### Refactor PluginManager mutability
**Impact:** MEDIUM

`src/plugins.rs:38-49` - Method signature requires `&mut self` but caches internally:
- [ ] Use interior mutability with `RwLock<Option<HashMap<...>>>` instead
- [ ] Remove need for `Arc<Mutex<PluginManager>>` in Workspace
- [ ] Or: Pre-load plugins at startup to avoid runtime mutation

### Optimize hot paths
**Impact:** LOW-MEDIUM

- [ ] Add benchmarks for Log operations on large timelines
- [ ] Profile clone-heavy operations (e.g., `Log::append_session` cloning entire timeline)
- [ ] Consider `Arc<Vec<T>>` or `Cow<>` patterns if cloning becomes bottleneck
- [ ] Move regex compilation outside loops (storage.rs glob handling)

### Improve borrow checker workarounds
**Impact:** LOW-MEDIUM

- [ ] Review `src/plugins.rs:154-161` - Complex borrow checker workarounds
- [ ] Simplify if possible or document why necessary

---

## Priority 4: Nice-to-Have

### Expand test coverage
**Impact:** LOW

Current coverage: ~14.7% (79 tests / 536 functions)

Well-tested:
- Log model (30 tests)
- Session model (20 tests)

Needs more coverage:
- [ ] Workspace (currently 2 tests)
- [ ] PluginManager (currently 1 test)
- [ ] FileSystemStorage (currently 4 tests)
- [ ] Plan model (currently 7 tests)

### Add property-based testing
**Impact:** LOW

- [ ] Add `proptest` dependency
- [ ] Write property tests for Session duration calculations
- [ ] Write property tests for timezone conversions
- [ ] Write property tests for immutable update operations

### Performance profiling guide
**Impact:** LOW

- [ ] Document how to profile plugin loading
- [ ] Create benchmarks for common operations
- [ ] Add flamegraph generation instructions

### Python binding improvements
**Impact:** LOW

- [ ] Handle older Python versions without zoneinfo gracefully
- [ ] Reduce reliance on `pythonize` for generic conversions
- [ ] Add more detailed error messages for plugin loading failures

---

## Metrics Tracking

| Metric | Before | Current | Target |
|--------|---------|---------|--------|
| Unwrap/Expect calls | 232 | 232 | < 20 |
| Test count | 79 | **116** (47% increase) | - |
| Unit tests | 79 | 109 | - |
| Integration tests | 0 | **7 ✓** | > 7 |
| Test coverage | ~14.7% | ~22% | > 50% |
| MockStorage duplication | 4 copies | **1 shared ✓** | 1 shared |
| FIXMEs/TODOs | 4+ | 4+ | 0 |

---

## Notes

- Address P1 items before any production deployment
- P2 items improve maintainability significantly
- P3/P4 items can be tackled as time allows
- Consider creating GitHub issues for each major bullet point
