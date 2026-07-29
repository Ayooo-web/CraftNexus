//! Issue #719 – Ensure every client error code in `Error` is exercised by at
//! least one unit test.
//!
//! This module adds a dedicated test for each `Error` variant that was
//! previously uncovered. The groupings mirror the error-code categories in
//! `Error` (Auth/Access, State/Transition, Config/Resource, Operational/Gates,
//! Validation).

#![cfg(test)]
extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, vec, Address, Bytes, BytesN, Env, String, Symbol,
};

// ─── Helpers ──────────────────────────────────────────────────────────────

/// Minimal test setup: initialise the platform, create a token, and return
/// the most commonly needed handles.
fn setup(
    env: &Env,
) -> (
    CraftNexusContractClient<'static>,
    Address,
    Address,
    Address,
    token::StellarAssetClient<'static>,
    Address,
    Address,
) {
    env.budget().reset_unlimited();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CraftNexusContract);
    let client = CraftNexusContractClient::new(env, &contract_id);

    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let admin = Address::generate(env);
    let arbitrator = Address::generate(env);

    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(env, &token_contract.address());

    let onboarding_contract = Address::generate(env);

    env.ledger().with_mut(|li| {
        li.timestamp = 1_711_368_000; // 2024-03-25
    });

    client.initialize(
        &platform_wallet,
        &admin,
        &arbitrator,
        &500,
        &Some(onboarding_contract),
    );
    client.set_min_escrow_amount(&token_contract.address(), &0);
    client.set_min_release_window(&1);

    (
        client,
        buyer,
        seller,
        token_contract.address(),
        token_admin_client,
        platform_wallet,
        admin,
    )
}

/// Create an active escrow and return its order ID.
fn create_active_escrow(
    client: &CraftNexusContractClient,
    buyer: &Address,
    seller: &Address,
    token: &Address,
    token_admin: &token::StellarAssetClient,
    amount: i128,
    order_id: u32,
) -> u32 {
    token_admin.mint(buyer, &amount);
    client.create_escrow(buyer, seller, token, &amount, &order_id, &None);
    order_id
}

// ═══════════════════════════════════════════════════════════════════════════
// Auth / Access (1–9)
// ═══════════════════════════════════════════════════════════════════════════

// ── Error::InvalidEscrowState (3) ─────────────────────────────────────────
// Disputing an already-released escrow triggers InvalidEscrowState.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_error_invalid_escrow_state_dispute_released() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    create_active_escrow(&client, &buyer, &seller, &token_id, &token_admin, 1_000, 1);

    // Release the escrow first
    client.release_funds(&1);

    // Attempting to dispute a released escrow panics with InvalidEscrowState (#3)
    client.dispute_escrow(&1, &Symbol::new(&env, "test"), &buyer);
}

// ── Error::TokenNotWhitelisted (5) ────────────────────────────────────────
// When at least one token is whitelisted, creating an escrow with an
// un-whitelisted token panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_error_token_not_whitelisted() {
    let env = Env::default();
    let (client, buyer, seller, token_id, _, _, _) = setup(&env);

    // Whitelist the existing token so enforcement becomes active
    client.whitelist_token(&token_id);

    // Register a *different* token that is NOT whitelisted
    let other_admin = Address::generate(&env);
    let other_token = env.register_stellar_asset_contract_v2(other_admin.clone());
    let other_token_admin = token::StellarAssetClient::new(&env, &other_token.address());
    other_token_admin.mint(&buyer, &1_000);

    // Creating an escrow with the non-whitelisted token should panic (#5)
    client.create_escrow(&buyer, &seller, &other_token.address(), &500, &1, &None);
}

// ── Error::UsernameAlreadyExists (4) — DEPRECATED ─────────────────────────
// Not returned by any contract function; assert the discriminant only.
#[test]
fn test_error_username_already_exists_discriminant() {
    assert_eq!(Error::UsernameAlreadyExists as u32, 4);
}

// ── Error::NotInDispute (8) — UNUSED ──────────────────────────────────────
// Not returned by any contract function; assert the discriminant only.
#[test]
fn test_error_not_in_dispute_discriminant() {
    assert_eq!(Error::NotInDispute as u32, 8);
}

// ── Error::AlreadyOnboarded (9) — DEPRECATED ─────────────────────────────
#[test]
fn test_error_already_onboarded_discriminant() {
    assert_eq!(Error::AlreadyOnboarded as u32, 9);
}

