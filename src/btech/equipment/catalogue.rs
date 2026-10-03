//! Single source of weapon identities, catalog profiles, mass, skill family and recycle feedback.
use super::WeaponProfile;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Define catalog facts once while keeping typed, exhaustive dispatch and static notice text.
macro_rules! weapon_catalogue {
    ($($variant:ident {
        name: ($namespace:literal, $label:literal), part_id: $part_id:literal, mass: $mass:literal, bv: $bv:literal, skill: $skill:literal,
        profile: ($heat:literal, $damage:literal, $missiles:literal, $minimum:literal,
                  [$short:literal, $medium:literal, $long:literal], $slots:literal,
                  $ammo:literal, $recycle:literal)
    }),+ $(,)?) => {
        /// Catalogue identities with typed rule dispatch; construction validation separately admits live equipment.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum BattleWeapon { $($variant),+ }

        impl BattleWeapon {
            /// All supported catalog identities in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Stable game-directory inventory identifier, independent of enum declaration order.
            pub fn part_id(self) -> i32 {
                match self { $(Self::$variant => $part_id),+ }
            }

            /// Resolve a stored weapon identity without admitting unknown catalogue slots.
            pub fn from_part_id(id: i32) -> Option<Self> {
                match id { $($part_id => Some(Self::$variant),)+ _ => None }
            }

            /// Resolve an ASCII case-insensitive asset spelling without accepting unknown weapons.
            pub fn parse(name: &str) -> Result<Self> {
                match name {
                    $(name if name.eq_ignore_ascii_case(concat!($namespace, ".", $label)) => Ok(Self::$variant),)+
                    _ => bail!("Unsupported weapon {name}"),
                }
            }

            /// Canonical asset spelling, independent of the serialized enum name.
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => concat!($namespace, ".", $label)),+ }
            }

            /// Intrinsic catalog facts; ammunition counts salvos and missile damage is per projectile.
            pub fn profile(self) -> WeaponProfile {
                match self { $(Self::$variant => WeaponProfile {
                    heat: $heat, damage: $damage, missiles: $missiles, minimum_range: $minimum,
                    short_range: $short, medium_range: $medium, long_range: $long,
                    critical_slots: $slots, ammunition_per_ton: $ammo, recycle_seconds: $recycle,
                }),+ }
            }

            /// Catalog mass in 1/1024-ton units before per-critical integer rounding.
            pub fn mass(self) -> u32 {
                match self { $(Self::$variant => $mass),+ }
            }

            /// Intrinsic catalogue Battle Value, before unit heat and movement adjustments.
            pub fn battle_value(self) -> u16 {
                match self { $(Self::$variant => $bv),+ }
            }

            /// Biped gunnery skill, optionally specialized by weapon family.
            pub fn gunnery_skill(self, extended: bool) -> &'static str {
                if self.is_artillery() { return "Gunnery-Artillery"; }
                if !extended { return "Gunnery-Battlemech"; }
                match self { $(Self::$variant => $skill),+ }
            }

            /// Static cockpit feedback used by committed-second recycle processing.
            pub(crate) fn recycle_notice(self) -> &'static str {
                match self { $(Self::$variant => concat!($label, " finished recycling.")),+ }
            }
        }
    };
}

