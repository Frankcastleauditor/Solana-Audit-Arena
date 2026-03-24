use crate::error::MissionxErrors;

pub fn get_amount_out(amount_in: u64, reserve_in: u64, reserve_out: u64) -> Result<u64, MissionxErrors> {
    let nominator = (amount_in as u128).checked_mul(reserve_out as u128).ok_or(MissionxErrors::MathOverflow)?;
    let denominator = (amount_in as u128).checked_add(reserve_in as u128).ok_or(MissionxErrors::MathOverflow)?;

    Ok((nominator / denominator) as u64)
}

pub fn get_amount_in(amount_out: u64, reserve_in: u64, reserve_out: u64) -> Result<u64, MissionxErrors> {
    let nominator = (amount_out as u128).checked_mul(reserve_in as u128).ok_or(MissionxErrors::MathOverflow)?;
    let denominator = (reserve_out as u128).checked_sub(amount_out as u128).ok_or(MissionxErrors::MathOverflow)?;
    let result = nominator / denominator + 1;

    Ok(result as u64)
}

pub fn get_amount_in_sol(token_out: u64, full_sol_reserve: u64, full_token_reserve: u64) -> Result<u64, MissionxErrors> {
    get_amount_in(token_out, full_sol_reserve, full_token_reserve)
}

pub fn get_amount_out_tokens(sol_in: u64, full_sol_reserve: u64, full_token_reserve: u64) -> Result<u64, MissionxErrors> {
    get_amount_out(sol_in, full_sol_reserve, full_token_reserve)
}

pub fn get_amount_in_tokens(sol_out: u64, full_sol_reserve: u64, full_token_reserve: u64) -> Result<u64, MissionxErrors> {
    get_amount_in(sol_out, full_token_reserve, full_sol_reserve)
}

pub fn get_amount_out_sol(token_in: u64, full_sol_reserve: u64, full_token_reserve: u64) -> Result<u64, MissionxErrors> {
    get_amount_out(token_in, full_token_reserve, full_sol_reserve)
}