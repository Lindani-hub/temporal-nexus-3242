use std::str::FromStr;

use colored::Colorize;
use ore_api::state::proof_pda;
use solana_program::{pubkey::Pubkey, native_token::lamports_to_sol};
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;
use tabled::{Table, settings::{Style, Remove, object::{Rows, Columns}, Alignment}};

use crate::{
    args::{AccountArgs, AccountCloseArgs, AccountCommand, ClaimArgs},
    utils::{amount_u64_to_f64, ask_confirm, format_timestamp, get_proof, get_proof_with_authority, ComputeBudget, TableData, TableSectionTitle},
    Miner,
};

impl Miner {
    pub async fn account(&self, args: AccountArgs) {
        if let Some(command) = args.command {
            match command {
                AccountCommand::Close(args) => self.close(args).await,
            }
        } else {
            self.get_account(args).await;
        }
    }

    async fn get_account(&self, args: AccountArgs) {
        // Parse account address
        let signer = self.signer();
        let address = if let Some(address) = &args.address {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                return;
            }
        } else if args.proof.is_some() {
            return self.get_proof_account(args).await;
        } else {
            signer.pubkey()
        };

        // Aggregate data   
        let mut data = vec![];
        self.get_account_data(address, &mut data).await;
        self.get_proof_data(address, &mut data).await;

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Account");
        table.section_title(3, "Proof");
 
        println!("{table}\n");
    }

    async fn get_proof_account(&self, args: AccountArgs) {
        // Parse account address
        let proof_address = if let Some(address) = &args.proof {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                return;
            }
        } else {
            return;
        };

        // Aggregate data   
        let proof = get_proof(&self.rpc_client, proof_address).await.expect("Failed to fetch proof account");
        let mut data = vec![];
        self.get_account_data(proof.authority, &mut data).await;
        self.get_proof_data(proof.authority, &mut data).await;

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Account");
        table.section_title(3, "Proof");
 
        println!("{table}\n");
    }

    async fn get_account_data(&self, authority: Pubkey, data: &mut Vec<TableData>) {
        // Get ORE balance
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &authority,
            &ore_api::consts::MINT_ADDRESS,
        );
        let token_balance = if let Ok(Some(token_account)) = self
            .rpc_client
            .get_token_account(&token_account_address)
            .await
        {
            token_account.token_amount.ui_amount_string
        } else {
            "0".to_string()
        };

        // Get SOL balance
        let sol_balance = self.rpc_client.get_balance(&authority).await.expect("Failed to fetch SOL balance");

        // Aggregate data
        data.push(TableData {
            key: "Address".to_string(),
            value: authority.to_string(),
        });
        data.push(TableData {
            key: "Balance".to_string(),
            value: format!("{} ORE", token_balance),
        });
        data.push(TableData {
            key: "SOL".to_string(),
            value: format!("{} SOL", lamports_to_sol(sol_balance)),
        });
    }

    async fn get_proof_data(&self, authority: Pubkey, data: &mut Vec<TableData>) {
        // Parse addresses
        let proof_address = proof_pda(authority).0;
        let proof = get_proof(&self.rpc_client, proof_address).await;

        // Aggregate data
        data.push(TableData {
            key: "Address".to_string(),
            value: proof_address.to_string(),
        });
        if let Ok(proof) = proof {
            data.push(TableData {
                key: "Authority".to_string(),
                value: authority.to_string(),
            });
            data.push(TableData {
                key: "Balance".to_string(),
                value: if proof.balance > 0 {
                    format!("{} ORE", amount_u64_to_f64(proof.balance)).bold().yellow().to_string()
                } else {
                    format!("{} ORE", amount_u64_to_f64(proof.balance))
                }
            });
            data.push(TableData {
                key: "Last hash".to_string(),
                value: solana_sdk::hash::Hash::new_from_array(proof.last_hash).to_string(),
            });
            data.push(TableData {
                key: "Last hash at".to_string(),
                value: format_timestamp(proof.last_hash_at),
            });
            data.push(TableData {
                key: "Lifetime hashes".to_string(),
                value: proof.total_hashes.to_string(),
            });
            data.push(TableData {
                key: "Lifetime rewards".to_string(),
                value: format!("{} ORE", amount_to_ui_amount(proof.total_rewards, ore_api::consts::TOKEN_DECIMALS)),
            });
            data.push(TableData {
                key: "Miner".to_string(),
                value: proof.miner.to_string(),
            });
        } else {
            data.push(TableData {
                key: "Status".to_string(),
                value: "Not found".red().bold().to_string(),
            });
        }
    }

    async fn close(&self, _args: AccountCloseArgs) {
        // Confirm proof exists
        let signer = self.signer();
        let proof = get_proof_with_authority(&self.rpc_client, signer.pubkey()).await.expect("Failed to fetch proof account");

        // Confirm the user wants to close.
        if !ask_confirm(
            format!("{} You have {} ORE staked in this account.\nAre you sure you want to {}close this account? [Y/n]", 
                "WARNING".yellow(),
                amount_to_ui_amount(proof.balance, ore_api::consts::TOKEN_DECIMALS),
                if proof.balance.gt(&0) { "claim your stake and "} else { "" }
            ).as_str()
        ) {
            return;
        }

        // Claim stake
        if proof.balance.gt(&0) {
            self.claim_from_proof(ClaimArgs {
                amount: None,
                to: None,
                pool_url: None,
            })
            .await;
        }

        // Submit close transaction
        let ix = ore_api::sdk::close(signer.pubkey());
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(500_000), false)
            .await
            .ok();
    }
}
