use super::types::AdvancedToken;

#[test]
fn test_token_creation() {
    let token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    assert_eq!(token.name, "Test Token");
    assert_eq!(token.symbol, "TEST");
    assert_eq!(token.total_supply, 1000000);
    assert_eq!(token.balance_of("owner"), 1000000);
}

#[test]
fn test_transfer() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    let result = token.transfer("owner", "user1", 100000, 1, 1000);
    assert!(result.is_ok());
    
    assert_eq!(token.balance_of("owner"), 900000);
    assert_eq!(token.balance_of("user1"), 100000);
}

#[test]
fn test_transfer_with_fee() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    token.transfer_fee_rate = 100; // 1%
    token.fee_recipient = Some("fee_collector".to_string());
    
    let result = token.transfer("owner", "user1", 100000, 1, 1000);
    assert!(result.is_ok());
    
    assert_eq!(token.balance_of("owner"), 900000);
    assert_eq!(token.balance_of("user1"), 99000); // 100000 - 1000 fee
    assert_eq!(token.balance_of("fee_collector"), 1000);
}

#[test]
fn test_minting() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    let result = token.mint("user1", 50000, "owner");
    assert!(result.is_ok());
    
    assert_eq!(token.total_supply, 1050000);
    assert_eq!(token.balance_of("user1"), 50000);
}

#[test]
fn test_burning() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    let result = token.burn("owner", 50000, "owner");
    assert!(result.is_ok());
    
    assert_eq!(token.total_supply, 950000);
    assert_eq!(token.balance_of("owner"), 950000);
}

#[test]
fn test_approval_and_transfer_from() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    // Approve spender
    let approve_result = token.approve("owner", "spender", 100000);
    assert!(approve_result.is_ok());
    assert_eq!(token.allowance("owner", "spender"), 100000);
    
    // Transfer from
    let transfer_result = token.transfer_from("spender", "owner", "user1", 50000, 1, 1000);
    assert!(transfer_result.is_ok());
    
    assert_eq!(token.balance_of("owner"), 950000);
    assert_eq!(token.balance_of("user1"), 50000);
    assert_eq!(token.allowance("owner", "spender"), 50000);
}

#[test]
fn test_pause_functionality() {
    let mut token = AdvancedToken::new(
        "Test Token".to_string(),
        "TEST".to_string(),
        18,
        1000000,
        "owner".to_string(),
    );
    
    // Pause token
    let pause_result = token.pause("owner");
    assert!(pause_result.is_ok());
    assert!(token.is_paused);
    
    // Try to transfer while paused
    let transfer_result = token.transfer("owner", "user1", 100000, 1, 1000);
    assert!(transfer_result.is_err());
    
    // Unpause token
    let unpause_result = token.unpause("owner");
    assert!(unpause_result.is_ok());
    assert!(!token.is_paused);
    
    // Transfer should work now
    let transfer_result = token.transfer("owner", "user1", 100000, 1, 1000);
    assert!(transfer_result.is_ok());
}
