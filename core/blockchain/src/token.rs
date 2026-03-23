use crate::types::{Address, Amount};
use std::collections::HashSet;

/// Errors related to token operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("Unauthorized mint: {0} is not an authorized minter")]
    UnauthorizedMint(Address),

    #[error("Unauthorized burn: {0} is not an authorized burner")]
    UnauthorizedBurn(Address),

    #[error("Insufficient balance for burn")]
    InsufficientBalance,

    #[error("Supply overflow: operation would exceed maximum u128")]
    SupplyOverflow,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Unauthorized admin operation: {0} is not the admin")]
    UnauthorizedAdmin(Address),
}

/// Summary of token supply information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSupplyInfo {
    /// Current total supply
    pub total_supply: Amount,
    /// Total tokens burned since genesis
    pub total_burned: Amount,
    /// Total tokens minted beyond initial supply
    pub total_minted: Amount,
    /// Circulating supply (total_supply as tracked)
    pub circulating_supply: Amount,
}

/// Token state tracking for the ISA native chain asset.
///
/// Maintains supply accounting (mint/burn totals) and access-control
/// lists for addresses authorized to trigger mint or burn operations.
pub struct TokenState {
    /// Total supply (initially set to INITIAL_SUPPLY, adjusted by mint/burn)
    total_supply: Amount,
    /// Total burned since genesis
    total_burned: Amount,
    /// Total minted beyond the initial supply (e.g., provider payments)
    total_minted: Amount,
    /// Addresses allowed to trigger mint operations
    authorized_minters: HashSet<Address>,
    /// Addresses allowed to trigger burn operations
    authorized_burners: HashSet<Address>,
    /// Protocol admin address (can authorize/revoke minters and burners)
    admin: Address,
}

impl TokenState {
    /// Create a new token state with the given initial supply.
    ///
    /// The caller-supplied `admin` address is the only address that can
    /// authorize or revoke minters/burners.
    pub fn new(initial_supply: Amount, admin: Address) -> Self {
        TokenState {
            total_supply: initial_supply,
            total_burned: 0,
            total_minted: 0,
            authorized_minters: HashSet::new(),
            authorized_burners: HashSet::new(),
            admin,
        }
    }

    // ----------------------------------------------------------------
    // Supply queries
    // ----------------------------------------------------------------

    /// Current total supply (initial + minted - burned).
    pub fn get_total_supply(&self) -> Amount {
        self.total_supply
    }

    /// Total tokens burned since genesis.
    pub fn get_total_burned(&self) -> Amount {
        self.total_burned
    }

    /// Total tokens minted beyond the initial supply.
    pub fn get_total_minted(&self) -> Amount {
        self.total_minted
    }

    /// Circulating supply — same as total_supply since all minted tokens
    /// are immediately in circulation and burned tokens are already
    /// subtracted from total_supply.
    pub fn get_circulating_supply(&self) -> Amount {
        self.total_supply
    }

    /// Full supply snapshot.
    pub fn get_supply_info(&self) -> TokenSupplyInfo {
        TokenSupplyInfo {
            total_supply: self.total_supply,
            total_burned: self.total_burned,
            total_minted: self.total_minted,
            circulating_supply: self.get_circulating_supply(),
        }
    }

    // ----------------------------------------------------------------
    // Authorization queries
    // ----------------------------------------------------------------

    pub fn is_authorized_minter(&self, address: &Address) -> bool {
        self.authorized_minters.contains(address)
    }

    pub fn is_authorized_burner(&self, address: &Address) -> bool {
        self.authorized_burners.contains(address)
    }

    pub fn is_admin(&self, address: &Address) -> bool {
        *address == self.admin
    }

    // ----------------------------------------------------------------
    // Mint / Burn
    // ----------------------------------------------------------------

    /// Mint `amount` new tokens.
    ///
    /// Only an authorized minter may call this.  Updates `total_minted`
    /// and `total_supply`.  Returns `Ok(())` on success.
    ///
    /// **Note:** this method only tracks supply accounting.  The caller
    /// (i.e., `Blockchain`) is responsible for crediting the recipient
    /// account balance.
    pub fn mint(&mut self, amount: Amount, minter: &Address) -> Result<(), TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        if !self.authorized_minters.contains(minter) {
            return Err(TokenError::UnauthorizedMint(*minter));
        }
        let new_supply = self
            .total_supply
            .checked_add(amount)
            .ok_or(TokenError::SupplyOverflow)?;
        let new_minted = self
            .total_minted
            .checked_add(amount)
            .ok_or(TokenError::SupplyOverflow)?;