// ═══════════════════════════════════════════════════════════════════════════
// State / Transition (10–19)
// ═══════════════════════════════════════════════════════════════════════════

// ── Error::PlatformNotInitialized (12) ────────────────────────────────────
// Calling `cancel_upgrade_wasm` on an un-initialised contract returns the
// error via `get_admin → PlatformNotInitialized`.
#[test]
fn test_error_platform_not_initialized() {
    let env = Env::default();
    env.budget().reset_unlimited();
    env.mock_all_auths();

    // Register the contract but do NOT call `initialize`.
    let contract_id = env.register_contract(None, CraftNexusContract);
    let client = CraftNexusContractClient::new(&env, &contract_id);

    let result = client.try_cancel_upgrade_wasm();
    assert!(matches!(result, Err(Ok(Error::PlatformNotInitialized))));
}

// ── Error::ReleaseWindowNotElapsed (13) ───────────────────────────────────
// Calling `auto_release` before the window expires panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #13)")]
fn test_error_release_window_not_elapsed() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);

    // Use a large release window
    token_admin.mint(&buyer, &1_000);
    client.create_escrow(&buyer, &seller, &token_id, &1_000, &1, &Some(604800));

    // auto_release without advancing time should panic (#13)
    client.auto_release(&1);
}

// ── Error::BatchOperationFailed (14) — DEPRECATED ─────────────────────────
#[test]
fn test_error_batch_operation_failed_discriminant() {
    assert_eq!(Error::BatchOperationFailed as u32, 14);
}

// ── Error::ContractPaused (15) ────────────────────────────────────────────
// Creating an escrow while the contract is paused panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #15)")]
fn test_error_contract_paused() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&buyer, &1_000);

    // Admin pauses the contract
    client.set_paused(&true);

    // Attempting to create an escrow should panic (#15)
    client.create_escrow(&buyer, &seller, &token_id, &1_000, &1, &None);
}

// ── Error::DisputeExpired (16) ────────────────────────────────────────────
// Calling `resolve_expired_dispute` before the deadline returns the error.
#[test]
fn test_error_dispute_expired_not_yet() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    create_active_escrow(&client, &buyer, &seller, &token_id, &token_admin, 1_000, 1);

    // Dispute the escrow
    client.dispute_escrow(&1, &Symbol::new(&env, "test"), &buyer);

    // Without advancing time past max_dispute_duration, resolve_expired_dispute
    // should return DisputeExpired (#16).
    let result = client.try_resolve_expired_dispute(&1);
    assert!(matches!(result, Err(Ok(Error::DisputeExpired))));
}

// ── Error::InsufficientStake (17) ─────────────────────────────────────────
// When `min_stake_required > 0` and the seller has no stake, creating an
// escrow panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_error_insufficient_stake() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&buyer, &10_000);

    // Set a minimum stake requirement
    client.set_min_stake_required(&5_000);

    // seller has zero stake → panics (#17)
    client.create_escrow(&buyer, &seller, &token_id, &5_000, &1, &None);
}

// ── Error::StakeCooldownActive (18) ───────────────────────────────────────
// Unstaking immediately after staking panics because the cooldown hasn't
// elapsed.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_error_stake_cooldown_active() {
    let env = Env::default();
    let (client, _, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&seller, &5_000);

    // Stake tokens (starts cooldown)
    client.stake_tokens(&seller, &token_id, &5_000);

    // Immediately attempting to unstake should panic (#18)
    client.unstake_tokens(&seller, &token_id);
}

// ── Error::InvalidRefundAmount (19) ───────────────────────────────────────
// Proposing a partial refund with amount 0 triggers the error.
#[test]
fn test_error_invalid_refund_amount() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    create_active_escrow(&client, &buyer, &seller, &token_id, &token_admin, 1_000, 1);

    // Dispute the escrow so partial refund is allowed
    client.dispute_escrow(&1, &Symbol::new(&env, "test"), &buyer);

    // Propose with amount 0 → InvalidRefundAmount
    let result = client.try_propose_partial_refund(&1, &0, &buyer);
    assert!(matches!(result, Err(Ok(Error::InvalidRefundAmount))));
}

// ═══════════════════════════════════════════════════════════════════════════
// Config / Resource (20–29)
// ═══════════════════════════════════════════════════════════════════════════

