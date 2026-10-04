//! The megamek-convert tool turns MegaMek unit files into loadable templates and refuses units
//! stompymux cannot field, without writing anything for them.
use crate::repository_root;
use std::path::PathBuf;
use std::process::{Command, Output};
use stompymux_rs::BattleUnitTemplate;

/// The hand-written MegaMek fixtures.
fn fixture(name: &str) -> PathBuf {
    repository_root().join("tests/fixtures/megamek").join(name)
}

/// Run the converter with arguments.
fn convert(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_megamek-convert"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn units_are_written_as_loadable_templates_and_refusals_write_nothing() {
    let out = tempfile::tempdir().unwrap();
    let atlas = fixture("Atlas AS7-D.mtf");
    let bulldog = fixture("Bulldog Medium Tank.blk");
    let air_car = fixture("Air Car.blk");
    let args = [
        "--output-dir".as_ref(),
        out.path().as_os_str(),
        atlas.as_os_str(),
        bulldog.as_os_str(),
        air_car.as_os_str(),
    ];
    let output = convert(&args);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Air Car.blk: error: unsupported unit type: SupportTank"),
        "{stderr}"
    );
    assert!(stderr.contains("2 file(s) converted, 1 failed"), "{stderr}");
    let mut written: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    written.sort();
    assert_eq!(written, ["AS7-D.toml", "Bulldog_Medium_Tank.toml"]);
    for file in &written {
        let source = std::fs::read_to_string(out.path().join(file)).unwrap();
        let reference = file.trim_end_matches(".toml");
        let template = BattleUnitTemplate::parse(reference, &source).unwrap();
        match (reference, template) {
            ("AS7-D", BattleUnitTemplate::Mech(mech)) => assert_eq!(mech.tons, 100),
            ("Bulldog_Medium_Tank", BattleUnitTemplate::Vehicle(vehicle)) => {
                assert_eq!(vehicle.tons, 60)
            }
            (reference, _) => panic!("{reference} has the wrong class"),
        }
    }

    let again = convert(&args[..3]);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("pass --force to replace it"));
    let forced = convert(&[&["--force".as_ref()], &args[..3]].concat());
    assert!(forced.status.success());
}

#[test]
fn a_single_unit_prints_its_template_under_any_reference() {
    let atlas = fixture("Atlas AS7-D.mtf");
    let output = convert(&["--reference".as_ref(), "Atlas".as_ref(), atlas.as_os_str()]);
    assert!(output.status.success());
    let template = String::from_utf8(output.stdout).unwrap();
    assert!(template.starts_with("name = \"Atlas\"\nclass = \"mech\"\n"));
    assert!(BattleUnitTemplate::parse("Atlas", &template).is_ok());

    let bulldog = fixture("Bulldog Medium Tank.blk");
    let many = convert(&[atlas.as_os_str(), bulldog.as_os_str()]);
    assert!(!many.status.success());
    assert!(String::from_utf8_lossy(&many.stderr).contains("--output-dir"));
}
