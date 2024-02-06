use {
    crate::process::process_event_files,
    chrono::{DateTime, Utc},
    solana_alt_store::Store,
    solana_core::banking_trace::{BankingPacketBatch, ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_sdk::{
        pubkey::Pubkey,
        slot_history::Slot,
        transaction::{SanitizedTransaction, SanitizedVersionedTransaction, VersionedTransaction},
    },
    std::{collections::HashSet, path::PathBuf, time::SystemTime},
};

pub fn dump(
    event_file_paths: &[PathBuf],
    accounts: Option<HashSet<Pubkey>>,
    skip_alt_resolution: bool,
) -> std::io::Result<()> {
    let mut handler = Dumper::new(accounts, skip_alt_resolution);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    Ok(())
}

struct Dumper {
    accounts: Option<HashSet<Pubkey>>,
    alt_store: Option<Store>,
}

impl Dumper {
    pub fn new(accounts: Option<HashSet<Pubkey>>, skip_alt_resolution: bool) -> Self {
        const ALT_STORE_PATH: &str = "alt-store.bin";

        Self {
            accounts,
            alt_store: (!skip_alt_resolution)
                .then(|| Store::load_or_create(ALT_STORE_PATH).expect("failed to load alt store")),
        }
    }

    pub fn handle_event(&mut self, TimedTracedEvent(timestamp, event): TimedTracedEvent) {
        match event {
            TracedEvent::PacketBatch(label, packet_batches) => {
                self.handle_packet_batches(timestamp, label, packet_batches)
            }
            TracedEvent::BlockAndBankHash(slot, _, _) => {
                self.handle_block_and_bank_hash(timestamp, slot)
            }
        }
    }

    fn handle_packet_batches(
        &mut self,
        timestamp: SystemTime,
        label: ChannelLabel,
        packet_batches: BankingPacketBatch,
    ) {
        let timestamp = DateTime::<Utc>::from(timestamp);
        if matches!(label, ChannelLabel::NonVote) {
            // panic!("NonVote: {:?}", packet_batches);
            for packet_batch in packet_batches.0.iter() {
                for packet in packet_batch {
                    let Some(data) = packet.data(..) else {
                        continue;
                    };
                    let Some(versioned_transaction) =
                        bincode::deserialize::<VersionedTransaction>(data).ok()
                    else {
                        continue;
                    };
                    let Ok(sanitized_versioned_transaction) =
                        SanitizedVersionedTransaction::try_from(versioned_transaction)
                    else {
                        continue;
                    };

                    match &self.alt_store {
                        None => {
                            // Skipping ALT resolution, check static SVT account keys and dump.
                            let dump = if let Some(accounts) = &self.accounts {
                                sanitized_versioned_transaction
                                    .get_message()
                                    .message
                                    .static_account_keys()
                                    .iter()
                                    .any(|account| accounts.contains(account))
                            } else {
                                true
                            };
                            if dump {
                                println!("{timestamp:?} - {sanitized_versioned_transaction:?}");
                            }
                        }
                        Some(alt_store) => {
                            // Resolve ALT. If successful, check all account keys and dump.
                            let hash = sanitized_versioned_transaction.get_message().message.hash();
                            let Ok(sanitized_transaction) = SanitizedTransaction::try_new(
                                sanitized_versioned_transaction,
                                hash,
                                false,
                                alt_store,
                            ) else {
                                continue;
                            };

                            let message = sanitized_transaction.message();
                            let account_keys = message.account_keys();
                            let dump = if let Some(accounts) = &self.accounts {
                                account_keys
                                    .iter()
                                    .any(|account| accounts.contains(account))
                            } else {
                                true
                            };

                            if dump {
                                println!("{timestamp:?} - {sanitized_transaction:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_block_and_bank_hash(&mut self, timestamp: SystemTime, slot: Slot) {
        let timestamp = DateTime::<Utc>::from(timestamp);
        println!("{timestamp:?} - {slot:?}");
    }
}
