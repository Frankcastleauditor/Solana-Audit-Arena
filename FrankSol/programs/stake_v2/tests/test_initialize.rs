use stake_v2::utils::{franksol_to_sol, sol_to_franksol};

#[test]
fn stake_unstake_round_trip_math() {
    let total_sol = 1_000_000_000_u64;
    let supply = 1_000_000_000_u64;
    let stake_in = 250_000_000_u64;
    let minted = sol_to_franksol(stake_in, total_sol, supply).unwrap();
    assert_eq!(minted, stake_in);

    let new_total = total_sol.checked_add(stake_in).unwrap();
    let new_supply = supply.checked_add(minted).unwrap();
    let unstake_out = franksol_to_sol(minted, new_total, new_supply).unwrap();
    assert_eq!(unstake_out, stake_in);
}

#[test]
fn share_price_appreciation_math_with_yield() {
    let total_sol = 1_000_000_000_u64;
    let supply = 1_000_000_000_u64;
    let yield_added = 100_000_000_u64;
    let appreciated_total = total_sol.checked_add(yield_added).unwrap();

    let burn_amount = 100_000_000_u64;
    let sol_out = franksol_to_sol(burn_amount, appreciated_total, supply).unwrap();
    assert_eq!(sol_out, 110_000_000_u64);
}

#[test]
fn bootstrap_is_one_to_one() {
    let stake_in = 500_000_000_u64;
    let minted = sol_to_franksol(stake_in, 0, 0).unwrap();
    assert_eq!(minted, stake_in);
}
