//! Contact-list inclusion policy applied only after live acquired visibility is established.
use crate::{ObjectId, World};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Saved building inclusion; follow-brief uses the caller's current display policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleBuildingContactMode {
    FollowBrief,
    Include,
    #[default]
    Exclude,
}

impl BattleBuildingContactMode {
    /// Resolve this preference against whether the current brief mode includes structures.
    pub fn includes(self, brief_buildings: bool) -> bool {
        match self {
            Self::FollowBrief => brief_buildings,
            Self::Include => true,
            Self::Exclude => false,
        }
    }
}

/// Saved contact-list categories; selected targets may bypass categories but never visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BattleContactPreferences {
    pub include_dead: bool,
    pub include_shutdown: bool,
    pub include_enemies: bool,
    pub include_allies: bool,
    pub include_target: bool,
    pub buildings: BattleBuildingContactMode,
}

impl Default for BattleContactPreferences {
    fn default() -> Self {
        Self {
            include_dead: false,
            include_shutdown: true,
            include_enemies: true,
            include_allies: true,
            include_target: true,
            buildings: BattleBuildingContactMode::Exclude,
        }
    }
}

/// Read saved contact inclusion settings for a live player.
pub fn contact_preferences(world: &World, player: ObjectId) -> Result<BattleContactPreferences> {
    super::view_dimensions(world, player)?;
    Ok(world
        .btech
        .player_preferences
        .get(&player)
        .copied()
        .unwrap_or_default()
        .contacts)
}

/// Replace contact inclusion settings; trusted host code owns authorization to edit the player.
pub fn set_contact_preferences(
    world: &mut World,
    player: ObjectId,
    preferences: BattleContactPreferences,
) -> Result<()> {
    super::view_dimensions(world, player)?;
    Arc::make_mut(&mut world.btech.player_preferences)
        .entry(player)
        .or_default()
        .contacts = preferences;
    Ok(())
}

/// Filter an already eligible list without acquiring contacts, reading hidden identities or using dice.
pub fn filtered_contacts(
    world: &World,
    observer: ObjectId,
    preferences: BattleContactPreferences,
) -> Result<Vec<super::BattleContactView>> {
    filtered_for_source(world, observer.into(), preferences)
}

/// Selected-target inclusion follows the operator while visibility follows the physical unit.
pub(super) fn filtered_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    preferences: BattleContactPreferences,
) -> Result<Vec<super::BattleContactView>> {
    let contacts = super::visible_contacts(world, source.unit)?;
    let selected = source
        .selection(world)
        .and_then(|selection| match selection {
            super::BattleTargetSelection::Unit(lock) => Some(lock.target),
            super::BattleTargetSelection::Hex(_) => None,
        });
    Ok(contacts
        .into_iter()
        .filter(|contact| {
            if preferences.include_target && selected == Some(contact.target) {
                return true;
            }
            if (contact.friendly && !preferences.include_allies)
                || (!contact.friendly && !preferences.include_enemies)
            {
                return false;
            }
            let unit = super::scanner::scanner_unit(world, contact.target).expect("visible target");
            if unit.destroyed {
                return preferences.include_dead;
            }
            preferences.include_shutdown || unit.power == super::BattlePower::Running
        })
        .collect())
}

/// Decoded per-call unit options and diagnostics; these never update saved preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleContactOptions {
    pub buildings: bool,
    pub preferences: BattleContactPreferences,
    pub ignored: Vec<char>,
}

/// Decode up to fifty option characters. `!` resets all unit categories on and
/// makes following category letters exclusions until another reset; unknowns do not change mode.
pub fn parse_contact_options(options: &str) -> Result<BattleContactOptions> {
    parse_contact_options_for_display(options, false)
}

/// Decode explicit options with the unit's initial building policy; ! still clears buildings.
pub fn parse_contact_options_for_display(
    options: &str,
    brief_buildings: bool,
) -> Result<BattleContactOptions> {
    anyhow::ensure!(
        !options.chars().any(char::is_whitespace),
        "Contact options must be one word"
    );
    let mut preferences = BattleContactPreferences {
        include_dead: false,
        include_shutdown: false,
        include_enemies: false,
        include_allies: false,
        include_target: false,
        buildings: BattleBuildingContactMode::Exclude,
    };
    let mut buildings = brief_buildings;
    let mut excluded = false;
    let mut ignored = Vec::new();
    for option in options.chars().take(50) {
        if option == '!' {
            preferences = BattleContactPreferences {
                include_dead: true,
                ..Default::default()
            };
            buildings = false;
            excluded = true;
            continue;
        }
        let category = match option {
            'd' => &mut preferences.include_dead,
            's' => &mut preferences.include_shutdown,
            'e' => &mut preferences.include_enemies,
            'a' => &mut preferences.include_allies,
            't' => &mut preferences.include_target,
            'b' => &mut buildings,
            other => {
                ignored.push(other);
                continue;
            }
        };
        *category = !excluded;
    }
    Ok(BattleContactOptions {
        buildings,
        preferences,
        ignored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn building_policy_resolves_brief_and_explicit_modes() {
        for brief in [false, true] {
            assert_eq!(
                BattleBuildingContactMode::FollowBrief.includes(brief),
                brief
            );
            assert!(BattleBuildingContactMode::Include.includes(brief));
            assert!(!BattleBuildingContactMode::Exclude.includes(brief));
        }
    }

    #[test]
    fn options_preserve_exclusion_mode_resets_case_and_length_bound() {
        let options = parse_contact_options("as").unwrap();
        assert!(options.preferences.include_allies && options.preferences.include_shutdown);
        assert!(!options.preferences.include_enemies && !options.preferences.include_target);
        let options = parse_contact_options("!aZdse").unwrap();
        assert!(
            !options.preferences.include_allies
                && !options.preferences.include_dead
                && !options.preferences.include_shutdown
                && !options.preferences.include_enemies
        );
        assert!(options.preferences.include_target);
        assert_eq!(options.ignored, vec!['Z']);
        let options = parse_contact_options("!a!s").unwrap();
        assert!(options.preferences.include_allies && options.preferences.include_dead);
        assert!(!options.preferences.include_shutdown);
        assert!(
            !parse_contact_options(&format!("{}a", "z".repeat(50)))
                .unwrap()
                .preferences
                .include_allies
        );
        assert!(parse_contact_options("a s").is_err());
        assert!(parse_contact_options("b").unwrap().buildings);
        assert!(!parse_contact_options("!b").unwrap().buildings);
    }
}
