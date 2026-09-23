use ore_api::consts::{EPOCH_DURATION, BUS_ADDRESSES, TREASURY_TOKENS_ADDRESS, TREASURY_ADDRESS};
use tabled::{Table, settings::{Style, object::{Rows, Columns}, Alignment, Remove}};

use crate::{utils::{get_config, amount_u64_to_f64, format_timestamp, get_bus, TableData, TableSectionTitle}, Miner};


impl Miner {
    pub async fn program(&self) {
        // Aggregate data
        let mut data = vec![];
        self.fetch_config_data(&mut data).await;
        let len1 = data.len();
        self.fetch_busses_data(&mut data).await;
        let len2 = data.len();
        self.fetch_rewards_data(&mut data).await;
        let len3 = data.len();
        self.fetch_treasury_data(&mut data).await;

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Config");
        table.section_title(len1, "Busses");
        table.section_title(len2, "Reward rates");
        table.section_title(len3, "Treasury");
        println!("{table}\n");
    }

    async fn fetch_config_data(&self, data: &mut Vec<TableData>) {
        let config = get_config(&self.rpc_client).await;
        data.push(TableData {
            key: "Epoch duration".to_string(),
            value: format!("{} sec", EPOCH_DURATION),
        });
        data.push(TableData {
            key: "Epoch start at".to_string(),
            value: format_timestamp(config.last_reset_at),
        });
        data.push(TableData {
            key: "Min difficulty".to_string(),
            value: config.min_difficulty.to_string(),
        });
    }

    async fn fetch_busses_data(&self, data: &mut Vec<TableData>) {
        for address in BUS_ADDRESSES.iter() {
            let bus = get_bus(&self.rpc_client, *address).await.expect("Failed to fetch bus account");
            let rewards = amount_u64_to_f64(bus.rewards);
            data.push(TableData {
                key: format!("{}", bus.id),
                value: format!("{:#.11} ORE", rewards),
            });
        }
    }

    async fn fetch_rewards_data(&self, data: &mut Vec<TableData>) {
        let config = get_config(&self.rpc_client).await;
        for i in 0..32 {
            let reward_rate = config.base_reward_rate.saturating_mul(2u64.saturating_pow(i));
            let amount = amount_u64_to_f64(reward_rate).min(1.0);
            data.push(TableData {
                key: format!("{}{}", config.min_difficulty as u32 + i, if amount >= 1.0 { "+" } else { "" }),
                value: format!("{:#.11} ORE", amount),
            });
            if amount >= 1.0 {
                break;
            }
        }
    }

    async fn fetch_treasury_data(&self, data: &mut Vec<TableData>) {
        let token_balance =  self
            .rpc_client
            .get_token_account(&TREASURY_TOKENS_ADDRESS)
            .await
            .expect("Failed to fetch treasury tokens account")
            .expect("Failed to fetch treasury tokens account");
        data.push(TableData {
            key: "Address".to_string(),
            value: TREASURY_ADDRESS.to_string(),
        });
        data.push(TableData {
            key: "Balance".to_string(),
            value: format!("{} ORE", token_balance.token_amount.ui_amount_string),
        });
    }
}