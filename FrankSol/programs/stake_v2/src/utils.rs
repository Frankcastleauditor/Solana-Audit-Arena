use anchor_lang_v2::prelude::*;

use crate::error::StakeError;

pub fn checked_add_u64(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(StakeError::MathOverflow.into())
}

pub fn checked_sub_u64(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or(StakeError::MathOverflow.into())
}

pub fn sol_to_franksol(sol_in: u64, total_sol: u64, supply: u64) -> Result<u64> {
    if sol_in == 0 {
        return Err(StakeError::InvalidAmount.into());
    }
    if supply == 0 || total_sol == 0 {
        return Ok(sol_in);
    }
    let out = (sol_in as u128)
        .checked_div(total_sol as u128)
        .ok_or(StakeError::MathOverflow)?
        .checked_mul(supply as u128)
        .ok_or(StakeError::MathOverflow)?;
    if out == 0 {
        return Err(StakeError::InvalidAmount.into());
    }
    u64::try_from(out).map_err(|_| StakeError::MathOverflow.into())
}

pub fn franksol_to_sol(franksol_in: u64, total_sol: u64, supply: u64) -> Result<u64> {
    if franksol_in == 0 {
        return Err(StakeError::InvalidAmount.into());
    }
    if supply == 0 {
        return Err(StakeError::ZeroSupply.into());
    }
    let out = (franksol_in as u128)
        .checked_mul(total_sol as u128)
        .ok_or(StakeError::MathOverflow)?
        .checked_div(supply as u128)
        .ok_or(StakeError::MathOverflow)?;
    if out == 0 {
        return Err(StakeError::InvalidAmount.into());
    }
    u64::try_from(out).map_err(|_| StakeError::MathOverflow.into())
}
