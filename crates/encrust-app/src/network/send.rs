use super::Network;
use super::target::Sent;
use crate::i18n::tr;
use crate::job::{Action, SendJob, SendOutcome, SendRequest};
use crate::status::Status;
use std::path::{Path, PathBuf};

/// Where a stack goes when it is written only to be sent.
pub fn temporary_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// A file the window wrote for itself is deleted once it has been sent; one the user
/// named is theirs and stays.
pub(super) fn is_temporary(path: &Path) -> bool {
    path.starts_with(std::env::temp_dir())
}

impl Network {
    /// Sends a file the window has just written, and deletes it after when it was
    /// written only to be sent.
    pub fn send(&mut self, path: PathBuf, status: &mut Status) {
        let Some(target) = self.target() else {
            return;
        };
        let (name, wire) = (self.label(&target), target.wire());
        let temporary = is_temporary(&path);
        *status = Status::Info(tr!("network-sending", name = name.clone()));
        self.sent = None;
        self.job = Some(SendJob::spawn(SendRequest {
            name,
            wire,
            action: Action::Upload { path, temporary },
        }));
    }

    /// Starts the file that was sent last, which is the one the user just watched land.
    pub fn start_print(&mut self, status: &mut Status) {
        let (Some(sent), Some(target)) = (self.sent.clone(), self.target()) else {
            return;
        };
        let (name, wire) = (self.label(&target), target.wire());
        *status = Status::Info(tr!(
            "network-starting",
            file = sent.filename.clone(),
            name = name.clone()
        ));
        self.job = Some(SendJob::spawn(SendRequest {
            name,
            wire,
            action: Action::StartPrint {
                filename: sent.filename,
            },
        }));
    }

    /// Drains the scan and the errand into the stage notice. Returns whether either is
    /// still running, which is what tells the window to keep repainting.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        for watch in self.watches.values_mut() {
            while let Ok(reading) = watch.reports.try_recv() {
                watch.last = Some(reading);
            }
        }
        let extras = &mut self.extras;
        self.out
            .retain(|out| !out.take(extras.entry(out.key.clone()).or_default()));
        if !self.hold {
            for (key, errand) in self.wanted_pictures() {
                self.ask(&key, errand);
            }
        }
        let errands = !self.out.is_empty();
        self.take_scan() | self.take_job(status) | errands
    }

    pub(super) fn take_job(&mut self, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        self.job = None;
        match outcome {
            SendOutcome::Sent { printer, filename } => {
                *status = Status::Info(tr!(
                    "network-sent",
                    file = filename.clone(),
                    printer = printer.clone()
                ));
                self.sent = Some(Sent { printer, filename });
            }
            SendOutcome::Printing { printer, filename } => {
                *status = Status::Info(tr!(
                    "network-printing",
                    printer = printer.clone(),
                    file = filename.clone()
                ));
                self.sent = None;
            }
            SendOutcome::Cancelled => *status = Status::Info(tr!("network-cancelled")),
            SendOutcome::Failed(message) => {
                *status = Status::failed(&anyhow::Error::msg(message));
            }
        }
        false
    }
}
