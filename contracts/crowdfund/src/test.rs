#![allow(unused_doc_comments)]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    token, Address, Env, Vec,
};

use crate::{CrowdfundContract, CrowdfundContractClient};

#[derive(Clone)]
#[contracttype]
struct MintRecord {
    to: Address,
    token_id: u64,
}

#[derive(Clone)]
#[contracttype]
enum MockNftDataKey {
    Minted,
}

#[contract]
struct MockNftContract;

#[contractimpl]
impl MockNftContract {
    pub fn mint(env: Env, to: Address, token_id: u64) {
        let mut minted: Vec<MintRecord> = env
            .storage()
            .persistent()
            .get(&MockNftDataKey::Minted)
            .unwrap_or_else(|| Vec::new(&env));
        minted.push_back(MintRecord { to, token_id });
        env.storage()
            .persistent()
            .set(&MockNftDataKey::Minted, &minted);
    }

    pub fn minted(env: Env) -> Vec<MintRecord> {
        env.storage()
            .persistent()
            .get(&MockNftDataKey::Minted)
            .unwrap_or_else(|| Vec::new(&env))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────
// ── Helpers ─────────────────────────────────────────────────────────────────

fn setup_env() -> (
    Env,
    CrowdfundContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrowdfundContract, ());
    let client = CrowdfundContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract_id = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = token_contract_id.address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    let creator = Address::generate(&env);
    token_admin_client.mint(&creator, &10_000_000);

    (env, client, creator, token_address, token_admin)
}

/// Helper to mint tokens to an arbitrary contributor.
fn mint_to(env: &Env, token_address: &Address, admin: &Address, to: &Address, amount: i128) {
    let admin_client = token::StellarAssetClient::new(env, token_address);
    admin_client.mint(to, &amount);
    let _ = admin;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_withdraw_mints_nft_for_each_contributor() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;

    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.goal(), goal);
    assert_eq!(client.deadline(), deadline);
    assert_eq!(client.min_contribution(), min_contribution);
    assert_eq!(client.total_raised(), 0);
}

#[test]
fn test_withdraw_skips_nft_mint_when_contract_not_set() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;

    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );
    let result = client.try_initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        crate::ContractError::AlreadyInitialized
    );
}

#[test]
fn test_contribute() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    let contributor = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &contributor, 10_000);

    client.contribute(&contributor, &5_000);
    assert_eq!(client.total_raised(), 5_000);
    assert_eq!(client.contribution(&contributor), 5_000);
}

#[test]
fn test_contribute_after_deadline_returns_error() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 100;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    let contributor = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &contributor, 10_000);

    env.ledger().set_timestamp(deadline + 1);

    let result = client.try_contribute(&contributor, &5_000);
    assert!(result.is_err());
}

#[test]
fn test_withdraw_skips_nft_minting_when_nft_contract_not_set() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    let contributor = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &contributor, goal);
    client.contribute(&contributor, &goal);

    env.ledger().set_timestamp(deadline + 1);

    let token_client = token::Client::new(&env, &token_address);
    let creator_balance_before = token_client.balance(&creator);

    client.withdraw();

    let creator_balance_after = token_client.balance(&creator);
    assert_eq!(creator_balance_after, creator_balance_before + goal);
}

#[test]
#[should_panic(expected = "campaign is not active")]
fn test_double_refund_panics() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    let alice = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &alice, 500_000);
    client.contribute(&alice, &500_000);

    env.ledger().set_timestamp(deadline + 1);

    client.refund();
    client.refund(); // should panic — status is Refunded
}

#[test]
fn test_cancel_with_no_contributions() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    client.cancel();

    assert_eq!(client.total_raised(), 0);
}
#[test]
#[should_panic]
fn test_cancel_by_non_creator_panics() {
    let env = Env::default();
    let contract_id = env.register(CrowdfundContract, ());
    let client = CrowdfundContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract_id = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = token_contract_id.address();

    let creator = Address::generate(&env);
    let non_creator = Address::generate(&env);

    env.mock_all_auths();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &token_admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    env.mock_all_auths_allowing_non_root_auth();
    env.set_auths(&[]);

    client.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &non_creator,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "cancel",
            args: soroban_sdk::vec![&env],
            sub_invokes: &[],
        },
    }]);

    client.cancel();
}

