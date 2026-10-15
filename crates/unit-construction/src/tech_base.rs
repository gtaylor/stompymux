//! Technology base cross-checks shared by Mech and vehicle construction: a unit built from one
//! technology base mounts only that base's weapons and equipment, and builds its engine,
//! structure and armor from it, unless it is a mixed-technology design (TM p. 46; Tactical
//! Operations: Advanced Units & Equipment p. 189).
use super::construction::Component;
use super::{System, Weapon};
use anyhow::{Result, ensure};

/// A technology base: the Inner Sphere or the Clans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBase {
    InnerSphere,
    Clan,
}

impl TechBase {
    /// The base a `clan` flag names.
    pub fn of(clan: bool) -> Self {
        if clan { Self::Clan } else { Self::InnerSphere }
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::InnerSphere => "Inner Sphere",
            Self::Clan => "Clan",
        }
    }

    /// The name with its indefinite article: `an Inner Sphere` or `a Clan`.
    fn with_article(self) -> &'static str {
        match self {
            Self::InnerSphere => "an Inner Sphere",
            Self::Clan => "a Clan",
        }
    }
}

impl System {
    /// The one technology base that builds this system, or `None` when both bases build it
    /// under the same critical name (heat sinks, jump jets, CASE, ECM, the active probe, MASC,
    /// targeting computers, Artemis IV and the like, whose base is the chassis's). The
    /// Bloodhound probe, C3, the null signature system, Angel ECM, triple strength myomer,
    /// stealth armor and light and heavy ferro-fibrous armor are Inner Sphere only; the light
    /// active probe, light TAG and Artemis V are Clan only (TechManual pp. 204-245; Tactical
    /// Operations: Advanced Units & Equipment).
    pub fn technology_base(self) -> Option<TechBase> {
        match self {
            Self::BloodhoundProbe
            | Self::C3Master
            | Self::C3Slave
            | Self::C3i
            | Self::NullSignature
            | Self::AngelEcm
            | Self::TripleStrengthMyomer
            | Self::StealthArmor
            | Self::LightFerroFibrous
            | Self::HeavyFerroFibrous => Some(TechBase::InnerSphere),
            Self::LightProbe | Self::LightTag | Self::ArtemisV => Some(TechBase::Clan),
            _ => None,
        }
    }
}

impl Component {
    /// Display name.
    fn label(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Structure => "internal structure",
            Self::Armor => "armor",
        }
    }
}

/// What a unit is built from, for [`check_technology_base`].
pub struct TechnologyInventory<'a, W, S> {
    /// `Mech` or `vehicle`, for messages.
    pub unit: &'a str,
    pub base: TechBase,
    /// The design declares mixed technology.
    pub mixed_tech: bool,
    /// Components built from the other technology base.
    pub other_base_components: Vec<Component>,
    pub weapons: W,
    pub systems: S,
}

/// Refuse anything from the other technology base on a design that does not declare mixed
/// technology: an engine, structure or armor of the other base (TM p. 46), a weapon of the
/// other base, or a system only the other base builds.
pub fn check_technology_base<W, S>(inventory: TechnologyInventory<'_, W, S>) -> Result<()>
where
    W: IntoIterator<Item = Weapon>,
    S: IntoIterator<Item = System>,
{
    if inventory.mixed_tech {
        return Ok(());
    }
    let base = inventory.base;
    let unit = inventory.unit;
    if let Some(component) = inventory.other_base_components.first() {
        anyhow::bail!(
            "The {} is not {} technology like the chassis; only a mixed_tech design builds components from both bases (TechManual p. 46)",
            component.label(),
            base.name()
        );
    }
    for weapon in inventory.weapons {
        // The anti-personnel pod is the same item on both technology bases (TM pp. 341, 343),
        // and record sheets name either.
        if matches!(weapon, Weapon::APod | Weapon::ClanAPod) {
            continue;
        }
        let other = TechBase::of(weapon.is_clan());
        ensure!(
            other == base,
            "{} is {} technology on {} {unit}; declare mixed_tech in [construction]",
            weapon.name(),
            other.name(),
            base.with_article()
        );
    }
    for system in inventory.systems {
        let Some(other) = system.technology_base() else {
            continue;
        };
        ensure!(
            other == base,
            "{system:?} is {} technology on {} {unit}; declare mixed_tech in [construction]",
            other.name(),
            base.with_article()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory(
        base: TechBase,
        mixed_tech: bool,
        components: Vec<Component>,
        weapons: Vec<Weapon>,
        systems: Vec<System>,
    ) -> Result<()> {
        check_technology_base(TechnologyInventory {
            unit: "Mech",
            base,
            mixed_tech,
            other_base_components: components,
            weapons,
            systems,
        })
    }

    /// Weapons, base-specific systems and components follow the chassis unless the design is
    /// mixed technology; systems both bases build pass on either.
    #[test]
    fn the_chassis_base_governs_unless_mixed() {
        use TechBase::*;
        assert!(
            inventory(
                InnerSphere,
                false,
                vec![],
                vec![Weapon::MediumLaser],
                vec![]
            )
            .is_ok()
        );
        let error = inventory(Clan, false, vec![], vec![Weapon::MediumLaser], vec![])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("IS.MediumLaser is Inner Sphere technology on a Clan Mech"),
            "{error}"
        );
        let error = inventory(Clan, false, vec![], vec![], vec![System::C3Master])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("C3Master is Inner Sphere technology"),
            "{error}"
        );
        assert!(inventory(InnerSphere, false, vec![], vec![], vec![System::LightTag]).is_err());
        assert!(inventory(Clan, false, vec![], vec![], vec![System::LightTag]).is_ok());
        for system in [
            System::Case,
            System::Ecm,
            System::BeagleProbe,
            System::Masc,
            System::HeatSink,
        ] {
            assert!(inventory(Clan, false, vec![], vec![], vec![system]).is_ok());
            assert!(inventory(InnerSphere, false, vec![], vec![], vec![system]).is_ok());
        }
        let error = inventory(InnerSphere, false, vec![Component::Engine], vec![], vec![])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("The engine is not Inner Sphere technology"),
            "{error}"
        );
        assert!(
            inventory(
                Clan,
                true,
                vec![Component::Armor],
                vec![Weapon::MediumLaser],
                vec![System::C3Master]
            )
            .is_ok()
        );
        assert!(inventory(Clan, false, vec![], vec![Weapon::APod], vec![]).is_ok());
    }
}
