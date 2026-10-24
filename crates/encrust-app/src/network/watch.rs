use super::{Errand, Picture};
use super::reach::Trouble;
use super::{Network, RESCAN_S, WATCH_PERIOD};
use printer_link::{State, Wire};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

/// The last thing a watched printer said, or why it said nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    State(State),
    Failed(Trouble),
}

/// One printer being watched on a thread of its own, and what it said last.
pub(super) struct Watch {
    pub(super) wire: Wire,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) reports: Receiver<Reading>,
    pub(super) last: Option<Reading>,
}

impl Watch {
    pub(super) fn spawn(wire: Wire) -> Self {
        let (sender, reports) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, watched) = (Arc::clone(&stop), wire.clone());
        crate::job::spawn(move || {
            let stopped = || flag.load(Ordering::Relaxed);
            printer_link::watch(&watched, WATCH_PERIOD, &stopped, &mut |heard| {
                let reading = match heard {
                    Ok(state) => Reading::State(state),
                    Err(error) => Reading::Failed(Trouble::of(error)),
                };
                // The window let go of this watch, so nobody is listening any more.
                if sender.send(reading).is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
            });
        });
        Self {
            wire,
            stop,
            reports,
            last: None,
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Network {
    /// Sends `errand` to the printer under `key`, on a thread of its own.
    pub fn ask(&mut self, key: &str, errand: Errand) {
        let Some(target) = self.targets().into_iter().find(|t| t.key() == key) else {
            return;
        };
        let state = match self.reading(&target) {
            Some(Reading::State(state)) => state.clone(),
            _ => State::default(),
        };
        let wire = target.wire();
        let extras = self.extras.entry(key.to_owned()).or_default();
        match &errand {
            Errand::Picture { id, .. } => {
                extras.pictures.insert(id.clone(), Picture::Asked);
            }
            Errand::History => {
                extras.history_asked = true;
                extras.reading_history = true;
            }
            Errand::Control(_) | Errand::Camera(_) => {}
        }
        let out = super::errands::Out::send(key.to_owned(), wire, state, errand);
        if let Some(doing) = out.doing() {
            extras.doing = Some(doing);
            extras.trouble = None;
        }
        self.out.push(out);
    }

    /// Puts each picture that has come in onto a texture the cards can draw.
    pub fn show_pictures(&mut self, ctx: &egui::Context) {
        for (key, extras) in &mut self.extras {
            for (id, picture) in &mut extras.pictures {
                if let Picture::Decoded(image) = picture {
                    let name = format!("printer-picture-{key}-{id}");
                    let texture =
                        ctx.load_texture(name, image.clone(), egui::TextureOptions::LINEAR);
                    *picture = Picture::Shown(texture);
                }
            }
        }
    }

    /// The pictures the cards would draw and nobody has asked for yet: the print under way
    /// on each printer, and each print in a history that has been read.
    pub(super) fn wanted_pictures(&self) -> Vec<(String, Errand)> {
        let mut wanted = Vec::new();
        for target in self.targets() {
            let key = target.key();
            let extras = self.extras.get(&key);
            let known = |id: &str| extras.is_some_and(|extras| extras.pictures.contains_key(id));
            if let Some(Reading::State(state)) = self.reading(&target)
                && let Some(id) = state.job_id.as_deref()
                && !known(id)
            {
                let id = id.to_owned();
                wanted.push((key.clone(), Errand::Picture { id, task: None }));
            }
            let tasks = extras
                .and_then(|extras| extras.history.as_deref())
                .unwrap_or_default();
            for task in tasks {
                if !task.thumbnail.is_empty() && !known(&task.task_id) {
                    let id = task.task_id.clone();
                    let task = Some(task.clone());
                    wanted.push((key.clone(), Errand::Picture { id, task }));
                }
            }
        }
        wanted
    }

    /// Whether any printer is being watched, which is what keeps its card repainting.
    pub fn is_watching(&self) -> bool {
        !self.watches.is_empty()
    }

    /// What has been asked of a printer, open for a test to fill in as if it had answered.
    #[cfg(test)]
    pub(crate) fn extras_mut(&mut self, key: &str) -> &mut super::Extras {
        self.extras.entry(key.to_owned()).or_default()
    }

    /// What a watch hears, put straight into the window as if its thread had said it.
    #[cfg(test)]
    pub(crate) fn hear(&mut self, key: &str, reading: Reading) {
        let (_, reports) = mpsc::channel();
        let wire = self
            .targets()
            .into_iter()
            .find(|target| target.key() == key)
            .expect("the printer heard is listed")
            .wire();
        let watch = Watch {
            wire,
            stop: Arc::new(AtomicBool::new(true)),
            reports,
            last: Some(reading),
        };
        self.watches.insert(key.to_owned(), watch);
    }

    /// Watches every printer that answers, once each, and lets go of the ones gone or
    /// moved: silent at the last scan, and silent to their own watch too.
    pub(super) fn watch_all(&mut self) {
        let wanted: BTreeMap<String, Wire> = self
            .targets()
            .iter()
            .filter(|target| {
                self.seen.contains(&target.key())
                    || matches!(self.reading(target), Some(Reading::State(_)))
            })
            .map(|target| (target.key(), target.wire()))
            .collect();
        self.watches
            .retain(|key, watch| wanted.get(key) == Some(&watch.wire));
        self.extras.retain(|key, _| wanted.contains_key(key));
        for (key, wire) in wanted {
            self.watches
                .entry(key)
                .or_insert_with(|| Watch::spawn(wire));
        }
    }

    /// Asks the network again when a list of printers is in view and the last time the
    /// window asked on its own was a while ago; `now_s` is the window's clock.
    pub fn keep_fresh(&mut self, now_s: f64) {
        let due = self.asked_at.is_none_or(|at| now_s - at >= RESCAN_S);
        if due && !self.hold && !self.is_scanning() {
            self.asked_at = Some(now_s);
            self.scan();
        }
    }
}
