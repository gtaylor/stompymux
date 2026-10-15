//! Construction validation of a BattleMech template: the chassis fields, specials and
//! section layout a template must declare, and the engine, gyro, system and heat sink
//! installation its resolved loadout must contain. The server runs the same checks when it
//! constructs or revalidates a live Mech, so a template that passes here is one the game
//! would field.
use super::{MechLoadout, MechSection, MechTemplate, System};
use anyhow::{Result, ensure};

impl MechTemplate {
    /// Gate the conventional chassis features without silently discarding unknown fields.
    ///
    /// This covers everything decided by the template alone: identity, tonnage, speeds,
    /// heat sink count, every attribute and special, armor technology and the section
    /// layout. [`MechTemplate::validate_loadout`] checks the installed equipment afterwards.
    pub fn validate_definition(&self) -> Result<()> {
        super::validate_unit_metadata(&self.attributes)?;
        super::read_engine_sink_override(&self.attributes)?;
        super::read_template_speed(&self.attributes, self.max_speed)?;
        ensure!(
            (20..=100).contains(&self.tons) && self.tons.is_multiple_of(5),
            "Unsupported biped tonnage"
        );
        ensure!(
            self.max_speed.is_finite() && self.max_speed > 0.0 && self.max_speed <= 300.0,
            "Invalid maximum speed"
        );
        ensure!(
            self.jump_speed.is_finite()
                && self.jump_speed >= 0.0
                && self.jump_speed <= self.max_speed,
            "Invalid jump speed"
        );
        ensure!(
            (10..=100).contains(&self.heat_sinks),
            "Invalid heat sink count"
        );
        ensure!(
            !self.name.is_empty()
                && self.name.len() <= 128
                && !self.reference.is_empty()
                && self.reference.len() <= 128,
            "Invalid unit identity"
        );
        super::validate_radio_attributes(&self.attributes)?;
        self.validate_armor_slots()?;
        validate_unsupported_technology(self)?;
        for (field, value) in &self.attributes {
            match field.as_str() {
                "name" | "reference" | "tons" | "max_speed" | "jump_speed" | "heat_sinks"
                | "comment" | "hsengoverride" | "template_speed" => {}
                "type" => ensure!(value.eq_ignore_ascii_case("Mech"), "Unsupported unit type"),
                "move_type" => ensure!(
                    value.eq_ignore_ascii_case("Biped") || value.eq_ignore_ascii_case("Quad"),
                    "Unsupported movement type"
                ),
                "tac_range" | "lrs_range" | "scan_range" => ensure!(
                    value.parse::<u8>().is_ok_and(|range| range <= 127),
                    "Invalid sensor range {field}"
                ),
                "unit_era"
                | "unit_tro"
                | "radio_range"
                | "radiotype"
                | "radio"
                | "administrative_tonnage"
                | "administrative_unit_type"
                | "administrative_movement_type"
                | "carrier_maximum_tonnage" => {}
                "cargo_space" => ensure!(value.parse::<u32>().is_ok(), "Invalid cargo space"),
                "max_suits" => ensure!(
                    value.parse::<u16>() == Ok(0),
                    "Suit capacity is unsupported"
                ),
                "specials" => ensure!(
                    value == "-"
                        || (!value.trim().is_empty()
                            && value.split_ascii_whitespace().all(|flag| flag
                                .eq_ignore_ascii_case("FlipArms")
                                || flag.eq_ignore_ascii_case("Searchlight")
                                || flag.eq_ignore_ascii_case("CritProof_Tech")
                                || flag.eq_ignore_ascii_case("CargoTech")
                                || flag.eq_ignore_ascii_case("SalvageTech")
                                || flag.eq_ignore_ascii_case("Carrier_Tech")
                                || flag.eq_ignore_ascii_case("TripleMyomerTech")
                                || flag.eq_ignore_ascii_case("Masc")
                                || flag.eq_ignore_ascii_case("C3MasterTech")
                                || flag.eq_ignore_ascii_case("C3SlaveTech")
                                || flag.eq_ignore_ascii_case("C3I_Tech")
                                || flag.eq_ignore_ascii_case("SuperCharger_Tech")
                                || flag.eq_ignore_ascii_case("Clan")
                                || flag.eq_ignore_ascii_case("DoubleHS")
                                || flag.eq_ignore_ascii_case("LaserHS_Tech")
                                || flag.eq_ignore_ascii_case("ImprovedJJ_Tech")
                                || flag.eq_ignore_ascii_case("HDGYRO")
                                || flag.eq_ignore_ascii_case("XLGYRO")
                                || flag.eq_ignore_ascii_case("CGYRO")
                                || flag.eq_ignore_ascii_case("SMCPIT")
                                || flag.eq_ignore_ascii_case("ArtemisIV")
                                || flag.eq_ignore_ascii_case("ECM")
                                || flag.eq_ignore_ascii_case("AngelECM_Tech")
                                || flag.eq_ignore_ascii_case("StealthArmor_Tech")
                                || flag.eq_ignore_ascii_case("NullSigSys_Tech")
                                || flag.eq_ignore_ascii_case("TAG_Tech")
                                || flag.eq_ignore_ascii_case("BeagleProbe")
                                || flag.eq_ignore_ascii_case("LightBAP")
                                || flag.eq_ignore_ascii_case("BloodhoundProbe_Tech")
                                || flag.eq_ignore_ascii_case("FerroFibrous_Tech")
                                || flag.eq_ignore_ascii_case("EndoSteel_Tech")
                                || flag.eq_ignore_ascii_case("HvyFerroFibrous_Tech")
                                || flag.eq_ignore_ascii_case("LtFerroFibrous_Tech")
                                || flag.eq_ignore_ascii_case("XLEngine_Tech")
                                || super::mixed_technology_flag(flag)
                                || flag.eq_ignore_ascii_case("LightEngine_Tech")
                                || flag.eq_ignore_ascii_case("XXL_Tech")
                                || flag.eq_ignore_ascii_case("CompactEngine_Tech")
                                || super::Technology::recognizes(flag)
                                || (0..=56).any(|code| {
                                    super::administrative_technology(code)
                                        .is_some_and(|(name, _)| flag.eq_ignore_ascii_case(name))
                                }))),
                    "Unsupported chassis specials {value}"
                ),
                _ => anyhow::bail!("Unsupported chassis field {field}"),
            }
        }
        let chassis = self.chassis()?;
        ensure!(self.sections.len() == 8, "Missing biped sections");
        for (&section, layout) in &self.sections {
            let slots = chassis.critical_slots(section);
            ensure!(
                layout.internal > 0
                    && layout.internal <= 100
                    && layout.armor <= 200
                    && layout.rear <= 100,
                "Invalid section protection"
            );
            ensure!(
                layout.criticals.keys().all(|&slot| slot < slots),
                "Critical outside section capacity"
            );
            ensure!(
                layout
                    .configuration
                    .as_deref()
                    .is_none_or(|value| value == "-" || value.eq_ignore_ascii_case("Case")),
                "Unsupported section configuration"
            );
            ensure!(
                matches!(
                    section,
                    MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
                ) || layout.rear == 0,
                "Rear armor outside torso"
            );
        }
        Ok(())
    }

