//! Template document edits that scenarios build variants from.
use stompymux_rs::UnitTemplate;

/// Re-render a template document with extra chassis flags. Construction choices such as
/// `HardenedArmor_Tech` land in `[construction]` and features in `specials`, exactly as a
/// saved unit carrying those flags would be written.
pub fn with_flags(source: &str, flags: &[&str]) -> String {
    let add = |attributes: &mut std::collections::BTreeMap<String, String>| {
        let specials = attributes.entry("specials".into()).or_default();
        for flag in flags.iter().filter(|flag| !flag.is_empty()) {
            if !specials.is_empty() {
                specials.push(' ');
            }
            specials.push_str(flag);
        }
    };
    match UnitTemplate::parse("variant", source).unwrap() {
        UnitTemplate::Mech(mut template) => {
            add(&mut template.attributes);
            template.to_document().unwrap()
        }
        UnitTemplate::Vehicle(mut template) => {
            add(&mut template.attributes);
            template.to_document().unwrap()
        }
    }
}

/// Re-render a template document without the named chassis flags.
pub fn without_flags(source: &str, flags: &[&str]) -> String {
    let remove = |attributes: &mut std::collections::BTreeMap<String, String>| {
        if let Some(specials) = attributes.get_mut("specials") {
            *specials = specials
                .split_ascii_whitespace()
                .filter(|flag| {
                    !flags
                        .iter()
                        .any(|removed| removed.eq_ignore_ascii_case(flag))
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
    };
    match UnitTemplate::parse("variant", source).unwrap() {
        UnitTemplate::Mech(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
        UnitTemplate::Vehicle(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
    }
}

/// Give a mech a small cockpit under `flag` (`SmallCockpit_Tech` or `SMCPIT`), with the head
/// laid out as one: life support, sensors, cockpit and sensors, then the head's other
/// equipment in its original order.
pub fn small_cockpit(template: &mut stompymux_rs::MechTemplate, flag: &str) {
    use stompymux_rs::{CriticalDefinition, MechSection};
    let specials = template.attributes.entry("specials".into()).or_default();
    if !specials.is_empty() {
        specials.push(' ');
    }
    specials.push_str(flag);
    let head = template.sections.get_mut(&MechSection::Head).unwrap();
    let fixed = ["LifeSupport", "Sensors", "Cockpit"];
    let others: Vec<_> = head
        .criticals
        .values()
        .filter(|critical| !fixed.contains(&critical.equipment.as_str()))
        .cloned()
        .collect();
    head.criticals.clear();
    for (slot, item) in ["LifeSupport", "Sensors", "Cockpit", "Sensors"]
        .into_iter()
        .enumerate()
    {
        let critical = CriticalDefinition {
            equipment: item.into(),
            data: "-".into(),
            modes: Vec::new(),
        };
        head.criticals.insert(slot as u8, critical);
    }
    for (offset, critical) in others.into_iter().enumerate() {
        head.criticals.insert(4 + offset as u8, critical);
    }
}

/// The Demolisher rebuilt as a legal hovercraft for chassis-coverage tests: hovercraft weigh
/// at most 50 tons (TechManual p. 96), so it keeps its armor and turret but carries one AC/20
/// with two tons of ammunition, which brings it to exactly 50 tons.
pub fn hovercraft(tracked: &str) -> String {
    let hover = derived_engine_rating(tracked)
        .replace("movement = \"track\"", "movement = \"hover\"")
        .replace("tons = 80", "tons = 50")
        .replace(
            "{ at = \"1-2\", item = \"IS.AC/20\" }",
            "{ at = 1, item = \"IS.AC/20\" }",
        )
        .replace(
            "{ at = \"3-6\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
            "{ at = \"3-4\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
        );
    assert!(
        hover.contains("tons = 50") && hover.contains("{ at = 1, item = \"IS.AC/20\" }"),
        "expected the Demolisher template"
    );
    hover
}

/// Keep a vehicle that a test arms with extra equipment constructible under the TechManual
/// rules, changing as little as it can. It adds a heat sink per point of energy weapon heat
/// (TechManual p. 104); while the vehicle is still illegal, usually over its tonnage
/// (TechManual p. 97), it trims up to two tons of armor, fits a standard fusion engine in
/// place of an ICE, trims armor down to four points a location, empties the turret, slows it
/// to 1 cruising MP and finally raises it to its motive type's maximum tonnage.
pub fn fit_vehicle(template: &mut stompymux_rs::VehicleTemplate) {
    use stompymux_rs::{VehicleLoadout, VehicleSection, VehicleTemplate};
    template.restate_engine_rating();
    mark_vehicle_mixed_technology(template);
    let legal = |template: &VehicleTemplate| {
        stompymux_rs::check_battle_vehicle_template(template).constructible
    };
    let cover = |template: &mut VehicleTemplate| {
        let Ok(loadout) = VehicleLoadout::resolve(template) else {
            return;
        };
        let heat = u16::try_from(VehicleTemplate::energy_heat(&loadout)).unwrap();
        if template.heat_sink_capacity() < heat {
            template.heat_sinks = Some(heat);
        }
    };
    // Remove armor a point at a time from the most heavily armored location other than the
    // front, which tests usually hit, leaving every location at least four points.
    let trim = |template: &mut VehicleTemplate, limit: u32| {
        for _ in 0..limit {
            if legal(template) {
                return;
            }
            let Some(section) = template
                .sections
                .iter_mut()
                .filter(|(section, layout)| **section != VehicleSection::Rotor && layout.armor > 4)
                .max_by_key(|(section, layout)| (**section != VehicleSection::Front, layout.armor))
                .map(|(_, layout)| layout)
            else {
                return;
            };
            section.armor -= 1;
        }
    };
    cover(template);
    trim(template, 32);
    if legal(template) {
        return;
    }
    let specials = template.attributes.entry("specials".into()).or_default();
    let kept: Vec<_> = specials
        .split_ascii_whitespace()
        .filter(|flag| !flag.eq_ignore_ascii_case("ICEEngine_Tech"))
        .map(str::to_owned)
        .collect();
    *specials = if kept.is_empty() {
        "-".into()
    } else {
        kept.join(" ")
    };
    cover(template);
    trim(template, u32::MAX);
    if legal(template) {
        return;
    }
    if let Some(turret) = template.sections.get_mut(&VehicleSection::Turret) {
        turret.criticals.clear();
    }
    if legal(template) {
        return;
    }
    template.set_cruising_mp(1);
    if legal(template) {
        return;
    }
    if let Some(maximum) = template.movement.maximum_tonnage() {
        template.tons = template.tons.max(maximum);
    }
    template.restate_engine_rating();
}

/// [`fit_vehicle`] applied to a template a test is about to construct.
pub fn fitted(mut template: stompymux_rs::VehicleTemplate) -> stompymux_rs::VehicleTemplate {
    fit_vehicle(&mut template);
    template
}

/// [`fit_vehicle`] applied to a vehicle among the templates a test is about to construct;
/// Mechs pass through [`legal_unit`].
pub fn fitted_unit(template: stompymux_rs::UnitTemplate) -> stompymux_rs::UnitTemplate {
    match template {
        stompymux_rs::UnitTemplate::Vehicle(vehicle) => {
            stompymux_rs::UnitTemplate::Vehicle(fitted(vehicle))
        }
        mech => legal_unit(mech),
    }
}

/// The Demolisher with its `movement` replaced; a hovercraft becomes [`hovercraft`], since an
/// 80-ton hovercraft cannot be built (TechManual p. 96).
pub fn with_movement(tracked: &str, movement: &str) -> String {
    if movement == "hover" {
        return hovercraft(tracked);
    }
    derived_engine_rating(tracked).replace(
        "movement = \"track\"",
        &format!("movement = \"{movement}\""),
    )
}

/// The Demolisher template for a [`stompymux_rs::VehicleMovement`], with the same hovercraft
/// substitution as [`with_movement`].
pub fn demolisher(movement: stompymux_rs::VehicleMovement) -> stompymux_rs::VehicleTemplate {
    let source = include_str!("../../game/units/Demolisher.toml");
    let source = if movement == stompymux_rs::VehicleMovement::Hover {
        hovercraft(source)
    } else {
        source.to_owned()
    };
    let mut template = stompymux_rs::VehicleTemplate::parse("Demolisher", &source).unwrap();
    template.movement = movement;
    template.restate_engine_rating();
    template
}

/// A template document without its declared `engine_rating`, so the rating follows whatever
/// tonnage, movement points and motive type a scenario gives the variant.
pub fn derived_engine_rating(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.starts_with("engine_rating = "))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// Bring a scenario's edited Mech back within TechManual construction, so a test that adds or
/// removes equipment to exercise one rule still fields a legal design: the jumping MP follows
/// the installed jets, the declared heat sinks fit the engine and the installed sinks, a
/// targeting computer fits the direct-fire weapons, weapons of the other technology base mark
/// the design mixed technology, armor comes off the most heavily armored locations while the
/// Mech is overweight, and authored cargo space takes up spare tonnage while it is underweight.
pub fn book_legal(template: &mut stompymux_rs::MechTemplate) {
    book_legal_keeping(template, None);
}

/// [`book_legal`], keeping the equipment in `keep` when weapons have to go.
///
/// The balancing runs in rounds. A rating above 500 slows the Mech until it fits; weapons of
/// the other technology base mark it mixed technology; six myomer slots declare triple
/// strength myomer and twelve stealth slots stealth armor; a targeting computer is resized to
/// its weapons; the jumping MP follows the jets, dropping any beyond the walking MP; the
/// declared sinks fit the engine and the installed sinks, adding sink slots when the engine
/// holds too few. While the Mech is then overweight it sheds armor, speed, and the weapons and
/// ammunition outside `keep` other than the heaviest weapon (the one a scenario adds); with
/// `keep` it slows down first and sheds weapons before armor. Armor that turns out not to be
/// needed comes back.
/// Spare tonnage becomes authored cargo space.
pub fn book_legal_keeping(
    template: &mut stompymux_rs::MechTemplate,
    keep: Option<stompymux_rs::MechSection>,
) {
    use stompymux_rs::{MechLoadout, MechTemplate, System};
    template.restate_engine_rating();
    let resolve = |template: &MechTemplate| MechLoadout::resolve(template).ok();
    let Some(loadout) = resolve(template) else {
        return;
    };
    let heaviest = loadout
        .weapons
        .iter()
        .max_by_key(|mount| mount.weapon.mass())
        .map(|mount| mount.weapon.name().to_owned());
    let tons = u32::from(template.tons) * 1024;
    let original_armor: Vec<_> = template
        .sections
        .iter()
        .map(|(&section, layout)| (section, layout.armor))
        .collect();
    while template.walking_mp() > 1 && template.walking_mp() * u32::from(template.tons) > 500 {
        slow_down(template);
    }
    let base = stompymux_rs::TechBase::of(template.has_special("Clan"));
    if loadout
        .weapons
        .iter()
        .any(|mount| mount.weapon.is_clan() != template.has_special("Clan"))
        || loadout.systems.iter().any(|part| {
            part.system
                .technology_base()
                .is_some_and(|other| other != base)
        })
    {
        add_special(template, "MixedTech");
    }
    let myomer = loadout
        .systems
        .iter()
        .filter(|part| part.system == System::TripleStrengthMyomer)
        .count();
    if myomer == 6 {
        add_special(template, "TripleMyomerTech");
    }
    // Twelve stealth armor slots are the stealth armor choice.
    if loadout
        .systems
        .iter()
        .filter(|part| part.system == System::StealthArmor)
        .count()
        == 12
    {
        add_special(template, "StealthArmor_Tech");
    }
    resize_targeting_computer(template, &loadout);
    let mass = |template: &MechTemplate| {
        resolve(template)
            .and_then(|loadout| template.construction_mass(&loadout).ok())
            .map(|mass| mass.total)
            .unwrap_or(tons)
    };
    // A scenario that keeps one location fields its own weapon there: it slows down, then
    // sheds the other weapons, before giving up armor. Otherwise the design keeps its weapons
    // as long as it can.
    let order: [Shed; 3] = if keep.is_some() {
        [Shed::Speed, Shed::Weapon, Shed::Armor]
    } else {
        [Shed::Armor, Shed::Speed, Shed::Weapon]
    };
    'balance: loop {
        fit_jets_and_sinks(template, &resolve);
        if let Some(current) = resolve(template) {
            resize_targeting_computer(template, &current);
        }
        if mass(template) <= tons {
            break;
        }
        for step in order {
            let shed = match step {
                Shed::Armor => shed_armor(template),
                Shed::Speed => {
                    let slower = template.walking_mp() > 1;
                    if slower {
                        slow_down(template);
                    }
                    slower
                }
                Shed::Weapon => shed_weapon(template, keep, heaviest.as_deref()),
            };
            if shed {
                continue 'balance;
            }
        }
        break;
    }
    // Slowing down can free more than the armor needs: give the armor back first.
    for (section, armor) in original_armor {
        while template.sections[&section].armor < armor {
            template.sections.get_mut(&section).unwrap().armor += 1;
            if mass(template) > tons {
                template.sections.get_mut(&section).unwrap().armor -= 1;
                break;
            }
        }
    }
    let spare = tons.saturating_sub(mass(template));
    if spare >= 512 {
        let space: u32 = template
            .attributes
            .get("cargo_space")
            .and_then(|space| space.parse().ok())
            .unwrap_or(0);
        // Cargo space weighs a ton per 500 units, per 100 with CargoTech and per 1000 for a
        // carrier.
        let per_ton = if template.has_special("Carrier_Tech") {
            1000
        } else if template.has_special("CargoTech") {
            100
        } else {
            500
        };
        let added = (spare * per_ton).div_ceil(1024);
        template
            .attributes
            .insert("cargo_space".into(), (space + added).to_string());
        if mass(template) > tons {
            template
                .attributes
                .insert("cargo_space".into(), (space + added - 1).to_string());
        }
    }
}

/// What an overweight design gives up, one step at a time.
#[derive(Clone, Copy)]
enum Shed {
    Armor,
    Speed,
    Weapon,
}

/// Remove one armor point from the most heavily armored location.
fn shed_armor(template: &mut stompymux_rs::MechTemplate) -> bool {
    let Some(layout) = template
        .sections
        .values_mut()
        .filter(|layout| layout.armor + layout.rear > 0)
        .max_by_key(|layout| layout.armor + layout.rear)
    else {
        return false;
    };
    if layout.armor > 0 {
        layout.armor -= 1;
    } else {
        layout.rear -= 1;
    }
    true
}

/// Remove the lightest weapon or ammunition outside `keep`, other than the `heaviest` weapon
/// and its ammunition.
fn shed_weapon(
    template: &mut stompymux_rs::MechTemplate,
    keep: Option<stompymux_rs::MechSection>,
    heaviest: Option<&str>,
) -> bool {
    let kept = |equipment: &str| {
        heaviest
            .is_some_and(|name| equipment == name || equipment.strip_prefix("Ammo_") == Some(name))
    };
    let lightest = template
        .sections
        .iter()
        .filter(|(section, _)| Some(**section) != keep)
        .flat_map(|(&section, layout)| {
            layout
                .criticals
                .values()
                .filter(|critical| {
                    (critical.equipment.starts_with("IS.")
                        || critical.equipment.starts_with("CL.")
                        || critical.equipment.starts_with("Ammo_"))
                        && !kept(&critical.equipment)
                })
                .map(move |critical| (section, critical.equipment.clone()))
        })
        .min_by_key(|(_, equipment)| {
            stompymux_rs::Weapon::parse(equipment.trim_start_matches("Ammo_"))
                .map(|weapon| weapon.mass())
                .unwrap_or(0)
        });
    let Some((section, equipment)) = lightest else {
        return false;
    };
    template
        .sections
        .get_mut(&section)
        .unwrap()
        .criticals
        .retain(|_, critical| critical.equipment != equipment);
    true
}

/// Add a chassis flag unless the template already has it.
fn add_special(template: &mut stompymux_rs::MechTemplate, flag: &str) {
    if template.has_special(flag) {
        return;
    }
    let specials = template.attributes.entry("specials".into()).or_default();
    if !specials.is_empty() {
        specials.push(' ');
    }
    specials.push_str(flag);
}

/// Lower the walking MP by one, with the running speed that goes with it.
fn slow_down(template: &mut stompymux_rs::MechTemplate) {
    template.set_walking_mp(template.walk_mp - 1);
}

/// One empty critical slot: the critical definition construction expects there.
fn critical(equipment: &str) -> stompymux_rs::CriticalDefinition {
    stompymux_rs::CriticalDefinition {
        equipment: equipment.into(),
        data: "-".into(),
        modes: Vec::new(),
    }
}

/// The first run of `size` free slots, searching the sections in order.
fn free_run(
    template: &stompymux_rs::MechTemplate,
    size: u8,
    allowed: impl Fn(stompymux_rs::MechSection) -> bool,
) -> Option<(stompymux_rs::MechSection, u8)> {
    let chassis = template.chassis().ok()?;
    template.sections.iter().find_map(|(&section, layout)| {
        if !allowed(section) {
            return None;
        }
        let capacity = chassis.critical_slots(section);
        (0..capacity)
            .find(|&first| {
                (first..first + size)
                    .all(|slot| slot < capacity && !layout.criticals.contains_key(&slot))
            })
            .map(|first| (section, first))
    })
}

/// Give a targeting computer as many contiguous slots as its direct-fire weapons need.
fn resize_targeting_computer(
    template: &mut stompymux_rs::MechTemplate,
    loadout: &stompymux_rs::MechLoadout,
) {
    let Some(first) = loadout
        .systems
        .iter()
        .find(|part| part.system == stompymux_rs::System::TargetingComputer)
        .map(|part| part.location)
    else {
        return;
    };
    let weapons: u32 = loadout
        .weapons
        .iter()
        .filter(|mount| mount.weapon.is_direct_fire())
        .map(|mount| mount.weapon.mass())
        .sum();
    let divisor = if template.has_special("Clan") { 5 } else { 4 };
    let needed = weapons.div_ceil(1024 * divisor).max(1) as u8;
    for layout in template.sections.values_mut() {
        layout
            .criticals
            .retain(|_, critical| critical.equipment != "TargetingComputer");
    }
    let stays = (first.slot..first.slot + needed).all(|slot| {
        slot < 12
            && !template.sections[&first.section]
                .criticals
                .contains_key(&slot)
    });
    let (section, start) = if stays {
        (first.section, first.slot)
    } else {
        free_run(template, needed, |section| section == first.section)
            .or_else(|| free_run(template, needed, |_| true))
            .unwrap_or((first.section, first.slot))
    };
    for slot in start..start + needed {
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(slot, critical("TargetingComputer"));
    }
}

/// Match the jumping MP to the installed jets, dropping jets beyond the walking MP, and fit
/// the declared heat sinks to the engine and the installed sinks.
fn fit_jets_and_sinks(
    template: &mut stompymux_rs::MechTemplate,
    resolve: &impl Fn(&stompymux_rs::MechTemplate) -> Option<stompymux_rs::MechLoadout>,
) {
    let walking = template.walking_mp();
    let improved = template.has_special("ImprovedJJ_Tech");
    let Some(loadout) = resolve(template) else {
        return;
    };
    if let Ok(jets) = loadout.jump_jet_groups(improved) {
        let ceiling = if improved {
            template.running_mp()
        } else {
            walking
        } as usize;
        for group in jets.iter().skip(ceiling) {
            for location in group {
                template
                    .sections
                    .get_mut(&location.section)
                    .unwrap()
                    .criticals
                    .remove(&location.slot);
            }
        }
        template.jump_speed = jets.len().min(ceiling) as f64 * 10.75;
    }
    let Ok(groups) = loadout.heat_sink_groups(template.heat_sink_slots()) else {
        return;
    };
    let capacity = (walking * u32::from(template.tons) / 25) as u16;
    let mut external = groups.len() as u16;
    // An engine too small for ten sinks needs the rest in critical slots.
    let slots = template.heat_sink_slots() as u8;
    while external + capacity < 10 {
        let Some((section, first)) = free_run(template, slots, |_| true) else {
            break;
        };
        for slot in first..first + slots {
            template
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(slot, critical("HeatSink"));
        }
        external += 1;
    }
    template.heat_sinks = template
        .heat_sinks
        .max(10)
        .max(external)
        .min(external + capacity);
}

/// [`book_legal`] for a template document.
pub fn book_legal_source(source: &str) -> String {
    let mut template = stompymux_rs::MechTemplate::parse("variant", source).unwrap();
    book_legal(&mut template);
    template.to_document().unwrap()
}

/// [`book_legal`] for the definition inside a saved unit record, lowering live armor the
/// balancing removed; records of other unit classes are left alone.
pub fn book_legal_record(state: &mut serde_json::Value) {
    book_legal_record_keeping(state, None);
}

/// [`book_legal_record`] with [`book_legal_keeping`]'s `keep`, so weapons outside it go before
/// any armor does.
pub fn book_legal_record_keeping(
    state: &mut serde_json::Value,
    keep: Option<stompymux_rs::MechSection>,
) {
    let Ok(mut definition) =
        serde_json::from_value::<stompymux_rs::MechTemplate>(state["definition"].clone())
    else {
        return;
    };
    book_legal_keeping(&mut definition, keep);
    set_record_definition(state, definition);
}

/// [`book_legal`] by value, for passing an edited design straight on.
pub fn legal(mut template: stompymux_rs::MechTemplate) -> stompymux_rs::MechTemplate {
    book_legal(&mut template);
    template
}

/// [`book_legal`] for a parsed unit of either class; a vehicle only has a declared engine
/// rating brought in line with its tonnage, movement and motive type.
pub fn legal_unit(template: stompymux_rs::UnitTemplate) -> stompymux_rs::UnitTemplate {
    match template {
        stompymux_rs::UnitTemplate::Mech(template) => {
            stompymux_rs::UnitTemplate::Mech(legal(template))
        }
        stompymux_rs::UnitTemplate::Vehicle(mut vehicle) => {
            vehicle.restate_engine_rating();
            mark_vehicle_mixed_technology(&mut vehicle);
            stompymux_rs::UnitTemplate::Vehicle(vehicle)
        }
    }
}

/// Declare mixed technology on a vehicle a scenario arms with weapons or equipment of the
/// other technology base, as the Mech balancing in [`book_legal`] does (TechManual p. 46).
pub fn mark_vehicle_mixed_technology(template: &mut stompymux_rs::VehicleTemplate) {
    let Ok(loadout) = stompymux_rs::VehicleLoadout::resolve(template) else {
        return;
    };
    let clan = template.has_special("Clan");
    let base = stompymux_rs::TechBase::of(clan);
    let mixed = loadout
        .weapons
        .iter()
        .any(|mount| mount.weapon.is_clan() != clan)
        || loadout.systems.iter().any(|part| {
            part.system
                .technology_base()
                .is_some_and(|other| other != base)
        });
    if mixed && !template.has_special("MixedTech") {
        let specials = template.attributes.entry("specials".into()).or_default();
        if specials == "-" {
            specials.clear();
        }
        if !specials.is_empty() {
            specials.push(' ');
        }
        specials.push_str("MixedTech");
    }
}

/// Store an edited Mech design in a saved unit record after [`book_legal`], lowering the live
/// armor of any location whose armor the balancing removed.
pub fn set_record_definition(
    record: &mut serde_json::Value,
    definition: stompymux_rs::MechTemplate,
) {
    let definition = legal(definition);
    for (section, layout) in &definition.sections {
        let key = serde_json::to_value(section).unwrap();
        let Some(live) = record["sections"].get_mut(key.as_str().unwrap()) else {
            continue;
        };
        for (field, limit) in [("armor", layout.armor), ("rear", layout.rear)] {
            if live[field]
                .as_u64()
                .is_some_and(|value| value > u64::from(limit))
            {
                live[field] = limit.into();
            }
        }
    }
    record["definition"] = serde_json::to_value(definition).unwrap();
}

/// Install `count` jump jets in vacant torso and leg slots, the only locations jets may occupy
/// (TechManual p. 51); [`book_legal`] then sets the jumping MP from them.
pub fn install_jump_jets(template: &mut stompymux_rs::MechTemplate, count: usize) {
    use stompymux_rs::MechSection;
    let chassis = template.chassis().unwrap();
    let mut installed = 0;
    for section in MechSection::ALL {
        let torso = matches!(
            section,
            MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
        );
        if !torso && !chassis.is_leg(section) {
            continue;
        }
        for slot in 0..chassis.critical_slots(section) {
            if installed == count {
                return;
            }
            let criticals = &mut template.sections.get_mut(&section).unwrap().criticals;
            if criticals.contains_key(&slot) {
                continue;
            }
            criticals.insert(slot, critical("JumpJet"));
            installed += 1;
        }
    }
    assert_eq!(installed, count, "no room for {count} jump jets");
}

/// Give a biped's arms the four leg actuators a quad's front legs need (TechManual p. 45),
/// moving the arm's other equipment, in order, into the slots after them. Scenarios that turn
/// a biped into a quad start from this.
pub fn quad_ready_arms(template: &mut stompymux_rs::MechTemplate) {
    use stompymux_rs::{CriticalDefinition, MechSection};
    const ACTUATORS: [&str; 4] = [
        "ShoulderOrHip",
        "UpperActuator",
        "LowerActuator",
        "HandOrFootActuator",
    ];
    for section in [MechSection::LeftArm, MechSection::RightArm] {
        let Some(layout) = template.sections.get_mut(&section) else {
            continue;
        };
        let equipment: Vec<CriticalDefinition> = std::mem::take(&mut layout.criticals)
            .into_values()
            .filter(|critical| !ACTUATORS.contains(&critical.equipment.as_str()))
            .collect();
        for (slot, actuator) in ACTUATORS.into_iter().enumerate() {
            layout.criticals.insert(
                slot as u8,
                CriticalDefinition {
                    equipment: actuator.into(),
                    data: "-".into(),
                    modes: Vec::new(),
                },
            );
        }
        for (offset, critical) in equipment.into_iter().enumerate() {
            layout.criticals.insert(4 + offset as u8, critical);
        }
    }
}

/// [`quad_ready_arms`] applied to a saved unit record's design.
pub fn quad_ready_record(record: &mut serde_json::Value) {
    let mut definition =
        serde_json::from_value::<stompymux_rs::MechTemplate>(record["definition"].clone()).unwrap();
    quad_ready_arms(&mut definition);
    set_record_definition(record, definition);
}
