use {
    crate::process::process_event_files,
    solana_clock::Slot,
    solana_core::banking_trace::{TimedTracedEvent, TracedEvent},
    std::path::PathBuf,
};

pub fn slot_ranges(event_file_paths: &[PathBuf]) -> std::io::Result<()> {
    let mut handler = SlotRangesHandler::default();
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report_current_range();
    Ok(())
}

#[derive(Default)]
struct SlotRangesHandler {
    current_range: Option<(Slot, Slot)>,
}

impl SlotRangesHandler {
    pub fn handle_event(&mut self, TimedTracedEvent(_timestamp, event): TimedTracedEvent) {
        if let TracedEvent::BlockAndBankHash(slot, _, _) = event {
            match &mut self.current_range {
                Some((_start_slot, end_slot)) => {
                    if end_slot.saturating_add(1) == slot {
                        *end_slot = slot;
                    } else {
                        self.report_current_range();
                        self.current_range = Some((slot, slot));
                    }
                }
                None => self.current_range = Some((slot, slot)),
            }
        }
    }

    fn report_current_range(&self) {
        if let Some((start_slot, end_slot)) = self.current_range {
            println!("{start_slot}-{end_slot}");
        }
    }
}
