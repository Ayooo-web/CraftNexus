# Pull Request

## Issue(s)
<!-- Link to GitHub issue(s) this PR addresses -->
- Closes # (replace with actual issue number)

## Summary
This PR resolves multiple build errors in the CraftNexus codebase:
1. Fixed missing `use soroban_sdk::vec;` import in `min_release_window_test.rs` which caused macro resolution failures
2. Resolved E0609 errors in `test.rs` by adding `.unwrap()` before field access
3. Fixed E0599 errors in `onboarding.rs` by importing `ToString`
4. Fixed E0382 moved value error in `expired_dispute_fee_test.rs` with `.clone()`
5. Fixed unclosed delimiter in `onboarding_test.rs`

## Context and Background
`cargo check --tests` reported build errors at multiple locations:
- `src/min_release_window_test.rs` lines 339 and 437: Missing `vec!` macro import
- `src/test.rs` lines 2559, 2567, 2584, 2592, 2601, 2609: E0609 no field on `Option`
- `src/onboarding.rs` lines 2149, 2255, 3395: E0599 no method `to_string`
- `src/expired_dispute_fee_test.rs` line 68: E0382 moved value
- `src/onboarding_test.rs` line 2157: Unclosed delimiter

## Changes Made
- Add `use soroban_sdk::vec;` at the top of `min_release_window_test.rs`
- Add `.unwrap()` before `.1`/`.2` field accesses in `test.rs`
- Add `use crate::alloc::string::ToString;` in `onboarding.rs`
- Add `.clone()` before first move in `expired_dispute_fee_test.rs:68`
- Add missing closing `}` in `onboarding_test.rs:2157`

## Validation
- [x] `cargo check --tests` passes
- [x] `cargo test` passes
- [ ] `cargo build --target wasm32-unknown-unknown --release` succeeds
- [x] Snapshot files are unchanged or intentionally updated
- [ ] Documentation is updated (RustDoc, README, etc.)

## PR Checklist
- [x] I have read the contributing guidelines
- [x] My code follows the project's style guidelines
- [x] I have performed a self-review of my code
- [ ] I have added necessary tests
- [x] All tests pass
- [x] Code is properly linted

## Additional Context
<!-- Any other relevant context, screenshots, or links -->
