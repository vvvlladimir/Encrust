//! What the window asks one printer for besides its state: to pause, resume or stop, the
//! prints it remembers, pictures of them, and its camera. Each runs on a thread of its
//! own and answers once; see `docs/decisions/0263`.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver};

use printer_link::{Command, State, Task, Wire};

use super::Trouble;

/// How many remembered prints are listed under a printer.
const HISTORY_COUNT: usize = 8;

/// One thing asked of one printer.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    Control(Command),
    History,
    /// The picture of a remembered print, or of the one under way when `task` is `None`;
    /// `id` is what it is kept under.
    Picture {
        id: String,
        task: Option<Task>,
    },
    Camera(bool),
}

impl Errand {
    /// What the card says while it runs.
    fn doing(&self) -> &'static str {
        match self {
            Self::Control(Command::Pause) => "Pausing",
            Self::Control(Command::Resume) => "Resuming",
            Self::Control(Command::Stop) => "Stopping",
            Self::History => "Reading its prints",
            Self::Picture { .. } => "Fetching a picture",
            Self::Camera(true) => "Turning the camera on",
            Self::Camera(false) => "Turning the camera off",
        }
    }
}

/// What an errand came back with.
enum Heard {
    Done,
    History(Vec<Task>),
    Picture(Option<egui::ColorImage>),
    Camera(Option<String>),
}

/// A picture of a print, from asked for to on the card.
pub enum Picture {
    Asked,
    /// The printer keeps none, or it could not be read.
    Missing,
    Decoded(egui::ColorImage),
    Shown(egui::TextureHandle),
}

impl Picture {
    /// The texture to draw, once it is on the card.
    pub fn texture(&self) -> Option<&egui::TextureHandle> {
        match self {
            Self::Shown(texture) => Some(texture),
            Self::Asked | Self::Missing | Self::Decoded(_) => None,
        }
    }
}

/// Everything asked of one printer so far, and what is still out.
#[derive(Default)]
pub struct Extras {
    /// What the card's buttons wait on, while it runs.
    pub doing: Option<&'static str>,
    /// Why the last errand failed, until another is sent.
    pub trouble: Option<Trouble>,
    pub history: Option<Vec<Task>>,
    /// Whether the history has been asked for, so a failed list is not asked again unbidden.
    pub history_asked: bool,
    pub pictures: BTreeMap<String, Picture>,
    /// Where the camera streams to while it is on.
    pub camera: Option<String>,
}

/// An errand out on its thread, and the printer it was sent to.
pub(super) struct Out {
    pub(super) key: String,
    errand: Errand,
    answer: Receiver<Result<Heard, Trouble>>,
}

impl Out {
    /// Sends `errand` to the printer at the end of `wire`. `state` is what it said last,
    /// which names the print under way.
    pub(super) fn send(key: String, wire: Wire, state: State, errand: Errand) -> Self {
        let (sender, answer) = mpsc::channel();
        let asked = errand.clone();
        crate::job::spawn(move || {
            let _ = sender.send(run(&wire, &state, asked).map_err(Trouble::of));
        });
        Self {
            key,
            errand,
            answer,
        }
    }

    /// Whether the errand keeps the card's buttons waiting.
    pub(super) fn holds_the_buttons(&self) -> bool {
        !matches!(self.errand, Errand::Picture { .. })
    }

    pub(super) fn doing(&self) -> &'static str {
        self.errand.doing()
    }

    /// Files the answer into `extras` once it is in. Answers whether it was.
    pub(super) fn take(&self, extras: &mut Extras) -> bool {
        let Ok(answer) = self.answer.try_recv() else {
            return false;
        };
        if self.holds_the_buttons() {
            extras.doing = None;
        }
        match (answer, &self.errand) {
            (Ok(Heard::Done), _) => extras.trouble = None,
            (Ok(Heard::History(tasks)), _) => extras.history = Some(tasks),
            (Ok(Heard::Picture(image)), Errand::Picture { id, .. }) => {
                let picture = image.map_or(Picture::Missing, Picture::Decoded);
                extras.pictures.insert(id.clone(), picture);
            }
            (Ok(Heard::Camera(url)), _) => extras.camera = url,
            (Ok(Heard::Picture(_)), _) => {}
            (Err(trouble), Errand::Picture { id, .. }) => {
                tracing::debug!(%id, message = %trouble.message, "a picture could not be read");
                extras.pictures.insert(id.clone(), Picture::Missing);
            }
            (Err(trouble), _) => extras.trouble = Some(trouble),
        }
        true
    }
}

