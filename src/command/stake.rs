use std::str::FromStr;

use colored::*;
use ore_api::state::Proof;
use ore_boost_api::state::{boost_pda, stake_pda, Boost, Config as BoostConfig, Stake};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::{amount_to_ui_amount, state::Mint};
use steel::Numeric;
use tabled::{
    settings::{
        object::{Columns, Rows},
        style::BorderColor,
        Alignment, Border, Color, Highlight, Remove, Style,
    },
    Table, Tabled,
};

use crate::{
    args::{StakeArgs, StakeClaimArgs, StakeCommand, StakeDepositArgs, StakeWithdrawArgs},
    error::Error,
    utils::{
        amount_u64_to_f64, format_timestamp, get_boost, get_boost_config, get_boost_stake_accounts,
        get_boosts, get_mint, get_proof_with_authority, get_stake, ComputeBudget, TableData,
        TableSectionTitle,
    },
    Miner, StakeAccountsArgs,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        if let Some(subcommand) = args.command.clone() {
            match subcommand {
                StakeCommand::Claim(subargs) => self.stake_claim(subargs, args).await.unwrap(),
                StakeCommand::Deposit(subargs) => self.stake_deposit(subargs, args).await.unwrap(),
                StakeCommand::Withdraw(subargs) => {
                    self.stake_withdraw(subargs, args).await.unwrap()
                }
                StakeCommand::Accounts(subargs) => {
                    self.stake_accounts(subargs, args).await.unwrap()
                }
            }
        } else {
            if let Some(mint) = args.mint {
                self.stake_get(mint, args.authority).await.unwrap();
            } else {
                self.stake_list(args).await.unwrap();
            }
        }
    }

    async fn stake_claim(
        &self,
        claim_args: StakeClaimArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let mint_str = stake_args.mint.expect("Mint address is required");
        let mint_address = Pubkey::from_str(&mint_str).expect("Failed to parse mint address");
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(pubkey, boost_address).0;

        let mut ixs = vec![];
        let beneficiary = match claim_args.to {
            None => self.initialize_ata(pubkey).await,
            Some(to) => {
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let beneficiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &ore_api::consts::MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&beneficiary_tokens)
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
                beneficiary_tokens
            }
        };

        // Get stake account data to check rewards balance
        let stake = get_stake(&self.rpc_client, stake_address)
            .await
            .expect("Failed to fetch stake account");

        // Build claim instruction with amount or max rewards
        ixs.push(ore_boost_api::sdk::claim(
            pubkey,
            beneficiary,
            mint_address,
            claim_args
                .amount
                .map(|a| crate::utils::amount_f64_to_u64(a))
                .unwrap_or(stake.rewards),
        ));

        // Send and confirm transaction
        println!("Claiming staking yield...");
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(100_000), false)
            .await
            .ok();

        Ok(())
    }

    async fn stake_get(&self, mint: String, authority: Option<String>) -> Result<(), Error> {
        // Fetch onchain data
        let mint_address = Pubkey::from_str(&mint).expect("Failed to parse mint address");
        let boost_address = boost_pda(mint_address).0;
        let authority = if let Some(authority) = authority {
            Pubkey::from_str(&authority).expect("Failed to parse authority address")
        } else {
            self.signer().pubkey()
        };
        let stake_address = stake_pda(authority, boost_address).0;
        let boost = get_boost(&self.rpc_client, boost_address)
            .await
            .expect("Failed to fetch boost account");
        let mint = get_mint(&self.rpc_client, mint_address)
            .await
            .expect("Failed to fetch mint account");
        let metadata_address = mpl_token_metadata::accounts::Metadata::find_pda(&mint_address).0;
        let symbol = match self.rpc_client.get_account_data(&metadata_address).await {
            Ok(metadata_data) => {
                if let Ok(metadata) =
                    mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_data)
                {
                    format!(" {}", metadata.symbol.trim_matches('\0'))
                } else {
                    " ".to_string()
                }
            }
            Err(_) => " ".to_string(),
        };

        // Aggregate data
        let mut data = vec![];
        self.fetch_boost_data(boost_address, boost, mint, symbol.clone(), &mut data)
            .await;
        let len1 = data.len();
        self.fetch_stake_data(stake_address, boost, mint, symbol.clone(), &mut data)
            .await;
        let _len2 = data.len();

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Boost");
        table.section_title(len1, "Stake");
        println!("{table}\n");
        Ok(())
    }

    async fn fetch_stake_data(
        &self,
        address: Pubkey,
        boost: Boost,
        mint: Mint,
        symbol: String,
        data: &mut Vec<TableData>,
    ) {
        let boost_config_address = ore_boost_api::state::config_pda().0;
        let stake = get_stake(&self.rpc_client, address).await;
        let boost_config = get_boost_config(&self.rpc_client).await;
        let boost_proof = get_proof_with_authority(&self.rpc_client, boost_config_address)
            .await
            .unwrap();
        data.push(TableData {
            key: "Address".to_string(),
            value: address.to_string(),
        });
        if let Ok(stake) = stake {
            let claimable_yield =
                calculate_claimable_yield(boost, boost_config, boost_proof, stake);
            data.push(TableData {
                key: "Deposits".to_string(),
                value: format!(
                    "{}{} ({}% of total)",
                    amount_to_ui_amount(stake.balance, mint.decimals),
                    symbol,
                    (stake.balance as f64 / boost.total_deposits as f64) * 100f64
                ),
            });
            data.push(TableData {
                key: "Last claim at".to_string(),
                value: format_timestamp(stake.last_claim_at),
            });
            data.push(TableData {
                key: "Last deposit at".to_string(),
                value: format_timestamp(stake.last_deposit_at),
            });
            data.push(TableData {
                key: "Last withdraw at".to_string(),
                value: format_timestamp(stake.last_withdraw_at),
            });
            data.push(TableData {
                key: "Yield".to_string(),
                value: if claimable_yield > 0 {
                    format!("{} ORE", amount_u64_to_f64(claimable_yield))
                        .yellow()
                        .bold()
                        .to_string()
                } else {
                    format!("{} ORE", amount_u64_to_f64(claimable_yield))
                },
            });
        } else {
            data.push(TableData {
                key: "Status".to_string(),
                value: "Not found".red().bold().to_string(),
            });
        }
    }

    async fn fetch_boost_data(
        &self,
        address: Pubkey,
        boost: Boost,
        mint: Mint,
        symbol: String,
        data: &mut Vec<TableData>,
    ) {
        data.push(TableData {
            key: "Address".to_string(),
            value: address.to_string(),
        });
        data.push(TableData {
            key: "Expires at".to_string(),
            value: format_timestamp(boost.expires_at),
        });
        data.push(TableData {
            key: "Mint".to_string(),
            value: boost.mint.to_string(),
        });
        data.push(TableData {
            key: "Weight".to_string(),
            value: format!("{}", boost.weight),
        });
        data.push(TableData {
            key: "Total deposits".to_string(),
            value: format!(
                "{}{}",
                amount_to_ui_amount(boost.total_deposits, mint.decimals),
                symbol.trim_end_matches(' ')
            ),
        });
        data.push(TableData {
            key: "Total stakers".to_string(),
            value: boost.total_stakers.to_string(),
        });
    }

    async fn stake_list(&self, args: StakeArgs) -> Result<(), Error> {
        // Get the account address
        let authority = match &args.authority {
            Some(authority) => {
                Pubkey::from_str(&authority).expect("Failed to parse account address")
            }
            None => self.signer().pubkey(),
        };

        let boost_config_address = ore_boost_api::state::config_pda().0;
        let boost_config = get_boost_config(&self.rpc_client).await;
        let boost_proof = get_proof_with_authority(&self.rpc_client, boost_config_address)
            .await
            .unwrap();

        // Iterate over all boosts
        let mut data = vec![];
        let boosts = get_boosts(&self.rpc_client)
            .await
            .expect("Failed to fetch boosts");
        for (address, boost) in boosts {
            // Get relevant accounts
            let stake_address = stake_pda(authority, address).0;
            let stake = get_stake(&self.rpc_client, stake_address).await;
            let mint = get_mint(&self.rpc_client, boost.mint)
                .await
                .expect("Failed to fetch mint account");
            let metadata_address = mpl_token_metadata::accounts::Metadata::find_pda(&boost.mint).0;
            let symbol = match self.rpc_client.get_account_data(&metadata_address).await {
                Err(_) => "".to_string(),
                Ok(metadata_data) => {
                    if let Ok(metadata) =
                        mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_data)
                    {
                        format!(" {}", metadata.symbol.trim_matches('\0'))
                    } else {
                        "".to_string()
                    }
                }
            };

            // Parse optional stake data
            let (stake_balance, stake_rewards) = if let Ok(stake) = stake {
                (
                    stake.balance,
                    calculate_claimable_yield(boost, boost_config, boost_proof, stake),
                )
            } else {
                (0, 0)
            };

            // Aggregate data
            data.push(StakeTableData {
                mint: boost.mint.to_string(),
                symbol,
                weight: format!("{}", boost.weight),
                total_deposits: format!(
                    "{}",
                    amount_to_ui_amount(boost.total_deposits, mint.decimals)
                ),
                total_stakers: boost.total_stakers.to_string(),
                my_deposits: format!("{}", amount_to_ui_amount(stake_balance, mint.decimals)),
                my_share: if boost.total_deposits > 0 {
                    format!(
                        "{:.8}%",
                        (stake_balance as f64 / boost.total_deposits as f64) * 100f64
                    )
                } else {
                    "NaN".to_string()
                },
                my_yield: if stake_rewards > 0 {
                    format!("{} ORE", amount_u64_to_f64(stake_rewards))
                        .yellow()
                        .bold()
                        .to_string()
                } else {
                    format!("{} ORE", amount_u64_to_f64(stake_rewards))
                },
            });
        }

        // Build table
        let mut table = Table::new(data);
        table.with(Style::blank());
        table.modify(Rows::first(), Color::BOLD);
        table.modify(Columns::new(1..), Alignment::right());
        table.with(
            Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)),
        );
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{table}\n");
        Ok(())
    }

    async fn stake_deposit(
        &self,
        args: StakeDepositArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_str = stake_args.mint.expect("Mint address is required");
        let mint_address = Pubkey::from_str(&mint_str).expect("Failed to parse mint address");

        // Get signer
        let signer = self.signer();
        let sender = match &args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };

        // Get token account
        let mint_data = self
            .rpc_client
            .get_account_data(&mint_address)
            .await
            .expect("Failed to fetch mint account");
        let mint = Mint::unpack(&mint_data).expect("Failed to parse mint account");
        let token_account = self
            .rpc_client
            .get_token_account(&sender)
            .await
            .expect("Failed to fetch token account")
            .expect("Token account not found");

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            u64::from_str(token_account.token_amount.amount.as_str())
                .expect("Failed to parse token balance")
        };

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(signer.pubkey(), boost_address).0;
        let _boost = get_boost(&self.rpc_client, boost_address)
            .await
            .expect("Failed to fetch boost account");

        // Open stake account, if needed
        if self
            .rpc_client
            .get_account_data(&stake_address)
            .await
            .is_err()
        {
            println!("Initializing stake account...");
            let ix = ore_boost_api::sdk::open(signer.pubkey(), signer.pubkey(), mint_address);
            self.send_and_confirm(&[ix], ComputeBudget::Fixed(50_000), false)
                .await
                .ok();
        }

        // Send tx
        println!("Depositing stake...");
        let ix = ore_boost_api::sdk::deposit(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(200_000), false)
            .await
            .ok();
        Ok(())
    }

    async fn stake_withdraw(
        &self,
        args: StakeWithdrawArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_str = stake_args.mint.expect("Mint address is required");
        let mint_address = Pubkey::from_str(&mint_str).expect("Failed to parse mint address");

        // Get signer
        let signer = self.signer();

        // Get beneficiary token account
        let beneficiary = match &args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };

        // Create token account if necessary
        let mut ixs = vec![];
        if self
            .rpc_client
            .get_token_account(&beneficiary)
            .await
            .is_err()
        {
            ixs.push(
                spl_associated_token_account::instruction::create_associated_token_account(
                    &signer.pubkey(),
                    &signer.pubkey(),
                    &mint_address,
                    &spl_token::id(),
                ),
            );
        };

        // Get mint account
        let mint_data = self
            .rpc_client
            .get_account_data(&mint_address)
            .await
            .expect("Failed to fetch mint account");
        let mint = Mint::unpack(&mint_data).expect("Failed to parse mint account");

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(signer.pubkey(), boost_address).0;
        let _boost = get_boost(&self.rpc_client, boost_address)
            .await
            .expect("Failed to fetch boost account");
        let stake = get_stake(&self.rpc_client, stake_address)
            .await
            .expect("Failed to fetch stake account");

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            stake.balance
        };

        // Send tx
        ixs.push(ore_boost_api::sdk::withdraw(
            signer.pubkey(),
            mint_address,
            amount,
        ));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(200_000), false)
            .await
            .ok();

        Ok(())
    }

    async fn stake_accounts(
        &self,
        _args: StakeAccountsArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        let mint_str = stake_args.mint.expect("Mint address is required");
        let mint_address = Pubkey::from_str(&mint_str).expect("Failed to parse mint address");
        let boost_address = boost_pda(mint_address).0;
        let boost = get_boost(&self.rpc_client, boost_address)
            .await
            .expect("Failed to fetch boost account");
        let mint_account = get_mint(&self.rpc_client, mint_address)
            .await
            .expect("Failed to fetch mint account");
        let mut stake_accounts = get_boost_stake_accounts(&self.rpc_client, boost_address)
            .await
            .expect("Failed to fetch stake accounts");
        stake_accounts
            .sort_by(|(_addr1, stake1), (_addr2, stake2)| stake2.balance.cmp(&stake1.balance));
        let mut data = vec![];
        for (_stake_address, stake) in stake_accounts {
            data.push(StakerTableData {
                authority: stake.authority.to_string(),
                deposits: format!(
                    "{:#.11}",
                    amount_to_ui_amount(stake.balance, mint_account.decimals)
                ),
                share: if boost.total_deposits > 0 {
                    format!(
                        "{:.5}%",
                        stake.balance as f64 / boost.total_deposits as f64 * 100f64
                    )
                } else {
                    "NaN".to_string()
                },
                rewards: format!(
                    "{:#.11} ORE",
                    amount_to_ui_amount(stake.rewards, ore_api::consts::TOKEN_DECIMALS)
                ),
            });
        }
        let mut table = Table::new(data);
        table.with(Style::blank());
        table.modify(Rows::first(), Color::BOLD);
        table.modify(Columns::new(1..), Alignment::right());
        table.with(
            Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)),
        );
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{table}\n");
        Ok(())
    }
}

