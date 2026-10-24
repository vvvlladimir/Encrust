use crate::i18n::tr;
use printer_profiles::unique_id;
use printer_profiles::{
    Catalogue, Entry, Kind, MaterialProfile, PrinterProfile, PrinterTuning, ProfileError, Source,
    SupportProfile, own_id,
};
use std::collections::BTreeMap;

/// The resins set up on `printer`, in the catalogue's order.
pub fn resins_of<'a>(
    catalogue: &'a Catalogue,
    printer: &'a str,
) -> impl Iterator<Item = (&'a str, &'a MaterialProfile)> {
    catalogue
        .resins()
        .filter(move |entry| installed(&entry.source) && entry.profile.is_tuned_for(printer))
        .map(|entry| (entry.id.as_str(), &entry.profile))
}

/// Whether a profile is one the user has, rather than one the catalogue merely offers.
/// The window lists what is installed; the shipped profiles are the library behind the
/// `+` and the Add resin menu. See `docs/decisions/0158`.
pub fn installed(source: &Source) -> bool {
    matches!(source, Source::User(_))
}

/// The machines the user installed, in catalogue order.
pub fn installed_printers(catalogue: &Catalogue) -> impl Iterator<Item = &Entry<PrinterProfile>> {
    catalogue
        .printers()
        .filter(|entry| installed(&entry.source))
}

/// Every resin the catalogue has that `printer` is not already set up with: the shipped
/// ones as well, which is the only way they are reached.
pub fn pool_for<'a>(
    catalogue: &'a Catalogue,
    printer: &'a str,
) -> impl Iterator<Item = (&'a str, &'a MaterialProfile)> {
    catalogue
        .resins()
        .filter(move |entry| !installed(&entry.source) || !entry.profile.is_tuned_for(printer))
        .map(|entry| (entry.id.as_str(), &entry.profile))
}

/// Sets a resin from the pool up on `printer`, starting from the numbers of the printer
/// it was last tuned on.
pub fn add_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    let start = base.starting_point(printer);
    let tuning = PrinterTuning::of_changes(&base, &start);
    base.printers.insert(printer.to_owned(), tuning);
    base.last_printer = Some(printer.to_owned());
    catalogue.save_resin(resin, &base).map(drop)
}

/// A new resin on `printer` alone, with the stock numbers. Returns its id.
pub fn new_resin(catalogue: &mut Catalogue, printer: &str) -> Result<String, ProfileError> {
    save_as_own(catalogue, printer, stock_resin())
}

/// What **New resin** makes: the stock numbers under a name that says it is new.
pub(super) fn stock_resin() -> MaterialProfile {
    MaterialProfile {
        name: tr!("settings-new-resin"),
        ..MaterialProfile::default()
    }
}

/// Whether nothing has been typed into a resin since it was made: the stock numbers, the
/// stock name, and no machine's table carrying a change.
pub fn is_untouched(resin: &MaterialProfile) -> bool {
    let bare = MaterialProfile {
        printers: BTreeMap::new(),
        last_printer: None,
        ..resin.clone()
    };
    bare == stock_resin()
        && resin
            .printers
            .values()
            .all(|tuning| *tuning == PrinterTuning::default())
}

/// A copy of a resin as `printer` has it, on that printer alone. Returns its id.
pub fn duplicate_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<String, ProfileError> {
    let mut copy = catalogue.resin(resin)?.profile.for_printer(printer);
    copy.name = tr!("settings-copy-name", name = copy.name);
    save_as_own(catalogue, printer, copy)
}

/// Takes a resin off `printer`. It stays in the pool for any printer to take back, and is
/// deleted from there; one nobody ever typed into is not worth keeping, so taking that
/// off its last printer throws it away (ADR 0197).
pub fn take_resin_off(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    match is_thrown_away_with_the_printer(catalogue, printer, resin)? {
        true => delete_resin(catalogue, resin),
        false => remove_resin(catalogue, printer, resin),
    }
}

/// Whether taking this resin off `printer` takes it away altogether rather than leaving
/// it in the pool, which is what the question before it has to say.
pub fn is_thrown_away_with_the_printer(
    catalogue: &Catalogue,
    printer: &str,
    resin: &str,
) -> Result<bool, ProfileError> {
    let profile = &catalogue.resin(resin)?.profile;
    let last = profile.printers.keys().all(|id| id == printer);
    Ok(last && is_untouched(profile))
}