// ── Error::ProposalNotFound (20) ──────────────────────────────────────────
// Accepting a partial refund when none has been proposed returns the error.
#[test]
fn test_error_proposal_not_found() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    create_active_escrow(&client, &buyer, &seller, &token_id, &token_admin, 1_000, 1);

    // Dispute the escrow
    client.dispute_escrow(&1, &Symbol::new(&env, "test"), &buyer);

    // Accept without a proposal → ProposalNotFound (#20)
    let result = client.try_accept_partial_refund(&1);
    assert!(matches!(result, Err(Ok(Error::ProposalNotFound))));
}

// ── Error::StakeTokenMismatch (24) ────────────────────────────────────────
// Staking in one token then trying to stake more in a different token panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #24)")]
fn test_error_stake_token_mismatch() {
    let env = Env::default();
    let (client, _buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&seller, &5_000);

    // Stake with the first token
    client.stake_tokens(&seller, &token_id, &3_000);

    // Create a different token
    let other_admin = Address::generate(&env);
    let other_token = env.register_stellar_asset_contract_v2(other_admin.clone());
    let other_client = token::StellarAssetClient::new(&env, &other_token.address());
    other_client.mint(&seller, &5_000);

    // Stake with a different token → StakeTokenMismatch (#24)
    client.stake_tokens(&seller, &other_token.address(), &2_000);
}

// ── Error::InvalidAdminAddress (25) ───────────────────────────────────────
// Trying to transfer admin to the contract's own address panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #25)")]
fn test_error_invalid_admin_address() {
    let env = Env::default();
    env.budget().reset_unlimited();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CraftNexusContract);
    let client = CraftNexusContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let platform_wallet = Address::generate(&env);
    let arbitrator = Address::generate(&env);

    client.initialize(&platform_wallet, &admin, &arbitrator, &500, &None);

    // Transfer admin to the contract itself → InvalidAdminAddress (#25)
    client.update_admin(&contract_id);
}

// ── Error::CorruptedPlatformConfig (26) ───────────────────────────────────
// `get_platform_config_safe` returns this when no config or fallback exists.
// This is a private function, so we test the discriminant and the is_retryable
// classification.
#[test]
fn test_error_corrupted_platform_config_discriminant() {
    assert_eq!(Error::CorruptedPlatformConfig as u32, 26);
    // CorruptedPlatformConfig is NOT retryable (operator must act)
    assert!(!is_retryable(Error::CorruptedPlatformConfig));
}

// ── Error::StakeQueueFull (27) ────────────────────────────────────────────
// Fill the stake history queue beyond MAX_STAKE_HISTORY_SIZE and then
// observe the error. Since the queue prunes at 80, we need to fill it to 100.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #27)")]
fn test_error_stake_queue_full() {
    let env = Env::default();
    let (client, _buyer, seller, token_id, token_admin, _, _) = setup(&env);

    // Pre-fill the stake history count to MAX_STAKE_HISTORY_SIZE
    // to simulate a full queue
    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::StakeHistoryCount(seller.clone()),
            &MAX_STAKE_HISTORY_SIZE,
        );
    });

    // Staking should fail because the history queue is full (#27)
    token_admin.mint(&seller, &1_000);
    client.stake_tokens(&seller, &token_id, &1_000);
}

// ── Error::BatchLimitExceeded (29) ────────────────────────────────────────
// Attempting to create more than MAX_BATCH_SIZE escrows returns the error.
#[test]
fn test_error_batch_limit_exceeded() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&buyer, &1_000_000);

    // Build a batch of MAX_BATCH_SIZE + 1 escrows
    let mut params = soroban_sdk::Vec::new(&env);
    for i in 0..(MAX_BATCH_SIZE + 1) {
        params.push_back(EscrowCreateParams {
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token_id.clone(),
            amount: 100,
            order_id: i + 100,
            release_window: None,
            ipfs_hash: None,
            metadata_hash: None,
        });
    }

    let result = client.try_create_batch_escrow(&0, &params);
    assert!(matches!(result, Err(Ok(Error::BatchLimitExceeded))));
}

// ═══════════════════════════════════════════════════════════════════════════
// Operational / Gates (30–39)
// ═══════════════════════════════════════════════════════════════════════════

// ── Error::DeprecatedFunction (30) — UNUSED ───────────────────────────────
#[test]
fn test_error_deprecated_function_discriminant() {
    assert_eq!(Error::DeprecatedFunction as u32, 30);
}

