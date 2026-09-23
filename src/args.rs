use clap::{arg, command, Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct AccountArgs {
    #[arg(value_name = "ADDRESS", help = "The address to the account to fetch.")]
    pub address: Option<String>,

    #[arg(
        short,
        long,
        value_name = "PROOF_ADDRESS",
        help = "The address of the proof to fetch."
    )]
    pub proof: Option<String>,

    #[command(subcommand)]
    pub command: Option<AccountCommand>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum AccountCommand {
    #[command(about = "Close an account and reclaim rent.")]
    Close(AccountCloseArgs),
}

#[derive(Parser, Clone, Debug)]
pub struct AccountCloseArgs {}

#[derive(Parser, Debug)]
pub struct BalanceArgs {
    #[arg(
        value_name = "ADDRESS",
        help = "The account address to fetch the balance of."
    )]
    pub address: Option<String>,

    #[command(subcommand)]
    pub command: Option<BalanceCommand>,

    #[arg(
        long,
        short,
        value_name = "POOL_URL",
        help = "The optional pool url to fetch the balance from."
    )]
    pub pool_url: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum BalanceCommand {
    #[command(about = "Commit a pending pool balance to the chain.")]
    Commit(BalanceCommitArgs),
}

#[derive(Parser, Clone, Debug)]
pub struct BalanceCommitArgs {}

#[derive(Parser, Debug)]
pub struct BenchmarkArgs {
    #[arg(
        long,
        short,
        value_name = "THREAD_COUNT",
        help = "The number of cores to use during the benchmark",
        default_value = "1"
    )]
    pub cores: String,
}

#[derive(Parser, Debug)]
pub struct BoostArgs {
    #[arg(
        value_name = "MINT_ADDRESS",
        help = "The mint address of the boost to get."
    )]
    pub mint: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CheckpointArgs {
    #[arg(
        value_name = "MINT_ADDRESS",
        help = "The mint address of the boost to checkpoint"
    )]
    pub mint: String,

    #[arg(
        long,
        short,
        help = "Flag indicating whether or not to run in continuous mode.",
        default_value = "false"
    )]
    pub continuous: bool,
}

#[derive(Parser, Debug)]
pub struct ClaimArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of rewards to claim. Defaults to max."
    )]
    pub amount: Option<f64>,

    #[arg(
        long,
        value_name = "WALLET_ADDRESS",
        help = "Wallet address to receive claimed tokens."
    )]
    pub to: Option<String>,

    #[arg(
        long,
        short,
        value_name = "POOL_URL",
        help = "The optional pool url to claim rewards from."
    )]
    pub pool_url: Option<String>,
}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
pub struct InitializeArgs {}

#[derive(Parser, Debug)]
pub struct MineArgs {
    #[arg(
        long,
        short,
        value_name = "CORES_COUNT",
        help = "The number of CPU cores to allocate to mining.",
        default_value = "1"
    )]
    pub cores: String,

    #[arg(
        long,
        short,
        value_name = "SECONDS",
        help = "The number seconds before the deadline to stop mining and start submitting.",
        default_value = "5"
    )]
    pub buffer_time: u64,

    #[arg(
        long,
        short,
        value_name = "DEVICE_ID",
        help = "An optional device id to use for pool mining (max 5 devices per keypair)."
    )]
    pub device_id: Option<u64>,

    #[arg(
        long,
        short,
        value_name = "POOL_URL",
        help = "The optional pool url to join and forward solutions to."
    )]
    pub pool_url: Option<String>,

    #[arg(
        long,
        short,
        help = "Flag indicating whether or not to run in verbose mode.",
        default_value = "false"
    )]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
pub struct PoolArgs {
    #[arg(value_name = "POOL_URL", help = "The pool url to connect to.")]
    pub pool_url: Option<String>,

