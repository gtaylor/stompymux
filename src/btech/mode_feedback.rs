//! Cockpit feedback after a weapon mode or ammunition selection, shared by native commands
//! and Lua. The modes themselves live in `stompymux-template`; their wording lives here.
use super::{BattleAmmunitionMode, BattleFireMode};

/// Cockpit wording for a numbered weapon after its ammunition selection changes.
pub trait AmmunitionFeedback {
    /// Artemis command feedback distinguishes compatible missiles from ordinary ammunition.
    fn artemis_message(self, index: usize) -> String;

    /// Shared mode-switch feedback for native and Lua callers.
    fn message(self, index: usize) -> String;

    /// Artillery cluster feedback retains its distinct normal-round wording.
    fn cluster_message(self, index: usize) -> String;

    /// Cockpit feedback shared by native commands and Lua.
    fn stinger_message(self, index: usize) -> String;

    /// Cockpit wording for the selected ATM ammunition.
    fn atm_message(self, index: usize) -> String;

    /// Cockpit feedback shared by native commands and Lua.
    fn thunder_message(self, index: usize) -> String;

    /// Both host interfaces use the same cockpit message after a successful selection.
    fn special_round_message(self, index: usize) -> String;

    /// Shared cockpit feedback identifies the selected missile family explicitly.
    fn mml_message(self, index: usize) -> String;

    /// Shared cockpit feedback for normal and AP rounds.
    fn armor_piercing_message(self, index: usize) -> String;

    /// Shared cockpit feedback for normal and Precision rounds.
    fn precision_message(self, index: usize) -> String;

    /// Cockpit feedback shared by native and Lua controls.
    fn inferno_message(self, index: usize) -> String;

    /// Shared cockpit feedback for normal and CASELESS rounds.
    fn caseless_message(self, index: usize) -> String;

    /// Shared cockpit feedback for normal and Incendiary rounds.
    fn incendiary_message(self, index: usize) -> String;

    /// Shared cockpit feedback for normal and Flechette rounds.
    fn flechette_message(self, index: usize) -> String;

    /// Shared native/Lua cockpit feedback.
    fn semiguided_message(self, index: usize) -> String;

    /// Shared native and Lua ammunition selection feedback.
    fn swarm_message(self, index: usize) -> String;
}

impl AmmunitionFeedback for BattleAmmunitionMode {
    fn artemis_message(self, index: usize) -> String {
        if self.munition() == Self::Artemis {
            return self.message(index);
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }

    fn message(self, index: usize) -> String {
        if self.munition() == Self::Artemis {
            return format!("Weapon {index} has been set to fire Artemis IV compatible missiles.");
        }
        format!(
            "Weapon {index} has been set to {} fire mode",
            if self == Self::Cluster {
                "LBX"
            } else {
                "normal"
            }
        )
    }

    fn cluster_message(self, index: usize) -> String {
        if self == Self::Cluster {
            return format!("Weapon {index} has been set to fire cluster rounds.");
        }
        format!("Weapon {index} has been set to fire normal rounds")
    }

    fn stinger_message(self, index: usize) -> String {
        if self.munition() == Self::Stinger {
            return format!("Weapon {index} has been set to fire stinger missiles.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }

    fn atm_message(self, index: usize) -> String {
        let label = match self {
            Self::ExtendedRange => "Extended Range",
            Self::HighExplosive => "High Explosive",
            _ => "normal",
        };
        format!("Weapon {index} has been set to fire {label} missiles.")
    }

    fn thunder_message(self, index: usize) -> String {
        let name = match self.munition() {
            Self::ThunderAugmented => "Thunder-Augmented",
            Self::ThunderVibrabomb => "Thunder-Vibrabomb",
            Self::ThunderActive => "Thunder-Active",
            _ => return format!("Weapon {index} has been set to fire normal missiles"),
        };
        format!("Weapon {index} has been set to fire {name} missiles.")
    }

    fn special_round_message(self, index: usize) -> String {
        let round = match self {
            Self::Smoke => "smoke",
            Self::Mine => "mine",
            _ => {
                return format!("Weapon {index} has been set to fire normal rounds");
            }
        };
        format!("Weapon {index} has been set to fire {round} rounds.")
    }

    fn mml_message(self, index: usize) -> String {
        let family = if self.is_mml_lrm() { "LRM" } else { "SRM" };
        format!("Weapon {index} has been set to fire {family} missiles.")
    }

    fn armor_piercing_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::ArmorPiercing {
                "AP"
            } else {
                "normal"
            }
        )
    }

    fn precision_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Precision {
                "Precision"
            } else {
                "normal"
            }
        )
    }

    fn inferno_message(self, index: usize) -> String {
        if self == Self::Inferno {
            return format!("Weapon {index} has been set to fire Inferno missiles.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }

    fn caseless_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Caseless {
                "CASELESS"
            } else {
                "normal"
            }
        )
    }

    fn incendiary_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Incendiary {
                "Incendiary"
            } else {
                "normal"
            }
        )
    }

    fn flechette_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Flechette {
                "Flechette"
            } else {
                "normal"
            }
        )
    }

    fn semiguided_message(self, index: usize) -> String {
        if self.munition() == Self::SemiGuided {
            return format!("Weapon {index} has been set to fire Sguided missiles.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }

    fn swarm_message(self, index: usize) -> String {
        let name = match self.munition() {
            Self::Swarm => "Swarm",
            Self::Swarm1 => "Swarm1",
            _ => {
                return format!("Weapon {index} has been set to fire normal missiles");
            }
        };
        format!("Weapon {index} has been set to fire {name} missiles.")
    }
}

/// Cockpit wording for a numbered weapon after its firing mode changes.
pub trait FireModeFeedback {
    /// Shared cockpit feedback for native and Lua mode controls.
    fn rapid_message(self, index: usize) -> String;

    /// Shared cockpit feedback for native and Lua mode controls.
    fn gatling_message(self, index: usize) -> String;

    /// Shared cockpit feedback for native and Lua mode controls.
    fn ultra_message(self, index: usize) -> String;

    /// Hotload command feedback for both control surfaces.
    fn hotload_message(self, index: usize) -> String;

    /// Shared native/Lua cockpit feedback for a numbered weapon.
    fn message(self, index: usize) -> String;
}

impl FireModeFeedback for BattleFireMode {
    fn rapid_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} mode",
            if self == Self::Rapid {
                "Rapid Fire"
            } else {
                "normal fire"
            }
        )
    }

    fn gatling_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} mode",
            if self == Self::Gatling {
                "Gattling"
            } else {
                "normal fire"
            }
        )
    }

    fn ultra_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} fire mode",
            if self == Self::Ultra {
                "ultra"
            } else {
                "normal"
            }
        )
    }

    fn hotload_message(self, index: usize) -> String {
        format!(
            "Hotloading for weapon {index} has been toggled {}.",
            if self == Self::Hotload { "on" } else { "off" }
        )
    }

    fn message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} mode",
            if self == Self::Heat { "HEAT" } else { "normal" }
        )
    }
}
