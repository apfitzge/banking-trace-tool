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
    std::{collections::HashSet, path::PathBuf},
};

pub fn dump(
    event_file_paths: &[PathBuf],
    accounts: Option<HashSet<Pubkey>>,
    skip_alt_resolution: bool,
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
) -> std::io::Result<()> {
    let mut handler = Dumper::new(
        accounts,
        skip_alt_resolution,
        start_timestamp,
        end_timestamp,
    );
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    Ok(())
}

struct Dumper {
    accounts: Option<HashSet<Pubkey>>,
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
    alt_store: Option<Store>,
    started: bool,
    done: bool,
}

impl Dumper {
    pub fn new(
        accounts: Option<HashSet<Pubkey>>,
        skip_alt_resolution: bool,
        start_timestamp: Option<DateTime<Utc>>,
        end_timestamp: Option<DateTime<Utc>>,
    ) -> Self {
        const ALT_STORE_PATH: &str = "alt-store.bin";
        let started = start_timestamp.is_none();
        Self {
            accounts,
            start_timestamp,
            end_timestamp,
            alt_store: (!skip_alt_resolution)
                .then(|| Store::load_or_create(ALT_STORE_PATH).expect("failed to load alt store")),
            started,
            done: false,
        }
    }

    pub fn handle_event(&mut self, TimedTracedEvent(timestamp, event): TimedTracedEvent) {
        if self.done {
            return;
        }
        let timestamp = DateTime::<Utc>::from(timestamp);
        self.started = self.started
            || self
                .start_timestamp
                .map(|start| timestamp >= start)
                .unwrap_or(true);
        self.done = self.done
            || self
                .end_timestamp
                .map(|end| timestamp > end)
                .unwrap_or(false);

        if self.started && !self.done {
            match event {
                TracedEvent::PacketBatch(label, packet_batches) => {
                    self.handle_packet_batches(timestamp, label, packet_batches)
                }
                TracedEvent::BlockAndBankHash(slot, _, _) => {
                    self.handle_block_and_bank_hash(timestamp, slot)
                }
            }
        }
    }

    fn handle_packet_batches(
        &mut self,
        timestamp: DateTime<Utc>,
        label: ChannelLabel,
        packet_batches: BankingPacketBatch,
    ) {
        if matches!(label, ChannelLabel::NonVote) {
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

    fn handle_block_and_bank_hash(&mut self, timestamp: DateTime<Utc>, slot: Slot) {
        println!("{timestamp:?} - {slot:?}");
    }
}