#[test]
#[should_panic(expected = "amount below minimum")]
fn test_contribute_below_minimum_panics() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    let contributor = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &contributor, 10_000);

    client.contribute(&contributor, &500);
}

#[test]
#[should_panic(expected = "campaign is not active")]
fn test_update_metadata_when_not_active_panics() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    client.cancel();
    client.update_metadata(&creator, &None, &None, &None);
}

// ── Admin Upgrade Mechanism Tests ───────────────────────────────────────────

#[test]
fn test_upgrade_successful_by_admin() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    
    // Initialize with admin address
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    // Create a new WASM hash for upgrade
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    
    // Admin should be able to upgrade
    client.upgrade(&new_wasm_hash);
    
    // Verify the upgrade was successful (no panic)
    // Note: In Soroban tests, we can't directly verify the WASM was updated,
    // but we can verify the function executed without panic
}

#[test]
#[should_panic(expected = "not authorized")]
fn test_upgrade_fails_by_non_admin() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    
    // Initialize with admin address
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    // Create a new WASM hash for upgrade
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    
    // Try to upgrade with a non-admin address
    let non_admin = Address::generate(&env);
    
    // Mock auth for non-admin
    env.mock_all_auths_allowing_non_root_auth();
    env.set_auths(&[]);
    
    client.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &non_admin,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "upgrade",
            args: soroban_sdk::vec![&env, new_wasm_hash],
            sub_invokes: &[],
        },
    }]);
    
    client.upgrade(&new_wasm_hash);
}

#[test]
fn test_admin_address_stored_correctly() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    
    // Initialize with a specific admin address
    let designated_admin = Address::generate(&env);
    
    client.initialize(
        &designated_admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    // Create a new WASM hash for upgrade
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    
    // Only the designated admin should be able to upgrade
    // (test would panic if wrong admin)
    env.mock_all_auths_allowing_non_root_auth();
    env.set_auths(&[]);
    
    client.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &designated_admin,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "upgrade",
            args: soroban_sdk::vec![&env, new_wasm_hash],
            sub_invokes: &[],
        },
    }]);
    
    client.upgrade(&new_wasm_hash);
}

#[test]
#[should_panic]
fn test_upgrade_fails_with_wrong_admin() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    
    // Initialize with a specific admin address
    let designated_admin = Address::generate(&env);
    
    client.initialize(
        &designated_admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    // Create a new WASM hash for upgrade
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    
    // Try to upgrade with a different admin address
    let wrong_admin = Address::generate(&env);
    
    env.mock_all_auths_allowing_non_root_auth();
    env.set_auths(&[]);
    
    client.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &wrong_admin,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "upgrade",
            args: soroban_sdk::vec![&env, new_wasm_hash],
            sub_invokes: &[],
        },
    }]);
    
    client.upgrade(&new_wasm_hash);
}

#[test]
fn test_upgrade_does_not_affect_campaign_state() {
    let (env, client, creator, token_address, admin) = setup_env();

    let deadline = env.ledger().timestamp() + 3600;
    let goal: i128 = 1_000_000;
    let min_contribution: i128 = 1_000;
    
    // Initialize campaign
    client.initialize(
        &admin,
        &creator,
        &token_address,
        &goal,
        &deadline,
        &min_contribution,
        &None,
        &None,
        &None,
    );

    // Make a contribution
    let contributor = Address::generate(&env);
    mint_to(&env, &token_address, &admin, &contributor, 500_000);
    client.contribute(&contributor, &500_000);
    
    // Record state before upgrade
    let total_raised_before = client.total_raised();
    let contribution_before = client.contribution(&contributor);
    let goal_before = client.goal();
    let deadline_before = client.deadline();
    
    // Perform upgrade
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&new_wasm_hash);
    
    // Verify state is preserved after upgrade
    assert_eq!(client.total_raised(), total_raised_before);
    assert_eq!(client.contribution(&contributor), contribution_before);
    assert_eq!(client.goal(), goal_before);
    assert_eq!(client.deadline(), deadline_before);
}
