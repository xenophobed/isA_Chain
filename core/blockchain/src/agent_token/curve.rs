use crate::types::Amount;
use serde::{Deserialize, Serialize};

/// Scale factor to avoid precision loss in fixed-point math
pub const SCALE: u128 = 1_000_000;

/// Pricing curve type for agent tokens
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BondingCurve {
    /// price = slope * supply / SCALE
    Linear { slope: Amount },

    /// price = coefficient * supply^2 / SCALE^2
    Quadratic { coefficient: Amount },

    /// S-curve: price = max_price / (1 + e^(-k * (supply - midpoint)))
    /// Approximated as piecewise linear for on-chain integer math
    Sigmoid { max_price: Amount, midpoint: Amount },
}

impl BondingCurve {
    /// Compute the current price per token given the current supply.
    pub fn price_at(&self, supply: Amount) -> Amount {
        match self {
            BondingCurve::Linear { slope } => slope * supply / SCALE,
            BondingCurve::Quadratic { coefficient } => {
                coefficient * supply / SCALE * supply / SCALE
            }
            BondingCurve::Sigmoid { max_price, midpoint } => {
                let mp = *midpoint;
                let low = mp / 5;        // 20% of midpoint
                let high = mp * 4 / 5;  // 80% of midpoint
                if supply <= low {
                    max_price / 20 // ~5%
                } else if supply >= high {
                    max_price * 19 / 20 // ~95%
                } else {
                    let range = high - low;
                    let pos = supply - low;
                    let min_price = max_price / 20;
                    let price_range = max_price * 9 / 10; // 90% spread
                    min_price + price_range * pos / range
                }
            }
        }
    }

    /// Cost to buy `tokens` starting at `current_supply` (integral of price curve).
    pub fn buy_cost(&self, current_supply: Amount, tokens: Amount) -> Amount {
        if tokens == 0 {
            return 0;
        }
        let new_supply = current_supply + tokens;
        match self {
            BondingCurve::Linear { slope } => {
                let new_sq = new_supply * new_supply;
                let old_sq = current_supply * current_supply;
                slope * (new_sq - old_sq) / (2 * SCALE)
            }
            BondingCurve::Quadratic { coefficient } => {
                let ns = new_supply / SCALE;
                let os = current_supply / SCALE;
                let new_cb = ns.saturating_mul(ns).saturating_mul(ns);
                let old_cb = os.saturating_mul(os).saturating_mul(os);
                coefficient.saturating_mul(new_cb.saturating_sub(old_cb)) / 3
            }
            BondingCurve::Sigmoid { .. } => {
                let p_start = self.price_at(current_supply);
                let p_end = self.price_at(new_supply);
                (p_start + p_end) * tokens / 2
            }
        }
    }

    /// ISA returned for selling `tokens` from a supply of `current_supply`.
    pub fn sell_proceeds(&self, current_supply: Amount, tokens: Amount) -> Amount {
        if tokens == 0 || tokens > current_supply {
            return 0;
        }
        let new_supply = current_supply - tokens;
        match self {
            BondingCurve::Linear { slope } => {
                let new_sq = new_supply * new_supply;
                let old_sq = current_supply * current_supply;
                slope * (old_sq - new_sq) / (2 * SCALE)
            }
            BondingCurve::Quadratic { coefficient } => {
                let ns = current_supply / SCALE;
                let os = new_supply / SCALE;
                let old_cb = ns.saturating_mul(ns).saturating_mul(ns);
                let new_cb = os.saturating_mul(os).saturating_mul(os);
                coefficient.saturating_mul(old_cb.saturating_sub(new_cb)) / 3
            }
            BondingCurve::Sigmoid { .. } => {
                let p_start = self.price_at(current_supply);
                let p_end = self.price_at(new_supply);
                (p_start + p_end) * tokens / 2
            }
        }
    }

    /// Solve for the number of tokens receivable for a given ISA spend,
    /// starting at `current_supply`.
    pub fn tokens_for_isa(&self, current_supply: Amount, isa_amount: Amount) -> Amount {
        if isa_amount == 0 {
            return 0;
        }
        match self {
            BondingCurve::Linear { slope } => {
                if *slope == 0 {
                    return 0;
                }
                let old_sq = current_supply * current_supply;
                let numerator = 2 * SCALE * isa_amount;
                let addition = numerator / slope;
                let new_sq = old_sq + addition;
                let new_supply = isqrt(new_sq);
                new_supply.saturating_sub(current_supply)
            }
            BondingCurve::Quadratic { coefficient } => {
                if *coefficient == 0 {
                    return 0;
                }
                binary_search_tokens(self, current_supply, isa_amount)
            }
            BondingCurve::Sigmoid { .. } => {
                binary_search_tokens(self, current_supply, isa_amount)
            }
        }
    }
}

/// Integer square root (floor)
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Binary search for the maximum whole tokens purchasable with `isa_amount`
fn binary_search_tokens(curve: &BondingCurve, current_supply: Amount, isa_amount: Amount) -> Amount {
    let mut lo: Amount = 0;
    let mut hi: Amount = isa_amount + 1;
    let price = curve.price_at(current_supply).max(1);
    hi = (isa_amount / price + 1).min(hi);

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let cost = curve.buy_cost(current_supply, mid);
        if cost <= isa_amount {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}
