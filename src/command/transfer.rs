use std::str::FromStr;

use colored::*;
use ore_api::consts::MINT_ADDRESS;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;

use crate::{
    args::TransferArgs,
    utils::{amount_f64_to_u64, ask_confirm, ComputeBudget},
    Miner,
};

impl Miner {
    pub async fn transfer(&self, args: TransferArgs) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let sender_tokens =
            spl_associated_token_account::get_associated_token_address(&pubkey, &MINT_ADDRESS);
        let mut ixs = vec![];

        // Initialize recipient, if needed
        let to = Pubkey::from_str(&args.to).expect("Failed to parse recipient wallet address");
        let recipient_tokens =
            spl_associated_token_account::get_associated_token_address(&to, &MINT_ADDRESS);
        if self
            .rpc_client
            .get_token_account(&recipient_tokens)
            .await
            .is_err()
        {
            ixs.push(
                spl_associated_token_account::instruction::create_associated_token_account(
                    &signer.pubkey(),
                    &to,
                    &ore_api::consts::MINT_ADDRESS,
                    &spl_token::id(),
                ),
            );
        }

        // Parse amount to claim
        let amount = amount_f64_to_u64(args.amount);

        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to transfer {}.\n\nAre you sure you want to continue? [Y/n]",
                format!(
                    "{} ORE",
                    amount_to_ui_amount(amount, ore_api::consts::TOKEN_DECIMALS)
                )
                .bold(),
            )
            .as_str(),
        ) {
            return;
        }

        // Send and confirm
        ixs.push(
            spl_token::instruction::transfer(
                &spl_token::id(),
                &sender_tokens,
                &recipient_tokens,
                &pubkey,
                &[&pubkey],
                amount,
            )
            .unwrap(),
        );
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(32_000), false)
            .await
            .ok();
    }
}
