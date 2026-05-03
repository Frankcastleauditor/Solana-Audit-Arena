use anchor_lang_v2::prelude::*;

use crate::error::YieldError;
use crate::state::YieldState;
use crate::STATE_SEED;

#[derive(Accounts)]
pub struct SetYieldDirection {
    pub authority: Signer,
    #[account(mut, seeds = [STATE_SEED], bump = state.state_bump)]
    pub state: Account<YieldState>,
}

pub fn handler(ctx: &mut Context<SetYieldDirection>, is_positive: bool) -> Result<()> {
    require_keys_eq!(
        *ctx.accounts.authority.address(),
        ctx.accounts.state.authority,
        YieldError::Unauthorized
    );
    ctx.accounts.state.yield_direction_positive = PodBool::from(is_positive);
    Ok(())
}
