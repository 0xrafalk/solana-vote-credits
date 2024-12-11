use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_vote_program::vote_state::VoteState;
use std::str::FromStr;

pub async fn test_rpc_client() {
    let rpc_url = "https://api.mainnet-beta.solana.com"; // Replace with your RPC URL
    let vote_account = "Chorus6Kis8tFHA7AowrPMcRJk3LbApHTYpgSNXzY5KE";
    
    let rpc_client = RpcClient::new(rpc_url.to_string()); // Convert &str to String
    
    println!("Fetching data for vote account: {}", vote_account);
    
    let vote_pubkey = Pubkey::from_str(vote_account).expect("Invalid vote account pubkey");
    match rpc_client.get_account(&vote_pubkey).await {
        Ok(account_data) => {
            println!("Fetched Account Data: {:?}", account_data.data);
            match bincode::deserialize::<VoteState>(&account_data.data) {
                Ok(vote_state) => println!("Deserialized Vote State: {:?}", vote_state),
                Err(e) => eprintln!("Failed to deserialize VoteState: {}", e),
            }
        }
        Err(err) => eprintln!("Error fetching account data: {}", err),
    }
}