    #[command(subcommand)]
    pub command: Option<PoolCommand>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum PoolCommand {
    #[command(about = "Commit a pending pool balance to the chain.")]
    Commit(PoolCommitArgs),
}

#[derive(Parser, Clone, Debug)]
pub struct PoolCommitArgs {}

#[derive(Parser, Debug)]
pub struct ProgramArgs {}

#[derive(Parser, Debug)]
pub struct ProofArgs {
    #[arg(value_name = "ADDRESS", help = "The address of the proof to fetch.")]
    pub address: Option<String>,
}

#[derive(Clone, Parser, Debug)]
pub struct StakeArgs {
    #[command(subcommand)]
    pub command: Option<StakeCommand>,

    #[arg(value_name = "MINT_ADDRESS", help = "The mint to stake with.")]
    pub mint: Option<String>,

    #[arg(
        long,
        short,
        value_name = "ACCOUNT_ADDRESS",
        help = "List the stake accounts of another authority."
    )]
    pub authority: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum StakeCommand {
    #[command(about = "Claim rewards from a stake account.")]
    Claim(StakeClaimArgs),

    #[command(about = "Deposit tokens into a stake account.")]
    Deposit(StakeDepositArgs),

    #[command(about = "Withdraw tokens from a stake account.")]
    Withdraw(StakeWithdrawArgs),

    #[command(about = "Get the list of stake accounts in a boost.")]
    Accounts(StakeAccountsArgs),
}

#[derive(Parser, Clone, Debug)]
pub struct StakeClaimArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of rewards to claim. Defaults to max."
    )]
    pub amount: Option<f64>,

    #[arg(
        long,
        value_name = "WALLET_ADDRESS",
        help = "Wallet address to receive claimed tokens."
    )]
    pub to: Option<String>,
}

#[derive(Parser, Clone, Debug)]
pub struct StakeDepositArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of stake to deposit. Defaults to max."
    )]
    pub amount: Option<f64>,

    #[arg(
        long,
        value_name = "TOKEN_ACCOUNT_ADDRESS",
        help = "Token account to deposit from. Defaults to the associated token account."
    )]
    pub token_account: Option<String>,
}

#[derive(Parser, Clone, Debug)]
pub struct StakeWithdrawArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of stake to withdraw. Defaults to max."
    )]
    pub amount: Option<f64>,

    #[arg(
        long,
        value_name = "TOKEN_ACCOUNT_ADDRESS",
        help = "Token account to withdraw to. Defaults to the associated token account."
    )]
    pub token_account: Option<String>,
}

#[derive(Parser, Clone, Debug)]
pub struct StakeMigrateArgs {}

#[derive(Parser, Clone, Debug)]
pub struct StakeAccountsArgs {}

#[derive(Parser, Debug)]
pub struct TransactionArgs {
    #[arg(value_name = "SIGNATURE", help = "The signature of the transaction.")]
    pub signature: String,
}

#[derive(Parser, Debug)]
pub struct TransferArgs {
    #[arg(value_name = "AMOUNT", help = "The amount of ORE to transfer.")]
    pub amount: f64,

    #[arg(
        value_name = "RECIPIENT_ADDRESS",
        help = "The account address of the receipient."
    )]
    pub to: String,
}

#[derive(Parser, Debug)]
pub struct UnstakeArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of the token to unstake. Defaults to max."
    )]
    pub amount: Option<f64>,

    #[arg(value_name = "MINT_ADDRESS", help = "The mint to unstake.")]
    pub mint: String,

    #[arg(
        long,
        value_name = "TOKEN_ACCOUNT_ADDRESS",
        help = "Token account to receive unstaked funds. Defaults to the associated token account."
    )]
    pub token_account: Option<String>,

    #[arg(
        long,
        short,
        value_name = "POOL_URL",
        help = "The optional pool url to unstake from."
    )]
    pub pool_url: Option<String>,
}

#[derive(Parser, Debug)]
pub struct UpgradeArgs {
    #[arg(
        value_name = "AMOUNT",
        help = "The amount of ORE to upgrade from v1 to v2. Defaults to max."
    )]
    pub amount: Option<f64>,
}
