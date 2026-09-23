use std::sync::Arc;

use crossterm::style::Stylize;
use drillx::Solution;
use ore_api::state::proof_pda;
use ore_pool_api::state::member_pda;
use ore_pool_types::{
    BalanceUpdate, ContributePayload, Member, MemberChallenge, PoolAddress, RegisterPayload,
    UpdateBalancePayload,
};
use reqwest::StatusCode;
use solana_rpc_client::spinner;
use solana_sdk::{
    compute_budget, pubkey::Pubkey, signature::Signature, signer::Signer, transaction::Transaction,
};
use steel::AccountDeserialize;
use tabled::{
    settings::{
        object::{Columns, Rows},
        style::BorderColor,
        Alignment, Border, Color, Highlight, Remove, Style,
    },
    Table, Tabled,
};

use crate::{
    args::{PoolArgs, PoolCommand, PoolCommitArgs},
    error::Error,
    utils::{
        self, amount_u64_to_f64, format_timestamp, get_member, get_pool, get_pools, get_proof,
        ComputeBudget, TableData, TableSectionTitle,
    },
    Miner,
};

impl Miner {
    // TODO
    pub async fn pool(&self, args: PoolArgs) {
        if let Some(subcommand) = args.command.clone() {
            match subcommand {
                PoolCommand::Commit(commit_args) => {
                    self.pool_commit(args, commit_args).await.unwrap()
                }
            }
        } else {
            if let Some(pool_url) = args.pool_url {
                self.get_pool(pool_url).await.unwrap();
            } else {
                self.list_pools(args).await.unwrap();
            }
        }
    }

