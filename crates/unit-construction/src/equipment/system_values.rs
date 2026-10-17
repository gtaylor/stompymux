//! Book Battle Value, installation size and construction price of slot-mounted systems:
//! electronics, fire control, CASE and physical weapons. Weapons carry the same facts in the
//! weapon catalogue.
use super::System;

/// Chassis facts that size or price a system: technology base, the Watchdog flag that turns
/// ECM slots into a Watchdog CEWS, tonnage, engine rating and legs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemContext {
    pub clan: bool,
    pub watchdog: bool,
    pub tons: u16,
    pub engine_rating: u16,
    /// Legs a Mech stands on (two or four); zero for other units.
    pub legs: u16,
}

/// How a physical weapon's Battle Value is found (TechManual p. 317; Tactical Operations:
/// Advanced Units & Equipment pp. 195-197).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicalBattleValue {
    /// A fixed Battle Value.
    Fixed(u16),
    /// The weapon's final damage (after modifiers such as TSM) times this factor.
    DamageTimes(f64),
}

impl SystemContext {
    /// The context of one installed system: the part's own technology base (see
    /// [`super::super::SystemCritical::is_clan`]) replaces the chassis's for the facts it
    /// sizes, values and prices.
    pub fn for_part<L>(self, part: &super::super::SystemCritical<L>) -> Self {
        Self {
            clan: part.is_clan(self.clan),
            ..self
        }
    }
}

impl System {
    /// Defensive Battle Value of one installation of this system (the † entries of
    /// TechManual pp. 317-318 and Tactical Operations: Advanced Units & Equipment pp. 195-197):
    /// Beagle 10, Clan active probe 12, light active probe 7, Bloodhound 25, Guardian or Clan
    /// ECM 61, Watchdog CEWS 68 and Angel ECM 100, and a spike cluster 4 (Tactical Operations:
    /// Advanced Units & Equipment p. 194). `None` for systems without a defensive BV.
    pub fn defensive_battle_value(self, context: SystemContext) -> Option<u16> {
        Some(match self {
            Self::Spikes => 4,
            Self::BeagleProbe if context.clan => 12,
            Self::BeagleProbe => 10,
            Self::LightProbe => 7,
            Self::BloodhoundProbe => 25,
            Self::Ecm if context.watchdog => 68,
            Self::Ecm => 61,
            Self::AngelEcm => 100,
            _ => return None,
        })
    }

    /// Multiplier applied to the BV of each missile launcher this fire control system serves:
    /// Artemis IV 1.2 (TechManual p. 317, note B), Artemis V 1.3 (Tactical Operations:
    /// Advanced Units & Equipment p. 197, note J) and the MRM "Apollo" system 1.15 (ibid.
    /// p. 195, note L). The increase does not apply to ammunition.
    pub fn launcher_battle_value_multiplier(self) -> Option<f64> {
        match self {
            Self::ArtemisIv => Some(1.2),
            Self::ArtemisV => Some(1.3),
            Self::ApolloFcs => Some(1.15),
            _ => None,
        }
    }

    /// Offensive Battle Value of a physical weapon (TechManual p. 317; Tactical Operations:
    /// Advanced Units & Equipment pp. 195-197). Claws, lances and maces use the final damage
    /// times the listed factor; flails, dual saws, vibroblades and wrecking balls are fixed.
    pub fn physical_battle_value(self) -> Option<PhysicalBattleValue> {
        use PhysicalBattleValue::{DamageTimes, Fixed};
        Some(match self {
            Self::Axe => DamageTimes(1.5),
            Self::Sword | Self::RetractableBlade | Self::ChainWhip => DamageTimes(1.725),
            Self::Claw => DamageTimes(1.275),
            Self::Mace | Self::Lance => DamageTimes(1.0),
            Self::Flail => Fixed(11),
            Self::DualSaw => Fixed(9),
            Self::WreckingBall => Fixed(8),
            Self::SmallVibroblade => Fixed(12),
            Self::MediumVibroblade => Fixed(17),
            Self::LargeVibroblade => Fixed(24),
            _ => return None,
        })
    }

