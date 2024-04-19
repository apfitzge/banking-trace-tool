use {
    crate::process::process_event_files,
    chrono::{DateTime, Utc},
    solana_core::banking_trace::{BankingPacketBatch, ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_sdk::{signature::Signature, slot_history::Slot, transaction::VersionedTransaction},
    std::{
        collections::{hash_map::Entry, HashMap},
        path::PathBuf,
    },
};

pub fn duplicate_check(
    event_file_paths: &[PathBuf],
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
) -> std::io::Result<()> {
    let mut handler = DuplicateChecker::new(start_timestamp, end_timestamp);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report();
    Ok(())
}

struct DuplicateChecker {
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
    started: bool,
    done: bool,
    signature_states: HashMap<Signature, DuplicateCheckState>,
}

struct DuplicateCheckState {
    initial_forwarded: bool,
    // initial_staked: bool, // don't have this meta until update.
    duplicate_tpu_count: usize,
    duplicate_forwarded_count: usize,
    // duplicate_staked_count: usize, // don't have this meta until update.
}

impl DuplicateChecker {
    pub fn new(
        start_timestamp: Option<DateTime<Utc>>,
        end_timestamp: Option<DateTime<Utc>>,
    ) -> Self {
        let started = start_timestamp.is_none();
        Self {
            start_timestamp,
            end_timestamp,
            started,
            done: false,
            signature_states: HashMap::new(),
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
                    self.handle_packet_batches(label, packet_batches)
                }
                TracedEvent::BlockAndBankHash(slot, _, _) => {
                    self.handle_block_and_bank_hash(timestamp, slot)
                }
            }
        }
    }

    fn handle_packet_batches(&mut self, label: ChannelLabel, packet_batches: BankingPacketBatch) {
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
                    let signature = versioned_transaction.signatures[0];
                    match self.signature_states.entry(signature) {
                        Entry::Occupied(mut state) => {
                            let state = state.get_mut();
                            if state.initial_forwarded != packet.meta().forwarded() {
                                state.duplicate_forwarded_count += 1;
                            }
                        }
                        Entry::Vacant(state) => {
                            state.insert(DuplicateCheckState {
                                initial_forwarded: packet.meta().forwarded(),
                                duplicate_tpu_count: 0,
                                duplicate_forwarded_count: 0,
                            });
                        }
                    }
                }
            }
        }
    }

    fn handle_block_and_bank_hash(&mut self, timestamp: DateTime<Utc>, slot: Slot) {
        println!("{timestamp:?} - {slot:?}");
    }

    fn report(&self) {
        // Determine percentage of duplicate packets that were forwarded vs not.
        let mut total_packets = 0;
        let mut total_duplicate_packets = 0;

        let mut total_tpu_packets = 0;
        let mut total_forwarded_packets = 0;

        let mut duplicate_tpu_packets = 0;
        let mut duplicate_forwarded_packets = 0;

        for (_signature, state) in self.signature_states.iter() {
            total_packets += 1 + state.duplicate_tpu_count + state.duplicate_forwarded_count;
            total_duplicate_packets += state.duplicate_tpu_count + state.duplicate_forwarded_count;

            total_tpu_packets += state.duplicate_tpu_count + usize::from(!state.initial_forwarded);
            total_forwarded_packets +=
                state.duplicate_forwarded_count + usize::from(state.initial_forwarded);

            duplicate_tpu_packets += state.duplicate_tpu_count;
            duplicate_forwarded_packets += state.duplicate_forwarded_count;
        }

        let duplicate_packet_percentage =
            100.0 * total_duplicate_packets as f64 / total_packets as f64;

        let tpu_packet_percentage = 100.0 * total_tpu_packets as f64 / total_packets as f64;
        let forwarded_packet_percentage =
            100.0 * total_forwarded_packets as f64 / total_packets as f64;

        let tpu_percent_duplicate = 100.0 * duplicate_tpu_packets as f64 / total_tpu_packets as f64;
        let forwarded_percent_duplicate =
            100.0 * duplicate_forwarded_packets as f64 / total_forwarded_packets as f64;

        println!("Total packets: {total_packets}");
        println!("Total duplicate packets: {total_duplicate_packets} ({duplicate_packet_percentage:.2}%)");
        println!("Total TPU packets: {total_tpu_packets} ({tpu_packet_percentage:.2}%)");
        println!("Total forwarded packets: {total_forwarded_packets} ({forwarded_packet_percentage:.2}%)");
        println!("Duplicate TPU packets: {duplicate_tpu_packets} ({tpu_percent_duplicate:.2}%)");
        println!("Duplicate forwarded packets: {duplicate_forwarded_packets} ({forwarded_percent_duplicate:.2}%)");
    }
}