pub fn calculate_claimable_yield(
    boost: Boost,
    boost_config: BoostConfig,
    boost_proof: Proof,
    stake: Stake,
) -> u64 {
    let mut rewards = stake.rewards;
    let mut config_rewards_factor = boost_config.rewards_factor;
    let mut boost_rewards_factor = boost.rewards_factor;

    if boost_proof.balance > 0 {
        config_rewards_factor +=
            Numeric::from_fraction(boost_proof.balance, boost_config.total_weight);
    }

    if config_rewards_factor > boost.last_rewards_factor {
        let accumulated_rewards = config_rewards_factor - boost.last_rewards_factor;
        let boost_rewards = accumulated_rewards * Numeric::from_u64(boost.weight);
        boost_rewards_factor += boost_rewards / Numeric::from_u64(boost.total_deposits);
    }

    if boost_rewards_factor > stake.last_rewards_factor {
        let accumulated_rewards = boost_rewards_factor - stake.last_rewards_factor;
        let personal_rewards = accumulated_rewards * Numeric::from_u64(stake.balance);
        rewards += personal_rewards.to_u64();
    }

    rewards
}

#[derive(Tabled)]
pub struct StakeTableData {
    #[tabled(rename = "Mint")]
    pub mint: String,
    #[tabled(rename = "Symbol")]
    pub symbol: String,
    #[tabled(rename = "Weight")]
    pub weight: String,
    #[tabled(rename = "Stakers")]
    pub total_stakers: String,
    #[tabled(rename = "Deposits")]
    pub total_deposits: String,
    #[tabled(rename = "My deposits")]
    pub my_deposits: String,
    #[tabled(rename = "My share")]
    pub my_share: String,
    #[tabled(rename = "My yield")]
    pub my_yield: String,
}

#[derive(Tabled)]
pub struct StakerTableData {
    #[tabled(rename = "Authority")]
    pub authority: String,
    #[tabled(rename = "Deposits")]
    pub deposits: String,
    #[tabled(rename = "Share")]
    pub share: String,
    #[tabled(rename = "Yield")]
    pub rewards: String,
}