    /// BattleMech critical slots one installation fills, for systems whose installation has a
    /// fixed or tonnage-derived size (TechManual pp. 342-344; Tactical Operations: Advanced
    /// Units & Equipment pp. 217-221). Systems priced or valued per slot (CASE, targeting
    /// computer, hatchet, MASC) count one slot per installation.
    pub fn installation_slots(self, context: SystemContext) -> Option<u16> {
        let tons = context.tons;
        Some(match self {
            Self::Case | Self::CaseIi | Self::C3Slave | Self::ArtemisIv | Self::ApolloFcs => 1,
            Self::Tag | Self::LightTag | Self::LightProbe | Self::Supercharger => 1,
            Self::TargetingComputer | Self::Axe | Self::Masc => 1,
            Self::BeagleProbe if context.clan => 1,
            Self::Ecm if context.clan && !context.watchdog => 1,
            Self::BeagleProbe | Self::Ecm => 2,
            Self::AngelEcm | Self::C3i | Self::ArtemisV | Self::ChainWhip => 2,
            Self::BloodhoundProbe => 3,
            Self::C3Master => 5,
            Self::NullSignature | Self::DualSaw => 7,
            Self::Flail | Self::LargeVibroblade => 4,
            Self::WreckingBall => 5,
            Self::SmallVibroblade | Self::Spikes | Self::MachineGunArray => 1,
            Self::MediumVibroblade => 2,
            // Talons fill two slots in each leg as one installation.
            Self::Talons => 2 * context.legs.max(2),
            Self::Sword | Self::Claw => tons.div_ceil(15),
            Self::Mace => tons.div_ceil(10),
            Self::Lance => tons.div_ceil(20),
            Self::RetractableBlade => tons.div_ceil(20) + 1,
            _ => return None,
        })
    }

    /// Construction price in C-bills of one installation (TechManual pp. 290-294; Tactical
    /// Operations: Advanced Units & Equipment pp. 216-223). Per-ton items price the tons one
    /// installation weighs, rounded up to the whole ton (TechManual p. 291 note †). Returns
    /// `Some(0.0)` for structure, actuators and other parts priced with the chassis, and `None`
    /// for parts the books do not price, which fall back to market prices.
    pub fn installation_price(self, context: SystemContext) -> Option<f64> {
        let tons = f64::from(context.tons);
        let rating = f64::from(context.engine_rating);
        Some(match self {
            Self::Case => 50_000.0,
            Self::CaseIi => 175_000.0,
            Self::BeagleProbe => 200_000.0,
            Self::LightProbe => 50_000.0,
            Self::BloodhoundProbe => 500_000.0,
            Self::Ecm if context.watchdog => 600_000.0,
            Self::Ecm => 200_000.0,
            Self::AngelEcm => 750_000.0,
            Self::C3Master => 1_500_000.0,
            Self::C3Slave => 250_000.0,
            Self::C3i => 750_000.0,
            Self::ArtemisIv => 100_000.0,
            Self::ArtemisV => 250_000.0,
            Self::ApolloFcs => 125_000.0,
            Self::Tag => 50_000.0,
            Self::LightTag => 40_000.0,
            Self::NullSignature => 1_400_000.0,
            Self::TargetingComputer => 10_000.0,
            Self::Axe => 5_000.0,
            // A sword weighs a twentieth of the Mech, rounded up to the half ton.
            Self::Sword => 10_000.0 * (f64::from(context.tons.div_ceil(10)) / 2.0).ceil(),
            // The blade weighs a twentieth of the Mech, rounded up to the half ton.
            Self::RetractableBlade => {
                10_000.0 + 10_000.0 * (f64::from(context.tons.div_ceil(10)) / 2.0).ceil()
            }
            Self::Mace => 130_000.0,
            Self::Claw => 200.0 * tons,
            Self::Lance => 150.0 * tons,
            Self::Flail => 110_000.0,
            Self::ChainWhip => 120_000.0,
            Self::SmallVibroblade => 150_000.0,
            Self::MediumVibroblade => 400_000.0,
            Self::LargeVibroblade => 750_000.0,
            // Talons and spikes are priced by the Mech's tonnage (Tactical Operations: Advanced
            // Units & Equipment p. 217).
            Self::Talons => 300.0 * tons,
            Self::Spikes => 50.0 * tons,
            // TechManual p. 292.
            Self::MachineGunArray => 1_250.0,
            Self::DualSaw => 100_000.0,
            Self::WreckingBall => 80_000.0,
            Self::Masc => 1_000.0 * rating,
            Self::Supercharger => 10_000.0 * rating,
            Self::ShoulderOrHip
            | Self::UpperActuator
            | Self::LowerActuator
            | Self::HandOrFootActuator
            | Self::Engine
            | Self::Gyro
            | Self::Cockpit
            | Self::LifeSupport
            | Self::Sensors
            | Self::HeatSink
            | Self::JumpJet
            | Self::FerroFibrous
            | Self::HeavyFerroFibrous
            | Self::LightFerroFibrous
            | Self::EndoSteel
            | Self::TripleStrengthMyomer
            | Self::StealthArmor
            | Self::LaserReflective => 0.0,
            Self::FuelTank => return None,
        })
    }