// Profile columns: heat, damage, missiles, minimum, [short, medium, long], slots, ammo/ton, recycle.
weapon_catalogue! {
    ArrowIv { name: ("IS", "ArrowIVSystem"), part_id: 115, mass: 15360, bv: 171, skill: "Gunnery-Artillery",
        profile: (10, 20, 0, 0, [0, 0, 5], 15, 5, 60) },
    ClanArrowIv { name: ("CL", "ArrowIVSystem"), part_id: 64, mass: 12288, bv: 171, skill: "Gunnery-Artillery",
        profile: (10, 20, 0, 0, [0, 0, 6], 12, 5, 60) },
    LongTom { name: ("IS", "LongTom"), part_id: 5, mass: 30720, bv: 171, skill: "Gunnery-Artillery",
        profile: (20, 20, 0, 0, [0, 0, 20], 30, 5, 60) },
    Sniper { name: ("IS", "Sniper"), part_id: 116, mass: 20480, bv: 86, skill: "Gunnery-Artillery",
        profile: (10, 10, 0, 0, [0, 0, 12], 20, 10, 60) },
    Thumper { name: ("IS", "Thumper"), part_id: 117, mass: 15360, bv: 40, skill: "Gunnery-Artillery",
        profile: (6, 5, 0, 0, [0, 0, 14], 15, 20, 60) },
    LongTomCannon { name: ("IS", "LongTomCannon"), part_id: 121, mass: 20480, bv: 348, skill: "Gunnery-Artillery",
        profile: (20, 20, 0, 4, [6, 13, 20], 15, 5, 30) },
    SniperCannon { name: ("IS", "SniperCannon"), part_id: 122, mass: 15360, bv: 115, skill: "Gunnery-Artillery",
        profile: (10, 10, 0, 2, [4, 8, 12], 10, 10, 25) },
    ThumperCannon { name: ("IS", "ThumperCannon"), part_id: 123, mass: 10240, bv: 58, skill: "Gunnery-Artillery",
        profile: (6, 5, 0, 3, [4, 9, 14], 7, 20, 25) },
    APod { name: ("IS", "A-Pod"), part_id: 159, mass: 512, bv: 1, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    ClanAPod { name: ("CL", "A-Pod"), part_id: 1, mass: 512, bv: 1, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    INarcBeacon { name: ("IS", "iNarcBeacon"), part_id: 142, mass: 5120, bv: 75, skill: "Gunnery-Missile",
        profile: (1, 6, 1, 0, [4, 9, 15], 3, 4, 30) },
    NarcBeacon { name: ("IS", "NarcBeacon"), part_id: 141, mass: 3072, bv: 30, skill: "Gunnery-Missile",
        profile: (1, 4, 1, 0, [3, 6, 9], 2, 6, 30) },
    ClanNarcBeacon { name: ("CL", "NarcBeacon"), part_id: 73, mass: 2048, bv: 30, skill: "Gunnery-Missile",
        profile: (1, 4, 1, 0, [4, 8, 12], 1, 6, 30) },
    AntiMissileSystem { name: ("IS", "Anti-MissileSystem"), part_id: 101, mass: 512, bv: 32, skill: "Gunnery-Missile",
        profile: (1, 2, 0, 0, [1, 1, 1], 1, 12, 10) },
    ClanAntiMissileSystem { name: ("CL", "Anti-MissileSystem"), part_id: 51, mass: 512, bv: 63, skill: "Gunnery-Missile",
        profile: (1, 2, 0, 0, [1, 1, 1], 1, 24, 10) },
    LaserAms { name: ("IS", "LaserAMS"), part_id: 170, mass: 512, bv: 105, skill: "Gunnery-Laser",
        profile: (12, 2, 0, 0, [1, 1, 1], 1, 24, 25) },
    ClanLaserAms { name: ("CL", "LaserAMS"), part_id: 169, mass: 512, bv: 105, skill: "Gunnery-Laser",
        profile: (1, 2, 0, 0, [1, 1, 1], 1, 24, 25) },
    ClanLbx2 { name: ("CL", "LB2-XAC"), part_id: 53, mass: 5120, bv: 47, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [10, 20, 30], 3, 45, 15) },
    ClanLbx5 { name: ("CL", "LB5-XAC"), part_id: 54, mass: 7168, bv: 93, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [8, 15, 24], 4, 20, 20) },
    ClanLbx10 { name: ("CL", "LB10-XAC"), part_id: 55, mass: 10240, bv: 148, skill: "Gunnery-Ballistic",
        profile: (2, 10, 0, 0, [6, 12, 18], 5, 10, 25) },
    ClanLbx20 { name: ("CL", "LB20-XAC"), part_id: 56, mass: 12288, bv: 237, skill: "Gunnery-Ballistic",
        profile: (6, 20, 0, 0, [4, 8, 12], 9, 5, 30) },
    ClanUltraAc2 { name: ("CL", "UltraAC/2"), part_id: 60, mass: 5120, bv: 62, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 2, [9, 18, 27], 2, 45, 12) },
    ClanUltraAc5 { name: ("CL", "UltraAC/5"), part_id: 61, mass: 7168, bv: 123, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [7, 14, 21], 3, 20, 20) },
    ClanUltraAc10 { name: ("CL", "UltraAC/10"), part_id: 62, mass: 10240, bv: 211, skill: "Gunnery-Ballistic",
        profile: (3, 10, 0, 0, [6, 12, 18], 4, 10, 25) },
    ClanUltraAc20 { name: ("CL", "UltraAC/20"), part_id: 63, mass: 12288, bv: 337, skill: "Gunnery-Ballistic",
        profile: (7, 20, 0, 0, [4, 8, 12], 8, 5, 30) },
    Mml3 { name: ("IS", "MML-3"), part_id: 126, mass: 1536, bv: 29, skill: "Gunnery-Missile",
        profile: (2, 2, 3, 0, [3, 6, 9], 2, 33, 30) },
    Mml5 { name: ("IS", "MML-5"), part_id: 127, mass: 3072, bv: 45, skill: "Gunnery-Missile",
        profile: (3, 2, 5, 0, [3, 6, 9], 3, 20, 30) },
    Mml7 { name: ("IS", "MML-7"), part_id: 128, mass: 4608, bv: 67, skill: "Gunnery-Missile",
        profile: (4, 2, 7, 0, [3, 6, 9], 4, 14, 30) },
    Mml9 { name: ("IS", "MML-9"), part_id: 129, mass: 6144, bv: 86, skill: "Gunnery-Missile",
        profile: (5, 2, 9, 0, [3, 6, 9], 5, 11, 30) },
    ClanAtm3 { name: ("CL", "ATM-3"), part_id: 65, mass: 1536, bv: 53, skill: "Gunnery-Missile",
        profile: (2, 2, 3, 4, [5, 10, 15], 2, 20, 15) },
    ClanAtm6 { name: ("CL", "ATM-6"), part_id: 66, mass: 3584, bv: 105, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 4, [5, 10, 15], 3, 10, 20) },
    ClanAtm9 { name: ("CL", "ATM-9"), part_id: 67, mass: 5120, bv: 147, skill: "Gunnery-Missile",
        profile: (6, 2, 9, 4, [5, 10, 15], 4, 7, 25) },
    ClanAtm12 { name: ("CL", "ATM-12"), part_id: 68, mass: 7168, bv: 212, skill: "Gunnery-Missile",
        profile: (8, 2, 12, 4, [5, 10, 15], 5, 5, 30) },
    ClanLrm5 { name: ("CL", "LRM-5"), part_id: 69, mass: 1024, bv: 55, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 0, [7, 14, 21], 1, 24, 15) },
    ClanLrm10 { name: ("CL", "LRM-10"), part_id: 70, mass: 2560, bv: 109, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [7, 14, 21], 1, 12, 20) },
    ClanLrm15 { name: ("CL", "LRM-15"), part_id: 71, mass: 3584, bv: 164, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 0, [7, 14, 21], 2, 8, 25) },
    ClanLrm20 { name: ("CL", "LRM-20"), part_id: 72, mass: 5120, bv: 220, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [7, 14, 21], 4, 6, 20) },
    ClanSrm2 { name: ("CL", "SRM-2"), part_id: 74, mass: 512, bv: 21, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    ClanSrm4 { name: ("CL", "SRM-4"), part_id: 75, mass: 1024, bv: 39, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    ClanSrm6 { name: ("CL", "SRM-6"), part_id: 76, mass: 1536, bv: 59, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 1, 15, 15) },
    ClanStreakSrm2 { name: ("CL", "StreakSRM-2"), part_id: 2, mass: 1024, bv: 40, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [4, 8, 12], 1, 50, 15) },
    ClanStreakSrm4 { name: ("CL", "StreakSRM-4"), part_id: 3, mass: 2048, bv: 79, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [4, 8, 12], 1, 25, 15) },
    ClanStreakSrm6 { name: ("CL", "StreakSRM-6"), part_id: 4, mass: 3072, bv: 119, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [4, 8, 12], 2, 15, 15) },
    ClanStreakLrm5 { name: ("CL", "StreakLRM-5"), part_id: 155, mass: 2048, bv: 87, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 6, [7, 14, 21], 1, 24, 15) },
    ClanStreakLrm10 { name: ("CL", "StreakLRM-10"), part_id: 156, mass: 5120, bv: 173, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 6, [7, 14, 21], 2, 12, 20) },
    ClanStreakLrm15 { name: ("CL", "StreakLRM-15"), part_id: 157, mass: 7168, bv: 260, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 6, [7, 14, 21], 3, 8, 25) },
    ClanStreakLrm20 { name: ("CL", "StreakLRM-20"), part_id: 158, mass: 10240, bv: 346, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 6, [7, 14, 21], 5, 6, 30) },
    ClanGaussRifle { name: ("CL", "GaussRifle"), part_id: 52, mass: 12288, bv: 321, skill: "Gunnery-Ballistic",
        profile: (1, 15, 0, 2, [7, 15, 22], 6, 8, 30) },
    ClanMachineGun { name: ("CL", "MachineGun"), part_id: 57, mass: 256, bv: 5, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [1, 2, 3], 1, 200, 7) },
    ClanLightMachineGun { name: ("CL", "LightMachineGun"), part_id: 58, mass: 256, bv: 5, skill: "Gunnery-Ballistic",
        profile: (0, 1, 0, 0, [2, 4, 6], 1, 200, 7) },
    ClanHeavyMachineGun { name: ("CL", "HeavyMachineGun"), part_id: 59, mass: 512, bv: 6, skill: "Gunnery-Ballistic",
        profile: (0, 3, 0, 0, [1, 2, 3], 1, 100, 7) },
    ClanErLargeLaser { name: ("CL", "ERLargeLaser"), part_id: 35, mass: 4096, bv: 249, skill: "Gunnery-Laser",
        profile: (12, 10, 0, 0, [8, 15, 25], 1, 0, 20) },
    ClanErMediumLaser { name: ("CL", "ERMediumLaser"), part_id: 36, mass: 1024, bv: 108, skill: "Gunnery-Laser",
        profile: (5, 7, 0, 0, [5, 10, 15], 1, 0, 15) },
    ClanErSmallLaser { name: ("CL", "ERSmallLaser"), part_id: 37, mass: 512, bv: 31, skill: "Gunnery-Laser",
        profile: (2, 5, 0, 0, [2, 4, 6], 1, 0, 10) },
    ClanErMicroLaser { name: ("CL", "ERMicroLaser"), part_id: 38, mass: 256, bv: 7, skill: "Gunnery-Laser",
        profile: (1, 2, 0, 0, [1, 2, 4], 1, 0, 15) },
    ClanErPpc { name: ("CL", "ERPPC"), part_id: 39, mass: 6144, bv: 412, skill: "Gunnery-Laser",
        profile: (15, 15, 0, 0, [7, 14, 23], 2, 0, 25) },
    ClanFlamer { name: ("CL", "Flamer"), part_id: 40, mass: 512, bv: 6, skill: "Gunnery-Laser",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 0, 10) },
    ClanHeavyLargeLaser { name: ("CL", "HeavyLargeLaser"), part_id: 41, mass: 4096, bv: 243, skill: "Gunnery-Laser",
        profile: (18, 16, 0, 0, [5, 10, 15], 3, 0, 30) },
    ClanHeavyMediumLaser { name: ("CL", "HeavyMediumLaser"), part_id: 42, mass: 1024, bv: 76, skill: "Gunnery-Laser",
        profile: (7, 10, 0, 0, [3, 6, 9], 2, 0, 25) },
    ClanHeavySmallLaser { name: ("CL", "HeavySmallLaser"), part_id: 43, mass: 512, bv: 15, skill: "Gunnery-Laser",
        profile: (3, 6, 0, 0, [1, 2, 3], 1, 0, 20) },
    ClanLargePulseLaser { name: ("CL", "LargePulseLaser"), part_id: 44, mass: 6144, bv: 265, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 0, [6, 14, 20], 2, 0, 23) },
    ClanMediumPulseLaser { name: ("CL", "MediumPulseLaser"), part_id: 45, mass: 2048, bv: 111, skill: "Gunnery-Laser",
        profile: (4, 7, 0, 0, [4, 8, 12], 1, 0, 18) },
    ClanSmallPulseLaser { name: ("CL", "SmallPulseLaser"), part_id: 46, mass: 1024, bv: 24, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [2, 4, 6], 1, 0, 13) },
    ClanMicroPulseLaser { name: ("CL", "MicroPulseLaser"), part_id: 47, mass: 512, bv: 12, skill: "Gunnery-Laser",
        profile: (1, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    ClanErLargePulseLaser { name: ("CL", "ERLargePulseLaser"), part_id: 48, mass: 6144, bv: 271, skill: "Gunnery-Laser",
        profile: (13, 10, 0, 0, [7, 15, 23], 3, 0, 30) },
    ClanErMediumPulseLaser { name: ("CL", "ERMediumPulseLaser"), part_id: 49, mass: 2048, bv: 116, skill: "Gunnery-Laser",
        profile: (6, 7, 0, 0, [5, 9, 14], 2, 0, 30) },
    ClanErSmallPulseLaser { name: ("CL", "ERSmallPulseLaser"), part_id: 50, mass: 1536, bv: 36, skill: "Gunnery-Laser",
        profile: (3, 5, 0, 0, [2, 4, 6], 1, 0, 30) },
    ClanPlasmaRifle { name: ("CL", "PlasmaRifle"), part_id: 168, mass: 6144, bv: 400, skill: "Gunnery-Laser",
        profile: (15, 10, 0, 0, [7, 14, 22], 2, 0, 25) },
    Flamer { name: ("IS", "Flamer"), part_id: 81, mass: 1024, bv: 6, skill: "Gunnery-Laser",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 0, 10) },
    CoolantGun { name: ("IS", "CoolantGun"), part_id: 161, mass: 1024, bv: 15, skill: "Gunnery-Ballistic",
        profile: (0, 3, 0, 0, [1, 2, 3], 1, 25, 15) },
    HeavyFlamer { name: ("IS", "HeavyFlamer"), part_id: 118, mass: 1024, bv: 20, skill: "Gunnery-Ballistic",
        profile: (5, 4, 0, 0, [2, 4, 6], 1, 10, 15) },
    VehicleFlamer { name: ("IS", "VehicleFlamer"), part_id: 99, mass: 512, bv: 5, skill: "Gunnery-Ballistic",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 20, 10) },
    VehicleHeavyFlamer { name: ("IS", "VehicleHeavyFlamer"), part_id: 163, mass: 1024, bv: 20, skill: "Gunnery-Ballistic",
        profile: (5, 4, 0, 0, [2, 4, 6], 1, 20, 10) },
    PlasmaRifle { name: ("IS", "PlasmaRifle"), part_id: 125, mass: 6144, bv: 210, skill: "Gunnery-Ballistic",
        profile: (10, 10, 0, 0, [5, 10, 15], 2, 10, 30) },
    AcidThrower { name: ("IS", "AcidThrower"), part_id: 162, mass: 1536, bv: 30, skill: "Gunnery-Ballistic",
        profile: (3, 3, 0, 0, [1, 2, 3], 2, 10, 25) },
    Thunderbolt5 { name: ("IS", "Thunderbolt-5"), part_id: 149, mass: 3072, bv: 64, skill: "Gunnery-Missile",
        profile: (3, 5, 1, 5, [6, 12, 18], 1, 12, 20) },
    Thunderbolt10 { name: ("IS", "Thunderbolt-10"), part_id: 150, mass: 7168, bv: 127, skill: "Gunnery-Missile",
        profile: (5, 10, 1, 5, [6, 12, 18], 2, 6, 20) },
    Thunderbolt15 { name: ("IS", "Thunderbolt-15"), part_id: 151, mass: 11264, bv: 229, skill: "Gunnery-Missile",
        profile: (7, 15, 1, 5, [6, 12, 18], 3, 4, 30) },
    Thunderbolt20 { name: ("IS", "Thunderbolt-20"), part_id: 152, mass: 15360, bv: 305, skill: "Gunnery-Missile",
        profile: (8, 20, 1, 5, [6, 12, 18], 5, 3, 30) },
    HyperAc2 { name: ("IS", "HyperAC/2"), part_id: 32, mass: 8192, bv: 1000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 3, [10, 20, 35], 4, 30, 12) },
    HyperAc5 { name: ("IS", "HyperAC/5"), part_id: 33, mass: 12288, bv: 1000, skill: "Gunnery-Ballistic",
        profile: (3, 5, 0, 0, [8, 16, 28], 5, 15, 20) },
    HyperAc10 { name: ("IS", "HyperAC/10"), part_id: 34, mass: 14336, bv: 1000, skill: "Gunnery-Ballistic",
        profile: (7, 10, 0, 0, [6, 12, 20], 6, 8, 25) },
    MachineGun { name: ("IS", "MachineGun"), part_id: 100, mass: 512, bv: 5, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [1, 2, 3], 1, 200, 7) },
    HeavyMachineGun { name: ("IS", "HeavyMachineGun"), part_id: 124, mass: 1024, bv: 6, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [2, 4, 6], 1, 100, 7) },
    LightAc2 { name: ("IS", "LightAC/2"), part_id: 119, mass: 4096, bv: 30, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [6, 12, 18], 1, 45, 12) },
    LightAc5 { name: ("IS", "LightAC/5"), part_id: 120, mass: 5120, bv: 62, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [5, 10, 15], 2, 20, 20) },
    SmallLaser { name: ("IS", "SmallLaser"), part_id: 80, mass: 512, bv: 9, skill: "Gunnery-Laser",
        profile: (1, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    MediumLaser { name: ("IS", "MediumLaser"), part_id: 79, mass: 1024, bv: 46, skill: "Gunnery-Laser",
        profile: (3, 5, 0, 0, [3, 6, 9], 1, 0, 20) },
    LargeLaser { name: ("IS", "LargeLaser"), part_id: 78, mass: 5120, bv: 124, skill: "Gunnery-Laser",
        profile: (8, 8, 0, 0, [5, 10, 15], 2, 0, 25) },
    Ppc { name: ("IS", "PPC"), part_id: 77, mass: 7168, bv: 176, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 3, [6, 12, 18], 3, 0, 30) },
    ErSmallLaser { name: ("IS", "ERSmallLaser"), part_id: 85, mass: 512, bv: 17, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [2, 4, 5], 1, 0, 15) },
    ErMediumLaser { name: ("IS", "ERMediumLaser"), part_id: 84, mass: 1024, bv: 62, skill: "Gunnery-Laser",
        profile: (5, 5, 0, 0, [4, 8, 12], 1, 0, 20) },
    ErLargeLaser { name: ("IS", "ERLargeLaser"), part_id: 83, mass: 5120, bv: 163, skill: "Gunnery-Laser",
        profile: (12, 8, 0, 0, [7, 14, 19], 2, 0, 25) },
    ErPpc { name: ("IS", "ERPPC"), part_id: 82, mass: 7168, bv: 229, skill: "Gunnery-Laser",
        profile: (15, 10, 0, 0, [7, 14, 23], 3, 0, 30) },
    SmallPulseLaser { name: ("IS", "SmallPulseLaser"), part_id: 88, mass: 1024, bv: 12, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    MediumPulseLaser { name: ("IS", "MediumPulseLaser"), part_id: 87, mass: 2048, bv: 48, skill: "Gunnery-Laser",
        profile: (4, 6, 0, 0, [2, 4, 6], 1, 0, 20) },
    LargePulseLaser { name: ("IS", "LargePulseLaser"), part_id: 86, mass: 7168, bv: 119, skill: "Gunnery-Laser",
        profile: (10, 9, 0, 0, [3, 7, 10], 2, 0, 25) },
    XSmallPulseLaser { name: ("IS", "X-SmallPulseLaser"), part_id: 91, mass: 1024, bv: 21, skill: "Gunnery-Laser",
        profile: (3, 3, 0, 0, [2, 4, 5], 1, 0, 17) },
    XMediumPulseLaser { name: ("IS", "X-MediumPulseLaser"), part_id: 90, mass: 2048, bv: 71, skill: "Gunnery-Laser",
        profile: (6, 6, 0, 0, [3, 6, 9], 1, 0, 22) },
    XLargePulseLaser { name: ("IS", "X-LargePulseLaser"), part_id: 89, mass: 7168, bv: 178, skill: "Gunnery-Laser",
        profile: (14, 9, 0, 0, [5, 10, 15], 2, 0, 27) },
    LightPpc { name: ("IS", "LightPPC"), part_id: 93, mass: 3072, bv: 88, skill: "Gunnery-Laser",
        profile: (5, 5, 0, 3, [6, 12, 18], 2, 0, 30) },
    HeavyPpc { name: ("IS", "HeavyPPC"), part_id: 94, mass: 10240, bv: 317, skill: "Gunnery-Laser",
        profile: (15, 15, 0, 3, [6, 12, 18], 4, 0, 30) },
    SnubNosedPpc { name: ("IS", "SnubNosedPPC"), part_id: 92, mass: 6144, bv: 165, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 0, [9, 13, 15], 2, 0, 30) },
    Srm2 { name: ("IS", "SRM-2"), part_id: 134, mass: 1024, bv: 21, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    Rocket10 { name: ("IS", "RL-10"), part_id: 146, mass: 512, bv: 18, skill: "Gunnery-Missile",
        profile: (3, 1, 10, 0, [5, 11, 18], 1, 0, 30) },
    Rocket15 { name: ("IS", "RL-15"), part_id: 147, mass: 1024, bv: 23, skill: "Gunnery-Missile",
        profile: (4, 1, 15, 0, [4, 9, 15], 2, 0, 30) },
    Rocket20 { name: ("IS", "RL-20"), part_id: 148, mass: 1536, bv: 24, skill: "Gunnery-Missile",
        profile: (5, 1, 20, 0, [3, 7, 12], 3, 0, 30) },
    Mrm10 { name: ("IS", "MRM-10"), part_id: 137, mass: 3072, bv: 56, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [3, 8, 15], 2, 24, 20) },
    Mrm20 { name: ("IS", "MRM-20"), part_id: 138, mass: 7168, bv: 112, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [3, 8, 15], 3, 12, 30) },
    Mrm30 { name: ("IS", "MRM-30"), part_id: 139, mass: 10240, bv: 168, skill: "Gunnery-Missile",
        profile: (10, 1, 30, 0, [3, 8, 15], 5, 8, 30) },
    Mrm40 { name: ("IS", "MRM-40"), part_id: 140, mass: 12288, bv: 224, skill: "Gunnery-Missile",
        profile: (12, 1, 40, 0, [3, 8, 15], 7, 6, 30) },
    StreakSrm2 { name: ("IS", "StreakSRM-2"), part_id: 143, mass: 1536, bv: 30, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    StreakSrm4 { name: ("IS", "StreakSRM-4"), part_id: 144, mass: 3072, bv: 59, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    StreakSrm6 { name: ("IS", "StreakSRM-6"), part_id: 145, mass: 4608, bv: 89, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    LrDfm5 { name: ("IS", "LR_DFM-5"), part_id: 25, mass: 2048, bv: 1000, skill: "Gunnery-Missile",
        profile: (2, 2, 5, 4, [6, 12, 18], 1, 24, 15) },
    LrDfm10 { name: ("IS", "LR_DFM-10"), part_id: 26, mass: 5120, bv: 1000, skill: "Gunnery-Missile",
        profile: (4, 2, 10, 4, [6, 12, 18], 2, 12, 20) },
    LrDfm15 { name: ("IS", "LR_DFM-15"), part_id: 27, mass: 7168, bv: 1000, skill: "Gunnery-Missile",
        profile: (5, 2, 15, 4, [6, 12, 18], 3, 8, 25) },
    LrDfm20 { name: ("IS", "LR_DFM-20"), part_id: 28, mass: 10240, bv: 1000, skill: "Gunnery-Missile",
        profile: (6, 2, 20, 4, [6, 12, 18], 5, 6, 30) },
    SrDfm2 { name: ("IS", "SR_DFM-2"), part_id: 29, mass: 1024, bv: 1000, skill: "Gunnery-Missile",
        profile: (2, 3, 2, 0, [2, 4, 6], 1, 50, 15) },
    SrDfm4 { name: ("IS", "SR_DFM-4"), part_id: 30, mass: 2048, bv: 1000, skill: "Gunnery-Missile",
        profile: (3, 3, 4, 0, [2, 4, 6], 1, 25, 15) },
    SrDfm6 { name: ("IS", "SR_DFM-6"), part_id: 31, mass: 3072, bv: 1000, skill: "Gunnery-Missile",
        profile: (4, 3, 6, 0, [2, 4, 6], 2, 15, 15) },
    Elrm5 { name: ("IS", "ELRM-5"), part_id: 21, mass: 6144, bv: 1000, skill: "Gunnery-Missile",
        profile: (3, 1, 5, 10, [12, 24, 36], 1, 18, 30) },
    Elrm10 { name: ("IS", "ELRM-10"), part_id: 22, mass: 8192, bv: 1000, skill: "Gunnery-Missile",
        profile: (6, 1, 10, 10, [12, 24, 36], 4, 9, 30) },
    Elrm15 { name: ("IS", "ELRM-15"), part_id: 23, mass: 12288, bv: 1000, skill: "Gunnery-Missile",
        profile: (8, 1, 15, 10, [12, 24, 36], 6, 6, 30) },
    Elrm20 { name: ("IS", "ELRM-20"), part_id: 24, mass: 18432, bv: 1000, skill: "Gunnery-Missile",
        profile: (10, 1, 20, 10, [12, 24, 36], 8, 4, 30) },
    Nlrm5 { name: ("IS", "NLRM-5"), part_id: 179, mass: 3072, bv: 67, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 3, [7, 14, 21], 2, 24, 15) },
    Nlrm10 { name: ("IS", "NLRM-10"), part_id: 180, mass: 6144, bv: 104, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 3, [7, 14, 21], 4, 12, 20) },
    Nlrm15 { name: ("IS", "NLRM-15"), part_id: 181, mass: 9216, bv: 157, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 3, [7, 14, 21], 6, 8, 25) },
    Nlrm20 { name: ("IS", "NLRM-20"), part_id: 182, mass: 12288, bv: 210, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 3, [7, 14, 21], 9, 6, 30) },
    Lrt5 { name: ("IS", "LRT-5"), part_id: 183, mass: 2048, bv: 45, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 6, [7, 14, 21], 1, 24, 15) },
    Lrt10 { name: ("IS", "LRT-10"), part_id: 184, mass: 5120, bv: 90, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 6, [7, 14, 21], 2, 12, 20) },
    Lrt15 { name: ("IS", "LRT-15"), part_id: 185, mass: 7168, bv: 136, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 6, [7, 14, 21], 3, 8, 25) },
    Lrt20 { name: ("IS", "LRT-20"), part_id: 186, mass: 10240, bv: 181, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 6, [7, 14, 21], 5, 6, 30) },
    Srt2 { name: ("IS", "SRT-2"), part_id: 187, mass: 1024, bv: 21, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    Srt4 { name: ("IS", "SRT-4"), part_id: 188, mass: 2048, bv: 39, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    Srt6 { name: ("IS", "SRT-6"), part_id: 189, mass: 3072, bv: 59, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    ClanLrt5 { name: ("CL", "LRT-5"), part_id: 190, mass: 1024, bv: 55, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 0, [7, 14, 21], 1, 24, 15) },
    ClanLrt10 { name: ("CL", "LRT-10"), part_id: 191, mass: 2560, bv: 109, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [7, 14, 21], 1, 12, 20) },
    ClanLrt15 { name: ("CL", "LRT-15"), part_id: 192, mass: 3584, bv: 164, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 0, [7, 14, 21], 2, 8, 25) },
    ClanLrt20 { name: ("CL", "LRT-20"), part_id: 193, mass: 5120, bv: 220, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [7, 14, 21], 4, 6, 20) },
    ClanSrt2 { name: ("CL", "SRT-2"), part_id: 194, mass: 512, bv: 21, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    ClanSrt4 { name: ("CL", "SRT-4"), part_id: 195, mass: 1024, bv: 39, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    ClanSrt6 { name: ("CL", "SRT-6"), part_id: 196, mass: 1536, bv: 59, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 1, 15, 15) },
    Lrm5 { name: ("IS", "LRM-5"), part_id: 130, mass: 2048, bv: 45, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 6, [7, 14, 21], 1, 24, 15) },
    Lrm10 { name: ("IS", "LRM-10"), part_id: 131, mass: 5120, bv: 90, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 6, [7, 14, 21], 2, 12, 20) },
    Lrm15 { name: ("IS", "LRM-15"), part_id: 132, mass: 7168, bv: 136, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 6, [7, 14, 21], 3, 8, 25) },
    Srm4 { name: ("IS", "SRM-4"), part_id: 135, mass: 2048, bv: 39, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    Srm6 { name: ("IS", "SRM-6"), part_id: 136, mass: 3072, bv: 59, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    Lrm20 { name: ("IS", "LRM-20"), part_id: 133, mass: 10240, bv: 181, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 6, [7, 14, 21], 5, 6, 30) },
    HeavyGaussRifle { name: ("IS", "HeavyGaussRifle"), part_id: 104, mass: 18432, bv: 346, skill: "Gunnery-Ballistic",
        profile: (2, 25, 0, 4, [6, 13, 20], 11, 4, 30) },
    GaussRifle { name: ("IS", "GaussRifle"), part_id: 102, mass: 15360, bv: 321, skill: "Gunnery-Ballistic",
        profile: (1, 15, 0, 2, [7, 15, 22], 7, 8, 30) },
    LightGaussRifle { name: ("IS", "LightGaussRifle"), part_id: 103, mass: 12288, bv: 159, skill: "Gunnery-Ballistic",
        profile: (1, 8, 0, 3, [8, 17, 25], 5, 16, 20) },
    MagshotGaussRifle { name: ("IS", "MagshotGaussRifle"), part_id: 160, mass: 512, bv: 15, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [3, 6, 9], 2, 50, 12) },
    Lbx2 { name: ("IS", "LB2-XAC"), part_id: 105, mass: 6144, bv: 42, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [9, 18, 27], 4, 45, 15) },
    Lbx5 { name: ("IS", "LB5-XAC"), part_id: 106, mass: 8192, bv: 83, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [7, 14, 21], 5, 20, 20) },
    Lbx10 { name: ("IS", "LB10-XAC"), part_id: 107, mass: 11264, bv: 148, skill: "Gunnery-Ballistic",
        profile: (2, 10, 0, 0, [6, 12, 18], 6, 10, 25) },
    Lbx20 { name: ("IS", "LB20-XAC"), part_id: 108, mass: 14336, bv: 237, skill: "Gunnery-Ballistic",
        profile: (6, 20, 0, 0, [4, 8, 12], 11, 5, 30) },
    Ac2 { name: ("IS", "AC/2"), part_id: 95, mass: 6144, bv: 37, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [8, 16, 24], 1, 45, 12) },
    Ac5 { name: ("IS", "AC/5"), part_id: 96, mass: 8192, bv: 70, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [6, 12, 18], 4, 20, 20) },
    Ac10 { name: ("IS", "AC/10"), part_id: 97, mass: 12288, bv: 124, skill: "Gunnery-Ballistic",
        profile: (3, 10, 0, 0, [5, 10, 15], 7, 10, 25) },
    Ac20 { name: ("IS", "AC/20"), part_id: 98, mass: 14336, bv: 178, skill: "Gunnery-Ballistic",
        profile: (7, 20, 0, 0, [3, 6, 9], 10, 5, 30) },
    UltraAc2 { name: ("IS", "UltraAC/2"), part_id: 111, mass: 7168, bv: 56, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [8, 17, 25], 3, 45, 12) },
    UltraAc5 { name: ("IS", "UltraAC/5"), part_id: 112, mass: 9216, bv: 113, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 2, [6, 13, 20], 5, 20, 20) },
    UltraAc10 { name: ("IS", "UltraAC/10"), part_id: 113, mass: 13312, bv: 253, skill: "Gunnery-Ballistic",
        profile: (4, 10, 0, 0, [6, 12, 18], 7, 10, 25) },
    UltraAc20 { name: ("IS", "UltraAC/20"), part_id: 114, mass: 15360, bv: 282, skill: "Gunnery-Ballistic",
        profile: (8, 20, 0, 0, [3, 7, 10], 10, 5, 30) },
    RotaryAc2 { name: ("IS", "RotaryAC/2"), part_id: 109, mass: 8192, bv: 118, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [6, 12, 18], 3, 45, 15) },
    RotaryAc5 { name: ("IS", "RotaryAC/5"), part_id: 110, mass: 10240, bv: 247, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [5, 10, 15], 6, 20, 22) },
    ClanRotaryAc2 { name: ("CL", "RotaryAC/2"), part_id: 164, mass: 7168, bv: 75, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 2, [9, 18, 27], 4, 45, 10) },
    ClanRotaryAc5 { name: ("CL", "RotaryAC/5"), part_id: 165, mass: 10240, bv: 345, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [7, 14, 21], 8, 20, 15) },
    ClanRotaryAc10 { name: ("CL", "RotaryAC/10"), part_id: 166, mass: 14336, bv: 250, skill: "Gunnery-Ballistic",
        profile: (3, 10, 0, 0, [6, 12, 18], 7, 10, 20) },
    ClanRotaryAc20 { name: ("CL", "RotaryAC/20"), part_id: 167, mass: 16384, bv: 400, skill: "Gunnery-Ballistic",
        profile: (7, 20, 0, 0, [4, 8, 12], 10, 5, 25) }
}

#[cfg(test)]
mod tests {
    use super::BattleWeapon;

    /// Recycle feedback follows each canonical equipment label with no namespace leakage.
    #[test]
    fn catalogue_recycle_feedback_matches_weapon_labels() {
        for &weapon in BattleWeapon::ALL {
            let (_, label) = weapon.name().split_once('.').unwrap();
            assert_eq!(
                weapon.recycle_notice(),
                format!("{label} finished recycling.")
            );
        }
    }
}
