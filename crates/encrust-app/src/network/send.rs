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
pub fn is_temporary(path: &Path) -> bool {
    path.starts_with(std::env::temp_dir())
}

impl Network {
    /// Sends a file to the bound printer, and deletes it after when `temporary`: when the
    /// window wrote it only to be sent.
    pub fn send(&mut self, path: PathBuf, temporary: bool, status: &mut Status) {
        let Some(target) = self.target() else {
            return;
        };
        let (name, wire) = (self.label(&target), target.wire());
        *status = Status::Info(tr!("network-sending", name = name.clone()));
        self.sent = None;
        self.job = Some(SendJob::spawn(SendRequest {
            name,
            wire,
            action: Action::Upload { path, temporary },
        }));
    }

    /// The bound printer a sliced file picked from disk goes to, by name, and why it cannot
    /// take one now; `None` while the profile in hand sends to no printer over `connection`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn offer_picked(
        &self,
        connection: printer_profiles::Connection,
    ) -> Option<(String, Option<String>)> {
        if connection == printer_profiles::Connection::None {
            return None;
        }
        let listed = self.chosen_machine()?;
        let answering = self
            .resolve(listed)
            .filter(|target| self.reach(target) == super::Reach::Answered);
        let Some(target) = answering else {
            let name = listed.label().to_owned();
            return Some((
                name.clone(),
                Some(tr!("network-not-answering", name = name)),
            ));
        };
        let name = self.label(&target);
        let refused = if !target.speaks(connection) {
            Some(tr!(
                "slice-speaks-other",
                name = name.clone(),
                theirs = target.protocol().label(),
                ours = super::connection_label(connection)
            ))
        } else if self.job.is_some() {
            Some(tr!("network-sending", name = name.clone()))
        } else {
            self.busy()
        };
        Some((name, refused))
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

#[cfg(test)]
mod tests;
