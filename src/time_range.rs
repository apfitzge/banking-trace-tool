use {
    crate::process::process_event_files,
    chrono::{DateTime, Utc},
    solana_core::banking_trace::TimedTracedEvent,
    std::path::PathBuf,
};

pub fn time_range(event_file_paths: &[PathBuf]) -> std::io::Result<()> {
    let mut handler = TimeRangeHandler::default();
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report();
    Ok(())
}

#[derive(Default)]
struct TimeRangeHandler {
    min: Option<DateTime<Utc>>,
    max: Option<DateTime<Utc>>,
}

impl TimeRangeHandler {
    fn handle_event(&mut self, TimedTracedEvent(timestamp, _event): TimedTracedEvent) {
        let timestamp = DateTime::<Utc>::from(timestamp);
        match &mut self.min {
            Some(min) => {
                *min = (*min).min(timestamp);
            }
            None => {
                self.min = Some(timestamp);
            }
        }

        match &mut self.max {
            Some(max) => {
                *max = (*max).max(timestamp);
            }
            None => {
                self.max = Some(timestamp);
            }
        }
    }

    fn report(&self) {
        println!(
            "{} - {}",
            self.min.unwrap_or_default(),
            self.max.unwrap_or_default()
        );
    }
}
