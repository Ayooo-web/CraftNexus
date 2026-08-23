# Versioned State Migration Runbook

This document details the step-by-step procedure required to safely execute state migrations for the smart contract across different schema versions.

---

## General Migration Lifecycle Workflow

For every migration version, operators must strictly adhere to the following sequence:

1. **Pre-Migration Checks:** Verify the current version in contract storage.
2. **Migration Invocation:** Execute the targeted Soroban contract command.
3. **Post-Migration Verification:** Ensure the state matches the structural rules of the new schema version.

## Differential Upgrade Compatibility Gate

An uploaded WASM and a successful unit-test run are not sufficient evidence for
an upgrade. Before execution, run the old and new artifacts against an isolated
fixture containing legacy profiles, active and disputed escrows, recurring
balances, stake queues, pending upgrades, and paused configuration. Compare
read results, authorization decisions, error classifications, invariants, and
events. Commit the pre-migration snapshot returned by
`get_upgrade_state_commitment` and the interface/authentication test results in
an `UpgradeCompatibilityManifest`.

Submit the manifest with `submit_upgrade_compatibility_manifest`. It must:

- identify the exact source and target contract versions;
- commit to storage preconditions, postconditions, interface behavior,
  authorization behavior, and rollback limitations;
- include a resumable migration checkpoint;
- report `migration_complete: true` and `manual_records: 0`.

`execute_upgrade` rejects missing, stale, incomplete, or manually unresolved
manifests before calling `update_current_contract_wasm`. On success,
`UpgradeHistory` records the source and target versions, WASM hash, state
commitment, and migration checkpoint. The manifest is removed only after the
upgrade record and version update are written. A migration runner may replace
the manifest for the same hash while it resumes; each execution attempt is
idempotently blocked until the final checkpoint is submitted.

The manifest is an attestation boundary, not a substitute for isolated test
execution. CI and release tooling must fail closed when the differential
fixture, invariant suite, or rollback documentation does not produce all
non-zero commitments required by the on-chain gate. Records that cannot be
automatically migrated must remain outside execution until they are handled
and the manifest is resubmitted with a new checkpoint.

---

## Migration 1: UserProfile (v1 -&gt; v2)

### 1. Pre-Migration Checks

Verify that the `UserProfile` entries are on `v1` structure before applying the layout change. Ensure contract balance constraints are satisfied.

### 2. Migration Invocation Command

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  --network <NETWORK> \
  -- \
  migrate_user_profile
```

### 3. Post-Migration Verification
Query individual user state fields using a read-only instance to verify the presence of the updated fields introduced in v2.

## Migration 2: WhitelistedTokens (Map -> Individual Keys)
### 1. Pre-Migration Checks
Read the legacy configuration Map to ensure total token allocations match current baseline expectations.

### 2. Migration Invocation Command

stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  --network <NETWORK> \
  -- \
  migrate_token_whitelist

### 3. Post-Migration Verification
Confirm that separate storage slot configurations can be fetched individually per token address instead of a singular monolith Map structure.

## Migration 3: ArtisanStakeQueue (Vec -> Indexed Queue)
### 1. Pre-Migration Checks
Assert that the legacy sequential Vec structure does not exceed maximum heap layout sizes, checking data continuity flags.

### 2. Migration Invocation Command

stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  --network <NETWORK> \
  -- \
  migrate_stake_queue

### 3. Post-Migration Verification
Run an index query range verification step to ensure elements read correctly from their respective indexed queue positions without errors.