// ── Error::NoPendingAdmin (31) ────────────────────────────────────────────
// Cancelling admin transfer when no transfer is pending returns the error.
#[test]
fn test_error_no_pending_admin() {
    let env = Env::default();
    let (client, _, _, _, _, _, _) = setup(&env);

    // No pending admin transfer exists
    let result = client.try_cancel_admin_transfer();
    assert!(matches!(result, Err(Ok(Error::NoPendingAdmin))));
}

// ── Error::NoUpgradeProposed (32) ─────────────────────────────────────────
// Cancelling an upgrade when none is proposed returns the error.
#[test]
fn test_error_no_upgrade_proposed() {
    let env = Env::default();
    let (client, _, _, _, _, _, _) = setup(&env);

    let result = client.try_cancel_upgrade_wasm();
    assert!(matches!(result, Err(Ok(Error::NoUpgradeProposed))));
}

// ── Error::UpgradeCooldownActive (33) ─────────────────────────────────────
// Re-proposing immediately after a cancellation returns the error.
#[test]
fn test_error_upgrade_cooldown_active() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);

    // Propose and cancel so the cancel-cooldown starts
    client.propose_upgrade_wasm(&admin, &hash);
    client.cancel_upgrade_wasm();

    // Re-proposing immediately should fail with UpgradeCooldownActive (#33)
    let result = client.try_propose_upgrade_wasm(&admin, &hash);
    assert!(matches!(result, Err(Ok(Error::UpgradeCooldownActive))));
}

// ── Error::UpgradeProposalExists (34) ─────────────────────────────────────
// Proposing a second upgrade while one is already pending returns the error.
#[test]
fn test_error_upgrade_proposal_exists() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let hash1 = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let hash2 = BytesN::<32>::from_array(&env, &[2u8; 32]);

    client.propose_upgrade_wasm(&admin, &hash1);

    // Second proposal should fail (#34)
    let result = client.try_propose_upgrade_wasm(&admin, &hash2);
    assert!(matches!(result, Err(Ok(Error::UpgradeProposalExists))));
}

// ── Error::InvalidUpgradeHash (35) ────────────────────────────────────────
// Proposing a WASM upgrade with the all-zero hash returns the error.
#[test]
fn test_error_invalid_upgrade_hash_propose() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let zero_hash = BytesN::<32>::from_array(&env, &[0u8; 32]);

    let result = client.try_propose_upgrade_wasm(&admin, &zero_hash);
    assert!(matches!(result, Err(Ok(Error::InvalidUpgradeHash))));
}

// ── Error::InvalidUpgradeHash (35) — execute path ─────────────────────────
// Executing with a mismatched hash returns the error.
#[test]
fn test_error_invalid_upgrade_hash_execute() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    client.propose_upgrade_wasm(&admin, &hash);

    // Advance time past cooldown
    env.ledger().with_mut(|li| {
        li.timestamp += (DEFAULT_WASM_UPGRADE_COOLDOWN as u64) + 1;
    });

    // Execute with a different hash → InvalidUpgradeHash
    let wrong_hash = BytesN::<32>::from_array(&env, &[2u8; 32]);
    let result = client.try_execute_upgrade(&wrong_hash);
    assert!(matches!(result, Err(Ok(Error::InvalidUpgradeHash))));
}

// ── Error::RecurringEscrowNotFound (36) ───────────────────────────────────
// Releasing a cycle for a non-existent recurring escrow panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #36)")]
fn test_error_recurring_escrow_not_found() {
    let env = Env::default();
    let (client, _, _, _, _, _, _) = setup(&env);

    // No recurring escrow with ID 999 exists
    client.release_next_cycle(&999);
}

// ── Error::CycleNotReady (37) ─────────────────────────────────────────────
// Releasing a cycle before the frequency period has elapsed panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #37)")]
fn test_error_cycle_not_ready() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&buyer, &10_000);

    // Create a recurring escrow: 2 cycles, 1 hour frequency
    let escrow = client.create_recurring_escrow(
        &buyer, &seller, &token_id, &10_000, &3600, &2,
    );

    // Without advancing time, try to release the next cycle → CycleNotReady (#37)
    client.release_next_cycle(&escrow.id);
}

