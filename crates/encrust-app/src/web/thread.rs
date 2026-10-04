//! Threads in a browser: each is a worker of its own, running this module on its shared
//! memory, and the page's thread hands it a task over a channel. See
//! `docs/design/web-build.md`.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, Once, OnceLock, PoisonError};

use wasm_bindgen::prelude::*;

type Task = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// Tasks waiting for the worker started to run them. Each worker takes one, whichever it
/// finds: they are all alike until one is taken.
struct Queue {
    sender: Sender<Task>,
    receiver: Mutex<Receiver<Task>>,
}

static QUEUE: OnceLock<Queue> = OnceLock::new();

/// Where this module's JavaScript bindings are, which every worker imports to start.
static BINDINGS: OnceLock<String> = OnceLock::new();

#[wasm_bindgen(module = "/src/web/thread.js")]
extern "C" {
    #[wasm_bindgen(js_name = startThread)]
    fn start_thread(bindings: &str, module: JsValue, memory: JsValue);
}

/// Readies threads, on the page's thread and before anything is spawned: `bindings` is the
/// URL of the module's own script.
///
/// The page's thread joins a pool of its own, of one thread, so that a parallel loop it
/// runs runs there, in order, instead of waiting for workers — which it may never do.
pub fn init(bindings: String) {
    let _ = BINDINGS.set(bindings);
    let (sender, receiver) = mpsc::channel();
    let _ = QUEUE.set(Queue {
        sender,
        receiver: Mutex::new(receiver),
    });
    if let Ok(pool) = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .use_current_thread()
        .build()
    {
        // The page's thread stays a member for as long as the page is open.
        std::mem::forget(pool);
    }
}

/// Runs `work` on a thread of its own.
pub fn spawn(work: impl FnOnce() + Send + 'static) {
    spawn_async(move || async move { work() });
}

/// Runs `work` on a thread of its own, where it may also wait on the browser: what a
/// browser gives a worker, such as a file in its private storage, it gives by a promise.
pub fn spawn_async<F, Fut>(work: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + 'static,
{
    start(Box::new(move || {
        global_pool();
        Box::pin(work())
    }));
}

/// The cores the machine has, as the browser reports them.
pub fn cores() -> usize {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &"navigator".into())
        .and_then(|navigator| js_sys::Reflect::get(&navigator, &"hardwareConcurrency".into()))
        .ok()
        .and_then(|cores| cores.as_f64())
        .map_or(1, |cores| (cores as usize).max(1))
}

/// Builds rayon's global pool on the first worker that asks. Building it waits for its
/// threads to start, which only a worker may do; until it is built, a parallel loop on a
/// worker would make one of its own on the spot, on that worker alone.
fn global_pool() {
    static BUILT: Once = Once::new();
    BUILT.call_once(|| {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(crate::job::worker_threads())
            .spawn_handler(|thread| {
                start(Box::new(move || {
                    thread.run();
                    Box::pin(async {})
                }));
                Ok(())
            })
            .build_global();
    });
}

fn start(task: Task) {
    let (Some(queue), Some(bindings)) = (QUEUE.get(), BINDINGS.get()) else {
        return;
    };
    if queue.sender.send(task).is_ok() {
        start_thread(bindings, wasm_bindgen::module(), wasm_bindgen::memory());
    }
}

/// What a new worker calls once the module is up on it: takes a task and runs it.
#[wasm_bindgen(js_name = encrustRunThread)]
pub async fn run_thread() {
    let Some(queue) = QUEUE.get() else {
        return;
    };
    let task = queue
        .receiver
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .recv();
    if let Ok(task) = task {
        task().await;
    }
}
