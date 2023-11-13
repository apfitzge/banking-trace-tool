use {
    crate::process::process_event_files,
    prio_graph::{AccessKind, PrioGraph, TopLevelId},
    serde::Serialize,
    solana_alt_store::Store,
    solana_core::banking_trace::{BankingPacketBatch, ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_sdk::{
        borsh0_10::try_from_slice_unchecked,
        clock::Slot,
        compute_budget::{self, ComputeBudgetInstruction},
        transaction::{SanitizedTransaction, SanitizedVersionedTransaction, VersionedTransaction},
    },
    std::path::PathBuf,
};

pub fn graphia_input(
    event_file_paths: &[PathBuf],
    slot: Slot,
    output: PathBuf,
) -> std::io::Result<()> {
    let mut handler = GraphiaInputHandler::new(slot);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report(output)
}

struct GraphiaInputHandler {
    slot: Slot,
    current_packet_batches: Vec<BankingPacketBatch>,
    done: bool,
    alt_store: Store,
}

impl GraphiaInputHandler {
    pub fn new(slot: Slot) -> Self {
        const ALT_STORE_PATH: &str = "alt-store.bin";

        Self {
            slot,
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

    /// Write JSON for prio-graph of the current slot.
    /// Each transaction has following attributes:
    /// - Signature
    /// - Priority
    /// - Requested CUs
    pub fn report(&self, output: PathBuf) -> std::io::Result<()> {
        // Buffer all (transaction, priority, requested_cus) tuples.
        let mut transaction_tuples: Vec<_> = self
            .current_packet_batches
            .iter()
            .flat_map(|b| b.0.iter().flat_map(|b| b.iter().cloned()))
            .filter_map(|p| bincode::deserialize::<VersionedTransaction>(p.data(..)?).ok())
            .filter_map(|tx| SanitizedVersionedTransaction::try_from(tx).ok())
            .map(|tx| {
                let (priority, requested_cus) = get_priority_and_requested_cus(&tx);
                (tx, priority, requested_cus)
            })
            .filter_map(|(tx, priority, requested_cus)| {
                let hash = tx.get_message().message.hash();
                SanitizedTransaction::try_new(tx, hash, false, &self.alt_store)
                    .ok()
                    .map(|tx| (tx, priority, requested_cus))
            })
            .collect();

        // Sort by priority. Highest priority first.
        transaction_tuples.sort_by(|a, b| b.1.cmp(&a.1));

        // Insert into prio-graph in order of priority.
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        struct PriorityIndex {
            priority: u64,
            index: usize,
        }
        impl TopLevelId<PriorityIndex> for PriorityIndex {
            fn id(&self) -> PriorityIndex {
                *self
            }
        }
        impl Ord for PriorityIndex {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.priority.cmp(&other.priority)
            }
        }
        impl PartialOrd for PriorityIndex {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut graphia_input = GraphiaInput::default();
        let mut prio_graph = PrioGraph::new(|pi, _| *pi);
        let mut transaction_iterator = transaction_tuples.iter().enumerate();
        let mut insert_next_transaction = |prio_graph: &mut PrioGraph<_, _, _, _>| {
            let Some((index, (transaction, priority, _))) = transaction_iterator.next() else {
                return false;
            };

            let account_locks = transaction.get_account_locks_unchecked();
            let write_locks = account_locks
                .writable
                .iter()
                .map(|a| (*a, AccessKind::Write));
            let read_locks = account_locks
                .readonly
                .iter()
                .map(|a| (*a, AccessKind::Read));
            let transaction_access = write_locks.chain(read_locks);

            prio_graph.insert_transaction(
                PriorityIndex {
                    priority: *priority,
                    index,
                },
                transaction_access,
            );

            true
        };

        while insert_next_transaction(&mut prio_graph) {}

        let mut edge_count = 0;
        while !prio_graph.is_empty() {
            let mut popped = Vec::new();
            while let Some(id) = prio_graph.pop() {
                popped.push(id);

                // Insert a new node into the graphia input graph.
                let (tx, priority, requested_cus) = &transaction_tuples[id.index];
                graphia_input.graph.nodes.push(GraphiaInputNode {
                    id: id.index.to_string(),
                    metadata: GraphiaInputNodeMetaData {
                        signature: tx.signature().to_string(),
                        priority: *priority,
                        requested_cus: *requested_cus,
                    },
                });
            }

            for popped in popped {
                let unblocked = prio_graph.unblock(&popped);

                // Add edges to graphia input graph.
                for target in unblocked {
                    if !prio_graph.is_blocked(target) {
                        graphia_input.graph.edges.push(GraphiaInputEdge {
                            id: edge_count.to_string(),
                            // metadata: GraphiaInputEdgeMetaData {},
                            source: popped.index.to_string(),
                            target: target.index.to_string(),
                        });
                        edge_count += 1;
                    }
                }
            }
        }

        let file = std::fs::File::options()
            .write(true)
            .create(true)
            .append(false)
            .truncate(true)
            .open(output)?;
        serde_json::to_writer(file, &graphia_input).unwrap();
        Ok(())
    }

    fn handle_packet_batches(&mut self, label: ChannelLabel, packet_batches: BankingPacketBatch) {
        if matches!(label, ChannelLabel::NonVote) {
            self.current_packet_batches.push(packet_batches);
        }
    }

    fn handle_block_and_bank_hash(&mut self, slot: Slot) {
        if self.slot != slot {
            self.current_packet_batches.clear();
        } else {
            self.done = true;
        }
    }
}

#[derive(Default, Serialize)]
struct GraphiaInput {
    graph: GraphiaInputGraph,
}

#[derive(Serialize)]
struct GraphiaInputGraph {
    directed: bool,
    edges: Vec<GraphiaInputEdge>,
    nodes: Vec<GraphiaInputNode>,
}

impl Default for GraphiaInputGraph {
    fn default() -> Self {
        Self {
            directed: true,
            edges: Vec::new(),
            nodes: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct GraphiaInputEdge {
    id: String,
    // metadata: GraphiaInputEdgeMetaData,
    source: String,
    target: String,
}

// #[derive(Serialize)]
// struct GraphiaInputEdgeMetaData {}

#[derive(Serialize)]
struct GraphiaInputNode {
    id: String,
    metadata: GraphiaInputNodeMetaData,
}

#[derive(Serialize)]
struct GraphiaInputNodeMetaData {
    signature: String,
    priority: u64,
    requested_cus: u64,
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
            ComputeBudgetInstruction::RequestUnitsDeprecated {
                units,
                additional_fee,
            } => {
                requested_cus = Some(units as u64);
                priority = additional_fee as u64 * 1_000_000 / units as u64;
            }
            ComputeBudgetInstruction::RequestHeapFrame(_) => {}
            ComputeBudgetInstruction::SetComputeUnitLimit(units) => {
                requested_cus = Some(units as u64)
            }
            ComputeBudgetInstruction::SetComputeUnitPrice(cu_price) => priority = cu_price,
            ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit(_) => {}
        }
    }

    (
        priority,
        requested_cus
            .unwrap_or(non_compute_budget_ix_count * 200_000)
            .max(1_400_000),
    )
}
