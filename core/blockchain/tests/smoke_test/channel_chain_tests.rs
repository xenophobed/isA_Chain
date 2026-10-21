use crate::helpers::addr;
use isa_chain_core::{
    constants::{INITIAL_SUPPLY, MAIN_CHAIN_ID},
    Address, Blockchain, ChannelManager, ChannelStatus, ChannelUpdate, GenesisConfig, Hash,
};

// ============================================================================
// 8. Genesis Config Validation
// ============================================================================

mod genesis_config {
    use super::*;

    #[test]
    fn default_mainnet_genesis_is_valid() {
        let config = GenesisConfig::default_mainnet();

        // 1. Validates successfully.
        config.validate().expect("mainnet genesis should be valid");

        // 2. Total allocated equals INITIAL_SUPPLY (1B ISA).
        assert_eq!(
            config.total_allocated(),
            INITIAL_SUPPLY,
            "total allocated must equal 1B ISA"
        );
        assert_eq!(config.initial_supply, INITIAL_SUPPLY);

        // 3. Verify allocation percentages match PRD (40/20/15/15/10).
        //    Treasury immediate = 15%.
        let treasury_expected = INITIAL_SUPPLY * 15 / 100;
        let treasury_alloc = config.allocations.iter().map(|a| a.amount).sum::<u128>();
        assert_eq!(treasury_alloc, treasury_expected, "treasury should be 15%");

        //    Vesting schedules: ecosystem(40) + team(20) + provider_incentives(15) + early_supporters(10).
        let vested_total: u128 = config.vesting_schedules.iter().map(|v| v.total_amount).sum();
        let expected_vested = INITIAL_SUPPLY * 85 / 100; // everything except 15% treasury
        assert_eq!(vested_total, expected_vested, "vested should be 85%");

        // 4. Chain ID is mainnet.
        assert_eq!(config.chain_id, MAIN_CHAIN_ID);
    }
}

// ============================================================================
// 9. Payment Channel Flow
// ============================================================================

mod payment_channel_flow {
    use super::*;

    const DEPOSIT: u128 = 10_000;
    const DISPUTE_PERIOD: u64 = 50;
    const DEFAULT_EXPIRY: u64 = 100_000;

    #[test]
    fn channel_open_update_close_finalize() {
        let sender = addr(0xAA);
        let receiver = addr(0xBB);

        let mut mgr = ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY);

        // 1. Open channel between two parties.
        let channel_id = mgr
            .open_channel(sender, receiver, DEPOSIT, 100)
            .unwrap();

        let ch = mgr.get_channel(&channel_id).unwrap();
        assert_eq!(ch.sender, sender);
        assert_eq!(ch.receiver, receiver);
        assert_eq!(ch.deposit, DEPOSIT);
        assert_eq!(ch.sender_balance, DEPOSIT);
        assert_eq!(ch.receiver_balance, 0);
        assert_eq!(ch.status, ChannelStatus::Open);

        // 2. Update state: transfer 3000 to receiver.
        let update = ChannelUpdate {
            channel_id,
            nonce: 1,
            sender_balance: 7_000,
            receiver_balance: 3_000,
        };
        mgr.update_channel(&channel_id, update).unwrap();

        let ch = mgr.get_channel(&channel_id).unwrap();
        assert_eq!(ch.nonce, 1);
        assert_eq!(ch.sender_balance, 7_000);
        assert_eq!(ch.receiver_balance, 3_000);

