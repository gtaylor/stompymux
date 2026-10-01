//! Temporary probe: remaining regularity facts for derived template data.
use std::collections::BTreeMap;
use stompymux_rs::{BattleSection, BattleTemplate, RawTemplate, RawUnitClass};

fn flag(specials: Option<&String>, name: &str) -> bool {
    specials.is_some_and(|v| v.split_whitespace().any(|f| f.eq_ignore_ascii_case(name)))
}

fn main() {
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut bump = |key: String| *tally.entry(key).or_default() += 1;
    for entry in std::fs::read_dir("game/mechs").unwrap() {
        let path = entry.unwrap().path();
        let reference = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(raw) = RawTemplate::parse(&reference, &source) else {
            continue;
        };
        let class = format!("{:?}", raw.class);
        let integral = |speed: f64| ((speed / 10.75) - (speed / 10.75).round()).abs() < 1e-9;
        bump(format!(
            "{class} speed integral={}",
            integral(raw.max_speed)
        ));
        if !integral(raw.jump_speed) {
            println!("jump exception {reference} {}", raw.jump_speed);
        }
        let specials = raw.attributes.get("specials");
        if raw.class == RawUnitClass::Vehicle || raw.class == RawUnitClass::Vtol {
            let expected = ((raw.tons + 5).max(10) / 10) as u16;
            for (code, section) in &raw.sections {
                bump(format!(
                    "vehicle internals {}",
                    if section.internal == 0 {
                        "zero".into()
                    } else if section.internal == expected {
                        "formula".into()
                    } else {
                        format!("other({code:?})")
                    }
                ));
            }
        }
        if raw.class != RawUnitClass::Mech {
            continue;
        }
        let Ok(t) = BattleTemplate::parse(&reference, &source) else {
            bump("mech unloadable".into());
            continue;
        };
        let raw_internals: Vec<u16> = raw.sections.values().map(|s| s.internal).collect();
        let loaded: u32 = t.sections.values().map(|s| u32::from(s.internal)).sum();
        bump(format!(
            "mech authored internals match chart={}",
            raw_internals.iter().map(|v| u32::from(*v)).sum::<u32>() == loaded
        ));
        use BattleSection::*;
        for side in [LeftTorso, RightTorso] {
            let slots: Vec<u8> = t.sections[&side]
                .criticals
                .iter()
                .filter(|(_, c)| c.equipment == "Engine")
                .map(|(s, _)| *s + 1)
                .collect();
            if !slots.is_empty() {
                bump(format!("side engine slots {slots:?}"));
            }
        }
        let arms_bare = [LeftArm, RightArm].iter().all(|arm| {
            !t.sections[arm]
                .criticals
                .values()
                .any(|c| c.equipment == "LowerActuator" || c.equipment == "HandOrFootActuator")
        });
        bump(format!(
            "fliparms flag={} layout={arms_bare} quad={}",
            flag(specials, "FlipArms"),
            raw.movement == stompymux_rs::RawMovement::Quad
        ));
        bump(format!(
            "clan={} doublehs={}",
            flag(specials, "Clan"),
            flag(specials, "DoubleHS")
        ));
        bump(format!("ice={}", flag(specials, "ICEEngine_Tech")));
        let fixed = [
            "ShoulderOrHip",
            "UpperActuator",
            "LowerActuator",
            "HandOrFootActuator",
            "Engine",
            "Gyro",
            "Cockpit",
            "LifeSupport",
            "Sensors",
        ];
        let mut brands = BTreeMap::new();
        for (section, layout) in &t.sections {
            for c in layout.criticals.values() {
                if fixed.contains(&c.equipment.as_str()) {
                    brands
                        .entry(c.brand)
                        .or_insert_with(Vec::new)
                        .push(format!("{section:?}:{}", c.equipment));
                }
            }
        }
        if brands.len() > 1 {
            println!(
                "mixed brands {reference}: {:?}",
                brands
                    .iter()
                    .map(|(b, v)| (b, v.len(), v.iter().take(3).collect::<Vec<_>>()))
                    .collect::<Vec<_>>()
            );
        }
    }
    for (key, n) in tally {
        println!("{n:5} {key}");
    }
}
