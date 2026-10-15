//! Combat vehicle transport bays other than the standard cargo bay: infantry compartments,
//! infantry and battle armor bays, and the specialized cargo bays of the TechManual Transport
//! Bay table (p. 239). The game has no infantry units and loads stock only into the standard
//! cargo bay that `cargo_space` describes, so these bays are construction data: they weigh
//! what the table says, claim item slots, cost C-bills and add no Battle Value.
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The template document table and the attribute that stores it.
pub const TRANSPORT_BAYS: &str = "transport_bays";

/// One kind of transport bay (TechManual p. 239, Transport Bay table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransportBay {
    /// Seats and floor space for infantry or battle armor: capacity equals its weight.
    InfantryCompartment,
    /// A 5-ton bay for a 30-trooper foot platoon.
    FootInfantryBay,
    /// A 6-ton bay for a 30-trooper jump platoon.
    JumpInfantryBay,
    /// A 7-ton bay for a 30-trooper motorized platoon.
    MotorizedInfantryBay,
    /// An 8-ton bay for a 7-trooper mechanized platoon.
    MechanizedInfantryBay,
    /// An 8-ton Inner Sphere bay for a four-trooper battle armor squad.
    BattleArmorBay,
    /// A 10-ton bay for a five-trooper Clan battle armor Point.
    ClanBattleArmorBay,
    /// A 12-ton bay for a six-trooper battle armor squad.
    SixTrooperBattleArmorBay,
    /// Fluid cargo: capacity is 0.91 of its weight.
    LiquidCargo,
    /// Insulated cargo: capacity is 0.87 of its weight.
    InsulatedCargo,
    /// Refrigerated cargo: capacity is 0.87 of its weight.
    RefrigeratedCargo,
    /// Livestock: capacity is 0.83 of its weight.
    LivestockCargo,
    /// Standard 10-ton shipping containers, each claiming its own item slot.
    CargoContainer,
}

/// How a bay's size is stated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaySize {
    /// Any weight the designer chooses, stated in tons.
    Variable,
    /// Whole bays (or containers) of a fixed weight in tons.
    Fixed(u32),
}

impl TransportBay {
    /// Every bay, in document order.
    pub const ALL: [Self; 13] = [
        Self::InfantryCompartment,
        Self::FootInfantryBay,
        Self::JumpInfantryBay,
        Self::MotorizedInfantryBay,
        Self::MechanizedInfantryBay,
        Self::BattleArmorBay,
        Self::ClanBattleArmorBay,
        Self::SixTrooperBattleArmorBay,
        Self::LiquidCargo,
        Self::InsulatedCargo,
        Self::RefrigeratedCargo,
        Self::LivestockCargo,
        Self::CargoContainer,
    ];

    /// The `[transport_bays]` key.
    pub fn key(self) -> &'static str {
        match self {
            Self::InfantryCompartment => "infantry_compartment",
            Self::FootInfantryBay => "foot_infantry_bay",
            Self::JumpInfantryBay => "jump_infantry_bay",
            Self::MotorizedInfantryBay => "motorized_infantry_bay",
            Self::MechanizedInfantryBay => "mechanized_infantry_bay",
            Self::BattleArmorBay => "battle_armor_bay",
            Self::ClanBattleArmorBay => "clan_battle_armor_bay",
            Self::SixTrooperBattleArmorBay => "six_trooper_battle_armor_bay",
            Self::LiquidCargo => "liquid_cargo",
            Self::InsulatedCargo => "insulated_cargo",
            Self::RefrigeratedCargo => "refrigerated_cargo",
            Self::LivestockCargo => "livestock_cargo",
            Self::CargoContainer => "cargo_containers",
        }
    }

    /// The bay a `[transport_bays]` key names.
    pub fn parse(key: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|bay| bay.key() == key)
            .with_context(|| format!("unknown transport bay {key}"))
    }

    /// Weight per bay, or a variable weight (TechManual p. 239).
    pub fn size(self) -> BaySize {
        match self {
            Self::InfantryCompartment
            | Self::LiquidCargo
            | Self::InsulatedCargo
            | Self::RefrigeratedCargo
            | Self::LivestockCargo => BaySize::Variable,
            Self::FootInfantryBay => BaySize::Fixed(5),
            Self::JumpInfantryBay => BaySize::Fixed(6),
            Self::MotorizedInfantryBay => BaySize::Fixed(7),
            Self::MechanizedInfantryBay | Self::BattleArmorBay => BaySize::Fixed(8),
            Self::ClanBattleArmorBay | Self::CargoContainer => BaySize::Fixed(10),
            Self::SixTrooperBattleArmorBay => BaySize::Fixed(12),
        }
    }

    /// C-bills per ton of bay (TechManual p. 293, Industrial Equipment table): infantry and
    /// battle armor bays cost 15,000, compartments, containers and standard cargo nothing.
    pub fn cost_per_ton(self) -> u32 {
        match self {
            Self::InfantryCompartment | Self::CargoContainer => 0,
            Self::FootInfantryBay
            | Self::JumpInfantryBay
            | Self::MotorizedInfantryBay
            | Self::MechanizedInfantryBay
            | Self::BattleArmorBay
            | Self::ClanBattleArmorBay
            | Self::SixTrooperBattleArmorBay => 15_000,
            Self::LiquidCargo => 100,
            Self::InsulatedCargo => 250,
            Self::RefrigeratedCargo => 200,
            Self::LivestockCargo => 2_500,
        }
    }
}

