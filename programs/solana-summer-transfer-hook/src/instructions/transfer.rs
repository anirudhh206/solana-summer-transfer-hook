use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

// CHALLENGE 4 (solved): perform the transfer_checked CPI from inside our own
// program, instead of relying on a client to build it. Token-2022 will, as
// part of processing transfer_checked, CPI back into *our* program's
// `transfer_hook` instruction (because the mint's transfer_hook extension
// points at us) - so this single instruction ends up re-entering our own
// program one level deep: Transfer -> (CPI) -> Token-2022 -> (CPI) -> TransferHook.
//
// To keep that safe:
//   - `rate_limit` and `extra_account_meta_list` are left as `UncheckedAccount`
//     here (not `Account<'info, RateLimit>` / a typed PDA check beyond seeds).
//     They are never read or written in *this* instruction - only the nested
//     `TransferHook::handler` call touches `rate_limit`'s data. If we typed
//     them strongly and marked them `mut` here too, we'd be asking Anchor to
//     manage a borrow/write-back for an account this instruction never
//     actually uses, which is unnecessary and risks colliding with the
//     borrow the re-entered hook call takes on the same account.
//   - We never call `transfer` again inside `TransferHook::handler`, so the
//     re-entry is exactly one level deep and terminates - there is no cycle.
#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    /// CHECK: this program itself, acting as the transfer hook program. Token-2022
    /// requires the hook program account to be present among the extra accounts.
    #[account(address = crate::ID)]
    pub hook_program: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList account, read by Token-2022 (via CPI helper below)
    /// to resolve which extra accounts the transfer hook needs.
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    /// CHECK: the per-mint, per-owner rate limit PDA. Not touched directly by
    /// this instruction - only the nested `transfer_hook` CPI reads/writes it.
    #[account(
        mut,
        seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()],
        bump,
    )]
    pub rate_limit: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<Transfer>, amount: u64, decimals: u8) -> Result<()> {
    // Build the base transfer_checked instruction, exactly as a client would.
    let mut cpi_instruction = spl_token_2022::instruction::transfer_checked(
        ctx.accounts.token_program.key,
        ctx.accounts.source_token.to_account_info().key,
        ctx.accounts.mint.to_account_info().key,
        ctx.accounts.destination_token.to_account_info().key,
        ctx.accounts.owner.key,
        &[],
        amount,
        decimals,
    )?;

    let mut cpi_account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // The accounts Token-2022 will need to CPI into our transfer_hook
    // instruction: our program itself, the validation account, and every
    // extra account it describes (here, just `rate_limit`).
    let additional_accounts = vec![
        ctx.accounts.hook_program.to_account_info(),
        ctx.accounts.extra_account_meta_list.to_account_info(),
        ctx.accounts.rate_limit.to_account_info(),
    ];

    // Reads the ExtraAccountMetaList account and appends the resolved extra
    // accounts (+ the hook program + the validation account) onto
    // `cpi_instruction` / `cpi_account_infos`, so the token program can find
    // everything it needs to CPI into our hook mid-transfer.
    add_extra_accounts_for_execute_cpi(
        &mut cpi_instruction,
        &mut cpi_account_infos,
        &crate::ID,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        &additional_accounts,
    )?;

    invoke(&cpi_instruction, &cpi_account_infos)?;

    Ok(())
}
