use std::str::FromStr;

use colored::*;
use ore_api::consts::MINT_ADDRESS;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::{Signature, Signer};
use spl_token::amount_to_ui_amount;

use crate::{
    args::ClaimArgs,
    utils::{amount_f64_to_u64, ask_confirm, get_proof_with_authority, ComputeBudget},
    Miner,
};

use super::pool::Pool;

impl Miner {
    pub async fn claim(&self, args: ClaimArgs) -> Result<(), crate::error::Error> {
        match args.pool_url {
            Some(ref pool_url) => {
                let pool = &Pool {
                    http_client: reqwest::Client::new(),
                    pool_url: pool_url.clone(),
                };
                let _ = self.claim_from_pool(args, pool).await?;
                Ok(())
            }
            None => {
                self.claim_from_proof(args).await;
                Ok(())
            }
        }
    }

    pub async fn claim_from_proof(&self, args: ClaimArgs) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let proof = get_proof_with_authority(&self.rpc_client, pubkey).await.expect("Failed to fetch proof account");
        let mut ixs = vec![];
        let beneficiary = match args.to {
            None => self.initialize_ata(pubkey).await,
            Some(to) => {
                // Create beneficiary token account, if needed
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let benefiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&benefiary_tokens)
                    .await
                    .is_err()
                {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &pubkey,
                            &wallet,
                            &ore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                benefiary_tokens
            }
        };

        // Parse amount to claim
        let amount = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            proof.balance
        };

        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to claim {}.\n\nAre you sure you want to continue? [Y/n]",
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
        ixs.push(ore_api::sdk::claim(pubkey, beneficiary, amount));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(32_000), false)
            .await
            .ok();
    }

    async fn claim_from_pool(
        &self,
        args: ClaimArgs,
        pool: &Pool,
    ) -> Result<Signature, crate::error::Error> {
        let pool_address = pool.get_pool_address().await?;
        let member = pool
            .get_pool_member_onchain(self, pool_address.address)
            .await?;
        let mut ixs = vec![];

        // Create beneficiary token account, if needed
        let beneficiary = match args.to {
            None => self.initialize_ata(self.signer().pubkey()).await,
            Some(to) => {
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let benefiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&benefiary_tokens)
                    .await
                    .is_err()
                {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &self.signer().pubkey(),
                            &wallet,
                            &ore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                benefiary_tokens
            }
        };

        // Parse amount to claim
        let amount = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            member.balance
        };

        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to claim {}.\n\nAre you sure you want to continue? [Y/n]",
                format!(
                    "{} ORE",
                    amount_to_ui_amount(amount, ore_api::consts::TOKEN_DECIMALS)
                )
                .bold(),
            )
            .as_str(),
        ) {
            return Err(crate::error::Error::Internal("exited claim".to_string()));
        }

        // Send and confirm
        ixs.push(ore_pool_api::sdk::claim(
            self.signer().pubkey(),
            beneficiary,
            pool_address.address,
            amount,
        ));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(50_000), false)
            .await
            .map_err(From::from)
    }

    pub async fn initialize_ata(&self, wallet: Pubkey) -> Pubkey {
        // Initialize client.
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &wallet,
            &ore_api::consts::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }
        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &signer.pubkey(),
            &signer.pubkey(),
            &ore_api::consts::MINT_ADDRESS,
            &spl_token::id(),
        );
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(400_000), false)
            .await
            .ok();

        // Return token account address
        token_account_pubkey
    }
}
