#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        Id, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{setup, setup_mint_and_extra_metas, create_ata, mint_tokens};

// CHALLENGE 4: exercises the in-program `transfer` instruction, which itself
// performs the transfer_checked CPI (instead of a client building it).
//
// This demonstrates - and documents - Solana's CPI reentrancy protection:
// the call stack here is `our program -> Token-2022 -> our program again`
// (our program's ID appears twice, with Token-2022 in between). The Solana
// runtime disallows this "indirect" reentrancy unconditionally - a program
// may only call itself *directly* (adjacent stack frames), never through
// another program in between. No account/seed/signer arrangement can work
// around this; it's enforced purely by program ID on the call stack.
//
// This is precisely why every real transfer-hook example has transfer_checked
// initiated by something other than the hook program itself - either the
// client calls it directly (see `build_transfer_with_hook_ix` in
// tests/helpers/mod.rs, used by test_transfer_hook.rs - hook program appears
// only once), or a separate on-chain program (a vault, a DEX, etc. - a
// different program ID) calls it. A single program cannot be both the
// initiator and the hook in the same call chain.
#[test]
fn test_program_transfer_hits_reentrancy_protection() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    let ix = Instruction::new_with_bytes(
        program_id,
        &solana_summer_transfer_hook::instruction::Transfer { amount: 100, decimals: 9 }.data(),
        solana_summer_transfer_hook::accounts::Transfer {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::Token2022::id(),
            hook_program: program_id,
            extra_account_meta_list,
            rate_limit,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err(
        "Expected this to fail: a program cannot CPI into Token-2022 and have \
         Token-2022 CPI back into that same program (the transfer hook) - \
         Solana blocks this indirect reentrancy at the runtime level.",
    );
    let err_string = format!("{err:?}");
    assert!(
        err_string.contains("ReentrancyNotAllowed"),
        "Expected a ReentrancyNotAllowed error, got: {err_string}"
    );
}
