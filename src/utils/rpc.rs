use std::time::Duration;

use colored::Colorize;
use ore_api::{
    consts::{CONFIG_ADDRESS, TREASURY_ADDRESS},
    state::{proof_pda, Bus, Config, Proof, Treasury},
};
use ore_boost_api::state::{Boost, Stake};
use ore_pool_api::state::{Member, Pool};
use serde::Deserialize;
use solana_account_decoder::UiAccountEncoding;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::{
    client_error::{reqwest::StatusCode, ClientError, ClientErrorKind},
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_program::{program_pack::Pack, pubkey::Pubkey, sysvar};
use solana_sdk::{clock::Clock, hash::Hash};
use spl_token::state::Mint;
use steel::{AccountDeserialize, Discriminator};
use tokio::time::sleep;

pub const BLOCKHASH_QUERY_RETRIES: usize = 5;
pub const BLOCKHASH_QUERY_DELAY: u64 = 500;

pub enum ComputeBudget {
    #[allow(dead_code)]
    Dynamic,
    Fixed(u32),
}

pub async fn get_program_accounts<T>(
    client: &RpcClient,
    program_id: Pubkey,
    filters: Vec<RpcFilterType>,
) -> Result<Vec<(Pubkey, T)>, anyhow::Error>
where
    T: AccountDeserialize + Discriminator + Clone,
{
    let mut all_filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
        0,
        &T::discriminator().to_le_bytes(),
    ))];
    all_filters.extend(filters);
    let result = client
        .get_program_accounts_with_config(
            &program_id,
            RpcProgramAccountsConfig {
                filters: Some(all_filters),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await;

    match result {
        Ok(accounts) => {
            let accounts = accounts
                .into_iter()
                .map(|(pubkey, account)| {
                    let account = T::try_from_bytes(&account.data).unwrap().clone();
                    (pubkey, account)
                })
                .collect();
            Ok(accounts)
        }
        Err(err) => match err.kind {
            ClientErrorKind::Reqwest(err) => {
                if let Some(status_code) = err.status() {
                    if status_code == StatusCode::GONE {
                        panic!(
                                "\n{} Your RPC provider does not support the getProgramAccounts endpoint, needed to execute this command. Please use a different RPC provider.\n",
                                "ERROR".bold().red().to_string()
                            );
                    }
                }
                return Err(anyhow::anyhow!("Failed to get program accounts: {}", err));
            }
            _ => return Err(anyhow::anyhow!("Failed to get program accounts: {}", err)),
        },
    }
}

pub async fn _get_treasury(client: &RpcClient) -> Treasury {
    let data = client
        .get_account_data(&TREASURY_ADDRESS)
        .await
        .expect("Failed to get treasury account");
    *Treasury::try_from_bytes(&data).expect("Failed to parse treasury account")
}

pub async fn get_mint(client: &RpcClient, address: Pubkey) -> Result<Mint, anyhow::Error> {
    let mint_data = client.get_account_data(&address).await?;
    let mint = Mint::unpack(&mint_data)?;
    Ok(mint)
}

pub async fn get_config(client: &RpcClient) -> Config {
    let data = client
        .get_account_data(&CONFIG_ADDRESS)
        .await
        .expect("Failed to get config account");
    *Config::try_from_bytes(&data).expect("Failed to parse config account")
}

pub async fn get_boost_config(client: &RpcClient) -> ore_boost_api::state::Config {
    let data = client
        .get_account_data(&ore_boost_api::state::config_pda().0)
        .await
        .expect("Failed to get config account");
    *ore_boost_api::state::Config::try_from_bytes(&data).expect("Failed to parse config account")
}

pub async fn get_boost(client: &RpcClient, address: Pubkey) -> Result<Boost, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Boost::try_from_bytes(&data).expect("Failed to parse boost account"))
}

pub async fn get_boosts(client: &RpcClient) -> Result<Vec<(Pubkey, Boost)>, anyhow::Error> {
    get_program_accounts::<Boost>(client, ore_boost_api::ID, vec![]).await
}

pub async fn get_pools(client: &RpcClient) -> Result<Vec<(Pubkey, Pool)>, anyhow::Error> {
    get_program_accounts::<Pool>(client, ore_pool_api::ID, vec![]).await
}

