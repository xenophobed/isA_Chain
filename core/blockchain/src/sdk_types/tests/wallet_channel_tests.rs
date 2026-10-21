use super::*;
use crate::sdk_types::types::{
    ChannelHubConfig, ChannelManagerAction, ChannelManagerConfig, CreditTopUpRequest,
    PaymentMethod, WalletEndpoint,
};
use crate::settlement::ServiceType;

#[test]
fn test_wallet_endpoints() {
    let addr = zero_addr();
    let endpoints = vec![
        WalletEndpoint::GetBalance(addr),
        WalletEndpoint::GetNonce(addr),
        WalletEndpoint::GetAccount(addr),
        WalletEndpoint::GetTransactionHistory(addr, 50),
        WalletEndpoint::GetTokenBalance(addr),
        WalletEndpoint::GetCreditBalance(addr),
        WalletEndpoint::GetStakeInfo(addr),
    ];
    // All variants must be constructable and cloneable
    for ep in &endpoints {
        let _ = ep.clone();
    }
    assert_eq!(endpoints.len(), 7);
}

#[test]
fn test_wallet_endpoint_history_limit() {
    let ep = WalletEndpoint::GetTransactionHistory(zero_addr(), 100);
    if let WalletEndpoint::GetTransactionHistory(_, limit) = ep {
        assert_eq!(limit, 100);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn test_payment_methods() {
    let methods = vec![
        PaymentMethod::OnChainISA,
        PaymentMethod::CreditCard,
        PaymentMethod::BankTransfer,
        PaymentMethod::CryptoExchange,
    ];
    for m in &methods {
        let cloned = m.clone();
        assert_eq!(*m, cloned);
    }
}

#[test]
fn test_credit_top_up_request() {
    let req = CreditTopUpRequest {
        user: zero_addr(),
        isa_amount: 1_000_000,
        payment_method: PaymentMethod::OnChainISA,
        timestamp: 1_700_000_000_000,
    };
    assert_eq!(req.isa_amount, 1_000_000);
    assert_eq!(req.payment_method, PaymentMethod::OnChainISA);
}

#[test]
fn test_channel_manager_config() {
    let cfg = ChannelManagerConfig {
        auto_open_threshold: 500,
        auto_close_idle_blocks: 1_000,
        default_deposit: 10_000,
        default_dispute_period: 50,
    };
    assert_eq!(cfg.auto_open_threshold, 500);
    assert_eq!(cfg.auto_close_idle_blocks, 1_000);
    assert_eq!(cfg.default_deposit, 10_000);
    assert_eq!(cfg.default_dispute_period, 50);
}

#[test]
fn test_channel_actions() {
    let addr = zero_addr();
    let hash = zero_hash();

    let open = ChannelManagerAction::OpenChannel {
        sender: addr,
        receiver: addr,
        deposit: 10_000,
    };
    let close = ChannelManagerAction::CloseChannel { channel_id: hash };
    let update = ChannelManagerAction::UpdateState {
        channel_id: hash,
        sender_balance: 6_000,
        receiver_balance: 4_000,
    };
    let topup = ChannelManagerAction::TopUp {
        channel_id: hash,
        amount: 5_000,
    };

    // Verify all variants are reachable
    match open {
        ChannelManagerAction::OpenChannel { deposit, .. } => assert_eq!(deposit, 10_000),
        _ => panic!("wrong variant"),
    }
    match close {
        ChannelManagerAction::CloseChannel { .. } => {}
        _ => panic!("wrong variant"),
    }
    match update {
        ChannelManagerAction::UpdateState {
            sender_balance,
            receiver_balance,
            ..
        } => {
            assert_eq!(sender_balance + receiver_balance, 10_000);
        }
        _ => panic!("wrong variant"),
    }
    match topup {
        ChannelManagerAction::TopUp { amount, .. } => assert_eq!(amount, 5_000),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_channel_hub_config() {
    let cfg = ChannelHubConfig {
        hub_address: zero_addr(),
        supported_routes: vec![
            (ServiceType::ModelInference, zero_addr()),
            (ServiceType::ToolExecution, zero_addr()),
        ],
        max_channels_per_route: 16,
        settlement_interval_blocks: 100,
    };
    assert_eq!(cfg.supported_routes.len(), 2);
    assert_eq!(cfg.max_channels_per_route, 16);
    assert_eq!(cfg.settlement_interval_blocks, 100);
}

#[test]
fn test_service_type_custom() {
    let svc = ServiceType::Custom("video-transcoding".to_string());
    if let ServiceType::Custom(name) = svc {
        assert_eq!(name, "video-transcoding");
    } else {
        panic!("wrong variant");
    }
}