/// The transport bays a vehicle carries: half tons of each variable bay, or the number of
/// each fixed bay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportBays {
    bays: BTreeMap<TransportBay, u32>,
}

impl TransportBays {
    /// Decode the `[transport_bays]` document table: tons in half-ton steps for a variable bay,
    /// a whole number of bays for a fixed one.
    pub fn decode(value: toml::Value) -> Result<Self> {
        let toml::Value::Table(table) = value else {
            bail!("{TRANSPORT_BAYS} must be a table");
        };
        let mut bays = BTreeMap::new();
        for (key, value) in table {
            let bay = TransportBay::parse(&key)?;
            let quantity = match (bay.size(), value) {
                (BaySize::Variable, toml::Value::Integer(tons)) => u32::try_from(tons)
                    .ok()
                    .and_then(|tons| tons.checked_mul(2)),
                (BaySize::Variable, toml::Value::Float(tons)) => {
                    let halves = tons * 2.0;
                    (tons.is_finite() && halves.fract() == 0.0 && halves <= f64::from(u32::MAX))
                        .then_some(halves as u32)
                }
                (BaySize::Fixed(_), toml::Value::Integer(count)) => u32::try_from(count).ok(),
                (BaySize::Variable, _) => bail!("{key} must be a weight in tons"),
                (BaySize::Fixed(_), _) => bail!("{key} must be a whole number of bays"),
            };
            let quantity = quantity
                .filter(|quantity| *quantity > 0)
                .with_context(|| match bay.size() {
                    BaySize::Variable => format!("{key} must be a positive weight in half tons"),
                    BaySize::Fixed(_) => format!("{key} must be a positive number of bays"),
                })?;
            bays.insert(bay, quantity);
        }
        Ok(Self { bays })
    }

    /// Read the stored attribute, `key=quantity` words; absent means no bays.
    pub fn from_attribute(value: Option<&str>) -> Result<Self> {
        let mut bays = BTreeMap::new();
        for word in value.unwrap_or_default().split_ascii_whitespace() {
            let (key, quantity) = word
                .split_once('=')
                .with_context(|| format!("invalid transport bay {word}"))?;
            let quantity: u32 = quantity
                .parse()
                .with_context(|| format!("invalid transport bay quantity {word}"))?;
            ensure!(quantity > 0, "invalid transport bay quantity {word}");
            bays.insert(TransportBay::parse(key)?, quantity);
        }
        Ok(Self { bays })
    }

    /// The stored attribute form, or `None` without bays.
    pub fn to_attribute(&self) -> Option<String> {
        (!self.bays.is_empty()).then(|| {
            self.bays
                .iter()
                .map(|(bay, quantity)| format!("{}={quantity}", bay.key()))
                .collect::<Vec<_>>()
                .join(" ")
        })
    }

    /// The `[transport_bays]` document table, or an empty string without bays.
    pub fn render(&self) -> String {
        let mut output = String::new();
        if self.bays.is_empty() {
            return output;
        }
        let _ = writeln!(output, "\n[{TRANSPORT_BAYS}]");
        for (bay, quantity) in &self.bays {
            match bay.size() {
                BaySize::Variable if quantity % 2 == 0 => {
                    let _ = writeln!(output, "{} = {}", bay.key(), quantity / 2);
                }
                BaySize::Variable => {
                    let _ = writeln!(output, "{} = {}.5", bay.key(), quantity / 2);
                }
                BaySize::Fixed(_) => {
                    let _ = writeln!(output, "{} = {quantity}", bay.key());
                }
            }
        }
        output
    }

