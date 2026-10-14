//! How long the card spends on the viewport's scene pass, from timestamps it writes around
//! the pass and that are read back frames later, so no frame waits on them.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Frames whose times can be on their way back at once; a frame finding none free goes
/// untimed.
const SLOTS: usize = 4;
/// Bytes from one slot's pair of timestamps to the next in the resolved buffer.
const STRIDE: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const PAIR: u64 = 2 * wgpu::QUERY_SIZE as u64;

const WAITING: u8 = 0;
const READABLE: u8 = 1;
const FAILED: u8 = 2;

/// The timestamps of the last few scene passes, each in its own slot until it is read.
pub(super) struct CardTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    slots: Vec<Slot>,
    /// The slot the scene pass under way writes, between `begin` and `end`.
    current: Option<usize>,
}

struct Slot {
    read: wgpu::Buffer,
    state: State,
    /// The models' faces the timed pass drew, where the frame counted them.
    faces: Option<usize>,
    mapped: Arc<AtomicU8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Free,
    Recorded,
    Mapping,
}

impl CardTimer {
    /// `None` on a device that cannot time a pass, a WebGL canvas among them.
    pub(super) fn new(device: &wgpu::Device) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("viewport_card_time"),
            ty: wgpu::QueryType::Timestamp,
            count: 2 * SLOTS as u32,
        });
        let resolved = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport_card_time_resolved"),
            size: STRIDE * SLOTS as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..SLOTS)
            .map(|_| Slot {
                read: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_card_time_read"),
                    size: PAIR,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                state: State::Free,
                faces: None,
                mapped: Arc::default(),
            })
            .collect();
        Some(Self {
            queries,
            resolved,
            slots,
            current: None,
        })
    }

    /// Where the scene pass about to begin writes its timestamps, or `None` while every
    /// slot still waits on the card.
    pub(super) fn begin(&mut self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == State::Free)?;
        self.current = Some(index);
        let first = 2 * index as u32;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        })
    }

    /// Copies out the timestamps the scene pass just wrote, to be read once the frame is
    /// sent. `faces` is what the pass drew of the models.
    pub(super) fn end(&mut self, encoder: &mut wgpu::CommandEncoder, faces: Option<usize>) {
        let Some(index) = self.current.take() else {
            return;
        };
        let first = 2 * index as u32;
        let offset = STRIDE * index as u64;
        encoder.resolve_query_set(&self.queries, first..first + 2, &self.resolved, offset);
        let slot = &mut self.slots[index];
        encoder.copy_buffer_to_buffer(&self.resolved, offset, &slot.read, 0, PAIR);
        slot.state = State::Recorded;
        slot.faces = faces;
    }

    /// What the card took over the passes it has finished since the last call, seconds,
    /// with the models' faces each drew. Every pass recorded before this frame has been
    /// sent by now, so its read can begin.
    pub(super) fn collect(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<(f32, Option<usize>)> {
        for slot in &mut self.slots {
            if slot.state == State::Recorded {
                let mapped = Arc::clone(&slot.mapped);
                mapped.store(WAITING, Ordering::Release);
                slot.read.map_async(wgpu::MapMode::Read, .., move |result| {
                    let state = if result.is_ok() { READABLE } else { FAILED };
                    mapped.store(state, Ordering::Release);
                });
                slot.state = State::Mapping;
            }
        }
        // A poll that does not wait has no timeout to report.
        let _ = device.poll(wgpu::PollType::Poll);
        let tick_s = queue.get_timestamp_period() * 1e-9;
        let mut times = Vec::new();
        for slot in &mut self.slots {
            if slot.state != State::Mapping {
                continue;
            }
            match slot.mapped.load(Ordering::Acquire) {
                WAITING => continue,
                READABLE => times.extend(read(slot, tick_s)),
                _ => {}
            }
            slot.state = State::Free;
        }
        times
    }
}

/// The time between the two timestamps in `slot`'s mapped buffer. A pass whose frame was
/// never sent leaves them unwritten, and comes to nothing.
fn read(slot: &Slot, tick_s: f32) -> Option<(f32, Option<usize>)> {
    let ticks = match slot.read.get_mapped_range(..) {
        Ok(bytes) => Some(bytemuck::pod_read_unaligned::<[u64; 2]>(&bytes)),
        Err(error) => {
            tracing::debug!(target: "frame_time", %error, "the card's time could not be read");
            None
        }
    };
    slot.read.unmap();
    let [began, ended] = ticks?;
    (ended > began).then(|| ((ended - began) as f32 * tick_s, slot.faces))
}