        // 3. Close channel.
        mgr.close_channel(&channel_id, &sender, 200).unwrap();
        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Closing { initiated_at: 200 }
        );

        // 4. Finalize after dispute period (200 + 50 = 250).
        let (refund, payout) = mgr.finalize_channel(&channel_id, 250).unwrap();

        // 5. Verify refund + payout = deposit.
        assert_eq!(refund, 7_000);
        assert_eq!(payout, 3_000);
        assert_eq!(refund + payout, DEPOSIT);
        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Closed
        );
    }

    #[test]
    fn channel_dispute_flow() {
        let sender = addr(0xAA);
        let receiver = addr(0xBB);
        let mut mgr = ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY);

        let channel_id = mgr.open_channel(sender, receiver, DEPOSIT, 100).unwrap();

        // Advance state to nonce 1.
        mgr.update_channel(
            &channel_id,
            ChannelUpdate {
                channel_id,
                nonce: 1,
                sender_balance: 6_000,
                receiver_balance: 4_000,
            },
        )
        .unwrap();

        // Sender tries to close at stale state.
        mgr.close_channel(&channel_id, &sender, 200).unwrap();

        // Receiver disputes with a newer nonce.
        mgr.dispute_channel(
            &channel_id,
            ChannelUpdate {
                channel_id,
                nonce: 2,
                sender_balance: 2_000,
                receiver_balance: 8_000,
            },
            &receiver,
            210,
        )
        .unwrap();

        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Disputed { disputed_at: 210 }
        );

        // Finalize after dispute period from dispute time (210 + 50 = 260).
        let (refund, payout) = mgr.finalize_channel(&channel_id, 260).unwrap();
        assert_eq!(refund, 2_000);
        assert_eq!(payout, 8_000);
        assert_eq!(refund + payout, DEPOSIT);
    }
}

// ============================================================================
// 10. Blockchain Integration
// ============================================================================

mod blockchain_integration {
    use super::*;

    #[test]
    fn blockchain_struct_accessible_fields() {
        let mut chain = Blockchain::new(MAIN_CHAIN_ID);

        // 1. Treasury and staking_vault are accessible.
        let _treasury = chain.treasury();
        let _vault = chain.staking_vault();

        // 2. token_state is accessible.
        assert_eq!(chain.token_state().get_total_supply(), INITIAL_SUPPLY);

        // 3. Mint tokens via blockchain (genesis-level, bypasses authority).
        let recipient = addr(0x42);
        let amount: u128 = 1_000_000_000_000_000_000_000;
        chain.mint(recipient, amount);
        assert_eq!(chain.get_balance(&recipient), amount);

        // 4. Authorize a minter via token_state_mut, then mint via mint_tokens.
        let _admin = addr(0xAD);
        let minter = addr(0xB0);

        // First configure admin by creating a fresh token state — Blockchain uses Address::ZERO
        // as default admin, so we use the public token_state_mut to authorize.
        chain
            .token_state_mut()
            .authorize_minter(minter, &Address::ZERO)
            .unwrap();

        let recipient2 = addr(0x43);
        let mint_amount: u128 = 5_000_000_000_000_000_000_000;
        chain.mint_tokens(recipient2, mint_amount, &minter).unwrap();
        assert_eq!(chain.get_balance(&recipient2), mint_amount);
        assert_eq!(
            chain.token_state().get_total_supply(),
            INITIAL_SUPPLY + mint_amount
        );

        // 5. Build and produce a block (empty block is valid).
        let _block = chain.build_block(100).unwrap();
        let block_hash = chain.produce_block(100).unwrap();
        assert_ne!(block_hash, Hash::ZERO);
        assert_eq!(chain.get_height(), 1);
    }

    #[test]
    fn blockchain_height_and_genesis() {
        let chain = Blockchain::new(MAIN_CHAIN_ID);

        assert_eq!(chain.get_height(), 0);
        assert_ne!(chain.genesis_hash, Hash::ZERO);
        // Genesis block is retrievable.
        assert!(chain.get_block_by_height(0).is_some());
    }

    #[test]
    fn blockchain_burn_tokens() {
        let mut chain = Blockchain::new(MAIN_CHAIN_ID);

        let burner = addr(0xB1);
        let user = addr(0xC0);
        let amount: u128 = 10_000_000_000_000_000_000_000;

        // Fund the user directly.
        chain.mint(user, amount);

        // Authorize burner.
        chain
            .token_state_mut()
            .authorize_burner(burner, &Address::ZERO)
            .unwrap();

        // Burn half.
        let burn_amount = amount / 2;
        chain.burn_tokens(user, burn_amount, &burner).unwrap();

        assert_eq!(chain.get_balance(&user), amount - burn_amount);
        assert_eq!(chain.token_state().get_total_burned(), burn_amount);
        assert_eq!(
            chain.token_state().get_total_supply(),
            INITIAL_SUPPLY - burn_amount
        );
    }
}
