pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang_v2::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf");

#[program]
pub mod yield_generator {
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }

    pub fn deposit(ctx: &mut Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::handler(ctx, amount)
    }

    pub fn withdraw(
        ctx: &mut Context<Withdraw>,
        principal_returned: u64,
        _yield_amount: u64,
    ) -> Result<()> {
        instructions::withdraw::handler(ctx, principal_returned, _yield_amount)
    }

    pub fn set_yield_direction(
        ctx: &mut Context<SetYieldDirection>,
        is_positive: bool,
    ) -> Result<()> {
        instructions::set_yield_direction::handler(ctx, is_positive)
    }
}

#[cfg(feature = "cpi")]
pub mod cpi {
    extern crate alloc;

    use alloc::vec;
    use alloc::vec::Vec;
    use anchor_lang_v2::{cpi::InstructionAccount, CpiContext, CpiHandle, InstructionData, ToCpiAccounts};
    use solana_program_error::ProgramError;

    pub mod accounts {
        use super::*;

        pub struct Deposit<'a> {
            pub operator: CpiHandle<'a>,
            pub state: CpiHandle<'a>,
            pub position: CpiHandle<'a>,
            pub source_vault: CpiHandle<'a>,
            pub yield_vault: CpiHandle<'a>,
            pub system_program: CpiHandle<'a>,
        }

        impl<'a> ToCpiAccounts<'a> for Deposit<'a> {
            fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
                vec![
                    InstructionAccount::readonly_signer(self.operator.address()),
                    InstructionAccount::writable(self.state.address()),
                    InstructionAccount::writable(self.position.address()),
                    InstructionAccount::readonly_signer(self.source_vault.address()),
                    InstructionAccount::writable(self.yield_vault.address()),
                    InstructionAccount::readonly(self.system_program.address()),
                ]
            }

            fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
                vec![
                    self.operator,
                    self.state,
                    self.position,
                    self.source_vault,
                    self.yield_vault,
                    self.system_program,
                ]
            }
        }

        pub struct Withdraw<'a> {
            pub operator: CpiHandle<'a>,
            pub state: CpiHandle<'a>,
            pub position: CpiHandle<'a>,
            pub yield_vault: CpiHandle<'a>,
            pub destination_vault: CpiHandle<'a>,
            pub system_program: CpiHandle<'a>,
        }

        impl<'a> ToCpiAccounts<'a> for Withdraw<'a> {
            fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
                vec![
                    InstructionAccount::readonly_signer(self.operator.address()),
                    InstructionAccount::writable(self.state.address()),
                    InstructionAccount::writable(self.position.address()),
                    InstructionAccount::writable(self.yield_vault.address()),
                    InstructionAccount::writable(self.destination_vault.address()),
                    InstructionAccount::readonly(self.system_program.address()),
                ]
            }

            fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
                vec![
                    self.operator,
                    self.state,
                    self.position,
                    self.yield_vault,
                    self.destination_vault,
                    self.system_program,
                ]
            }
        }
    }

    pub fn deposit<'a>(
        ctx: CpiContext<'a, accounts::Deposit<'a>>,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let ix_data = crate::instruction::Deposit { amount }.data();
        ctx.invoke(&ix_data)
    }

    pub fn withdraw<'a>(
        ctx: CpiContext<'a, accounts::Withdraw<'a>>,
        principal_returned: u64,
        yield_amount: u64,
    ) -> Result<(), ProgramError> {
        let ix_data = crate::instruction::Withdraw {
            principal_returned,
            _yield_amount: yield_amount,
        }
        .data();
        ctx.invoke(&ix_data)
    }
}