// ── Error::OnboardingContractNotSet (39) ──────────────────────────────────
// Clearing the onboarding contract, then calling get_onboarding_contract
// returns the error.
#[test]
fn test_error_onboarding_contract_not_set() {
    let env = Env::default();
    let (client, _, _, _, _, _, _) = setup(&env);

    // Clear the onboarding contract
    client.clear_onboarding_contract();

    // Now trying to get it should fail
    let result = client.try_get_onboarding_contract();
    assert!(matches!(result, Err(Ok(Error::OnboardingContractNotSet))));
}

// ── Error::OnboardingContractNotSet (39) — clear twice ────────────────────
// Clearing when already cleared also returns OnboardingContractNotSet.
#[test]
fn test_error_onboarding_contract_not_set_double_clear() {
    let env = Env::default();
    let (client, _, _, _, _, _, _) = setup(&env);

    client.clear_onboarding_contract();

    // Second clear should fail because it's already unset
    let result = client.try_clear_onboarding_contract();
    assert!(matches!(result, Err(Ok(Error::OnboardingContractNotSet))));
}

// ═══════════════════════════════════════════════════════════════════════════
// Validation (40+)
// ═══════════════════════════════════════════════════════════════════════════

// ── Error::InvalidIpfsHash (41) ───────────────────────────────────────────
// Creating an escrow with an obviously malformed IPFS hash panics.
#[test]
#[should_panic(expected = "HostError: Error(Contract, #41)")]
fn test_error_invalid_ipfs_hash() {
    let env = Env::default();
    let (client, buyer, seller, token_id, token_admin, _, _) = setup(&env);
    token_admin.mint(&buyer, &1_000);

    // "INVALID" is not a valid CIDv0 or CIDv1 hash
    let bad_hash = String::from_str(&env, "INVALID");
    client.create_escrow_with_metadata(
        &buyer,
        &seller,
        &token_id,
        &1_000,
        &1,
        &None,
        &Some(bad_hash),
        &None,
    );
}

// ── Error::NotAnUpgradeSigner (42) ────────────────────────────────────────
// A non-signer trying to approve an upgrade proposal returns the error.
#[test]
fn test_error_not_an_upgrade_signer() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let random = Address::generate(&env);

    // Configure an explicit signers list with threshold 2
    let signers = vec![&env, admin.clone(), Address::generate(&env)];
    client.set_upgrade_signers(&signers);
    client.set_upgrade_threshold(&2);

    // `random` is not in the signers list
    let result = client.try_propose_upgrade_wasm(&random, &hash);
    assert!(matches!(result, Err(Ok(Error::NotAnUpgradeSigner))));
}

// ── Error::AlreadyApproved (43) ───────────────────────────────────────────
// The same signer approving the same upgrade twice returns the error.
#[test]
fn test_error_already_approved() {
    let env = Env::default();
    let (client, _, _, _, _, _, admin) = setup(&env);

    let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let signer2 = Address::generate(&env);

    // Configure multi-sig: admin + signer2, threshold = 2
    let signers = vec![&env, admin.clone(), signer2.clone()];
    client.set_upgrade_signers(&signers);
    client.set_upgrade_threshold(&2);

    // First approval by admin succeeds
    client.propose_upgrade_wasm(&admin, &hash);

    // Second approval by admin should fail (#43)
    let result = client.try_propose_upgrade_wasm(&admin, &hash);
    assert!(matches!(result, Err(Ok(Error::AlreadyApproved))));
}

// ═══════════════════════════════════════════════════════════════════════════
// Meta: `is_retryable` coverage
// ═══════════════════════════════════════════════════════════════════════════

