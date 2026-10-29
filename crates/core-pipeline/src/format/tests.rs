use super::*;

#[test]
fn the_extension_decides_what_is_written() {
    let of = |name: &str| SlicedFormat::of(Path::new(name), CtbVersion::V4);
    assert_eq!(of("cube.goo"), Some(SlicedFormat::Goo));
    assert_eq!(of("cube.GOO"), Some(SlicedFormat::Goo));
    assert_eq!(of("cube.PRZ"), Some(SlicedFormat::Prz));
    assert_eq!(of("cube.ctb"), Some(SlicedFormat::Ctb(CtbVersion::V4)));
    assert_eq!(
        of("cube.cbddlp"),
        Some(SlicedFormat::Cbddlp(CbddlpFlavour::Cbddlp))
    );
    assert_eq!(
        of("cube.PHOTON"),
        Some(SlicedFormat::Cbddlp(CbddlpFlavour::Photon))
    );
    assert_eq!(
        of("cube.pwmx"),
        Some(SlicedFormat::Anycubic(
            AnycubicFlavour::Pwmx,
            AnycubicVersion::V516
        ))
    );
    assert_eq!(
        of("cube.DLP"),
        Some(SlicedFormat::Anycubic(
            AnycubicFlavour::Dlp,
            AnycubicVersion::V515
        ))
    );
    assert_eq!(of("cube.sl1"), Some(SlicedFormat::Sl1(Sl1Flavour::Sl1)));
    assert_eq!(of("cube.ZIP"), Some(SlicedFormat::GcodeZip));
    assert_eq!(of("cube.crmaslice"), Some(SlicedFormat::Crmaslice));
    assert_eq!(
        of("cube.cxdlp"),
        Some(SlicedFormat::Cxdlp(CxdlpVersion::V3))
    );
    assert_eq!(of("cube.svgx"), Some(SlicedFormat::Svgx));
    assert_eq!(of("cube.cws"), Some(SlicedFormat::Cws));
    assert_eq!(of("cube.PM7"), Some(SlicedFormat::Pwsz(PwszFlavour::Pm7)));
    assert_eq!(of("cube.sl1s"), Some(SlicedFormat::Sl1(Sl1Flavour::Sl1s)));
    assert_eq!(
        SlicedFormat::of(Path::new("cube.ctb"), CtbVersion::V5),
        Some(SlicedFormat::Ctb(CtbVersion::V5))
    );
    assert_eq!(
        of("out"),
        None,
        "a name without an extension is a PNG directory"
    );
    assert_eq!(of("out.d/cube"), None);
}

#[test]
fn another_choice_still_names_a_revision_for_a_ctb_name_under_it() {
    assert_eq!(SlicedFormat::Goo.ctb_version(), CtbVersion::V4);
    assert_eq!(
        SlicedFormat::Cbddlp(CbddlpFlavour::Photon).ctb_version(),
        CtbVersion::V4
    );
    assert_eq!(
        SlicedFormat::Ctb(CtbVersion::V5).ctb_version(),
        CtbVersion::V5
    );
}

#[test]
fn a_name_without_a_revision_takes_the_one_the_profile_states() {
    let named = SlicedFormat::Anycubic(AnycubicFlavour::Pwmb, AnycubicVersion::V517);
    let stated = OutputFormat::Anycubic {
        extension: AnycubicExtension::Pwmb,
        revision: PhotonRevision::V516,
    };
    assert_eq!(
        named.at_revision_of(stated),
        SlicedFormat::Anycubic(AnycubicFlavour::Pwmb, AnycubicVersion::V516)
    );
    assert_eq!(
        SlicedFormat::Goo.at_revision_of(stated),
        SlicedFormat::Goo,
        "a name of another container keeps what it named"
    );
    assert_eq!(
        SlicedFormat::Goo.at_revision_of(OutputFormat::Goo5),
        SlicedFormat::Goo5,
        "a .goo name keeps the revision the machine reads"
    );
    assert_eq!(
        SlicedFormat::Anycubic(AnycubicFlavour::Pm5, AnycubicVersion::V517).at_revision_of(stated),
        SlicedFormat::Anycubic(AnycubicFlavour::Pm5, AnycubicVersion::V517),
        "another extension is another container"
    );
}

#[test]
fn every_shipped_machine_names_a_container_a_writer_exists_for() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");

    for entry in catalogue.printers() {
        let format = SlicedFormat::from(entry.profile.output);
        let named = format!("job.{}", format.extension());

        // The revision is not in a file name, so only the writer has to come back:
        // a profile is what states which revision of a container a machine reads.
        let routed = SlicedFormat::of(Path::new(&named), format.ctb_version())
            .unwrap_or_else(|| panic!("{} writes {named}, which routes nowhere", entry.id));
        assert_eq!(
            routed.extension(),
            format.extension(),
            "{} is written as {named}, which routes to another writer",
            entry.id
        );
    }
}

#[test]
fn every_shipped_machine_is_offered_the_container_it_states() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");

    for entry in catalogue.printers() {
        let stated = SlicedFormat::from(entry.profile.output);
        assert!(
            stated.writable_for(&entry.profile),
            "{} states {} and cannot be written one",
            entry.id,
            stated.extension()
        );
    }
}

#[test]
fn a_machine_whose_name_carries_no_model_code_is_offered_no_cxdlp() {
    let mut printer = PrinterProfile {
        name: "Mars 4 Ultra".to_owned(),
        machine_name: None,
        ..Default::default()
    };

    let offered: Vec<_> = SlicedFormat::choices_for(&printer).collect();
    assert!(
        !offered.iter().any(|choice| choice.extension() == "cxdlp"),
        "the firmware matches a CL or CT code this name has none of"
    );
    assert_eq!(
        offered.len(),
        SlicedFormat::CHOICES.len() - 2,
        "only the two .cxdlp revisions come off the list"
    );

    printer.machine_name = Some("Halot One CL-60".to_owned());
    assert_eq!(
        SlicedFormat::choices_for(&printer).count(),
        SlicedFormat::CHOICES.len(),
        "a name with a code is offered every container"
    );
}

#[test]
fn a_format_survives_the_round_trip_through_a_profile() {
    for format in SlicedFormat::CHOICES {
        let family: OutputFormat = format.into();
        assert_eq!(SlicedFormat::from(family), format);
    }
}
