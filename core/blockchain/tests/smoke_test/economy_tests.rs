use crate::helpers::addr;
use isa_chain_core::{
    constants::{INITIAL_SUPPLY, VALIDATOR_MIN_STAKE},
    CreditSystem, StakingVault, TokenState, DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE,
};

// ============================================================================
// 1. Token Economy Flow
// ============================================================================

mod token_economy {
    use super::*;

    /// Full token lifecycle: genesis supply → authorize → mint → burn → accounting.
    #[test]
    fn token_economy_flow() {
        let admin = addr(0xAD);
        let minter = addr(0xB0);
        let burner = addr(0xB1);

        // 1. Create token state with 1B ISA genesis supply.
        let mut ts = TokenState::new(INITIAL_SUPPLY, admin);
        assert_eq!(ts.get_total_supply(), INITIAL_SUPPLY);
        assert_eq!(ts.get_total_minted(), 0);
        assert_eq!(ts.get_total_burned(), 0);

        // 2. Authorize minter and burner.
        ts.authorize_minter(minter, &admin).unwrap();
        ts.authorize_burner(burner, &admin).unwrap();
        assert!(ts.is_authorized_minter(&minter));
        assert!(ts.is_authorized_burner(&burner));

        // 3. Mint tokens to user.
        let mint_amount: u128 = 1_000_000_000_000_000_000_000; // 1000 ISA
        ts.mint(mint_amount, &minter).unwrap();
        assert_eq!(ts.get_total_supply(), INITIAL_SUPPLY + mint_amount);
        assert_eq!(ts.get_total_minted(), mint_amount);

        // 4. Burn tokens (settlement).
        let burn_amount: u128 = 500_000_000_000_000_000_000; // 500 ISA
        ts.burn(burn_amount, &burner).unwrap();
        assert_eq!(
            ts.get_total_supply(),
            INITIAL_SUPPLY + mint_amount - burn_amount
        );
        assert_eq!(ts.get_total_burned(), burn_amount);

        // 5. Supply info snapshot is consistent.
        let info = ts.get_supply_info();
        assert_eq!(info.total_supply, ts.get_total_supply());
        assert_eq!(info.total_minted, mint_amount);
        assert_eq!(info.total_burned, burn_amount);
        assert_eq!(info.circulating_supply, info.total_supply);
    }
}

// ============================================================================
// 2. Staking Flow
// ============================================================================

mod staking_flow {
    use super::*;

    #[test]
    fn staking_full_lifecycle() {
        let provider = addr(0x01);
        let mut vault = StakingVault::default_vault(); // 32k ISA min, 100-block unbond

        // 1. Stake above the minimum.
        vault.stake(provider, VALIDATOR_MIN_STAKE, 1).unwrap();
        assert!(vault.is_staked(&provider));
        assert_eq!(vault.get_total_staked(), VALIDATOR_MIN_STAKE);

        let entry = vault.get_stake(&provider).unwrap();
        assert_eq!(entry.amount, VALIDATOR_MIN_STAKE);

        // 2. Begin unstaking half.
        let unstake_amount = VALIDATOR_MIN_STAKE / 2;
        vault.begin_unstake(&provider, unstake_amount, 100).unwrap();
        assert_eq!(vault.get_total_staked(), VALIDATOR_MIN_STAKE - unstake_amount);

        let entry_after = vault.get_stake(&provider).unwrap();
        assert_eq!(entry_after.unbonding.len(), 1);
        assert_eq!(entry_after.unbonding[0].amount, unstake_amount);
        assert_eq!(entry_after.unbonding[0].completion_height, 200); // 100 + 100

        // 3. Trying to complete before period ends returns 0.
        let early = vault.complete_unbonding(&provider, 150);
        assert_eq!(early, 0);

        // 4. Complete unbonding after period.
        let released = vault.complete_unbonding(&provider, 200);
        assert_eq!(released, unstake_amount);
        assert!(vault.get_stake(&provider).unwrap().unbonding.is_empty());

        // 5. Slash the remaining active stake (10% = 1000 bps).
        let slashed = vault.slash(&provider, 1000).unwrap();
        let remaining_before_slash = VALIDATOR_MIN_STAKE - unstake_amount;
        let expected_slash = remaining_before_slash / 10;
        assert_eq!(slashed, expected_slash);
        assert_eq!(
            vault.get_stake(&provider).unwrap().amount,
            remaining_before_slash - expected_slash
        );
    }
}

// ============================================================================
// 3. Credit System Flow
// ============================================================================

mod credit_system_flow {
    use super::*;

    /// ISA price: $0.50 = 500_000 micro-USD
    const ISA_PRICE_USD: u128 = 500_000;

    #[test]
    fn credit_purchase_and_spend() {
        let admin = addr(0xAD);
        let user = addr(0xC0);

        // 1. Create CreditSystem with $0.00001 per credit.
        let mut cs = CreditSystem::new(DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE, admin);
        assert_eq!(cs.credit_price_usd, DEFAULT_CREDIT_PRICE_USD);

        // 2. Purchase credits: 1 ISA (1_000_000 micro-ISA) at $0.50
        //    credits = (1_000_000 * 500_000) / (100 * 1_000_000) = 5_000
        let isa_amount: u128 = 1_000_000;
        let credits = cs
            .purchase_credits(user, isa_amount, ISA_PRICE_USD, 10)
            .unwrap();
        assert_eq!(credits, 5_000, "1 ISA at $0.50 should yield 5_000 credits");
        assert_eq!(cs.get_balance(&user), 5_000);

        // 3. Spend some credits.
        let spend = 2_500;
        cs.spend_credits(&user, spend).unwrap();
        assert_eq!(cs.get_balance(&user), 2_500);

        // 4. Verify total accounting.
        assert_eq!(cs.total_credits_issued, 5_000);
        assert_eq!(cs.total_credits_burned, 2_500);
        assert_eq!(cs.total_credits_in_circulation(), 2_500);

        let acct = cs.get_account(&user).unwrap();
        assert_eq!(acct.total_credits_purchased, 5_000);
        assert_eq!(acct.total_credits_spent, 2_500);
    }

    #[test]
    fn credit_conversion_math() {
        // Verify the key math from the spec comment:
        // "1 ISA at $0.50 = 5000 credits"
        let credits = CreditSystem::credits_for_isa(1_000_000, ISA_PRICE_USD);
        assert_eq!(credits, 5_000);
    }
}
