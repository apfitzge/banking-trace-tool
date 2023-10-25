use {
    crate::process::process_event_files, solana_core::banking_trace::TimedTracedEvent,
    solana_sdk::slot_history::Slot, std::path::PathBuf,
};

pub fn update_address_lookup_table_store(
    event_file_paths: &[PathBuf],
    _start_slot: Slot,
    _end_slot: Slot,
) -> std::io::Result<()> {
    let mut handler = UpdateAddressLookupTableStoreHandler::default();
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    Ok(())
}

#[derive(Default)]
struct UpdateAddressLookupTableStoreHandler {}

impl UpdateAddressLookupTableStoreHandler {
    pub fn handle_event(&mut self, TimedTracedEvent(_timestamp, _event): TimedTracedEvent) {
        todo!()
    }
}
