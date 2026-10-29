//! A shipped printer the user installed is kept as their edits over it, so every field they
//! left alone follows the build they run. See docs/decisions/0372.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use toml::{Table, Value};

use super::{Entry, Kind, Source, Unreadable, shipped_printer};
use crate::{PrinterProfile, ProfileError, ProfileStore};

/// Marks a file as edits over the shipped printer of its id, not a whole profile.
const MARKER: &str = "edits_over_shipped";
/// Dotted keys the user left unset where the shipped printer states them.
const UNSET: &str = "unset";
/// Always the shipped machine's (ADR 0301), so never an edit.
const LOCKED: &str = "locked";
/// What a whole copy an older release left is kept aside as once the shipped printer replaces it.
const OLD_COPY_ASIDE: &str = "bak";
const HEADER: &str =
    "# Your edits over the shipped printer of this name; every other field follows it.\n";

/// Lays the user's printers over `map`: edits over a shipped machine, a whole copy an older
/// release left of one, put back to the shipped one, or a printer of their own, as it stands.
pub(super) fn overlay_printers(
    map: &mut BTreeMap<String, Entry<PrinterProfile>>,
    store: &dyn ProfileStore,
) -> Result<Vec<Unreadable>, ProfileError> {
    let mut unreadable = Vec::new();
    for (id, path, text) in store.read_all(Kind::Printer)? {
        match read(store, &id, &path, &text) {
            Ok(profile) => {
                let source = Source::User(path);
                map.insert(
                    id.clone(),
                    Entry {
                        id,
                        profile,
                        source,
                    },
                );
            }
            Err(error) => unreadable.push(Unreadable {
                kind: Kind::Printer,
                id,
                path,
                error: Arc::new(error),
            }),
        }
    }
    Ok(unreadable)
}

/// What `profile` keeps as the file of a shipped printer: only where it differs from `shipped`.
pub(super) fn written_over(
    profile: &PrinterProfile,
    shipped: &PrinterProfile,
    path: &Path,
) -> Result<String, ProfileError> {
    profile.validate()?;
    written(edits(profile, shipped, path)?, path)
}

fn read(
    store: &dyn ProfileStore,
    id: &str,
    path: &Path,
    text: &str,
) -> Result<PrinterProfile, ProfileError> {
    let table: Table = toml::from_str(text).map_err(|source| ProfileError::Parse {
        path: path.to_owned(),
        source,
    })?;
    let shipped = shipped_printer(id);
    if table.contains_key(MARKER) {
        let shipped = shipped.ok_or_else(|| ProfileError::EditsOverNothing {
            id: id.to_owned(),
            path: path.to_owned(),
        })?;
        return apply(&shipped, table, path);
    }
    let Some(shipped) = shipped else {
        return PrinterProfile::from_toml_str(text, path);
    };
    let carried = machines_carried_over(&shipped, &table, path)?;
    // A store that refuses either write keeps the copy, to be put back the same way next run.
    if store
        .keep_aside(Kind::Printer, id, OLD_COPY_ASIDE, text)
        .is_ok()
    {
        let _ = store.write(Kind::Printer, id, &written_over(&carried, &shipped, path)?);
    }
    Ok(carried)
}

/// `shipped` with the printers on the network an old whole copy sends to, as nothing ships them.
/// Only the `machines` table is read, so a copy no release would load still gives them up.
fn machines_carried_over(
    shipped: &PrinterProfile,
    old: &Table,
    path: &Path,
) -> Result<PrinterProfile, ProfileError> {
    let machines = match old.get("machines") {
        Some(machines) => machines
            .clone()
            .try_into()
            .map_err(|source| ProfileError::Parse {
                path: path.to_owned(),
                source,
            })?,
        None => Vec::new(),
    };
    Ok(PrinterProfile {
        machines,
        ..shipped.clone()
    })
}

/// The fields of `profile` that differ from `shipped`, with those it leaves unset listed.
fn edits(
    profile: &PrinterProfile,
    shipped: &PrinterProfile,
    path: &Path,
) -> Result<Table, ProfileError> {
    let mut mine = to_table(profile, path)?;
    let mut theirs = to_table(shipped, path)?;
    mine.remove(LOCKED);
    theirs.remove(LOCKED);
    let mut unset = Vec::new();
    let mut edits = difference(mine, &theirs, "", &mut unset);
    if !unset.is_empty() {
        edits.insert(
            UNSET.to_owned(),
            Value::Array(unset.into_iter().map(Value::String).collect()),
        );
    }
    Ok(edits)
}

fn difference(mine: Table, theirs: &Table, prefix: &str, unset: &mut Vec<String>) -> Table {
    let missing = theirs.keys().filter(|key| !mine.contains_key(*key));
    unset.extend(missing.map(|key| format!("{prefix}{key}")));
    mine.into_iter()
        .filter_map(|(key, value)| match (value, theirs.get(&key)) {
            (Value::Table(inner), Some(Value::Table(shipped))) => {
                let inner = difference(inner, shipped, &format!("{prefix}{key}."), unset);
                (!inner.is_empty()).then_some((key, Value::Table(inner)))
            }
            (value, shipped) => (shipped != Some(&value)).then_some((key, value)),
        })
        .collect()
}

/// `shipped` with `edits` laid over it, field by field.
fn apply(
    shipped: &PrinterProfile,
    mut edits: Table,
    path: &Path,
) -> Result<PrinterProfile, ProfileError> {
    edits.remove(MARKER);
    edits.remove(LOCKED);
    let mut merged = to_table(shipped, path)?;
    if let Some(Value::Array(keys)) = edits.remove(UNSET) {
        for key in keys.iter().filter_map(Value::as_str) {
            remove_dotted(&mut merged, key);
        }
    }
    merge(&mut merged, edits);
    let text = toml::to_string(&merged).map_err(|source| ProfileError::Serialise {
        path: path.to_owned(),
        source,
    })?;
    PrinterProfile::from_toml_str(&text, path)
}

fn remove_dotted(table: &mut Table, key: &str) {
    match key.split_once('.') {
        Some((head, rest)) => {
            if let Some(Value::Table(inner)) = table.get_mut(head) {
                remove_dotted(inner, rest);
            }
        }
        None => {
            table.remove(key);
        }
    }
}

fn merge(into: &mut Table, edits: Table) {
    for (key, value) in edits {
        if let Value::Table(edit) = value {
            if let Some(Value::Table(inner)) = into.get_mut(&key) {
                merge(inner, edit);
                continue;
            }
            into.insert(key, Value::Table(edit));
        } else {
            into.insert(key, value);
        }
    }
}

fn written(mut edits: Table, path: &Path) -> Result<String, ProfileError> {
    edits.insert(MARKER.to_owned(), Value::Boolean(true));
    let body = toml::to_string_pretty(&edits).map_err(|source| ProfileError::Serialise {
        path: path.to_owned(),
        source,
    })?;
    Ok(format!("{HEADER}{body}"))
}

fn to_table(profile: &PrinterProfile, path: &Path) -> Result<Table, ProfileError> {
    Table::try_from(profile).map_err(|source| ProfileError::Serialise {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests;
