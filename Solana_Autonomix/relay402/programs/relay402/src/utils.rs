use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::Relay402Error;

/// Fee rounded up, so tiny payments cannot avoid the fee through rounding.
/// With fee_bps <= MAX_FEE_BPS the result is always <= amount.
pub fn compute_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .and_then(|v| v.checked_add((BPS_DENOMINATOR - 1) as u128))
        .and_then(|v| v.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(Relay402Error::MathOverflow)?;
    let fee = u64::try_from(fee).map_err(|_| Relay402Error::MathOverflow)?;
    require!(fee <= amount, Relay402Error::MathOverflow);
    Ok(fee)
}

pub fn validate_endpoint(endpoint: &str) -> Result<()> {
    require!(
        !endpoint.is_empty() && endpoint.len() <= MAX_ENDPOINT_LEN,
        Relay402Error::InvalidEndpoint
    );
    Ok(())
}

pub fn validate_metadata_uri(uri: &str) -> Result<()> {
    require!(
        !uri.is_empty() && uri.len() <= MAX_METADATA_URI_LEN,
        Relay402Error::InvalidMetadataUri
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_rounds_up() {
        assert_eq!(compute_fee(1, 100).unwrap(), 1);
        assert_eq!(compute_fee(10_000, 100).unwrap(), 100);
        assert_eq!(compute_fee(10_001, 100).unwrap(), 101);
        assert_eq!(compute_fee(0, 100).unwrap(), 0);
        assert_eq!(compute_fee(5, 0).unwrap(), 0);
        assert_eq!(compute_fee(u64::MAX, MAX_FEE_BPS).unwrap(), u64::MAX / 10 + 1);
    }
}
