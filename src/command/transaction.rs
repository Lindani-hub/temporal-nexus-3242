use std::str::FromStr;

use b64::FromBase64;
use colored::Colorize;
use ore_api::event::MineEvent;
use solana_sdk::signature::Signature;
use solana_transaction_status::{option_serializer::OptionSerializer, UiTransactionEncoding};
use tabled::{settings::{object::{Columns, Rows}, Alignment, Remove, Style}, Table};

use crate::{error::Error, utils::{amount_u64_to_string, format_timestamp, TableData, TableSectionTitle}, Miner, TransactionArgs};

impl Miner {
    pub async fn transaction(&self, args: TransactionArgs) -> Result<(), Error> {
        let signature = args.signature;
        let signature = Signature::from_str(&signature).expect("Failed to parse signature");
        match self.rpc_client.get_transaction(&signature, UiTransactionEncoding::Json).await {
            Ok(tx) => {
                let mut data = vec![];
                
                // Parse transaction response
                if let Some(meta) = tx.transaction.meta {
                    if let OptionSerializer::Some(log_messages) = meta.log_messages {
                        if let Some(return_log) = log_messages.iter().find(|log| log.starts_with("Program return: ")) {
                            if let Some(return_data) = return_log.strip_prefix(&format!("Program return: {} ", ore_api::ID)) {
                                if let Ok(return_data) = return_data.from_base64() {
                                    let event = MineEvent::from_bytes(&return_data);
                                    data.push(TableData {
                                        key: "Signature".to_string(),
                                        value: signature.to_string(),
                                    });
                                    data.push(TableData {
                                        key: "Block".to_string(),
                                        value: tx.slot.to_string(),
                                    });
                                    data.push(TableData {
                                        key: "Timestamp".to_string(),
                                        value: format_timestamp(tx.block_time.unwrap_or_default()),
                                    });
                                    data.push(TableData {
                                        key: "Difficulty".to_string(),
                                        value: event.difficulty.to_string(),
                                    });
                                    data.push(TableData {
                                        key: "Base Reward".to_string(),
                                        value: amount_u64_to_string(event.net_base_reward),
                                    });
                                    data.push(TableData {
                                        key: "Boost Reward".to_string(),
                                        value: amount_u64_to_string(event.net_miner_boost_reward),
                                    });
                                    data.push(TableData {
                                        key: "Total Reward".to_string(),
                                        value: amount_u64_to_string(event.net_reward),
                                    });
                                    data.push(TableData {
                                        key: "Timing".to_string(),
                                        value: format!("{}s", event.timing),
                                    });
                                    data.push(TableData {
                                        key: "Status".to_string(),
                                        value: match meta.status {
                                            Ok(()) => "Confirmed".bold().green().to_string(),
                                            Err(_e) => "Failed".bold().red().to_string(),
                                        },
                                    });

                                }
                            }
                        }
                    }
                }

                // Check if data is empty
                if data.is_empty() {
                    return Err(Error::Internal("Unknown transaction".to_string())).map_err(From::from);
                }

                // Print table
                let mut table = Table::new(data);
                table.with(Remove::row(Rows::first()));
                table.modify(Columns::single(1), Alignment::right());
                table.with(Style::blank());
                table.section_title(0, "Transaction");
                println!("{table}\n");
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
        Ok(())
    }
}