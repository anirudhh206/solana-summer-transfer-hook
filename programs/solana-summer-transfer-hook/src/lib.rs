pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use spl_discriminator::SplDiscriminate;
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("FbSAuywLK75SUEVhxv1CEHQNqRif4HMuQcEZT1gejbPz");

#[program]
pub mod solana_summer_transfer_hook {

    use super::*;

    pub fn initialize_mint(ctx: Context<InitializeMint>) -> Result<()> {
        initialize_mint::handler(ctx)
    }

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        init_extra_account_meta::handler(ctx)
    }

    // CHALLENGE 4 (solved): entrypoint that performs the transfer_checked CPI
    // itself (see instructions/transfer.rs), instead of requiring the client
    // to build that CPI directly against the Token-2022 program.
    pub fn transfer(ctx: Context<Transfer>, amount: u64, decimals: u8) -> Result<()> {
        transfer::handler(ctx, amount, decimals)
    }

    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        transfer_hook::handler(ctx, amount)
    }
}