        self.total_supply = new_supply;
        self.total_minted = new_minted;
        Ok(())
    }

    /// Burn `amount` tokens.
    ///
    /// Only an authorized burner may call this.  Updates `total_burned`
    /// and `total_supply`.  The caller is responsible for verifying /
    /// deducting the account balance; this method performs the supply
    /// accounting check (total_supply must not underflow).
    pub fn burn(&mut self, amount: Amount, burner: &Address) -> Result<(), TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        if !self.authorized_burners.contains(burner) {
            return Err(TokenError::UnauthorizedBurn(*burner));
        }
        if self.total_supply < amount {
            return Err(TokenError::InsufficientBalance);
        }

        self.total_supply -= amount;
        self.total_burned = self
            .total_burned
            .checked_add(amount)
            .ok_or(TokenError::SupplyOverflow)?;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Admin: authorize / revoke
    // ----------------------------------------------------------------

    pub fn authorize_minter(
        &mut self,
        address: Address,
        admin: &Address,
    ) -> Result<(), TokenError> {
        if !self.is_admin(admin) {
            return Err(TokenError::UnauthorizedAdmin(*admin));
        }
        self.authorized_minters.insert(address);
        Ok(())
    }

    pub fn revoke_minter(
        &mut self,
        address: &Address,
        admin: &Address,
    ) -> Result<(), TokenError> {
        if !self.is_admin(admin) {
            return Err(TokenError::UnauthorizedAdmin(*admin));
        }
        self.authorized_minters.remove(address);
        Ok(())
    }

    pub fn authorize_burner(
        &mut self,
        address: Address,
        admin: &Address,
    ) -> Result<(), TokenError> {
        if !self.is_admin(admin) {
            return Err(TokenError::UnauthorizedAdmin(*admin));
        }
        self.authorized_burners.insert(address);
        Ok(())
    }

    pub fn revoke_burner(
        &mut self,
        address: &Address,
        admin: &Address,
    ) -> Result<(), TokenError> {
        if !self.is_admin(admin) {
            return Err(TokenError::UnauthorizedAdmin(*admin));
        }
        self.authorized_burners.remove(address);
        Ok(())
    }
}

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::INITIAL_SUPPLY;

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn minter() -> Address {
        Address::from([0xBB; 20])
    }

    fn burner() -> Address {
        Address::from([0xCC; 20])
    }

    fn random_addr() -> Address {
        Address::from([0xDD; 20])
    }

    fn setup() -> TokenState {
        let mut state = TokenState::new(INITIAL_SUPPLY, admin());
        state.authorize_minter(minter(), &admin()).unwrap();
        state.authorize_burner(burner(), &admin()).unwrap();
        state
    }

    // ---- Mint tests ------------------------------------------------

    #[test]
    fn test_mint_authorized_success() {
        let mut state = setup();
        let before = state.get_total_supply();
        let amount: Amount = 1_000_000;

        assert!(state.mint(amount, &minter()).is_ok());
        assert_eq!(state.get_total_supply(), before + amount);
        assert_eq!(state.get_total_minted(), amount);
    }

    #[test]
    fn test_mint_unauthorized_fails() {
        let mut state = setup();
        let result = state.mint(1_000, &random_addr());
        assert!(result.is_err());
        assert!(matches!(result, Err(TokenError::UnauthorizedMint(_))));
    }

    #[test]
    fn test_mint_zero_amount_fails() {
        let mut state = setup();
        let result = state.mint(0, &minter());
        assert_eq!(result, Err(TokenError::InvalidAmount));
    }

    #[test]
    fn test_mint_overflow_protection() {
        let mut state = TokenState::new(u128::MAX, admin());
        state.authorize_minter(minter(), &admin()).unwrap();

        let result = state.mint(1, &minter());
        assert_eq!(result, Err(TokenError::SupplyOverflow));
    }

    // ---- Burn tests ------------------------------------------------

    #[test]
    fn test_burn_authorized_success() {
        let mut state = setup();
        let before = state.get_total_supply();
        let amount: Amount = 500_000;

        assert!(state.burn(amount, &burner()).is_ok());
        assert_eq!(state.get_total_supply(), before - amount);
        assert_eq!(state.get_total_burned(), amount);
    }

    #[test]
    fn test_burn_unauthorized_fails() {
        let mut state = setup();
        let result = state.burn(1_000, &random_addr());
        assert!(matches!(result, Err(TokenError::UnauthorizedBurn(_))));
    }

    #[test]
    fn test_burn_zero_amount_fails() {
        let mut state = setup();
        let result = state.burn(0, &burner());
        assert_eq!(result, Err(TokenError::InvalidAmount));
    }

    #[test]
    fn test_burn_more_than_supply_fails() {
        let mut state = setup();
        let result = state.burn(INITIAL_SUPPLY + 1, &burner());
        assert_eq!(result, Err(TokenError::InsufficientBalance));
    }

    // ---- Supply tracking -------------------------------------------

    #[test]
    fn test_supply_tracking_after_mint_and_burn() {
        let mut state = setup();
        let mint_amount: Amount = 2_000_000;
        let burn_amount: Amount = 500_000;

        state.mint(mint_amount, &minter()).unwrap();
        state.burn(burn_amount, &burner()).unwrap();

        assert_eq!(
            state.get_total_supply(),
            INITIAL_SUPPLY + mint_amount - burn_amount
        );
        assert_eq!(state.get_total_minted(), mint_amount);
        assert_eq!(state.get_total_burned(), burn_amount);
    }

    #[test]
    fn test_circulating_supply_equals_total_supply() {
        let mut state = setup();
        state.mint(1_000_000, &minter()).unwrap();
        assert_eq!(state.get_circulating_supply(), state.get_total_supply());
    }

    #[test]
    fn test_supply_info() {
        let mut state = setup();
        state.mint(100, &minter()).unwrap();
        state.burn(50, &burner()).unwrap();

        let info = state.get_supply_info();
        assert_eq!(info.total_supply, INITIAL_SUPPLY + 100 - 50);
        assert_eq!(info.total_minted, 100);
        assert_eq!(info.total_burned, 50);
        assert_eq!(info.circulating_supply, info.total_supply);
    }

    // ---- Authorization tests ---------------------------------------

    #[test]
    fn test_authorize_and_revoke_minter() {
        let mut state = TokenState::new(INITIAL_SUPPLY, admin());
        let addr = random_addr();

        assert!(!state.is_authorized_minter(&addr));

        state.authorize_minter(addr, &admin()).unwrap();
        assert!(state.is_authorized_minter(&addr));

        state.revoke_minter(&addr, &admin()).unwrap();
        assert!(!state.is_authorized_minter(&addr));
    }

    #[test]
    fn test_authorize_and_revoke_burner() {
        let mut state = TokenState::new(INITIAL_SUPPLY, admin());
        let addr = random_addr();

        assert!(!state.is_authorized_burner(&addr));

        state.authorize_burner(addr, &admin()).unwrap();
        assert!(state.is_authorized_burner(&addr));

        state.revoke_burner(&addr, &admin()).unwrap();
        assert!(!state.is_authorized_burner(&addr));
    }

    #[test]
    fn test_non_admin_cannot_authorize_minter() {
        let mut state = TokenState::new(INITIAL_SUPPLY, admin());
        let result = state.authorize_minter(random_addr(), &random_addr());
        assert!(matches!(result, Err(TokenError::UnauthorizedAdmin(_))));
    }

    #[test]
    fn test_non_admin_cannot_authorize_burner() {
        let mut state = TokenState::new(INITIAL_SUPPLY, admin());
        let result = state.authorize_burner(random_addr(), &random_addr());
        assert!(matches!(result, Err(TokenError::UnauthorizedAdmin(_))));
    }

    #[test]
    fn test_non_admin_cannot_revoke_minter() {
        let mut state = setup();
        let result = state.revoke_minter(&minter(), &random_addr());
        assert!(matches!(result, Err(TokenError::UnauthorizedAdmin(_))));
    }

    #[test]
    fn test_non_admin_cannot_revoke_burner() {
        let mut state = setup();
        let result = state.revoke_burner(&burner(), &random_addr());
        assert!(matches!(result, Err(TokenError::UnauthorizedAdmin(_))));
    }

    #[test]
    fn test_revoked_minter_cannot_mint() {
        let mut state = setup();
        state.revoke_minter(&minter(), &admin()).unwrap();
        let result = state.mint(1_000, &minter());
        assert!(matches!(result, Err(TokenError::UnauthorizedMint(_))));
    }

    #[test]
    fn test_revoked_burner_cannot_burn() {
        let mut state = setup();
        state.revoke_burner(&burner(), &admin()).unwrap();
        let result = state.burn(1_000, &burner());
        assert!(matches!(result, Err(TokenError::UnauthorizedBurn(_))));
    }
}
