use super::object::SceneObject;
use super::{ObjectId, Scene};
use crate::i18n::tr;

/// What a plate nobody has renamed is called.
pub(super) fn plate_name(index: usize) -> String {
    tr!("scene-plate-name", index = index + 1)
}

impl Scene {
    /// Makes `plate` the one being edited. A selection left on another plate is dropped,
    /// because every tool works on what is in front of the user.
    pub fn show_plate(&mut self, plate: u32) {
        if plate as usize >= self.plates.len() || plate == self.active {
            return;
        }
        self.active = plate;
        // A pick left on another plate would have the tools working out of sight.
        let here: Vec<ObjectId> = self.here().map(|object| object.id).collect();
        self.selected.retain(|id| here.contains(id));
    }

    /// Adds an empty plate at the end and makes it the one being edited.
    ///
    /// Its name is the lowest default nobody is using: counting off the plate count
    /// hands out a name already taken as soon as one has been removed.
    pub fn add_plate(&mut self) -> u32 {
        let name = (0..)
            .map(plate_name)
            .find(|name| !self.plates.contains(name))
            .unwrap_or_else(|| plate_name(self.plates.len()));
        self.plates.push(name);
        let plate = (self.plates.len() - 1) as u32;
        self.active = plate;
        self.selected.clear();
        plate
    }

    pub fn rename_plate(&mut self, plate: u32, name: String) {
        if let Some(slot) = self.plates.get_mut(plate as usize) {
            *slot = name;
        }
    }

    /// Takes a plate away with everything on it, and shifts the plates above it down. The
    /// last plate cannot go: a project always has somewhere to put a model.
    pub fn remove_plate(&mut self, plate: u32) {
        if self.plates.len() < 2 || plate as usize >= self.plates.len() {
            return;
        }
        self.plates.remove(plate as usize);
        self.objects.retain(|object| object.plate != plate);
        for object in &mut self.objects {
            if object.plate > plate {
                object.plate -= 1;
            }
        }
        self.active = self.active.min(self.plates.len() as u32 - 1);
        let left: Vec<ObjectId> = self.objects.iter().map(|object| object.id).collect();
        self.selected.retain(|id| left.contains(id));
    }

    /// Adds a copy of `plate` and of everything standing on it at the end, and makes it the
    /// one being edited.
    pub fn duplicate_plate(&mut self, plate: u32) -> Option<u32> {
        let name = self.plates.get(plate as usize)?;
        let name = (1..)
            .map(|count| match count {
                1 => tr!("settings-copy-name", name = name.clone()),
                count => tr!("scene-copy-numbered", name = name.clone(), count = count),
            })
            .find(|name| !self.plates.contains(name))?;
        self.plates.push(name);
        let copy = (self.plates.len() - 1) as u32;
        let models: Vec<SceneObject> = self.on_plate(plate).cloned().collect();
        let first = self.next_id;
        let copied = |id: ObjectId| {
            let index = models.iter().position(|model| model.id == id)?;
            Some(ObjectId(first + index as u64))
        };
        let plugs: Vec<Option<ObjectId>> = models
            .iter()
            .map(|model| model.plug_of.and_then(copied))
            .collect();
        for (mut model, plug_of) in models.into_iter().zip(plugs) {
            model.id = ObjectId(self.next_id);
            self.next_id += 1;
            model.plate = copy;
            model.plug_of = plug_of;
            self.objects.push(model);
        }
        self.active = copy;
        self.selected.clear();
        Some(copy)
    }

    /// Moves a model to another plate. It keeps where it stands, so a plate it does not
    /// fit is the arranger's problem rather than this one's.
    pub fn move_to_plate(&mut self, id: ObjectId, plate: u32) {
        if plate as usize >= self.plates.len() {
            return;
        }
        if let Some(object) = self.get_mut(id) {
            object.plate = plate;
        }
        if plate != self.active {
            self.selected.retain(|picked| *picked != id);
        }
    }
}