/// Throws a resin away, off every printer that had it and out of the user's directory.
pub fn delete_resin(catalogue: &mut Catalogue, resin: &str) -> Result<(), ProfileError> {
    catalogue.forget_user_copy(Kind::Resin, resin)
}

/// The printers other than `printer` a resin carries numbers for, by name, which is what
/// a question about deleting it has to say out loud.
pub fn other_printers(
    catalogue: &Catalogue,
    printer: &str,
    resin: &str,
) -> Result<Vec<String>, ProfileError> {
    let profile = &catalogue.resin(resin)?.profile;
    Ok(profile
        .printers
        .keys()
        .filter(|id| id.as_str() != printer)
        .map(|id| match catalogue.printer(id) {
            Ok(entry) => format!("{} {}", entry.profile.manufacturer, entry.profile.name),
            Err(_) => id.clone(),
        })
        .collect())
}

/// Takes a resin off `printer`. It stays in the pool for any printer to take back.
pub fn remove_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    base.printers.remove(printer);
    if base.last_printer.as_deref() == Some(printer) {
        base.last_printer = base.printers.keys().next().cloned();
    }
    catalogue.save_resin(resin, &base).map(drop)
}

/// Renames a resin as `printer` lists it. A resin other printers use too is split off
/// under the new name, so theirs keeps its name. Returns the id the resin now has here.
pub fn rename_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
    name: &str,
) -> Result<String, ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    let shared = base.printers.keys().any(|other| other != printer);
    if !shared {
        base.name = name.to_owned();
        catalogue.save_resin(resin, &base)?;
        return Ok(resin.to_owned());
    }
    let mut own = base.for_printer(printer);
    own.name = name.to_owned();
    let id = save_as_own(catalogue, printer, own)?;
    remove_resin(catalogue, printer, resin)?;
    Ok(id)
}

/// Writes `values` as a new resin whose own numbers are this printer's, under the first
/// free id its name makes.
pub(super) fn save_as_own(
    catalogue: &mut Catalogue,
    printer: &str,
    values: MaterialProfile,
) -> Result<String, ProfileError> {
    let resin = MaterialProfile {
        printers: BTreeMap::from([(printer.to_owned(), PrinterTuning::default())]),
        last_printer: Some(printer.to_owned()),
        ..values
    };
    let id = own_id(&resin.name, |id| catalogue.is_taken(Kind::Resin, id))?;
    catalogue.save_resin(&id, &resin)?;
    Ok(id)
}

/// A new printer with the stock numbers. Returns its id.
pub fn new_printer(catalogue: &mut Catalogue) -> Result<String, ProfileError> {
    let printer = PrinterProfile {
        name: tr!("settings-new-printer"),
        ..PrinterProfile::default()
    };
    let id = own_id(&printer.name, |id| catalogue.is_taken(Kind::Printer, id))?;
    catalogue.save_printer(&id, &printer)?;
    Ok(id)
}

/// A new support profile with the stock numbers, or a copy of `from`. Returns its id.
pub fn new_support(catalogue: &mut Catalogue, from: Option<&str>) -> Result<String, ProfileError> {
    let profile = match from {
        Some(id) => {
            let mut copy = catalogue.support(id)?.profile.clone();
            copy.name = tr!("settings-copy-name", name = copy.name);
            copy
        }
        None => SupportProfile {
            name: tr!("settings-new-profile"),
            ..SupportProfile::default()
        },
    };
    keep_support(catalogue, &profile)
}

/// Keeps `profile` as a support profile of its own. Returns its id.
pub fn keep_support(
    catalogue: &mut Catalogue,
    profile: &SupportProfile,
) -> Result<String, ProfileError> {
    let id = unique_id(&profile.name, |id| catalogue.is_taken(Kind::Support, id));
    catalogue.save_support(&id, profile)?;
    Ok(id)
}

/// Whether a profile can be thrown away: one only the user ever had. A shipped one would
/// only come back.
pub fn can_delete(catalogue: &Catalogue, kind: Kind, id: &str) -> bool {
    !catalogue.is_shipped(kind, id)
}
