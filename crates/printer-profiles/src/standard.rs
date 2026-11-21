//! The machine a pose is proved on where nobody names one.

use crate::{Catalogue, MaterialProfile, PrinterProfile, ProfileError};

/// The catalogue id of the standard printer.
pub const STANDARD_PRINTER: &str = "elegoo-saturn-4-ultra";

/// The printer `printer` names (the standard one where none) with the resin `resin` names, the
/// stock numbers where none, so an unnamed resin never depends on a user's folder (ADR 0196).
pub fn machine_named(
    catalogue: &Catalogue,
    printer: Option<&str>,
    resin: Option<&str>,
) -> Result<(PrinterProfile, MaterialProfile), ProfileError> {
    let printer_id = printer.unwrap_or(STANDARD_PRINTER);
    let machine = catalogue.printer(printer_id)?.profile.clone();
    let Some(id) = resin else {
        return Ok((machine, MaterialProfile::default()));
    };
    Ok((
        machine,
        catalogue.resin(id)?.profile.starting_point(printer_id),
    ))
}

/// The standard printer with the stock resin numbers.
pub fn standard_machine(
    catalogue: &Catalogue,
) -> Result<(PrinterProfile, MaterialProfile), ProfileError> {
    machine_named(catalogue, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_machine_is_the_standard_printer_with_stock_numbers_where_no_resin_ships() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue reads");

        let (printer, material) = standard_machine(&catalogue).expect("the standard printer ships");

        let shipped = &catalogue
            .printer(STANDARD_PRINTER)
            .expect("it ships")
            .profile;
        assert_eq!(printer.name, shipped.name);
        assert_eq!(material, MaterialProfile::default());
    }

    #[test]
    fn a_printer_or_resin_the_catalogue_lacks_is_unknown_by_its_kind() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue reads");

        let printer = machine_named(&catalogue, Some("nope"), None).expect_err("no such printer");
        let resin = machine_named(&catalogue, None, Some("nope")).expect_err("no such resin");

        assert!(matches!(
            printer,
            ProfileError::Unknown {
                kind: "printer",
                ..
            }
        ));
        assert!(matches!(resin, ProfileError::Unknown { kind: "resin", .. }));
    }
}
