//! What is handed to the window after a tool has answered: one line after another, in the
//! order the revisions were kept, none of them waited for (ADR 0467).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// How often a line that must wait for the queue looks again.
const DRAIN_POLL: Duration = Duration::from_millis(20);

type Handing = Pin<Box<dyn Future<Output = Option<Said>> + Send>>;

/// What the window said of one revision handed to it aside.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub job: String,
    pub revision: u32,
    pub answer: Value,
}

/// The hand-overs still to be taken, run one at a time, and what the window said of the last.
#[derive(Debug, Default)]
pub struct Aside {
    queue: OnceLock<UnboundedSender<Handing>>,
    waiting: Arc<AtomicUsize>,
    said: Arc<Mutex<Option<Said>>>,
}

impl Aside {
    /// Runs `handing` once every one queued before it has run, the answer its own.
    pub fn queue(&self, handing: impl Future<Output = Option<Said>> + Send + 'static) {
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let sender = self.queue.get_or_init(|| {
            let (sender, mut queued) = unbounded_channel::<Handing>();
            let (waiting, said) = (Arc::clone(&self.waiting), Arc::clone(&self.said));
            tokio::spawn(async move {
                while let Some(handing) = queued.recv().await {
                    let answer = handing.await;
                    if let (Some(answer), Ok(mut last)) = (answer, said.lock()) {
                        *last = Some(answer);
                    }
                    waiting.fetch_sub(1, Ordering::SeqCst);
                }
            });
            sender
        });
        if sender.send(Box::pin(handing)).is_err() {
            self.waiting.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Whether a hand-over is still queued or being taken.
    pub fn busy(&self) -> bool {
        self.waiting.load(Ordering::SeqCst) > 0
    }

    /// Once every hand-over queued has run, for a line that must come after them.
    pub async fn drained(&self) {
        while self.busy() {
            tokio::time::sleep(DRAIN_POLL).await;
        }
    }

    /// What the window said of `job`'s last revision handed aside where it did not show it,
    /// taken so it is told once.
    pub fn untold(&self, job: &str) -> Option<Said> {
        let mut said = self.said.lock().ok()?;
        let shown = |said: &Said| said.answer["shown"] == true || said.answer["applied"] == true;
        if said
            .as_ref()
            .is_some_and(|said| said.job == job && !shown(said))
        {
            return said.take();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn said(revision: u32, shown: bool) -> Said {
        Said {
            job: "cube-1".into(),
            revision,
            answer: json!({ "shown": shown }),
        }
    }

    #[tokio::test]
    async fn hand_overs_run_in_the_order_queued_and_none_is_waited_for() {
        let aside = Aside::default();
        let order = Arc::new(Mutex::new(Vec::new()));
        for (revision, pause_ms) in [(1, 40), (2, 0)] {
            let order = Arc::clone(&order);
            aside.queue(async move {
                tokio::time::sleep(Duration::from_millis(pause_ms)).await;
                order.lock().expect("not poisoned").push(revision);
                None
            });
        }
        assert!(aside.busy(), "queued, not yet run");
        while aside.busy() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(*order.lock().expect("not poisoned"), [1, 2]);
    }

    #[tokio::test]
    async fn a_revision_the_window_did_not_show_is_told_once() {
        let aside = Aside::default();
        aside.queue(async { Some(said(3, false)) });
        while aside.busy() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(aside.untold("other-job"), None);
        assert_eq!(aside.untold("cube-1"), Some(said(3, false)));
        assert_eq!(aside.untold("cube-1"), None, "told once");
    }

    #[tokio::test]
    async fn a_revision_shown_is_not_told() {
        let aside = Aside::default();
        aside.queue(async { Some(said(4, true)) });
        while aside.busy() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(aside.untold("cube-1"), None);
    }
}