/// Assert that every retryable error is correctly classified.
#[test]
fn test_is_retryable_classification() {
    // Retryable errors
    assert!(is_retryable(Error::InvalidEscrowState));
    assert!(is_retryable(Error::ReleaseWindowNotElapsed));
    assert!(is_retryable(Error::ContractPaused));
    assert!(is_retryable(Error::DisputeExpired));
    assert!(is_retryable(Error::StakeCooldownActive));
    assert!(is_retryable(Error::ReentryDetected));
    assert!(is_retryable(Error::StakeQueueFull));
    assert!(is_retryable(Error::UpgradeCooldownActive));
    assert!(is_retryable(Error::CycleNotReady));
    assert!(is_retryable(Error::BatchLimitExceeded));

    // Non-retryable errors (spot-check)
    assert!(!is_retryable(Error::Unauthorized));
    assert!(!is_retryable(Error::EscrowNotFound));
    assert!(!is_retryable(Error::TokenNotWhitelisted));
    assert!(!is_retryable(Error::InvalidFee));
    assert!(!is_retryable(Error::NoPendingAdmin));
    assert!(!is_retryable(Error::InvalidUpgradeHash));
    assert!(!is_retryable(Error::OnboardingContractNotSet));
    assert!(!is_retryable(Error::InvalidIpfsHash));
    assert!(!is_retryable(Error::NotAnUpgradeSigner));
    assert!(!is_retryable(Error::AlreadyApproved));
    assert!(!is_retryable(Error::InvalidTokenDecimals));
    assert!(!is_retryable(Error::StorageLayoutMismatch));
    assert!(!is_retryable(Error::UnsupportedToken));
}

/// Exhaustive discriminant test: every Error variant maps to the expected code.
/// This guards against accidental reordering or renumbering of ABI codes.
#[test]
fn test_all_error_discriminants() {
    assert_eq!(Error::Unauthorized as u32, 1);
    assert_eq!(Error::EscrowNotFound as u32, 2);
    assert_eq!(Error::InvalidEscrowState as u32, 3);
    assert_eq!(Error::UsernameAlreadyExists as u32, 4);
    assert_eq!(Error::TokenNotWhitelisted as u32, 5);
    assert_eq!(Error::AmountBelowMinimum as u32, 6);
    assert_eq!(Error::ReleaseWindowTooLong as u32, 7);
    assert_eq!(Error::NotInDispute as u32, 8);
    assert_eq!(Error::AlreadyOnboarded as u32, 9);
    assert_eq!(Error::InvalidFee as u32, 10);
    assert_eq!(Error::SameBuyerSeller as u32, 11);
    assert_eq!(Error::PlatformNotInitialized as u32, 12);
    assert_eq!(Error::ReleaseWindowNotElapsed as u32, 13);
    assert_eq!(Error::BatchOperationFailed as u32, 14);
    assert_eq!(Error::ContractPaused as u32, 15);
    assert_eq!(Error::DisputeExpired as u32, 16);
    assert_eq!(Error::InsufficientStake as u32, 17);
    assert_eq!(Error::StakeCooldownActive as u32, 18);
    assert_eq!(Error::InvalidRefundAmount as u32, 19);
    assert_eq!(Error::ProposalNotFound as u32, 20);
    assert_eq!(Error::ProposalAlreadyExists as u32, 21);
    assert_eq!(Error::ReentryDetected as u32, 22);
    assert_eq!(Error::ReleaseWindowTooShort as u32, 23);
    assert_eq!(Error::StakeTokenMismatch as u32, 24);
    assert_eq!(Error::InvalidAdminAddress as u32, 25);
    assert_eq!(Error::CorruptedPlatformConfig as u32, 26);
    assert_eq!(Error::StakeQueueFull as u32, 27);
    assert_eq!(Error::AdminRecoveryFailed as u32, 28);
    assert_eq!(Error::BatchLimitExceeded as u32, 29);
    assert_eq!(Error::DeprecatedFunction as u32, 30);
    assert_eq!(Error::NoPendingAdmin as u32, 31);
    assert_eq!(Error::NoUpgradeProposed as u32, 32);
    assert_eq!(Error::UpgradeCooldownActive as u32, 33);
    assert_eq!(Error::UpgradeProposalExists as u32, 34);
    assert_eq!(Error::InvalidUpgradeHash as u32, 35);
    assert_eq!(Error::RecurringEscrowNotFound as u32, 36);
    assert_eq!(Error::CycleNotReady as u32, 37);
    assert_eq!(Error::RecurringEscrowIdExhausted as u32, 38);
    assert_eq!(Error::OnboardingContractNotSet as u32, 39);
    assert_eq!(Error::InvalidMetadataHash as u32, 40);
    assert_eq!(Error::InvalidIpfsHash as u32, 41);
    assert_eq!(Error::NotAnUpgradeSigner as u32, 42);
    assert_eq!(Error::AlreadyApproved as u32, 43);
    assert_eq!(Error::InvalidTokenDecimals as u32, 44);
    assert_eq!(Error::StorageLayoutMismatch as u32, 45);
    assert_eq!(Error::UnsupportedToken as u32, 46);
}