    /// Whether the vehicle carries no transport bays.
    pub fn is_empty(&self) -> bool {
        self.bays.is_empty()
    }

    /// Each bay with its weight in 1/1024-ton units.
    pub fn masses(&self) -> impl Iterator<Item = (TransportBay, u32)> + '_ {
        self.bays.iter().map(|(&bay, &quantity)| {
            let mass = match bay.size() {
                BaySize::Variable => u64::from(quantity) * 512,
                BaySize::Fixed(tons) => u64::from(quantity) * u64::from(tons) * 1024,
            };
            (bay, u32::try_from(mass).unwrap_or(u32::MAX))
        })
    }

    /// Weight of every bay in 1/1024-ton units.
    pub fn mass(&self) -> u32 {
        self.masses()
            .fold(0u32, |total, (_, mass)| total.saturating_add(mass))
    }

    /// Item slots the bays claim on a combat vehicle: one per distinct type of bay, since all
    /// infantry bays of one type combine into a single slot, and one per cargo container
    /// (TechManual p. 239).
    pub fn item_slots(&self) -> u32 {
        self.bays
            .iter()
            .map(|(&bay, &quantity)| {
                if bay == TransportBay::CargoContainer {
                    quantity
                } else {
                    1
                }
            })
            .fold(0u32, u32::saturating_add)
    }

    /// C-bill cost of every bay (TechManual p. 293).
    pub fn cost(&self) -> f64 {
        self.masses()
            .map(|(bay, mass)| f64::from(mass) / 1024.0 * f64::from(bay.cost_per_ton()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(source: &str) -> Result<TransportBays> {
        let table: toml::Table = toml::from_str(source)?;
        TransportBays::decode(toml::Value::Table(table))
    }

    /// Weights, slots and costs follow the Transport Bay table and the equipment prices.
    #[test]
    fn bays_follow_the_transport_bay_table() {
        let bays = decode(
            "infantry_compartment = 4\nfoot_infantry_bay = 2\nmotorized_infantry_bay = 1\n\
             liquid_cargo = 1.5\ncargo_containers = 2\n",
        )
        .unwrap();
        // 4 + 2 x 5 + 7 + 1.5 + 2 x 10 tons.
        assert_eq!(bays.mass(), 85 * 512);
        // Compartment, foot bays, motorized bay and liquid cargo one slot each; a slot per
        // container.
        assert_eq!(bays.item_slots(), 6);
        // 17 tons of infantry bays at 15,000 and 1.5 tons of liquid cargo at 100.
        assert_eq!(bays.cost(), 17.0 * 15_000.0 + 150.0);
        let stored = bays.to_attribute().unwrap();
        assert_eq!(TransportBays::from_attribute(Some(&stored)).unwrap(), bays);
        assert_eq!(
            decode(&bays.render().replace("\n[transport_bays]\n", "")).unwrap(),
            bays
        );
        for (bay, tons) in [
            (TransportBay::JumpInfantryBay, 6),
            (TransportBay::MechanizedInfantryBay, 8),
            (TransportBay::BattleArmorBay, 8),
            (TransportBay::ClanBattleArmorBay, 10),
            (TransportBay::SixTrooperBattleArmorBay, 12),
        ] {
            assert_eq!(bay.size(), BaySize::Fixed(tons), "{bay:?}");
        }
        assert!(TransportBays::from_attribute(None).unwrap().is_empty());
    }

    /// Unknown bays, fractional bay counts and weights off the half ton are refused.
    #[test]
    fn malformed_bays_are_refused() {
        for source in [
            "troop_space = 4",
            "infantry_compartment = 0",
            "infantry_compartment = 1.25",
            "infantry_compartment = -1",
            "infantry_compartment = \"4\"",
            "foot_infantry_bay = 1.5",
            "foot_infantry_bay = 0",
        ] {
            assert!(decode(source).is_err(), "{source}");
        }
        assert!(TransportBays::from_attribute(Some("foot_infantry_bay")).is_err());
        assert!(TransportBays::from_attribute(Some("foot_infantry_bay=0")).is_err());
    }
}
