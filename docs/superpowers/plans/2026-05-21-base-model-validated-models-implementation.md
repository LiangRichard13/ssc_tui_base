# Base-Model Validated Models Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/model` include validated SAITEC base-model provider models after configuration and auth validation.

**Architecture:** Persist validated model ids in auth-validation records, then merge those ids into `MultiProvider::model_routes()` so the existing `/model` picker naturally consumes them. Keep the change backward compatible by making the new field optional and additive.

**Tech Stack:** Rust, serde JSON persistence, existing auth-test flow, MultiProvider route aggregation, focused cargo tests.

---

### Task 1: Add failing persistence tests

**Files:**
- Modify: `crates/jcode-auth-types/src/lib.rs`
- Modify: `src/auth/validation.rs`
- Modify: `src/auth/tests.rs`

- [ ] **Step 1: Add a backward-compatible serde test for validation records**
- [ ] **Step 2: Add a failing auth validation save/load test covering `validated_models`**
- [ ] **Step 3: Run focused tests to verify failure**

### Task 2: Add failing auth-test persistence tests

**Files:**
- Modify: `src/cli/auth_test/types.rs`
- Modify: `src/cli/auth_test/run.rs`

- [ ] **Step 1: Add a failing unit test for persisted validated model extraction**
- [ ] **Step 2: Run the focused auth-test unit test to verify failure**

### Task 3: Add failing route aggregation tests

**Files:**
- Modify: `src/provider/tests/model_resolution.rs`

- [ ] **Step 1: Add a failing test showing validated direct-compatible models appear in model routes**
- [ ] **Step 2: Run the focused provider test to verify failure**

### Task 4: Implement and verify

**Files:**
- Modify: `crates/jcode-auth-types/src/lib.rs`
- Modify: `src/auth/validation.rs`
- Modify: `src/auth/tests.rs`
- Modify: `src/cli/auth_test/types.rs`
- Modify: `src/cli/auth_test/run.rs`
- Modify: `src/provider/mod.rs`
- Modify: `src/provider/tests/model_resolution.rs`

- [ ] **Step 1: Implement minimal persistence changes**
- [ ] **Step 2: Implement auth-test validated-model capture**
- [ ] **Step 3: Implement provider route enrichment**
- [ ] **Step 4: Run focused tests until green**
- [ ] **Step 5: Run `cargo check` and final build**
