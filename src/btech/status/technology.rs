//! Shared equipment summaries for the cockpit weapon block.
use crate::btech::{self, *};
use crate::{ObjectId, World};
use anyhow::Result;

/// Project installation and physical availability without maintaining display-only state.
pub(super) fn device(world: &World, id: ObjectId, system: System) -> Result<(bool, bool)> {
    crate::btech::with_unit!(world.btech.unit(id).expect("unit record"), |u| {
        let parts = u
            .loadout()?
            .systems
            .into_iter()
            .filter(|p| p.system == system)
            .collect::<Vec<_>>();
        Ok((
            !parts.is_empty(),
            !parts.is_empty() && parts.iter().all(|p| !u.critical_unavailable(p.location)),
        ))
    })
}

/// Build the advanced-technology row using common suite states and chassis-owned boosters.
pub(super) fn lines(world: &World, id: ObjectId) -> Result<Vec<String>> {
    let mech = world.btech.constructed_units().get(&id);
    let vehicle = world.btech.vehicles().get(&id);
    let electronics = mech.map_or_else(|| vehicle.unwrap().electronics(), |u| u.electronics());
    let light = mech.map_or_else(|| vehicle.unwrap().searchlight(), |u| u.searchlight());
    let installed_light = mech.map_or_else(
        || vehicle.unwrap().definition().has_special("Searchlight"),
        |u| u.definition().has_special("Searchlight"),
    );
    let mut parts = Vec::new();
    for (system, name, mode) in [
        (System::Ecm, "ECM", electronics.guardian),
        (System::AngelEcm, "AngelECM", electronics.angel),
    ] {
        let (installed, available) = device(world, id, system)?;
        if installed {
            let status = if !available {
                "[fg=red bold]XX[reset]"
            } else {
                match mode {
                    ElectronicMode::Off => {
                        if electronics.field.countered {
                            "[fg=red]Off[reset]"
                        } else {
                            "[fg=green]Off[reset]"
                        }
                    }
                    ElectronicMode::Ecm => {
                        if !electronics.field.countered {
                            "[fg=green bold]ECM[reset]"
                        } else {
                            "[fg=red bold]ECM[reset]"
                        }
                    }
                    ElectronicMode::Eccm => "[fg=green bold]ECCM[reset]",
                }
            };
            parts.push(format!("{name}({status})  "));
        }
    }
    if let Some(u) = mech {
        if u.has_stealth_armor()? {
            parts.push(format!(
                "SthArmor({})  ",
                ready(u.stealth().enabled, device(world, id, System::Ecm)?.1)
            ));
        }
        if u.has_null_signature()? {
            parts.push(format!(
                "NullSigSys({})  ",
                ready(
                    u.null_signature().enabled,
                    device(world, id, System::NullSignature)?.1
                )
            ));
        }
    }
    if installed_light {
        parts.push(format!(
            "SLITE({}{})  ",
            if light.destroyed {
                "[fg=red bold]XX[reset]"
            } else if light.on {
                "[fg=green bold]On[reset]"
            } else {
                "[fg=green]Off[reset]"
            },
            if !light.destroyed && light.mode == super::super::SearchlightMode::Auto {
                "/Auto"
            } else {
                ""
            }
        ));
    }
    let hardware = mech.map_or_else(|| vehicle.unwrap().c3_hardware(), |u| u.c3_hardware())?;
    for (present, available, name) in [
        (hardware.masters > 0, hardware.working_masters > 0, "C3M"),
        (hardware.slave_installed, hardware.slave_operational, "C3S"),
        (hardware.c3i_installed, hardware.c3i_operational, "C3i"),
    ] {
        if present {
            let connected = if name == "C3i" {
                mech.map_or_else(
                    || vehicle.unwrap().c3i_network.is_some(),
                    |u| u.c3i_network.is_some(),
                )
            } else {
                mech.map_or_else(
                    || vehicle.unwrap().c3_network.is_some(),
                    |u| u.c3_network.is_some(),
                )
            };
            parts.push(format!(
                "[fg={}]{}[reset]  ",
                if !available {
                    "red"
                } else if electronics.field.blocks_outgoing_guidance() {
                    "yellow"
                } else if connected {
                    "green bold"
                } else {
                    "green"
                },
                name
            ));
        }
    }
    if let Some(u) = mech
        && u.definition().has_triple_myomer()
    {
        parts.push(format!(
            "TSM({})  ",
            if u.triple_myomer_active() {
                "[fg=green bold]On[reset]"
            } else {
                "[fg=green]Off[reset]"
            }
        ));
    }
    let (installed, available) = btech::tag::unit_hardware(world, id)?;
    if installed {
        let state = btech::tag::state(world, id).expect("constructed unit");
        let status = if !available {
            "[fg=red bold]XX[reset]".into()
        } else if let Some(target) = state.target {
            let name = btech::scanner::scanner_unit(world, target).map_or_else(
                || "unknown".into(),
                |s| {
                    format!(
                        "{} [{}]",
                        crate::text::escape(s.name),
                        s.label().unwrap_or_else(|| "??".into())
                    )
                },
            );
            format!("[bold]{name}[reset]")
        } else if state.remaining > 0 {
            "[fg=yellow bold]Not Rdy[reset]".into()
        } else {
            "[fg=green]Rdy[reset]".into()
        };
        parts.push(format!("TAG({status})  "));
    }
    if let Some(u) = mech {
        for (present, name, counter, on) in [
            (
                u.supercharger_installed(),
                "SCHARGE",
                u.supercharger().counter,
                u.supercharger().enabled,
            ),
            (
                u.masc_installed()?,
                "MASC",
                u.masc().counter,
                u.masc().enabled,
            ),
        ] {
            if present {
                parts.push(format!(
                    "{name}: [fg={}]{}[reset] ({})",
                    if counter > 3 {
                        "red bold"
                    } else if counter > 0 {
                        "yellow bold"
                    } else {
                        "green"
                    },
                    counter,
                    if on { "On" } else { "Off" }
                ));
            }
        }
    }
    let mut lines = Vec::new();
    if !parts.is_empty() {
        lines.push(format!("AdvTech: {}", parts.join("")));
    }
    let mut sensors = Vec::new();
    if btech::scanner::scanner_unit(world, id).is_some_and(|s| s.radar) {
        sensors.push("Radar");
    }
    for (system, label) in [
        (System::BeagleProbe, "BeagleProbe"),
        (System::BloodhoundProbe, "BloodhoundProbe"),
    ] {
        if device(world, id, system)?.0 {
            sensors.push(label);
        }
    }
    if !sensors.is_empty() {
        lines.push(format!("AdvSensors: {}", sensors.join(" ")));
    }
    Ok(lines)
}

/// Ready/on/failed presentation shared by signature systems.
fn ready(on: bool, available: bool) -> &'static str {
    if !available {
        "[fg=red bold]XX[reset]"
    } else if on {
        "[fg=green bold]On[reset]"
    } else {
        "[fg=green]Rdy[reset]"
    }
}
