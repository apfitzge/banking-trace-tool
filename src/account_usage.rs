use {
    crate::{cli::SlotRange, process::process_event_files},
    agave_banking_stage_ingress_types::BankingPacketBatch,
    solana_alt_store::Store,
    solana_borsh::v1::try_from_slice_unchecked,
    solana_clock::Slot,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_core::banking_trace::{ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_pubkey::Pubkey,
    solana_sdk_ids::compute_budget,
    solana_transaction::{
        sanitized::SanitizedTransaction,
        versioned::{sanitized::SanitizedVersionedTransaction, VersionedTransaction},
    },
    std::{
        collections::{HashMap, HashSet},
        ops::RangeInclusive,
        path::PathBuf,
    },
};

pub fn account_usage(event_file_paths: &[PathBuf], slot_range: SlotRange) -> std::io::Result<()> {
    let mut handler = AccountUsageHandler::new(slot_range);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report();
    Ok(())
}

struct AccountUsageHandler {
    range: RangeInclusive<Slot>,
    current_packet_batches: Vec<BankingPacketBatch>,
    done: bool,
    alt_store: Store,
}

impl AccountUsageHandler {
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

    /// Report account usage statistics:
    /// - Unique accounts
    /// - Per account:
    ///     - Number of reads and writes
    ///     - write priority min, max, avg
    pub fn report(&self) {
        // Build account usage statistics
        let mut account_usage_statistics = HashMap::new();

        for tx in self
            .current_packet_batches
            .iter()
            .flat_map(|b| b.iter().flat_map(|b| b.iter()))
            .filter_map(|p| bincode::deserialize::<VersionedTransaction>(p.data(..)?).ok())
            .filter_map(|tx| SanitizedVersionedTransaction::try_from(tx).ok())
        {
            let (priority, requested_cus) = get_priority_and_requested_cus(&tx);
            let hash = tx.get_message().message.hash();
            let Ok(tx) =
                SanitizedTransaction::try_new(tx, hash, false, &self.alt_store, &HashSet::new())
            else {
                eprintln!(
                    "failed to sanitize transaction. Possibly need to update the alt-store first."
                );
                continue;
            };

            let account_locks = tx.get_account_locks_unchecked();
            for account in &account_locks.writable {
                let statistics = account_usage_statistics
                    .entry(**account)
                    .or_insert_with(|| AccountUsageStatistics::new(**account));
                statistics.update(true, priority, requested_cus);
            }
            for account in &account_locks.readonly {
                let statistics = account_usage_statistics
                    .entry(**account)
                    .or_insert_with(|| AccountUsageStatistics::new(**account));
                statistics.update(false, priority, requested_cus);
            }
        }

        // Sort accounts by write usage before report. Higher usage first.
        let mut account_usage_statistics: Vec<_> = account_usage_statistics.values().collect();
        account_usage_statistics.sort_by_key(|s| -(s.num_writes as i64));

        // Report
        println!("Total unique accounts: {}", account_usage_statistics.len());
        for s in account_usage_statistics {
            AccountUsageStatistics::report(s);
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
        } else {
            self.current_packet_batches.clear();
        }
    }
}

struct AccountUsageStatistics {
    key: Pubkey,

    // Access kinds
    num_reads: usize,
    num_writes: usize,

    // Priority
    min_priority: u64,
    sum_priority: u64,
    max_priority: u64,

    // Requested CUs
    min_requested_cus: u64,
    sum_requested_cus: u64,
    max_requested_cus: u64,
}

impl AccountUsageStatistics {
    pub fn new(key: Pubkey) -> Self {
        Self {
            key,
            num_reads: 0,
            num_writes: 0,
            min_priority: u64::MAX,
            sum_priority: 0,
            max_priority: 0,
            min_requested_cus: u64::MAX,
            sum_requested_cus: 0,
            max_requested_cus: 0,
        }
    }

    pub fn update(&mut self, is_write: bool, priority: u64, requested_cus: u64) {
        if is_write {
            self.num_writes += 1;
        } else {
            self.num_reads += 1;
        }

        self.min_priority = self.min_priority.min(priority);
        self.sum_priority += priority;
        self.max_priority = self.max_priority.max(priority);

        self.min_requested_cus = self.min_requested_cus.min(requested_cus);
        self.sum_requested_cus += requested_cus;
        self.max_requested_cus = self.max_requested_cus.max(requested_cus);
    }

    pub fn report(
        Self {
            key,
            num_reads,
            num_writes,
            min_priority,
            sum_priority,
            max_priority,
            min_requested_cus,
            sum_requested_cus,
            max_requested_cus,
        }: &Self,
    ) {
        let num_txs = num_reads + num_writes;
        let avg_priority = sum_priority / num_txs as u64;
        let avg_requested_cus = sum_requested_cus / num_txs as u64;
        println!("{key}: [{num_reads}, {num_writes}] priority: [{min_priority}, {avg_priority}, {max_priority}] requested_cus: [{min_requested_cus}, {avg_requested_cus}, {max_requested_cus}]")
    }
}

/// Returns priorty and requested_cus
fn get_priority_and_requested_cus(tx: &SanitizedVersionedTransaction) -> (u64, u64) {
    let instructions = tx.get_message().program_instructions_iter();
    let mut non_compute_budget_ix_count = 0u64;
    let mut priority = 0u64;
    let mut requested_cus = None;
    for (program, ix) in instructions {
        if !compute_budget::check_id(program) {
            non_compute_budget_ix_count += 1;
            continue;
        }

        let ix: ComputeBudgetInstruction = try_from_slice_unchecked(&ix.data).unwrap();
        match ix {
            ComputeBudgetInstruction::RequestHeapFrame(_) => {}
            ComputeBudgetInstruction::SetComputeUnitLimit(units) => {
                requested_cus = Some(units as u64)
            }
            ComputeBudgetInstruction::SetComputeUnitPrice(cu_price) => priority = cu_price,
            ComputeBudgetInstruction::Unused
            | ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit(_) => {}
        }
    }

    (
        priority,
        requested_cus
            .unwrap_or(non_compute_budget_ix_count * 200_000)
            .max(1_400_000),
    )
}
