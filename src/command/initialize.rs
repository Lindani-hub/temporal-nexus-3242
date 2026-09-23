use ore_api::consts::TREASURY_ADDRESS;
use solana_sdk::signature::Signer;

use crate::{Miner, utils::ComputeBudget};

impl Miner {
    pub async fn initialize(&self) {
        // Return early if program is already initialized
        if self.rpc_client.get_account(&TREASURY_ADDRESS).await.is_ok() {
            return;
        }

        // Submit initialize tx
        let ix = ore_api::sdk::initialize(self.signer().pubkey());
        let _ = self.send_and_confirm(&[ix], ComputeBudget::Fixed(500_000), false).await.unwrap();
    }
}
