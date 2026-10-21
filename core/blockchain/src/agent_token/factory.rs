use std::collections::HashMap;
use crate::types::{Address, Amount, BlockHeight, Hash};
use super::curve::BondingCurve;
use super::types::{AgentToken, AgentTokenError};

/// Manages all agent tokens
#[derive(Clone, Debug)]
pub struct AgentTokenFactory {
    /// agent_id → token
    pub tokens: HashMap<Hash, AgentToken>,
    /// Running count of tokens ever created
    pub total_tokens_created: u64,
}

impl AgentTokenFactory {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            total_tokens_created: 0,
        }
    }

    /// Create a new bonding-curve token for `agent_id`.
    /// Fails if a token already exists for this agent.
    pub fn create_token(
        &mut self,
        agent_id: Hash,
        curve: BondingCurve,
        creator: Address,
        height: BlockHeight,
    ) -> Result<(), AgentTokenError> {
        if self.tokens.contains_key(&agent_id) {
            return Err(AgentTokenError::TokenAlreadyExists(agent_id));
        }
        let token = AgentToken::new(agent_id, curve, creator, height);
        self.tokens.insert(agent_id, token);
        self.total_tokens_created += 1;
        Ok(())
    }

    /// Buy agent tokens with `isa_amount` ISA. Returns the number of tokens received.
    pub fn buy(
        &mut self,
        agent_id: &Hash,
        buyer: Address,
        isa_amount: Amount,
    ) -> Result<Amount, AgentTokenError> {
        if isa_amount == 0 {
            return Err(AgentTokenError::ZeroAmount);
        }

        let token = self
            .tokens
            .get_mut(agent_id)
            .ok_or(AgentTokenError::TokenNotFound(*agent_id))?;

        let tokens_out = token
            .curve_type
            .tokens_for_isa(token.total_supply, isa_amount);

        if tokens_out == 0 {
            return Err(AgentTokenError::InvalidAmount);
        }

        // Verify the cost doesn't exceed what was sent (rounding safety)
        let actual_cost = token.curve_type.buy_cost(token.total_supply, tokens_out);
        if actual_cost > isa_amount {
            return Err(AgentTokenError::InsufficientFunds);
        }

        token.total_supply += tokens_out;
        token.reserve_balance += actual_cost;
        *token.holders.entry(buyer).or_insert(0) += tokens_out;

        Ok(tokens_out)
    }

    /// Sell `token_amount` agent tokens back to the curve. Returns ISA received.
    pub fn sell(
        &mut self,
        agent_id: &Hash,
        seller: &Address,
        token_amount: Amount,
    ) -> Result<Amount, AgentTokenError> {
        if token_amount == 0 {
            return Err(AgentTokenError::ZeroAmount);
        }

        let token = self
            .tokens
            .get_mut(agent_id)
            .ok_or(AgentTokenError::TokenNotFound(*agent_id))?;

        let balance = token.holders.get(seller).copied().unwrap_or(0);
        if balance < token_amount {
            return Err(AgentTokenError::InsufficientTokens);
        }

        let isa_out = token
            .curve_type
            .sell_proceeds(token.total_supply, token_amount);

        if isa_out > token.reserve_balance {
            return Err(AgentTokenError::InsufficientFunds);
        }

        token.total_supply -= token_amount;
        token.reserve_balance -= isa_out;

        let holder_balance = token.holders.get_mut(seller).unwrap();
        *holder_balance -= token_amount;
        if *holder_balance == 0 {
            token.holders.remove(seller);
        }

        Ok(isa_out)
    }

    /// Current price per token (at the current supply level).
    pub fn get_price(&self, agent_id: &Hash) -> Result<Amount, AgentTokenError> {
        let token = self
            .tokens
            .get(agent_id)
            .ok_or(AgentTokenError::TokenNotFound(*agent_id))?;
        Ok(token.curve_type.price_at(token.total_supply))
    }

    /// Retrieve the token record for an agent.
    pub fn get_token(&self, agent_id: &Hash) -> Option<&AgentToken> {
        self.tokens.get(agent_id)
    }

    /// Token balance for a holder.
    pub fn get_balance(&self, agent_id: &Hash, holder: &Address) -> Amount {
        self.tokens
            .get(agent_id)
            .and_then(|t| t.holders.get(holder).copied())
            .unwrap_or(0)
    }

    /// Market cap = reserve_balance (ISA locked in the curve).
    pub fn get_market_cap(&self, agent_id: &Hash) -> Result<Amount, AgentTokenError> {
        let token = self
            .tokens
            .get(agent_id)
            .ok_or(AgentTokenError::TokenNotFound(*agent_id))?;
        Ok(token.reserve_balance)
    }

    /// Total number of agent tokens ever created.
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens_created
    }
}

impl Default for AgentTokenFactory {
    fn default() -> Self {
        Self::new()
    }
}
