//! The browser's run, natively: time and peak live bytes of `slice_project` on a project.
//!
//! `cargo run --release -p web-engine --example measure -- plate.encrust`, with
//! `RAYON_NUM_THREADS=1` for the single thread the browser has. Peak memory is counted by
//! the allocator, so it is what the run holds and not what the process was given; see
//! `docs/design/web-build.md`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use web_engine::slice_project;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let grown = unsafe { System.realloc(pointer, layout, new_size) };
        if !grown.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            record(new_size);
        }
        grown
    }
}

fn record(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: measure <project.encrust>")?;
    let threads = rayon_threads();

    let project = std::fs::read(&path)?;
    let created_unix_s = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let started = Instant::now();
    let sliced = slice_project(&project, threads, created_unix_s)?;
    let seconds = started.elapsed().as_secs_f64();

    println!(
        "{{\"seconds\":{seconds:.2},\"peak_live_mb\":{:.0},\"output_mb\":{:.1},\"extension\":\"{}\",\"threads\":{threads}}}",
        PEAK.load(Ordering::Relaxed) as f64 / 1e6,
        sliced.bytes.len() as f64 / 1e6,
        sliced.extension,
    );
    Ok(())
}

/// The threads rayon runs on, which is how many layers are drawn at once.
fn rayon_threads() -> usize {
    std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|text| text.parse().ok())
        .filter(|&threads| threads > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        })
}
