use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use printer_link::{SendError, Wire};

/// What a job does to the printer it is pointed at.
pub enum Action {
    /// Send a file, deleting it afterwards when the window wrote it only to send it.
    Upload { path: PathBuf, temporary: bool },
    /// Start a file already on the printer.
    StartPrint { filename: String },
}

/// One errand to run against one printer, off the window's thread.
pub struct SendRequest {
    /// What the printer is called in front of the user.
    pub name: String,
    pub wire: Wire,
    pub action: Action,
}

/// One message from the errand to the window.
#[derive(Debug, Clone, PartialEq)]
pub enum SendProgress {
    Uploading { sent_bytes: u64, total_bytes: u64 },
    Finished(SendOutcome),
}

/// How an errand ended.
#[derive(Debug, Clone, PartialEq)]
pub enum SendOutcome {
    /// The file is on the printer under this name, and can be started.
    Sent {
        printer: String,
        filename: String,
    },
    Printing {
        printer: String,
        filename: String,
    },
    Cancelled,
    /// The error chain flattened into one line, as the window has nowhere to print it.
    Failed(String),
}

/// A transfer or a print command running on its own thread.
pub struct SendJob {
    progress: Receiver<SendProgress>,
    cancel: Arc<AtomicBool>,
    printer: String,
    sent_bytes: u64,
    total_bytes: u64,
    starting: bool,
    counts_bytes: bool,
}

impl SendJob {
    /// Starts the errand. The thread is detached and dropping the handle cancels it.
    pub fn spawn(request: SendRequest) -> Self {
        let (sender, progress) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let printer = request.name.clone();
        let starting = matches!(request.action, Action::StartPrint { .. });
        let counts_bytes = request.wire.counts_bytes();

        crate::job::spawn(move || {
            let outcome = run(&request, &worker_cancel, &sender);
            let _ = sender.send(SendProgress::Finished(outcome));
        });

        Self {
            progress,
            cancel,
            printer,
            sent_bytes: 0,
            total_bytes: 0,
            starting,
            counts_bytes,
        }
    }

