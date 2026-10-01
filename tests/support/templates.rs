//! Template document edits that scenarios build variants from.
use stompymux_rs::BattleUnitTemplate;

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
    match BattleUnitTemplate::parse("variant", source).unwrap() {
        BattleUnitTemplate::Mech(mut template) => {
            add(&mut template.attributes);
            template.to_document().unwrap()
        }
        BattleUnitTemplate::Vehicle(mut template) => {
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
    match BattleUnitTemplate::parse("variant", source).unwrap() {
        BattleUnitTemplate::Mech(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
        BattleUnitTemplate::Vehicle(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
    }
}