    /// Price of one BattleMech critical slot of this system: the installation price spread
    /// over the slots one installation fills, so that complete installations sum to the book
    /// price.
    pub fn slot_price(self, context: SystemContext) -> Option<f64> {
        let price = self.installation_price(context)?;
        let slots = self.installation_slots(context).unwrap_or(1).max(1);
        Some(price / f64::from(slots))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IS: SystemContext = SystemContext {
        clan: false,
        watchdog: false,
        tons: 65,
        engine_rating: 260,
        legs: 2,
    };

    /// Complete installations sum to the TechManual p. 291 and Tactical Operations: Advanced
    /// Units & Equipment p. 217 prices whatever their slot counts.
    #[test]
    fn slot_prices_sum_to_the_book_price() {
        let clan = SystemContext { clan: true, ..IS };
        let watchdog = SystemContext {
            clan: true,
            watchdog: true,
            ..IS
        };
        for (system, context, slots, price) in [
            (System::BeagleProbe, IS, 2, 200_000.0),
            (System::BeagleProbe, clan, 1, 200_000.0),
            (System::Ecm, IS, 2, 200_000.0),
            (System::Ecm, clan, 1, 200_000.0),
            (System::Ecm, watchdog, 2, 600_000.0),
            (System::AngelEcm, IS, 2, 750_000.0),
            (System::C3Master, IS, 5, 1_500_000.0),
            (System::C3i, IS, 2, 750_000.0),
            (System::BloodhoundProbe, IS, 3, 500_000.0),
            (System::ArtemisV, clan, 2, 250_000.0),
            (System::Mace, IS, 7, 130_000.0),
            (System::Sword, IS, 5, 40_000.0),
            (System::Talons, IS, 4, 19_500.0),
            (System::Talons, SystemContext { legs: 4, ..IS }, 8, 19_500.0),
            (System::Spikes, IS, 1, 3_250.0),
        ] {
            assert_eq!(
                system.installation_slots(context),
                Some(slots),
                "{system:?}"
            );
            let total = system.slot_price(context).unwrap() * f64::from(slots);
            assert!((total - price).abs() < 1e-6, "{system:?} {total}");
        }
        assert_eq!(System::Masc.slot_price(IS), Some(260_000.0));
        assert_eq!(System::Supercharger.slot_price(IS), Some(2_600_000.0));
        assert_eq!(System::Engine.slot_price(IS), Some(0.0));
        assert_eq!(System::FuelTank.slot_price(IS), None);
    }

    /// Defensive equipment values follow the † entries of the BV tables.
    #[test]
    fn defensive_values_follow_the_bv_tables() {
        let clan = SystemContext { clan: true, ..IS };
        let watchdog = SystemContext {
            watchdog: true,
            ..clan
        };
        assert_eq!(System::BeagleProbe.defensive_battle_value(IS), Some(10));
        assert_eq!(System::BeagleProbe.defensive_battle_value(clan), Some(12));
        assert_eq!(System::LightProbe.defensive_battle_value(clan), Some(7));
        assert_eq!(System::BloodhoundProbe.defensive_battle_value(IS), Some(25));
        assert_eq!(System::Ecm.defensive_battle_value(IS), Some(61));
        assert_eq!(System::Ecm.defensive_battle_value(clan), Some(61));
        assert_eq!(System::Ecm.defensive_battle_value(watchdog), Some(68));
        assert_eq!(System::AngelEcm.defensive_battle_value(IS), Some(100));
        assert_eq!(System::Tag.defensive_battle_value(IS), None);
        assert_eq!(System::Spikes.defensive_battle_value(IS), Some(4));
        assert_eq!(
            System::ArtemisV.launcher_battle_value_multiplier(),
            Some(1.3)
        );
        assert_eq!(
            System::ApolloFcs.launcher_battle_value_multiplier(),
            Some(1.15)
        );
        assert_eq!(System::ApolloFcs.slot_price(IS), Some(125_000.0));
        assert_eq!(
            System::Axe.physical_battle_value(),
            Some(PhysicalBattleValue::DamageTimes(1.5))
        );
        assert_eq!(
            System::Flail.physical_battle_value(),
            Some(PhysicalBattleValue::Fixed(11))
        );
    }
}