    /// Takes everything reported since the last frame, and the outcome once it ends.
    pub fn poll(&mut self) -> Option<SendOutcome> {
        loop {
            match self.progress.try_recv() {
                Ok(SendProgress::Uploading {
                    sent_bytes,
                    total_bytes,
                }) => {
                    self.sent_bytes = sent_bytes;
                    self.total_bytes = total_bytes;
                }
                Ok(SendProgress::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(SendOutcome::Failed(
                        "the network thread stopped without finishing".to_owned(),
                    ));
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Share of the file that has reached the printer, or `None` before the first packet
    /// and on a protocol that does not count them.
    pub fn fraction(&self) -> Option<f32> {
        (self.total_bytes > 0).then(|| self.sent_bytes as f32 / self.total_bytes as f32)
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return "Cancelling".to_owned();
        }
        if self.starting {
            return format!("Starting {}", self.printer);
        }
        // Without a byte count there is nothing to tell connecting from sending, so the
        // label says the thing that is true of both.
        match self.fraction().is_some() || !self.counts_bytes {
            true => format!("Sending to {}", self.printer),
            false => format!("Connecting to {}", self.printer),
        }
    }
}

/// A job whose handle is gone has nobody left to report to.
impl Drop for SendJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run(request: &SendRequest, cancel: &AtomicBool, sender: &Sender<SendProgress>) -> SendOutcome {
    let printer = request.name.clone();
    let cancelled = || cancel.load(Ordering::Relaxed);
    let finished = match &request.action {
        Action::Upload { path, temporary } => {
            let mut progress = |transfer: printer_link::Transfer| {
                let _ = sender.send(SendProgress::Uploading {
                    sent_bytes: transfer.sent_bytes,
                    total_bytes: transfer.total_bytes,
                });
            };
            let sent = printer_link::upload(&request.wire, path, &mut progress, &cancelled);
            if *temporary {
                let _ = std::fs::remove_file(path);
            }
            sent.map(|filename| SendOutcome::Sent { printer, filename })
        }
        Action::StartPrint { filename } => {
            printer_link::start_print(&request.wire, filename).map(|()| SendOutcome::Printing {
                printer,
                filename: filename.clone(),
            })
        }
    };
    finished.unwrap_or_else(|error| failed(&error))
}

/// The window has one line to print a failure in, so the error chain is flattened.
fn failed(error: &SendError) -> SendOutcome {
    match error.is_cancelled() {
        true => SendOutcome::Cancelled,
        false => SendOutcome::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_prusalink::Link;
    use net_sdcp::{Printer, Transport};
    use std::net::IpAddr;

    fn a_printer() -> Printer {
        Printer {
            name: "Saturn".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([127, 0, 0, 1]),
            mainboard_id: "ff".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport: Transport::WebSocket,
        }
    }

    /// A handle with no worker behind it, so the counters can be driven by hand.
    fn idle_job(progress: Receiver<SendProgress>, starting: bool) -> SendJob {
        SendJob {
            progress,
            cancel: Arc::new(AtomicBool::new(false)),
            printer: a_printer().name,
            sent_bytes: 0,
            total_bytes: 0,
            starting,
            counts_bytes: true,
        }
    }

    #[test]
    fn a_transfer_without_a_packet_yet_has_no_fraction() {
        let (_sender, progress) = mpsc::channel();
        let job = idle_job(progress, false);
        assert_eq!(job.fraction(), None);
        assert_eq!(job.label(), "Connecting to Saturn");
    }

    #[test]
    fn the_fraction_follows_the_bytes_that_land() {
        let (sender, progress) = mpsc::channel();
        let mut job = idle_job(progress, false);
        sender
            .send(SendProgress::Uploading {
                sent_bytes: 1024 * 1024,
                total_bytes: 4 * 1024 * 1024,
            })
            .expect("the job holds the receiver");
        assert_eq!(job.poll(), None, "an unfinished job has no outcome");
        assert_eq!(job.fraction(), Some(0.25));
        assert_eq!(job.label(), "Sending to Saturn");
    }

    #[test]
    fn a_protocol_that_counts_nothing_still_says_it_is_sending() {
        let (_sender, progress) = mpsc::channel();
        let mut job = idle_job(progress, false);
        job.counts_bytes = false;
        assert_eq!(job.fraction(), None);
        assert_eq!(job.label(), "Sending to Saturn");
    }

    #[test]
    fn starting_a_print_says_so_rather_than_counting_bytes() {
        let (_sender, progress) = mpsc::channel();
        assert_eq!(idle_job(progress, true).label(), "Starting Saturn");
    }

    #[test]
    fn cancelling_shows_in_the_label() {
        let (_sender, progress) = mpsc::channel();
        let job = idle_job(progress, false);
        job.cancel();
        assert!(job.is_cancelling());
        assert_eq!(job.label(), "Cancelling");
    }

    #[test]
    fn a_worker_that_dies_without_an_outcome_is_a_failure() {
        let (sender, progress) = mpsc::channel();
        let mut job = idle_job(progress, false);
        assert_eq!(job.poll(), None, "an empty channel means still running");
        drop(sender);
        let Some(SendOutcome::Failed(message)) = job.poll() else {
            panic!("a dead worker must end the job");
        };
        assert!(message.contains("stopped"), "got {message}");
    }

    #[test]
    fn a_temporary_file_is_deleted_even_when_the_transfer_fails() {
        let path = std::env::temp_dir().join("encrust-send-temp.goo");
        std::fs::write(&path, b"not a real stack").expect("the temporary directory is writable");
        let (sender, _progress) = mpsc::channel();
        let request = SendRequest {
            name: a_printer().name,
            wire: Wire::Sdcp(a_printer()),
            action: Action::Upload {
                path: path.clone(),
                temporary: true,
            },
        };
        let outcome = run(&request, &AtomicBool::new(false), &sender);
        assert!(matches!(outcome, SendOutcome::Failed(_)), "nothing listens");
        assert!(
            !path.exists(),
            "the window's own file does not outlive the job"
        );
    }

    #[test]
    fn a_prusa_machine_that_is_not_there_fails_rather_than_hangs() {
        let path = std::env::temp_dir().join("encrust-send-prusa.sl1");
        std::fs::write(&path, b"not a real archive").expect("the temporary directory is writable");
        let (sender, _progress) = mpsc::channel();
        let request = SendRequest {
            name: "sl1.local".to_owned(),
            wire: Wire::Prusa(Link::api_key("127.0.0.1:1", "key")),
            action: Action::Upload {
                path: path.clone(),
                temporary: true,
            },
        };
        let outcome = run(&request, &AtomicBool::new(false), &sender);
        assert!(matches!(outcome, SendOutcome::Failed(_)), "nothing listens");
        assert!(!path.exists());
    }
}