    /// Check the installed equipment of a template that passed
    /// [`MechTemplate::validate_definition`]: a complete engine, one gyro technology, the
    /// conventional cockpit systems, jump jet groups, heat sink criticals and cargo space.
    pub fn validate_loadout(&self, loadout: &MechLoadout) -> Result<()> {
        super::Engine::resolve(loadout, self.clan_engine())?;
        ensure!(
            ["HDGYRO", "XLGYRO", "CGYRO"]
                .iter()
                .filter(|flag| self.has_special(flag))
                .count()
                <= 1,
            "Conflicting gyro technologies"
        );
        for (system, count) in [
            (
                System::Gyro,
                super::Gyro::from_definition(self).critical_slots(),
            ),
            (System::Cockpit, 1),
            (System::Sensors, 2),
            // A small cockpit fits one life support slot beside its two sensors.
            (
                System::LifeSupport,
                if self.has_technology(super::Technology::SmallCockpit) {
                    1
                } else {
                    2
                },
            ),
        ] {
            ensure!(
                loadout
                    .systems
                    .iter()
                    .filter(|critical| critical.system == system)
                    .count()
                    == count,
                "Incomplete conventional {:?} system",
                system
            );
        }
        if self.has_special("ImprovedJJ_Tech") {
            let jets = loadout.jump_jet_groups(true)?.len();
            ensure!(
                (self.jump_speed - jets as f64 * 10.75).abs() < 0.001,
                "Improved jump jet capacity does not match installed groups"
            );
        }
        if self.has_double_heat_sinks() {
            ensure!(
                self.heat_sinks.is_multiple_of(2),
                "Invalid double heat sink capacity"
            );
        }
        let sinks = loadout
            .systems
            .iter()
            .filter(|critical| critical.system == System::HeatSink)
            .count();
        ensure!(
            if self.has_double_heat_sinks() {
                sinks / self.heat_sink_slots() * 2
            } else {
                sinks
            } <= usize::from(self.heat_sinks),
            "More heat sink criticals than declared heat sinks"
        );
        // A declared cargo installation must weigh out; live mass is measured from the same rule.
        if self
            .attributes
            .get("cargo_space")
            .is_some_and(|value| value != "0")
        {
            super::cargo_space_mass(
                self.attributes.get("cargo_space").map(String::as_str),
                self.has_special("Carrier_Tech"),
                self.has_special("CargoTech"),
            )?;
        }
        Ok(())
    }
}

/// Refuse technology whose rules stompymux does not model, rather than fielding it as something
/// else: reactive armor, compact heat sinks, Inner Sphere laser heat sinks, and null signature
/// systems beside stealth armor.
fn validate_unsupported_technology(definition: &MechTemplate) -> Result<()> {
    ensure!(
        !definition.has_special("ReactiveArmor_Tech"),
        "Reactive armor is unsupported"
    );
    ensure!(
        !definition.has_special("CompactHS"),
        "Compact heat sinks are unsupported"
    );
    ensure!(
        definition.has_special("Clan")
            || !definition.has_technology(super::Technology::LaserHeatSinks),
        "Laser heat sinks are Clan technology"
    );
    let installed = |system| {
        definition
            .sections
            .values()
            .flat_map(|section| section.criticals.values())
            .any(|critical| System::named(&critical.equipment).is_some_and(|s| s == system))
    };
    ensure!(
        !(installed(System::NullSignature) && installed(System::StealthArmor)),
        "A null signature system cannot be combined with stealth armor"
    );
    Ok(())
}
