use solana_sdk::pubkey::Pubkey;
use solana_vote_program::vote_state::VoteState;
use solana_client::rpc_client::RpcClient;
use slog::{Drain, Logger, info, warn, o};
use sqlx::{PgPool, query};
use std::str::FromStr;
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
struct Account {
    alias: String,
    address: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SolanaConfig {
    rpc_url: String,
    rpc_timeout_seconds: u64,
    accounts: Vec<Account>,
}

/// Inserts a row into the timely_vote_credits table.
async fn insert_vote_credits(
    db: &PgPool,
    alias: &str,
    epoch: u64,
    earned_credits: u64,
    max_credits: u64,
    score: f64,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO timely_vote_credits (alias, epoch, earned_credits, max_possible_credits, score) 
        VALUES ($1, $2, $3, $4, $5) 
        ON CONFLICT (alias, epoch) 
        DO UPDATE SET 
            earned_credits = EXCLUDED.earned_credits,
            max_possible_credits = EXCLUDED.max_possible_credits,
            score = EXCLUDED.score;",
        alias,
        epoch as i64,
        earned_credits as i64,
        max_credits as i64,
        score
    )
    .execute(db)
    .await?;
    Ok(())
}

async fn calculate_credits_score(
    db: &PgPool,
    vote_state: &VoteState,
    alias: &str,
) -> Result<()> {
    const MAX_CREDITS_PER_EPOCH: u64 = 432_000 * 16; 
    // There is a grace period of 2 slots meaning that votes with a slot latency of 2 slots or less will receive the maximum of 16 credits
    // https://www.anza.xyz/blog/feature-gate-spotlight-timely-vote-credits

    for (epoch, credits, prev_credits) in &vote_state.epoch_credits {
        let earned_credits = credits - prev_credits;
        let score = (earned_credits as f64 / MAX_CREDITS_PER_EPOCH as f64) * 100.0;

        insert_vote_credits(
            db,
            alias,
            *epoch,
            earned_credits,
            MAX_CREDITS_PER_EPOCH,
            score,
        )
        .await?;
    }

    Ok(())
}

async fn fetch_vote_state(client: &RpcClient, pubkey: &Pubkey) -> Result<VoteState> {
    let account_data = client.get_account(pubkey)?.data;
    let vote_state = VoteState::deserialize(&account_data)?;
    Ok(vote_state)
}

async fn process_accounts(log: Logger, solana_config: SolanaConfig, db: PgPool) -> Result<()> {
    let client = RpcClient::new_with_timeout(
        solana_config.rpc_url.clone(),
        Duration::from_secs(solana_config.rpc_timeout_seconds),
    );
/*
/// Used to populated the table. Commented out as the refresh functions fetches wnloads new data.
    for account in solana_config.accounts {
        let pubkey = Pubkey::from_str(&account.address)?;
        info!(log, "Fetching vote state for account: {} ({})", account.alias, account.address);

        match fetch_vote_state(&client, &pubkey).await {
            Ok(vote_state) => {
                info!(log, "Fetched and deserialized VoteState for: {}", account.alias);
                calculate_credits_score(&db, &vote_state, &account.alias).await?;
            }
            Err(err) => {
                warn!(
                    log,
                    "Failed to fetch or deserialize VoteState for {}: {:?}", account.alias, err
                );
            }
        }
    } */
    Ok(())
}

/// Refresh and check for new data periodically.
async fn refresh_and_check_for_new_data(
    log: Logger,
    solana_config: SolanaConfig,
    db: PgPool,
) -> Result<()> {
    loop {
        info!(log, "Refreshing and checking for new data...");
        match process_accounts(log.clone(), solana_config.clone(), db.clone()).await {
            Ok(_) => info!(log, "Successfully refreshed data."),
            Err(err) => warn!(log, "Failed to refresh data: {:?}", err),
        }
        tokio::time::sleep(Duration::from_secs(300)).await; // Wait for 5 minutes before refreshing again
    }
}

fn load_solana_config(file_path: &str) -> Result<SolanaConfig> {
    let config_data = fs::read_to_string(file_path)?;
    let config: SolanaConfig = toml::from_str(&config_data)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let async_drain = slog_async::Async::new(drain).build().fuse();
    let log = Logger::root(async_drain, o!());
    let solana_config = load_solana_config("config.toml")?;

    let database_url = "postgres://wagmi_app:wagmi_app@localhost/wagmi";
    let db_pool = PgPool::connect(&database_url).await?;

    // Run the refresh loop in the background
    refresh_and_check_for_new_data(log.clone(), solana_config, db_pool).await?;

    Ok(())
}