fn run(wire: &Wire, state: &State, errand: Errand) -> Result<Heard, printer_link::SendError> {
    Ok(match errand {
        Errand::Control(command) => {
            printer_link::control(wire, command)?;
            Heard::Done
        }
        Errand::History => Heard::History(printer_link::history(wire, HISTORY_COUNT)?),
        Errand::Picture { task, .. } => {
            let bytes = printer_link::picture(wire, state, task.as_ref())?;
            Heard::Picture(bytes.and_then(|bytes| decoded(&bytes)))
        }
        Errand::Camera(on) => Heard::Camera(printer_link::camera(wire, on)?),
    })
}

/// A picture as the card draws it. A board's bitmap may leave its fourth channel at zero,
/// which would draw nothing, so a picture with no opaque pixel at all is made opaque.
fn decoded(bytes: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory(bytes)
        .inspect_err(|error| tracing::debug!(%error, "a printer's picture did not decode"))
        .ok()?
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let mut pixels = image.into_raw();
    if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
        pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|pixel| pixel[3] = u8::MAX);
    }
    Some(egui::ColorImage::from_rgba_unmultiplied(size, &pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two by one bitmap whose fourth channel is zero, as a board writes its thumbnail:
    /// 32 bits a pixel, `BI_RGB`, stored top down.
    fn a_board_bitmap() -> Vec<u8> {
        let pixels: [u8; 8] = [0, 0, 255, 0, 255, 0, 0, 0];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&pixels);
        bytes
    }

    #[test]
    fn a_board_thumbnail_is_drawn_opaque_in_its_own_colours() {
        let image = decoded(&a_board_bitmap()).expect("a plain bitmap decodes");
        assert_eq!(image.size, [2, 1]);
        assert_eq!(image.pixels[0].to_array(), [255, 0, 0, 255], "BGR order");
        assert_eq!(image.pixels[1].to_array(), [0, 0, 255, 255]);
    }

    #[test]
    fn bytes_that_are_no_picture_are_none() {
        assert!(decoded(b"not a picture").is_none());
    }

    #[test]
    fn a_failed_picture_is_kept_as_missing_and_holds_no_button() {
        let (sender, answer) = mpsc::channel();
        let out = Out {
            key: "sdcp:aa".to_owned(),
            errand: Errand::Picture {
                id: "0e8a".to_owned(),
                task: None,
            },
            answer,
        };
        assert!(!out.holds_the_buttons());
        let trouble = Trouble {
            message: "network call failed".to_owned(),
            unreachable: false,
        };
        sender
            .send(Err(trouble))
            .expect("the errand holds the receiver");
        let mut extras = Extras::default();
        assert!(out.take(&mut extras));
        assert!(matches!(
            extras.pictures.get("0e8a"),
            Some(Picture::Missing)
        ));
        assert!(
            extras.trouble.is_none(),
            "a missing picture is not worth a warning"
        );
    }

    #[test]
    fn a_refused_command_is_kept_as_the_printers_trouble() {
        let (sender, answer) = mpsc::channel();
        let out = Out {
            key: "sdcp:aa".to_owned(),
            errand: Errand::Control(Command::Pause),
            answer,
        };
        let mut extras = Extras {
            doing: Some(out.doing()),
            ..Extras::default()
        };
        let trouble = Trouble {
            message: "the printer refused to pause".to_owned(),
            unreachable: false,
        };
        sender
            .send(Err(trouble.clone()))
            .expect("the errand holds the receiver");
        assert!(out.take(&mut extras));
        assert_eq!(extras.doing, None, "the buttons come back");
        assert_eq!(extras.trouble, Some(trouble));
    }
}
