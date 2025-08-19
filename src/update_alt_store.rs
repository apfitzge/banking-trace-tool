use {
    crate::{cli::SlotRange, process::process_event_files},
    agave_banking_stage_ingress_types::BankingPacketBatch,
    solana_alt_store::{Store, UpdateMode},
    solana_clock::Slot,
    solana_core::banking_trace::{ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_transaction::versioned::VersionedTransaction,
    std::{collections::HashSet, ops::RangeInclusive, path::PathBuf},
};

pub fn update_alt_store(
    event_file_paths: &[PathBuf],
    slot_range: SlotRange,
) -> std::io::Result<()> {
    let mut handler = UpdateAddressLookupTableStoreHandler::new(slot_range);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    Ok(())
}

struct UpdateAddressLookupTableStoreHandler {
    range: RangeInclusive<Slot>,
    current_packet_batches: Vec<BankingPacketBatch>,
    done: bool,
    alt_store: Store,
}

impl UpdateAddressLookupTableStoreHandler {
    pub fn new(slot_range: SlotRange) -> Self {
        const ALT_STORE_PATH: &str = "alt-store.bin";

        Self {
            range: slot_range.start_slot..=slot_range.end_slot,
            current_packet_batches: Vec::new(),
            done: false,
            alt_store: Store::load_or_create(ALT_STORE_PATH).expect("failed to load alt store"),
        }
    }

    pub fn handle_event(&mut self, TimedTracedEvent(_timestamp, event): TimedTracedEvent) {
        if self.done {
            return;
        }

        match event {
            TracedEvent::PacketBatch(label, packet_batches) => {
                self.handle_packet_batches(label, packet_batches)
            }
            TracedEvent::BlockAndBankHash(slot, _, _) => self.handle_block_and_bank_hash(slot),
        }
    }

    fn handle_packet_batches(&mut self, label: ChannelLabel, packet_batches: BankingPacketBatch) {
        if matches!(label, ChannelLabel::NonVote) {
            self.current_packet_batches.push(packet_batches);
        }
    }

    fn handle_block_and_bank_hash(&mut self, slot: Slot) {
        if !self.range.contains(&slot) {
            if slot > *self.range.end() {
                self.done = true;
            }
            return;
        }

        // Collect unique ALT addresses
        let mut unique_alts = HashSet::new();
        for tx in self
            .current_packet_batches
            .iter()
            .flat_map(|b| b.iter().flat_map(|b| b.iter()))
            .filter_map(|p| bincode::deserialize::<VersionedTransaction>(p.data(..)?).ok())
        {
            if let Some(atls) = tx.message.address_table_lookups() {
                for atl in atls {
                    unique_alts.insert(atl.account_key);
                }
            }
        }

        // Update the store with ALTs from this slot
        let unique_alts: Vec<_> = unique_alts.into_iter().collect();
        println!("Fetching {} ALTs for slot {}", unique_alts.len(), slot);
        self.alt_store
            .update(&unique_alts, UpdateMode::Append)
            .unwrap();

        self.current_packet_batches.clear();
    }
}