    async fn list_pools(&self, _args: PoolArgs) -> Result<(), Error> {
        let pools = get_pools(&self.rpc_client)
            .await
            .expect("Failed to fetch pool accounts");
        let mut data = vec![];
        for (pool_address, pool) in pools {
            let url = String::from_utf8(pool.url.to_vec()).unwrap_or_default();
            let url = url.trim_end_matches('\0');
            let mut point = PoolTableData {
                address: pool_address.to_string(),
                url: url.to_string(),
                balance: "".to_string(),
                last_hash_at: "".to_string(),
                lifetime_hashes: "".to_string(),
                lifetime_rewards: "".to_string(),
                members: pool.total_members.to_string(),
            };

            // Get proof data
            let proof_address = proof_pda(pool_address).0;
            if let Ok(proof) = get_proof(&self.rpc_client, proof_address).await {
                point.balance = format!("{} ORE", amount_u64_to_f64(proof.balance));
                point.last_hash_at = format_timestamp(proof.last_hash_at);
                point.lifetime_hashes = proof.total_hashes.to_string();
                point.lifetime_rewards = format!("{} ORE", amount_u64_to_f64(proof.total_rewards));
            }

            // Push data
            data.push(point);
        }

        // Print table
        let mut table = Table::new(&data);
        table.with(Style::blank());
        table.modify(Columns::new(1..), Alignment::right());
        table.modify(Rows::first(), Color::BOLD);
        table.with(
            Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)),
        );
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{}\n", table);

        Ok(())
    }

    async fn get_pool(&self, pool_url: String) -> Result<(), Error> {
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };

        // Fetch pool account
        let pool_address = pool.get_pool_address().await?.address;
        let pool_account = get_pool(&self.rpc_client, pool_address)
            .await
            .expect("Failed to fetch pool account");

        // Aggregate table data
        let mut data = vec![];
        data.push(TableData {
            key: "Address".to_string(),
            value: pool_address.to_string(),
        });
        data.push(TableData {
            key: "Total members".to_string(),
            value: pool_account.total_members.to_string(),
        });
        data.push(TableData {
            key: "Url".to_string(),
            value: pool_url.clone(),
        });

        // Get proof account
        let proof_address = proof_pda(pool_address).0;
        let proof = get_proof(&self.rpc_client, proof_address)
            .await
            .expect("Failed to fetch proof account");
        data.push(TableData {
            key: "Address".to_string(),
            value: proof_address.to_string(),
        });
        data.push(TableData {
            key: "Balance".to_string(),
            value: format!("{} ORE", amount_u64_to_f64(proof.balance)),
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
            value: format!("{} ORE", amount_u64_to_f64(proof.total_rewards)),
        });
        data.push(TableData {
            key: "Miner".to_string(),
            value: proof.miner.to_string(),
        });

        // Get member account
        let member_address = member_pda(self.signer().pubkey(), pool_address).0;
        let member = get_member(&self.rpc_client, member_address).await;
        if let Ok(member) = member {
            data.push(TableData {
                key: "Address".to_string(),
                value: member_address.to_string(),
            });
            data.push(TableData {
                key: "Balance".to_string(),
                value: format!("{} ORE", utils::amount_u64_to_string(member.balance))
                    .bold()
                    .yellow()
                    .to_string(),
            });
            // Get offchain data from pool server
            if let Ok(member_offchain) = pool.get_pool_member(&self).await {
                let pending_rewards = (member_offchain.total_balance as u64) - member.total_balance;
                data.push(TableData {
                    key: "Pending rewards".to_string(),
                    value: format!("{} ORE", utils::amount_u64_to_string(pending_rewards)),
                });
            }
            data.push(TableData {
                key: "Lifetime rewards".to_string(),
                value: format!("{} ORE", utils::amount_u64_to_string(member.total_balance)),
            });
        }

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Pool");
        table.section_title(3, "Proof");
        if member.is_ok() {
            table.section_title(10, "Member");
        }
        println!("\n{table}\n");
        if member.is_ok() {
            println!("Pool operators automatically commit pending rewards to the blockchain at regular intervals. To manually commit your pending rewards now, run the following command:\n\n`ore pool {} commit`\n", pool_url);
        }
        Ok(())
    }

    async fn pool_commit(&self, args: PoolArgs, _commit_args: PoolCommitArgs) -> Result<(), Error> {
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: args.pool_url.expect("Pool URL is required"),
        };
        if let Err(err) = pool.post_update_balance(self).await {
            println!("{:?}", err);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct Pool {
    pub http_client: reqwest::Client,
    pub pool_url: String,
}

impl Pool {
    pub async fn post_pool_register(&self, miner: &Miner) -> Result<Member, Error> {
        let pubkey = miner.signer().pubkey();
        let post_url = format!("{}/register", self.pool_url());
        // check if on-chain member account exists already
        let pool_pda = self.get_pool_address().await?;
        if let Err(_err) = self.get_pool_member_onchain(miner, pool_pda.address).await {
            // on-chain member account not found
            // create one before submitting register payload to pool
            let ix = ore_pool_api::sdk::join(pubkey, pool_pda.address, pubkey);
            let _ = miner
                .send_and_confirm(&[ix], ComputeBudget::Fixed(200_000), false)
                .await?;
        };
        // submit idempotent register payload
        // will simply return off-chain account if already registered
        let body = RegisterPayload { authority: pubkey };
        let resp = self.http_client.post(post_url).json(&body).send().await?;
        match resp.error_for_status() {
            Err(err) => {
                println!("{:?}", err);
                Err(err).map_err(From::from)
            }
            Ok(resp) => resp.json::<Member>().await.map_err(From::from),
        }
    }

    pub async fn get_pool_address(&self) -> Result<PoolAddress, Error> {
        let get_url = format!("{}/address", self.pool_url());
        let resp = self.http_client.get(get_url).send().await?;
        match resp.error_for_status() {
            Err(_err) => {
                // Backwards compatibility
                let get_url = format!("{}/pool-address", self.pool_url());
                let resp = self.http_client.get(get_url).send().await?;
                match resp.error_for_status() {
                    Err(err) => {
                        println!("{:?}", err);
                        Err(err).map_err(From::from)
                    }
                    Ok(resp) => resp.json::<PoolAddress>().await.map_err(From::from),
                }
            }
            Ok(resp) => resp.json::<PoolAddress>().await.map_err(From::from),
        }
    }

    pub async fn get_pool_member_onchain(
        &self,
        miner: &Miner,
        pool_address: Pubkey,
    ) -> Result<ore_pool_api::state::Member, Error> {
        let (member_pda, _) =
            ore_pool_api::state::member_pda(miner.signer().pubkey(), pool_address);
        let data = miner.rpc_client.get_account_data(&member_pda).await?;
        let pool = ore_pool_api::state::Member::try_from_bytes(data.as_slice())?;
        Ok(*pool)
    }

    pub async fn get_pool_member(&self, miner: &Miner) -> Result<Member, Error> {
        let pubkey = miner.signer().pubkey();
        let get_url = format!("{}/member/{}", self.pool_url(), pubkey);
        let resp = self.http_client.get(get_url).send().await?;
        match resp.error_for_status() {
            Err(err) => {
                println!("{:?}", err);
                Err(err).map_err(From::from)
            }
            Ok(resp) => resp.json::<Member>().await.map_err(From::from),
        }
    }

    pub async fn _get_staker_onchain(
        &self,
        miner: &Miner,
        pool_address: Pubkey,
        mint: Pubkey,
    ) -> Result<ore_pool_api::state::Share, Error> {
        let (share_pda, _) =
            ore_pool_api::state::share_pda(miner.signer().pubkey(), pool_address, mint);
        let data = miner.rpc_client.get_account_data(&share_pda).await?;
        let share = ore_pool_api::state::Share::try_from_bytes(data.as_slice())?;
        Ok(*share)
    }

    pub async fn get_updated_pool_challenge(
        &self,
        miner: &Miner,
        last_hash_at: i64,
    ) -> Result<MemberChallenge, Error> {
        let mut retries = 0;
        let progress_bar = Arc::new(spinner::new_progress_bar());
        loop {
            progress_bar.set_message(format!("Fetching new challenge... (retry {})", retries));
            let challenge = self.get_pool_challenge(miner).await?;
            if challenge.challenge.lash_hash_at == last_hash_at {
                retries += 1;
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            } else {
                progress_bar.finish_with_message("Found new challenge");
                return Ok(challenge);
            }
        }
    }

    pub async fn get_latest_pool_event(
        &self,
        authority: Pubkey,
        last_hash_at: i64,
    ) -> Result<ore_pool_types::PoolMemberMiningEvent, Error> {
        let get_url = format!("{}/event/latest/{}", self.pool_url(), authority);
        let mut attempts = 0;
        let progress_bar = Arc::new(spinner::new_progress_bar());
        progress_bar.set_message(format!("Fetching mining event... (retry {})", attempts));
        loop {
            // Parse pool event
            let resp = self.http_client.get(get_url.clone()).send().await?;
            match resp.error_for_status() {
                Err(err) => {
                    if let Some(status) = err.status() {
                        match status {
                            StatusCode::NOT_FOUND | StatusCode::BAD_GATEWAY => {
                                // No op. Retry.
                            }
                            _ => {
                                progress_bar.finish_and_clear();
                                return Err(Error::Internal(status.to_string()))
                                    .map_err(From::from);
                            }
                        }
                    }
                }
                Ok(resp) => {
                    if let Ok(event) = resp.json::<ore_pool_types::PoolMemberMiningEvent>().await {
                        if event.last_hash_at as i64 >= last_hash_at {
                            progress_bar.finish_and_clear();
                            return Ok(event);
                        }
                    }
                }
            }

            // Retry
            attempts += 1;
            if attempts > 10 {
                progress_bar.finish_with_message("Retry limit exceeded");
                return Err(Error::Internal("Retry limit exceeded".to_string()))
                    .map_err(From::from);
            }
            progress_bar.set_message(format!("Fetching mining event... (retry {})", attempts));
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    pub async fn post_update_balance(&self, miner: &Miner) -> Result<(), Error> {
        let signer = &miner.signer();
        let signer_pubkey = &signer.pubkey();

        // fetch offchain member balance
        let member = self.get_pool_member(miner).await?;

        // fetch pool for authority
        let pool = self.get_pool_address().await?;
        let data = miner.rpc_client.get_account_data(&pool.address).await?;
        let pool = ore_pool_api::state::Pool::try_from_bytes(data.as_slice())?;
        let pool_authority = pool.authority;

        // build attribute instruction
        let ix = ore_pool_api::sdk::attribute(
            pool_authority,
            *signer_pubkey,
            member.total_balance as u64,
        );
        let compute_budget_limit_ix =
            compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(100_000);
        let compute_budget_price_ix =
            compute_budget::ComputeBudgetInstruction::set_compute_unit_price(20_000);
        let mut tx = Transaction::new_with_payer(
            &[compute_budget_limit_ix, compute_budget_price_ix, ix],
            Some(signer_pubkey),
        );
        let hash = miner.rpc_client.get_latest_blockhash().await?;
        tx.partial_sign(&[signer], hash);

        // build payload
        let paylaod = UpdateBalancePayload {
            authority: *signer_pubkey,
            transaction: tx,
            hash,
        };

        // post
        let post_url = format!("{}/commit", self.pool_url());
        let resp = self
            .http_client
            .post(post_url)
            .json(&paylaod)
            .send()
            .await?;
        match resp.error_for_status() {
            Err(_err) => {
                // Backwards compatibility
                let post_url = format!("{}/update-balance", self.pool_url());
                let resp = self
                    .http_client
                    .post(post_url)
                    .json(&paylaod)
                    .send()
                    .await?;
                match resp.error_for_status() {
                    Err(err) => {
                        println!("{:?}", err);
                        Err(err).map_err(From::from)
                    }
                    Ok(resp) => {
                        let balance_update = resp.json::<BalanceUpdate>().await;
                        println!("{:?}", balance_update);
                        Ok(())
                    }
                }
            }
            Ok(resp) => {
                let balance_update = resp.json::<BalanceUpdate>().await;
                println!("{:?}", balance_update);
                Ok(())
            }
        }
    }

    async fn get_pool_challenge(&self, miner: &Miner) -> Result<MemberChallenge, Error> {
        let pubkey = miner.signer().pubkey();
        let get_url = format!("{}/challenge/{}", self.pool_url(), pubkey);
        let resp = self.http_client.get(get_url).send().await?;
        match resp.error_for_status() {
            Err(err) => {
                println!("{:?}", err);
                Err(err).map_err(From::from)
            }
            Ok(resp) => resp.json::<MemberChallenge>().await.map_err(From::from),
        }
    }

    pub async fn post_pool_solution(
        &self,
        miner: &Miner,
        solution: &Solution,
    ) -> Result<(), Error> {
        let pubkey = miner.signer().pubkey();
        let signature = Pool::sign_solution(miner, solution);
        let payload = ContributePayload {
            authority: pubkey,
            solution: *solution,
            signature,
        };
        let post_url = format!("{}/contribute", self.pool_url());
        let resp = self
            .http_client
            .post(post_url)
            .json(&payload)
            .send()
            .await?;
        match resp.error_for_status() {
            Err(err) => {
                println!("{:?}", err);
                Err(err).map_err(From::from)
            }
            Ok(_) => Ok(()),
        }
    }

    fn pool_url(&self) -> String {
        let pool_url = self.pool_url.clone();
        pool_url.trim_end_matches('/').to_string()
    }

    fn sign_solution(miner: &Miner, solution: &Solution) -> Signature {
        let keypair = &miner.signer();
        keypair.sign_message(solution.to_bytes().as_slice())
    }
}

// async fn parse_pool_id(pool_id: &String) -> Result<Pubkey, Error> {
//     if let Ok(_address) = Pubkey::from_str(pool_id) {
//         // Ok(address)
//         // TODO We need a way to lookup pool url from the address (url must be onchain)
//         panic!("Not implemented");
//     } else {
//         lookup_pool_address(pool_id.clone()).await
//     }
// }

// async fn lookup_pool_address(url: String) -> Result<Pubkey, Error> {
//     let get_url = format!("{}/address", url);
//     let resp = reqwest::Client::new().get(get_url).send().await?;
//     match resp.error_for_status() {
//         Err(err) => {
//             println!("{:?}", err);
//             Err(err).map_err(From::from)
//         }
//         Ok(resp) => resp.json::<Pubkey>().await.map_err(From::from),
//     }
// }

#[derive(Clone, Tabled)]
pub struct PoolTableData {
    #[tabled(rename = "Address")]
    address: String,
    #[tabled(rename = "Balance")]
    balance: String,
    #[tabled(rename = "Last hash at")]
    last_hash_at: String,
    #[tabled(rename = "Lifetime hashes")]
    lifetime_hashes: String,
    #[tabled(rename = "Lifetime rewards")]
    lifetime_rewards: String,
    #[tabled(rename = "Members")]
    members: String,
    #[tabled(rename = "Url")]
    url: String,
}
