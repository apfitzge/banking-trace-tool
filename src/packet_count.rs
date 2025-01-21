use {
    crate::process::process_event_files,
    chrono::{DateTime, Utc},
    solana_core::banking_trace::{BankingPacketBatch, ChannelLabel, TimedTracedEvent, TracedEvent},
    solana_sdk::{signature::Signature, slot_history::Slot, transaction::VersionedTransaction},
    std::{
        collections::{HashMap, HashSet},
        net::IpAddr,
        path::PathBuf,
    },
};

pub fn packet_count(
    event_file_paths: &[PathBuf],
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
) -> std::io::Result<()> {
    let mut handler = PacketCounter::new(start_timestamp, end_timestamp);
    process_event_files(event_file_paths, &mut |event| handler.handle_event(event))?;
    handler.report();
    Ok(())
}

struct PacketCounter {
    start_timestamp: Option<DateTime<Utc>>,
    end_timestamp: Option<DateTime<Utc>>,
    started: bool,
    done: bool,

    packet_metrics: PacketMetrics,
}

#[derive(Default)]
struct PacketMetrics {
    total_count: usize,
    valid_count: usize,
    valid_unique_count: usize,

    tpu_count: usize,
    fwd_count: usize,

    staked_count: usize,
    staked_tpu_count: usize,
    staked_fwd_count: usize,

    tpu_unique_count: usize,
    fwd_unique_count: usize,

    tpu_staked_unique_count: usize,
    fwd_staked_unique_count: usize,

    total_ip_counts: HashMap<IpAddr, IpPacketCounts>,
    tpu_ip_counts: HashMap<IpAddr, IpPacketCounts>,
    fwd_ip_counts: HashMap<IpAddr, IpPacketCounts>,

    signature_set: HashSet<Signature>,
}

#[derive(Default)]
struct IpPacketCounts {
    total: usize,
    unique: usize,
    staked: usize,
}

impl PacketCounter {
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
            packet_metrics: PacketMetrics::default(),
        }
    }

    pub fn report(&self) {
        // destructure packet_metrics
        let PacketMetrics {
            total_count,
            valid_count,
            valid_unique_count,
            tpu_count,
            fwd_count,
            staked_count,
            staked_tpu_count,
            staked_fwd_count,
            tpu_unique_count,
            fwd_unique_count,
            tpu_staked_unique_count,
            fwd_staked_unique_count,
            total_ip_counts,
            tpu_ip_counts,
            fwd_ip_counts,
            signature_set: _,
        } = &self.packet_metrics;

        println!("Total packets: {}", total_count);
        println!("Valid packets: {}", valid_count);
        println!("Valid unique packets: {}", valid_unique_count);
        println!("TPU packets: {}", tpu_count);
        println!("FWD packets: {}", fwd_count);
        println!("Staked packets: {}", staked_count);
        println!("TPU staked packets: {}", staked_tpu_count);
        println!("FWD staked packets: {}", staked_fwd_count);
        println!("TPU unique packets: {}", tpu_unique_count);
        println!("FWD unique packets: {}", fwd_unique_count);
        println!("TPU staked unique packets: {}", tpu_staked_unique_count);
        println!("FWD staked unique packets: {}", fwd_staked_unique_count);
        println!("Unique IPs: {}", total_ip_counts.len());
        println!("TPU IPs: {}", tpu_ip_counts.len());
        println!("FWD IPs: {}", fwd_ip_counts.len());

        // Print the top 5 IP addresses for each category
        let print_top_ips = |ip_count: &HashMap<IpAddr, IpPacketCounts>| {
            let mut ip_counts: Vec<_> = ip_count.iter().collect();
            ip_counts.sort_by_key(|(_, ip_packet_counts)| ip_packet_counts.total);
            for (ip, count) in ip_counts
                .iter()
                .rev()
                .take(5)
                .map(|(ip, count)| (ip, count))
            {
                println!(
                    "  {}: total={} unique={} staked={}",
                    ip, count.total, count.unique, count.staked
                );
            }
        };

        println!("Top 5 IPs by total packets:");
        print_top_ips(total_ip_counts);
        println!("Top 5 IPs by TPU packets:");
        print_top_ips(tpu_ip_counts);
        println!("Top 5 IPs by FWD packets:");
        print_top_ips(fwd_ip_counts);
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
        _timestamp: DateTime<Utc>,
        label: ChannelLabel,
        packet_batches: BankingPacketBatch,
    ) {
        if matches!(label, ChannelLabel::NonVote) {
            for packet_batch in packet_batches.0.iter() {
                for packet in packet_batch {
                    // Ignore any packet that was filtered by sigverify
                    self.packet_metrics.total_count += 1;

                    let valid = !packet.meta().discard();
                    let staked = packet.meta().is_from_staked_node();
                    let forwarded = packet.meta().forwarded();

                    let unique = if let Some(data) = packet.data(..) {
                        let Some(versioned_transaction) =
                            bincode::deserialize::<VersionedTransaction>(data).ok()
                        else {
                            continue;
                        };
                        self.packet_metrics
                            .signature_set
                            .insert(versioned_transaction.signatures[0])
                    } else {
                        false
                    };

                    self.packet_metrics.valid_count += usize::from(valid && unique);
                    self.packet_metrics.valid_unique_count += usize::from(valid && unique);

                    self.packet_metrics.tpu_count += usize::from(valid && !forwarded);
                    self.packet_metrics.fwd_count += usize::from(valid && forwarded);

                    self.packet_metrics.staked_count += usize::from(valid && staked);
                    self.packet_metrics.staked_tpu_count +=
                        usize::from(valid && staked && !forwarded);
                    self.packet_metrics.staked_fwd_count +=
                        usize::from(valid && staked && forwarded);

                    self.packet_metrics.tpu_unique_count +=
                        usize::from(valid && !forwarded && unique);
                    self.packet_metrics.fwd_unique_count +=
                        usize::from(valid && forwarded && unique);

                    self.packet_metrics.tpu_staked_unique_count +=
                        usize::from(valid && !forwarded && staked && unique);
                    self.packet_metrics.fwd_staked_unique_count +=
                        usize::from(valid && forwarded && staked && unique);

                    let update_ip_counts =
                        |ip_counts: &mut HashMap<IpAddr, IpPacketCounts>,
                         ip: IpAddr,
                         unique: bool,
                         staked: bool| {
                            let ip_packet_counts = ip_counts.entry(ip).or_default();
                            ip_packet_counts.total += 1;
                            ip_packet_counts.unique += usize::from(valid && unique);
                            ip_packet_counts.staked += usize::from(valid && staked);
                        };

                    update_ip_counts(
                        &mut self.packet_metrics.total_ip_counts,
                        packet.meta().addr,
                        unique,
                        staked,
                    );
                    if !forwarded {
                        update_ip_counts(
                            &mut self.packet_metrics.tpu_ip_counts,
                            packet.meta().addr,
                            unique,
                            staked,
                        );
                    } else {
                        update_ip_counts(
                            &mut self.packet_metrics.fwd_ip_counts,
                            packet.meta().addr,
                            unique,
                            staked,
                        );
                    }
                }
            }
        }
    }

    fn handle_block_and_bank_hash(&mut self, timestamp: DateTime<Utc>, slot: Slot) {
        println!("{timestamp:?} - {slot:?}");
    }
}
