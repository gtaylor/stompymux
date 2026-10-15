//! Technology base cross-checks shared by Mech and vehicle construction: a unit built from one
//! technology base mounts only that base's weapons and equipment, and builds its engine,
//! structure and armor from it, unless it is a mixed-technology design (TM p. 46; Tactical
//! Operations: Advanced Units & Equipment p. 189).
use super::construction::Component;
use super::{System, Weapon};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A technology base: the Inner Sphere or the Clans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    /// The item-name namespace of this base: `IS` or `CL`, as in `CL.Ecm`.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::InnerSphere => "IS",
            Self::Clan => "CL",
        }
    }

    /// The base an item-name namespace names, and the rest of the name: `CL.Ecm` is a Clan
    /// `Ecm`. `None` for a name without one.
    pub fn split_prefix(name: &str) -> Option<(Self, &str)> {
        [Self::InnerSphere, Self::Clan]
            .into_iter()
            .find_map(|base| {
                let head = name.get(..3)?;
                (head.eq_ignore_ascii_case(&format!("{}.", base.prefix())))
                    .then(|| (base, &name[3..]))
            })
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
    /// under the same critical name. The Bloodhound probe, C3, the null signature system,
    /// Angel ECM, triple strength myomer, stealth armor and light and heavy ferro-fibrous
    /// armor are Inner Sphere only, and so is CASE as an item: Clan CASE is built into every
    /// location of a Clan-structure unit rather than installed (TechManual p. 210). The light
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
            | Self::HeavyFerroFibrous
            | Self::Case => Some(TechBase::InnerSphere),
            Self::LightProbe | Self::LightTag | Self::ArtemisV => Some(TechBase::Clan),
            _ => None,
        }
    }

    /// Systems both technology bases build to their own specifications under one name, so a
    /// slot may state which base built it with an `IS.` or `CL.` prefix: CASE II (1 ton
    /// against half a ton), the Guardian and Clan ECM suites, MASC (5% against 4% of the
    /// tonnage), the targeting computer (a ton per 4 against 5 tons of weapons), Artemis IV,
    /// jump jets, and the Beagle and Clan active probes (TechManual pp. 204-238, 341-343;
    /// Tactical Operations: Advanced Units & Equipment p. 217). An unprefixed slot is built
    /// from the chassis's base.
    pub fn built_by_either_base(self) -> bool {
        matches!(
            self,
            Self::CaseIi
                | Self::Ecm
                | Self::Masc
                | Self::TargetingComputer
                | Self::ArtemisIv
                | Self::JumpJet
                | Self::BeagleProbe
        )
    }

    /// Resolve a slot name to its system and the technology base its prefix states, if any:
    /// `CL.Ecm` is a Clan ECM suite and `Ecm` one of the chassis's base. A prefix is accepted
    /// only on the systems [`System::built_by_either_base`] lists.
    pub fn installation(name: &str) -> Option<(Self, Option<TechBase>)> {
        let Some((base, rest)) = TechBase::split_prefix(name) else {
            return Self::unprefixed(name).map(|system| (system, None));
        };
        Self::unprefixed(rest)
            .filter(|system| system.built_by_either_base())
            .map(|system| (system, Some(base)))
    }

    /// The slot name without its technology prefix: `CL.Ecm` is stocked as an `Ecm`.
    pub fn stock_name(name: &str) -> &str {
        match TechBase::split_prefix(name) {
            Some((_, rest)) if Self::installation(name).is_some() => rest,
            _ => name,
        }
    }
}

/// A system as the technology-base check sees it: what it is and the base its slot states.
pub trait SystemInstallation {
    /// The installed system.
    fn system(&self) -> System;

    /// The base the slot's prefix states, if it states one.
    fn declared_base(&self) -> Option<TechBase>;
}

impl SystemInstallation for System {
    fn system(&self) -> System {
        *self
    }

    fn declared_base(&self) -> Option<TechBase> {
        None
    }
}

impl SystemInstallation for (System, Option<TechBase>) {
    fn system(&self) -> System {
        self.0
    }

    fn declared_base(&self) -> Option<TechBase> {
        self.1
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
/// other base, a system only the other base builds, or a system whose slot names the other
/// base.
pub fn check_technology_base<W, S>(inventory: TechnologyInventory<'_, W, S>) -> Result<()>
where
    W: IntoIterator<Item = Weapon>,
    S: IntoIterator,
    S::Item: SystemInstallation,
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
        // The anti-personnel and anti-battle armor pods are the same items on both technology
        // bases (TM pp. 204, 341, 343), and record sheets name either.
        if matches!(weapon, Weapon::APod | Weapon::ClanAPod) || weapon.is_b_pod() {
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
    for installation in inventory.systems {
        let system = installation.system();
        let declared = installation.declared_base();
        let Some(other) = declared.or(system.technology_base()) else {
            continue;
        };
        let name = match declared {
            Some(declared) => format!("{}.{system:?}", declared.prefix()),
            None => format!("{system:?}"),
        };
        ensure!(
            other == base,
            "{name} is {} technology on {} {unit}; declare mixed_tech in [construction]",
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
            System::Ecm,
            System::BeagleProbe,
            System::Masc,
            System::HeatSink,
        ] {
            assert!(inventory(Clan, false, vec![], vec![], vec![system]).is_ok());
            assert!(inventory(InnerSphere, false, vec![], vec![], vec![system]).is_ok());
        }
        // CASE is an Inner Sphere item; Clan CASE is built into Clan structure (TM p. 210).
        assert!(inventory(Clan, false, vec![], vec![], vec![System::Case]).is_err());
        assert!(inventory(InnerSphere, false, vec![], vec![], vec![System::Case]).is_ok());
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

    /// A slot may state the base of a system both bases build; the stated base must match
    /// the chassis unless the design is mixed technology.
    #[test]
    fn prefixed_systems_state_their_technology_base() {
        assert_eq!(
            System::installation("CL.Ecm"),
            Some((System::Ecm, Some(TechBase::Clan)))
        );
        assert_eq!(
            System::installation("is.TargetingComputer"),
            Some((System::TargetingComputer, Some(TechBase::InnerSphere)))
        );
        assert_eq!(System::installation("Masc"), Some((System::Masc, None)));
        assert_eq!(System::installation("CL.C3Master"), None);
        assert_eq!(System::installation("CL.ERSmallLaser"), None);
        assert_eq!(System::named("CL.JumpJet"), Some(System::JumpJet));
        assert_eq!(System::stock_name("CL.ArtemisIV"), "ArtemisIV");
        assert_eq!(System::stock_name("CL.ERSmallLaser"), "CL.ERSmallLaser");
        let check = |base, mixed, installed: (System, Option<TechBase>)| {
            check_technology_base(TechnologyInventory {
                unit: "Mech",
                base,
                mixed_tech: mixed,
                other_base_components: vec![],
                weapons: Vec::<Weapon>::new(),
                systems: vec![installed],
            })
        };
        let clan_ecm = (System::Ecm, Some(TechBase::Clan));
        let error = check(TechBase::InnerSphere, false, clan_ecm)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("CL.Ecm is Clan technology on an Inner Sphere Mech"),
            "{error}"
        );
        assert!(check(TechBase::InnerSphere, true, clan_ecm).is_ok());
        assert!(check(TechBase::Clan, false, clan_ecm).is_ok());
    }
}