pub async fn get_pool(client: &RpcClient, address: Pubkey) -> Result<Pool, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Pool::try_from_bytes(&data)?)
}

pub async fn get_member(client: &RpcClient, address: Pubkey) -> Result<Member, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Member::try_from_bytes(&data)?)
}

pub async fn get_stake(client: &RpcClient, address: Pubkey) -> Result<Stake, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Stake::try_from_bytes(&data)?)
}

pub async fn get_bus(client: &RpcClient, address: Pubkey) -> Result<Bus, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Bus::try_from_bytes(&data)?)
}

pub async fn get_boost_stake_accounts(
    rpc_client: &RpcClient,
    boost_address: Pubkey,
) -> Result<Vec<(Pubkey, Stake)>, anyhow::Error> {
    let filter = RpcFilterType::Memcmp(Memcmp::new_base58_encoded(48, &boost_address.to_bytes()));
    get_program_accounts::<Stake>(rpc_client, ore_boost_api::ID, vec![filter]).await
}

pub async fn get_proof_with_authority(
    client: &RpcClient,
    authority: Pubkey,
) -> Result<Proof, anyhow::Error> {
    let proof_address = proof_pda(authority).0;
    get_proof(client, proof_address).await
}

pub async fn get_updated_proof_with_authority(
    client: &RpcClient,
    authority: Pubkey,
    lash_hash_at: i64,
) -> Result<Proof, anyhow::Error> {
    loop {
        if let Ok(proof) = get_proof_with_authority(client, authority).await {
            if proof.last_hash_at.gt(&lash_hash_at) {
                return Ok(proof);
            }
        }
        tokio::time::sleep(Duration::from_millis(1_000)).await;
    }
}

pub async fn get_proof(client: &RpcClient, address: Pubkey) -> Result<Proof, anyhow::Error> {
    let data = client.get_account_data(&address).await?;
    Ok(*Proof::try_from_bytes(&data)?)
}

pub async fn get_clock(client: &RpcClient) -> Result<Clock, anyhow::Error> {
    retry(|| async {
        let data = client.get_account_data(&sysvar::clock::ID).await?;
        Ok(bincode::deserialize::<Clock>(&data)?)
    })
    .await
}

pub async fn get_latest_blockhash_with_retries(
    client: &RpcClient,
) -> Result<(Hash, u64), ClientError> {
    let mut attempts = 0;

    loop {
        if let Ok((hash, slot)) = client
            .get_latest_blockhash_with_commitment(client.commitment())
            .await
        {
            return Ok((hash, slot));
        }

        // Retry
        sleep(Duration::from_millis(BLOCKHASH_QUERY_DELAY)).await;
        attempts += 1;
        if attempts >= BLOCKHASH_QUERY_RETRIES {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom(
                    "Max retries reached for latest blockhash query".into(),
                ),
            });
        }
    }
}

pub async fn retry<F, Fut, T>(f: F) -> Result<T, anyhow::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
    const MAX_RETRIES: u32 = 8;
    const INITIAL_BACKOFF: Duration = Duration::from_millis(200);
    const TIMEOUT: Duration = Duration::from_secs(8);
    let mut backoff = INITIAL_BACKOFF;
    for attempt in 0..MAX_RETRIES {
        match tokio::time::timeout(TIMEOUT, f()).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(_)) if attempt < MAX_RETRIES - 1 => {
                tokio::time::sleep(backoff).await;
                backoff *= 2; // Exponential backoff
            }
            Ok(Err(e)) => return Err(e),
            Err(_) if attempt < MAX_RETRIES - 1 => {
                tokio::time::sleep(backoff).await;
                backoff *= 2; // Exponential backoff
            }
            Err(_) => return Err(anyhow::anyhow!("Retry failed")),
        }
    }

    Err(anyhow::anyhow!("Retry failed"))
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Tip {
    pub time: String,
    pub _landed_tips_25th_percentile: f64,
    pub landed_tips_50th_percentile: f64,
    pub landed_tips_75th_percentile: f64,
    pub landed_tips_95th_percentile: f64,
    pub landed_tips_99th_percentile: f64,
    pub ema_landed_tips_50th_percentile: f64,
}
