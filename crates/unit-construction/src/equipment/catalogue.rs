//! Single source of weapon identities, catalog profiles, mass, Battle Value, construction cost,
//! skill family and recycle feedback.
//!
//! Battle Values follow the TechManual weapon and equipment BV tables (pp. 317-318) and the
//! Tactical Operations: Advanced Units & Equipment addenda (pp. 194-197). Prices follow the
//! TechManual equipment cost tables (pp. 290-294) and Tactical Operations: Advanced Units &
//! Equipment pp. 216-221.
use super::WeaponProfile;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Define catalog facts once while keeping typed, exhaustive dispatch and static notice text.
macro_rules! weapon_catalogue {
    ($($variant:ident {
        name: ($namespace:literal, $label:literal), part_id: $part_id:literal, mass: $mass:literal,
        bv: ($bv:literal, $one_shot_bv:literal, $ammunition_bv:literal), cost: $cost:literal, skill: $skill:literal,
        profile: ($heat:literal, $damage:literal, $missiles:literal, $minimum:literal,
                  [$short:literal, $medium:literal, $long:literal], $slots:literal,
                  $ammo:literal, $recycle:literal)
    }),+ $(,)?) => {
        /// Catalogue identities with typed rule dispatch; construction validation separately admits live equipment.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Weapon { $($variant),+ }

        impl Weapon {
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

            /// Intrinsic catalogue Battle Value (BV 2.0), before unit heat and movement
            /// adjustments: the first figure of the TechManual pp. 317-318 tables and their
            /// Tactical Operations: Advanced Units & Equipment pp. 194-197 addenda.
            pub fn battle_value(self) -> u16 {
                match self { $(Self::$variant => $bv),+ }
            }

            /// Battle Value of the one-shot (OS) version of this launcher, the figure after the
            /// slash in the BV tables (TechManual p. 317), or `None` when the launcher has no
            /// one-shot version.
            pub fn one_shot_battle_value(self) -> Option<u16> {
                let value: u16 = match self { $(Self::$variant => $one_shot_bv),+ };
                (value > 0).then_some(value)
            }

            /// Battle Value of one ton of this weapon's standard ammunition (TechManual
            /// pp. 317-318, "Ammo BV" per ton). Zero for weapons without ammunition and for
            /// pods whose ammunition carries no BV.
            pub fn ammunition_battle_value(self) -> u16 {
                match self { $(Self::$variant => $ammunition_bv),+ }
            }

            /// Construction cost in C-bills of one installed weapon (TechManual pp. 290-294;
            /// Tactical Operations: Advanced Units & Equipment pp. 216-221).
            pub fn cost(self) -> u64 {
                match self { $(Self::$variant => $cost),+ }
            }

            /// Biped gunnery skill, optionally specialized by weapon family.
            pub fn gunnery_skill(self, extended: bool) -> &'static str {
                if self.is_artillery() { return "Gunnery-Artillery"; }
                if !extended { return "Gunnery-Battlemech"; }
                match self { $(Self::$variant => $skill),+ }
            }

            /// Static cockpit feedback used by committed-second recycle processing.
            pub fn recycle_notice(self) -> &'static str {
                match self { $(Self::$variant => concat!($label, " finished recycling.")),+ }
            }
        }
    };
}

// Value columns: bv is (weapon BV, one-shot BV or 0 when no one-shot version exists, BV per ton
// of standard ammunition); cost is C-bills per installed weapon.
// Profile columns: heat, damage, missiles, minimum, [short, medium, long], slots, ammo/ton, recycle.
// The Long Tom's slot count is its Mech space for reference only: Tactical Operations: Advanced
// Units & Equipment p. 217 lists Mech space "NA", so `Weapon::mech_mountable` refuses it.
// The Clan plasma cannon deals no direct damage (TechManual p. 343, Total Warfare p. 139); the
// combat rules apply its heat or 3D6 damage instead.
// Indirect artillery (Arrow IV, Long Tom, Sniper, Thumper) lists its long range in mapsheets, as
// Tactical Operations p. 149 does; `Weapon::effective_range` converts it to hexes.
weapon_catalogue! {
    ArrowIv { name: ("IS", "ArrowIVSystem"), part_id: 115, mass: 15360, bv: (240, 0, 30), cost: 450000, skill: "Gunnery-Artillery",
        profile: (10, 20, 0, 0, [0, 0, 8], 15, 5, 60) },
    ClanArrowIv { name: ("CL", "ArrowIVSystem"), part_id: 64, mass: 12288, bv: (240, 0, 30), cost: 450000, skill: "Gunnery-Artillery",
        profile: (10, 20, 0, 0, [0, 0, 9], 12, 5, 60) },
    LongTom { name: ("IS", "LongTom"), part_id: 5, mass: 30720, bv: (368, 0, 46), cost: 450000, skill: "Gunnery-Artillery",
        profile: (20, 25, 0, 0, [0, 0, 30], 30, 5, 60) },
    Sniper { name: ("IS", "Sniper"), part_id: 116, mass: 20480, bv: (85, 0, 11), cost: 300000, skill: "Gunnery-Artillery",
        profile: (10, 20, 0, 0, [0, 0, 18], 20, 10, 60) },
    Thumper { name: ("IS", "Thumper"), part_id: 117, mass: 15360, bv: (43, 0, 5), cost: 187500, skill: "Gunnery-Artillery",
        profile: (6, 15, 0, 0, [0, 0, 21], 15, 20, 60) },
    LongTomCannon { name: ("IS", "LongTomCannon"), part_id: 121, mass: 20480, bv: (329, 0, 41), cost: 650000, skill: "Gunnery-Artillery",
        profile: (20, 20, 0, 4, [6, 13, 20], 15, 5, 30) },
    SniperCannon { name: ("IS", "SniperCannon"), part_id: 122, mass: 15360, bv: (77, 0, 10), cost: 475000, skill: "Gunnery-Artillery",
        profile: (10, 10, 0, 2, [4, 8, 12], 10, 10, 25) },
    ThumperCannon { name: ("IS", "ThumperCannon"), part_id: 123, mass: 10240, bv: (41, 0, 5), cost: 200000, skill: "Gunnery-Artillery",
        profile: (5, 5, 0, 3, [4, 9, 14], 7, 20, 25) },
    APod { name: ("IS", "A-Pod"), part_id: 159, mass: 512, bv: (1, 0, 0), cost: 1500, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    ClanAPod { name: ("CL", "A-Pod"), part_id: 1, mass: 512, bv: (1, 0, 0), cost: 1500, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    // Anti-battle armor pods: one ton, one slot, one shot, 2,500 C-bills and BV 2 on both
    // technology bases (TechManual pp. 204, 341-343, 317-318).
    BPod { name: ("IS", "B-Pod"), part_id: 197, mass: 1024, bv: (2, 0, 0), cost: 2500, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    ClanBPod { name: ("CL", "B-Pod"), part_id: 198, mass: 1024, bv: (2, 0, 0), cost: 2500, skill: "Gunnery-Laser",
        profile: (0, 0, 0, 0, [1, 1, 1], 1, 0, 30) },
    INarcBeacon { name: ("IS", "iNarcBeacon"), part_id: 142, mass: 5120, bv: (75, 15, 0), cost: 250000, skill: "Gunnery-Missile",
        profile: (0, 6, 1, 0, [4, 9, 15], 3, 4, 30) },
    NarcBeacon { name: ("IS", "NarcBeacon"), part_id: 141, mass: 3072, bv: (30, 6, 0), cost: 100000, skill: "Gunnery-Missile",
        profile: (0, 4, 1, 0, [3, 6, 9], 2, 6, 30) },
    ClanNarcBeacon { name: ("CL", "NarcBeacon"), part_id: 73, mass: 2048, bv: (30, 6, 0), cost: 100000, skill: "Gunnery-Missile",
        profile: (0, 4, 1, 0, [4, 8, 12], 1, 6, 30) },
    AntiMissileSystem { name: ("IS", "Anti-MissileSystem"), part_id: 101, mass: 512, bv: (32, 0, 11), cost: 100000, skill: "Gunnery-Missile",
        profile: (1, 2, 0, 0, [1, 1, 1], 1, 12, 10) },
    ClanAntiMissileSystem { name: ("CL", "Anti-MissileSystem"), part_id: 51, mass: 512, bv: (32, 0, 22), cost: 100000, skill: "Gunnery-Missile",
        profile: (1, 2, 0, 0, [1, 1, 1], 1, 24, 10) },
    LaserAms { name: ("IS", "LaserAMS"), part_id: 170, mass: 1536, bv: (45, 0, 0), cost: 225000, skill: "Gunnery-Laser",
        profile: (7, 2, 0, 0, [1, 1, 1], 2, 0, 25) },
    ClanLaserAms { name: ("CL", "LaserAMS"), part_id: 169, mass: 1024, bv: (45, 0, 0), cost: 225000, skill: "Gunnery-Laser",
        profile: (5, 2, 0, 0, [1, 1, 1], 1, 0, 25) },
    ClanLbx2 { name: ("CL", "LB2-XAC"), part_id: 53, mass: 5120, bv: (47, 0, 6), cost: 150000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [10, 20, 30], 3, 45, 15) },
    ClanLbx5 { name: ("CL", "LB5-XAC"), part_id: 54, mass: 7168, bv: (93, 0, 12), cost: 250000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [8, 15, 24], 4, 20, 20) },
    ClanLbx10 { name: ("CL", "LB10-XAC"), part_id: 55, mass: 10240, bv: (148, 0, 19), cost: 400000, skill: "Gunnery-Ballistic",
        profile: (2, 10, 0, 0, [6, 12, 18], 5, 10, 25) },
    ClanLbx20 { name: ("CL", "LB20-XAC"), part_id: 56, mass: 12288, bv: (237, 0, 30), cost: 600000, skill: "Gunnery-Ballistic",
        profile: (6, 20, 0, 0, [4, 8, 12], 9, 5, 30) },
    ClanUltraAc2 { name: ("CL", "UltraAC/2"), part_id: 60, mass: 5120, bv: (62, 0, 8), cost: 120000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 2, [9, 18, 27], 2, 45, 12) },
    ClanUltraAc5 { name: ("CL", "UltraAC/5"), part_id: 61, mass: 7168, bv: (122, 0, 15), cost: 200000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [7, 14, 21], 3, 20, 20) },
    ClanUltraAc10 { name: ("CL", "UltraAC/10"), part_id: 62, mass: 10240, bv: (210, 0, 26), cost: 320000, skill: "Gunnery-Ballistic",
        profile: (3, 10, 0, 0, [6, 12, 18], 4, 10, 25) },
    ClanUltraAc20 { name: ("CL", "UltraAC/20"), part_id: 63, mass: 12288, bv: (335, 0, 42), cost: 480000, skill: "Gunnery-Ballistic",
        profile: (7, 20, 0, 0, [4, 8, 12], 8, 5, 30) },
    Mml3 { name: ("IS", "MML-3"), part_id: 126, mass: 1536, bv: (29, 0, 4), cost: 45000, skill: "Gunnery-Missile",
        profile: (2, 2, 3, 0, [3, 6, 9], 2, 33, 30) },
    Mml5 { name: ("IS", "MML-5"), part_id: 127, mass: 3072, bv: (45, 0, 6), cost: 75000, skill: "Gunnery-Missile",
        profile: (3, 2, 5, 0, [3, 6, 9], 3, 20, 30) },
    Mml7 { name: ("IS", "MML-7"), part_id: 128, mass: 4608, bv: (67, 0, 8), cost: 105000, skill: "Gunnery-Missile",
        profile: (4, 2, 7, 0, [3, 6, 9], 4, 14, 30) },
    Mml9 { name: ("IS", "MML-9"), part_id: 129, mass: 6144, bv: (86, 0, 11), cost: 125000, skill: "Gunnery-Missile",
        profile: (5, 2, 9, 0, [3, 6, 9], 5, 11, 30) },
    ClanAtm3 { name: ("CL", "ATM-3"), part_id: 65, mass: 1536, bv: (53, 0, 14), cost: 50000, skill: "Gunnery-Missile",
        profile: (2, 2, 3, 4, [5, 10, 15], 2, 20, 15) },
    ClanAtm6 { name: ("CL", "ATM-6"), part_id: 66, mass: 3584, bv: (105, 0, 26), cost: 125000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 4, [5, 10, 15], 3, 10, 20) },
    ClanAtm9 { name: ("CL", "ATM-9"), part_id: 67, mass: 5120, bv: (147, 0, 36), cost: 225000, skill: "Gunnery-Missile",
        profile: (6, 2, 9, 4, [5, 10, 15], 4, 7, 25) },
    ClanAtm12 { name: ("CL", "ATM-12"), part_id: 68, mass: 7168, bv: (212, 0, 52), cost: 350000, skill: "Gunnery-Missile",
        profile: (8, 2, 12, 4, [5, 10, 15], 5, 5, 30) },
    ClanLrm5 { name: ("CL", "LRM-5"), part_id: 69, mass: 1024, bv: (55, 11, 7), cost: 30000, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 0, [7, 14, 21], 1, 24, 15) },
    ClanLrm10 { name: ("CL", "LRM-10"), part_id: 70, mass: 2560, bv: (109, 22, 14), cost: 100000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [7, 14, 21], 1, 12, 20) },
    ClanLrm15 { name: ("CL", "LRM-15"), part_id: 71, mass: 3584, bv: (164, 33, 21), cost: 175000, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 0, [7, 14, 21], 2, 8, 25) },
    ClanLrm20 { name: ("CL", "LRM-20"), part_id: 72, mass: 5120, bv: (220, 44, 27), cost: 250000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [7, 14, 21], 4, 6, 20) },
    ClanSrm2 { name: ("CL", "SRM-2"), part_id: 74, mass: 512, bv: (21, 4, 3), cost: 10000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    ClanSrm4 { name: ("CL", "SRM-4"), part_id: 75, mass: 1024, bv: (39, 8, 5), cost: 60000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    ClanSrm6 { name: ("CL", "SRM-6"), part_id: 76, mass: 1536, bv: (59, 12, 7), cost: 80000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 1, 15, 15) },
    ClanStreakSrm2 { name: ("CL", "StreakSRM-2"), part_id: 2, mass: 1024, bv: (40, 8, 5), cost: 15000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [4, 8, 12], 1, 50, 15) },
    ClanStreakSrm4 { name: ("CL", "StreakSRM-4"), part_id: 3, mass: 2048, bv: (79, 16, 10), cost: 90000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [4, 8, 12], 1, 25, 15) },
    ClanStreakSrm6 { name: ("CL", "StreakSRM-6"), part_id: 4, mass: 3072, bv: (118, 24, 15), cost: 120000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [4, 8, 12], 2, 15, 15) },
    ClanStreakLrm5 { name: ("CL", "StreakLRM-5"), part_id: 155, mass: 2048, bv: (86, 17, 11), cost: 75000, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 0, [7, 14, 21], 1, 24, 15) },
    ClanStreakLrm10 { name: ("CL", "StreakLRM-10"), part_id: 156, mass: 5120, bv: (173, 35, 22), cost: 225000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [7, 14, 21], 2, 12, 20) },
    ClanStreakLrm15 { name: ("CL", "StreakLRM-15"), part_id: 157, mass: 7168, bv: (259, 52, 32), cost: 400000, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 0, [7, 14, 21], 3, 8, 25) },
    ClanStreakLrm20 { name: ("CL", "StreakLRM-20"), part_id: 158, mass: 10240, bv: (345, 69, 43), cost: 600000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [7, 14, 21], 5, 6, 30) },
    ClanGaussRifle { name: ("CL", "GaussRifle"), part_id: 52, mass: 12288, bv: (320, 0, 40), cost: 300000, skill: "Gunnery-Ballistic",
        profile: (1, 15, 0, 2, [7, 15, 22], 6, 8, 30) },
    ClanApGaussRifle { name: ("CL", "APGaussRifle"), part_id: 162, mass: 512, bv: (21, 0, 3), cost: 10000, skill: "Gunnery-Ballistic",
        profile: (1, 3, 0, 0, [3, 6, 9], 1, 40, 10) },
    ClanHag20 { name: ("CL", "HAG/20"), part_id: 163, mass: 10240, bv: (267, 0, 33), cost: 400000, skill: "Gunnery-Ballistic",
        profile: (4, 20, 0, 2, [8, 16, 24], 6, 6, 25) },
    ClanHag30 { name: ("CL", "HAG/30"), part_id: 166, mass: 13312, bv: (401, 0, 50), cost: 500000, skill: "Gunnery-Ballistic",
        profile: (6, 30, 0, 2, [8, 16, 24], 8, 4, 30) },
    ClanHag40 { name: ("CL", "HAG/40"), part_id: 167, mass: 16384, bv: (535, 0, 67), cost: 600000, skill: "Gunnery-Ballistic",
        profile: (8, 40, 0, 2, [8, 16, 24], 10, 3, 30) },
    ClanMachineGun { name: ("CL", "MachineGun"), part_id: 57, mass: 256, bv: (5, 0, 1), cost: 5000, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [1, 2, 3], 1, 200, 7) },
    ClanLightMachineGun { name: ("CL", "LightMachineGun"), part_id: 58, mass: 256, bv: (5, 0, 1), cost: 5000, skill: "Gunnery-Ballistic",
        profile: (0, 1, 0, 0, [2, 4, 6], 1, 200, 7) },
    ClanHeavyMachineGun { name: ("CL", "HeavyMachineGun"), part_id: 59, mass: 512, bv: (6, 0, 1), cost: 7500, skill: "Gunnery-Ballistic",
        profile: (0, 3, 0, 0, [1, 2, 2], 1, 100, 7) },
    ClanErLargeLaser { name: ("CL", "ERLargeLaser"), part_id: 35, mass: 4096, bv: (248, 0, 0), cost: 200000, skill: "Gunnery-Laser",
        profile: (12, 10, 0, 0, [8, 15, 25], 1, 0, 20) },
    ClanErMediumLaser { name: ("CL", "ERMediumLaser"), part_id: 36, mass: 1024, bv: (108, 0, 0), cost: 80000, skill: "Gunnery-Laser",
        profile: (5, 7, 0, 0, [5, 10, 15], 1, 0, 15) },
    ClanErSmallLaser { name: ("CL", "ERSmallLaser"), part_id: 37, mass: 512, bv: (31, 0, 0), cost: 11250, skill: "Gunnery-Laser",
        profile: (2, 5, 0, 0, [2, 4, 6], 1, 0, 10) },
    ClanErMicroLaser { name: ("CL", "ERMicroLaser"), part_id: 38, mass: 256, bv: (7, 0, 0), cost: 10000, skill: "Gunnery-Laser",
        profile: (1, 2, 0, 0, [1, 2, 4], 1, 0, 15) },
    ClanErPpc { name: ("CL", "ERPPC"), part_id: 39, mass: 6144, bv: (412, 0, 0), cost: 300000, skill: "Gunnery-Laser",
        profile: (15, 15, 0, 0, [7, 14, 23], 2, 0, 25) },
    ClanFlamer { name: ("CL", "Flamer"), part_id: 40, mass: 512, bv: (6, 0, 0), cost: 7500, skill: "Gunnery-Laser",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 0, 10) },
    ClanHeavyLargeLaser { name: ("CL", "HeavyLargeLaser"), part_id: 41, mass: 4096, bv: (244, 0, 0), cost: 250000, skill: "Gunnery-Laser",
        profile: (18, 16, 0, 0, [5, 10, 15], 3, 0, 30) },
    ClanHeavyMediumLaser { name: ("CL", "HeavyMediumLaser"), part_id: 42, mass: 1024, bv: (76, 0, 0), cost: 100000, skill: "Gunnery-Laser",
        profile: (7, 10, 0, 0, [3, 6, 9], 2, 0, 25) },
    ClanHeavySmallLaser { name: ("CL", "HeavySmallLaser"), part_id: 43, mass: 512, bv: (15, 0, 0), cost: 20000, skill: "Gunnery-Laser",
        profile: (3, 6, 0, 0, [1, 2, 3], 1, 0, 20) },
    ClanLargePulseLaser { name: ("CL", "LargePulseLaser"), part_id: 44, mass: 6144, bv: (265, 0, 0), cost: 175000, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 0, [6, 14, 20], 2, 0, 23) },
    ClanMediumPulseLaser { name: ("CL", "MediumPulseLaser"), part_id: 45, mass: 2048, bv: (111, 0, 0), cost: 60000, skill: "Gunnery-Laser",
        profile: (4, 7, 0, 0, [4, 8, 12], 1, 0, 18) },
    ClanSmallPulseLaser { name: ("CL", "SmallPulseLaser"), part_id: 46, mass: 1024, bv: (24, 0, 0), cost: 16000, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [2, 4, 6], 1, 0, 13) },
    ClanMicroPulseLaser { name: ("CL", "MicroPulseLaser"), part_id: 47, mass: 512, bv: (12, 0, 0), cost: 12500, skill: "Gunnery-Laser",
        profile: (1, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    ClanErLargePulseLaser { name: ("CL", "ERLargePulseLaser"), part_id: 48, mass: 6144, bv: (272, 0, 0), cost: 400000, skill: "Gunnery-Laser",
        profile: (13, 10, 0, 0, [7, 15, 23], 3, 0, 30) },
    ClanErMediumPulseLaser { name: ("CL", "ERMediumPulseLaser"), part_id: 49, mass: 2048, bv: (117, 0, 0), cost: 150000, skill: "Gunnery-Laser",
        profile: (6, 7, 0, 0, [5, 9, 14], 2, 0, 30) },
    ClanErSmallPulseLaser { name: ("CL", "ERSmallPulseLaser"), part_id: 50, mass: 1536, bv: (36, 0, 0), cost: 30000, skill: "Gunnery-Laser",
        profile: (3, 5, 0, 0, [2, 4, 6], 1, 0, 30) },
    ClanPlasmaCannon { name: ("CL", "PlasmaCannon"), part_id: 168, mass: 3072, bv: (170, 0, 21), cost: 320000, skill: "Gunnery-Laser",
        profile: (7, 0, 0, 0, [6, 12, 18], 1, 10, 25) },
    Flamer { name: ("IS", "Flamer"), part_id: 81, mass: 1024, bv: (6, 0, 0), cost: 7500, skill: "Gunnery-Laser",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 0, 10) },
    HeavyFlamer { name: ("IS", "HeavyFlamer"), part_id: 118, mass: 1536, bv: (15, 0, 2), cost: 11250, skill: "Gunnery-Ballistic",
        profile: (5, 4, 0, 0, [2, 3, 4], 1, 10, 15) },
    VehicleFlamer { name: ("IS", "VehicleFlamer"), part_id: 99, mass: 512, bv: (5, 0, 1), cost: 7500, skill: "Gunnery-Ballistic",
        profile: (3, 2, 0, 0, [1, 2, 3], 1, 20, 10) },
    PlasmaRifle { name: ("IS", "PlasmaRifle"), part_id: 125, mass: 6144, bv: (210, 0, 26), cost: 260000, skill: "Gunnery-Ballistic",
        profile: (10, 10, 0, 0, [5, 10, 15], 2, 10, 30) },
    Thunderbolt5 { name: ("IS", "Thunderbolt-5"), part_id: 149, mass: 3072, bv: (64, 13, 8), cost: 50000, skill: "Gunnery-Missile",
        profile: (3, 5, 1, 5, [6, 12, 18], 1, 12, 20) },
    Thunderbolt10 { name: ("IS", "Thunderbolt-10"), part_id: 150, mass: 7168, bv: (127, 25, 16), cost: 175000, skill: "Gunnery-Missile",
        profile: (5, 10, 1, 5, [6, 12, 18], 2, 6, 20) },
    Thunderbolt15 { name: ("IS", "Thunderbolt-15"), part_id: 151, mass: 11264, bv: (229, 46, 29), cost: 325000, skill: "Gunnery-Missile",
        profile: (7, 15, 1, 5, [6, 12, 18], 3, 4, 30) },
    Thunderbolt20 { name: ("IS", "Thunderbolt-20"), part_id: 152, mass: 15360, bv: (305, 61, 38), cost: 450000, skill: "Gunnery-Missile",
        profile: (8, 20, 1, 5, [6, 12, 18], 5, 3, 30) },
    HyperAc2 { name: ("IS", "HyperAC/2"), part_id: 32, mass: 8192, bv: (53, 0, 7), cost: 100000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 3, [10, 20, 35], 2, 30, 12) },
    HyperAc5 { name: ("IS", "HyperAC/5"), part_id: 33, mass: 12288, bv: (109, 0, 14), cost: 160000, skill: "Gunnery-Ballistic",
        profile: (3, 5, 0, 0, [8, 16, 28], 4, 15, 20) },
    HyperAc10 { name: ("IS", "HyperAC/10"), part_id: 34, mass: 14336, bv: (158, 0, 20), cost: 230000, skill: "Gunnery-Ballistic",
        profile: (7, 10, 0, 0, [6, 12, 20], 6, 8, 25) },
    MachineGun { name: ("IS", "MachineGun"), part_id: 100, mass: 512, bv: (5, 0, 1), cost: 5000, skill: "Gunnery-Ballistic",
        profile: (0, 2, 0, 0, [1, 2, 3], 1, 200, 7) },
    LightMachineGun { name: ("IS", "LightMachineGun"), part_id: 161, mass: 512, bv: (5, 0, 1), cost: 5000, skill: "Gunnery-Ballistic",
        profile: (0, 1, 0, 0, [2, 4, 6], 1, 200, 7) },
    HeavyMachineGun { name: ("IS", "HeavyMachineGun"), part_id: 124, mass: 1024, bv: (6, 0, 1), cost: 7500, skill: "Gunnery-Ballistic",
        profile: (0, 3, 0, 0, [1, 2, 2], 1, 100, 7) },
    LightAc2 { name: ("IS", "LightAC/2"), part_id: 119, mass: 4096, bv: (30, 0, 4), cost: 100000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [6, 12, 18], 1, 45, 12) },
    LightAc5 { name: ("IS", "LightAC/5"), part_id: 120, mass: 5120, bv: (62, 0, 8), cost: 150000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [5, 10, 15], 2, 20, 20) },
    SmallLaser { name: ("IS", "SmallLaser"), part_id: 80, mass: 512, bv: (9, 0, 0), cost: 11250, skill: "Gunnery-Laser",
        profile: (1, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    MediumLaser { name: ("IS", "MediumLaser"), part_id: 79, mass: 1024, bv: (46, 0, 0), cost: 40000, skill: "Gunnery-Laser",
        profile: (3, 5, 0, 0, [3, 6, 9], 1, 0, 20) },
    LargeLaser { name: ("IS", "LargeLaser"), part_id: 78, mass: 5120, bv: (123, 0, 0), cost: 100000, skill: "Gunnery-Laser",
        profile: (8, 8, 0, 0, [5, 10, 15], 2, 0, 25) },
    Ppc { name: ("IS", "PPC"), part_id: 77, mass: 7168, bv: (176, 0, 0), cost: 200000, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 3, [6, 12, 18], 3, 0, 30) },
    ErSmallLaser { name: ("IS", "ERSmallLaser"), part_id: 85, mass: 512, bv: (17, 0, 0), cost: 11250, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [2, 4, 5], 1, 0, 15) },
    ErMediumLaser { name: ("IS", "ERMediumLaser"), part_id: 84, mass: 1024, bv: (62, 0, 0), cost: 80000, skill: "Gunnery-Laser",
        profile: (5, 5, 0, 0, [4, 8, 12], 1, 0, 20) },
    ErLargeLaser { name: ("IS", "ERLargeLaser"), part_id: 83, mass: 5120, bv: (163, 0, 0), cost: 200000, skill: "Gunnery-Laser",
        profile: (12, 8, 0, 0, [7, 14, 19], 2, 0, 25) },
    ErPpc { name: ("IS", "ERPPC"), part_id: 82, mass: 7168, bv: (229, 0, 0), cost: 300000, skill: "Gunnery-Laser",
        profile: (15, 10, 0, 0, [7, 14, 23], 3, 0, 30) },
    SmallPulseLaser { name: ("IS", "SmallPulseLaser"), part_id: 88, mass: 1024, bv: (12, 0, 0), cost: 16000, skill: "Gunnery-Laser",
        profile: (2, 3, 0, 0, [1, 2, 3], 1, 0, 15) },
    MediumPulseLaser { name: ("IS", "MediumPulseLaser"), part_id: 87, mass: 2048, bv: (48, 0, 0), cost: 60000, skill: "Gunnery-Laser",
        profile: (4, 6, 0, 0, [2, 4, 6], 1, 0, 20) },
    LargePulseLaser { name: ("IS", "LargePulseLaser"), part_id: 86, mass: 7168, bv: (119, 0, 0), cost: 175000, skill: "Gunnery-Laser",
        profile: (10, 9, 0, 0, [3, 7, 10], 2, 0, 25) },
    XSmallPulseLaser { name: ("IS", "X-SmallPulseLaser"), part_id: 91, mass: 1024, bv: (21, 0, 0), cost: 31000, skill: "Gunnery-Laser",
        profile: (3, 3, 0, 0, [2, 4, 5], 1, 0, 17) },
    XMediumPulseLaser { name: ("IS", "X-MediumPulseLaser"), part_id: 90, mass: 2048, bv: (71, 0, 0), cost: 110000, skill: "Gunnery-Laser",
        profile: (6, 6, 0, 0, [3, 6, 9], 1, 0, 22) },
    XLargePulseLaser { name: ("IS", "X-LargePulseLaser"), part_id: 89, mass: 7168, bv: (178, 0, 0), cost: 275000, skill: "Gunnery-Laser",
        profile: (14, 9, 0, 0, [5, 10, 15], 2, 0, 27) },
    LightPpc { name: ("IS", "LightPPC"), part_id: 93, mass: 3072, bv: (88, 0, 0), cost: 150000, skill: "Gunnery-Laser",
        profile: (5, 5, 0, 3, [6, 12, 18], 2, 0, 30) },
    HeavyPpc { name: ("IS", "HeavyPPC"), part_id: 94, mass: 10240, bv: (317, 0, 0), cost: 250000, skill: "Gunnery-Laser",
        profile: (15, 15, 0, 3, [6, 12, 18], 4, 0, 30) },
    SnubNosedPpc { name: ("IS", "SnubNosedPPC"), part_id: 92, mass: 6144, bv: (165, 0, 0), cost: 300000, skill: "Gunnery-Laser",
        profile: (10, 10, 0, 0, [9, 13, 15], 2, 0, 30) },
    Srm2 { name: ("IS", "SRM-2"), part_id: 134, mass: 1024, bv: (21, 4, 3), cost: 10000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    Rocket10 { name: ("IS", "RL-10"), part_id: 146, mass: 512, bv: (18, 0, 0), cost: 15000, skill: "Gunnery-Missile",
        profile: (3, 1, 10, 0, [5, 11, 18], 1, 0, 30) },
    Rocket15 { name: ("IS", "RL-15"), part_id: 147, mass: 1024, bv: (23, 0, 0), cost: 30000, skill: "Gunnery-Missile",
        profile: (4, 1, 15, 0, [4, 9, 15], 2, 0, 30) },
    Rocket20 { name: ("IS", "RL-20"), part_id: 148, mass: 1536, bv: (24, 0, 0), cost: 45000, skill: "Gunnery-Missile",
        profile: (5, 1, 20, 0, [3, 7, 12], 3, 0, 30) },
    Mrm10 { name: ("IS", "MRM-10"), part_id: 137, mass: 3072, bv: (56, 11, 7), cost: 50000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [3, 8, 15], 2, 24, 20) },
    Mrm20 { name: ("IS", "MRM-20"), part_id: 138, mass: 7168, bv: (112, 22, 14), cost: 125000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [3, 8, 15], 3, 12, 30) },
    Mrm30 { name: ("IS", "MRM-30"), part_id: 139, mass: 10240, bv: (168, 34, 21), cost: 225000, skill: "Gunnery-Missile",
        profile: (10, 1, 30, 0, [3, 8, 15], 5, 8, 30) },
    Mrm40 { name: ("IS", "MRM-40"), part_id: 140, mass: 12288, bv: (224, 45, 28), cost: 350000, skill: "Gunnery-Missile",
        profile: (12, 1, 40, 0, [3, 8, 15], 7, 6, 30) },
    StreakSrm2 { name: ("IS", "StreakSRM-2"), part_id: 143, mass: 1536, bv: (30, 6, 4), cost: 15000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    StreakSrm4 { name: ("IS", "StreakSRM-4"), part_id: 144, mass: 3072, bv: (59, 12, 7), cost: 90000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    StreakSrm6 { name: ("IS", "StreakSRM-6"), part_id: 145, mass: 4608, bv: (89, 18, 11), cost: 120000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    Elrm5 { name: ("IS", "ELRM-5"), part_id: 21, mass: 6144, bv: (67, 13, 8), cost: 60000, skill: "Gunnery-Missile",
        profile: (3, 1, 5, 10, [12, 22, 38], 1, 18, 30) },
    Elrm10 { name: ("IS", "ELRM-10"), part_id: 22, mass: 8192, bv: (133, 27, 17), cost: 200000, skill: "Gunnery-Missile",
        profile: (6, 1, 10, 10, [12, 22, 38], 4, 9, 30) },
    Elrm15 { name: ("IS", "ELRM-15"), part_id: 23, mass: 12288, bv: (200, 40, 25), cost: 350000, skill: "Gunnery-Missile",
        profile: (8, 1, 15, 10, [12, 22, 38], 6, 6, 30) },
    Elrm20 { name: ("IS", "ELRM-20"), part_id: 24, mass: 18432, bv: (268, 54, 34), cost: 500000, skill: "Gunnery-Missile",
        profile: (10, 1, 20, 10, [12, 22, 38], 8, 4, 30) },
    Nlrm5 { name: ("IS", "NLRM-5"), part_id: 179, mass: 3072, bv: (52, 10, 7), cost: 37500, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 3, [7, 14, 21], 2, 24, 15) },
    Nlrm10 { name: ("IS", "NLRM-10"), part_id: 180, mass: 6144, bv: (104, 21, 13), cost: 125000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 3, [7, 14, 21], 4, 12, 20) },
    Nlrm15 { name: ("IS", "NLRM-15"), part_id: 181, mass: 9216, bv: (157, 31, 20), cost: 218750, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 3, [7, 14, 21], 6, 8, 25) },
    Nlrm20 { name: ("IS", "NLRM-20"), part_id: 182, mass: 12288, bv: (210, 42, 26), cost: 312500, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 3, [7, 14, 21], 9, 6, 30) },
    Lrt5 { name: ("IS", "LRT-5"), part_id: 183, mass: 2048, bv: (45, 0, 6), cost: 30000, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 6, [7, 14, 21], 1, 24, 15) },
    Lrt10 { name: ("IS", "LRT-10"), part_id: 184, mass: 5120, bv: (90, 0, 11), cost: 100000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 6, [7, 14, 21], 2, 12, 20) },
    Lrt15 { name: ("IS", "LRT-15"), part_id: 185, mass: 7168, bv: (136, 0, 17), cost: 175000, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 6, [7, 14, 21], 3, 8, 25) },
    Lrt20 { name: ("IS", "LRT-20"), part_id: 186, mass: 10240, bv: (181, 0, 23), cost: 250000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 6, [7, 14, 21], 5, 6, 30) },
    Srt2 { name: ("IS", "SRT-2"), part_id: 187, mass: 1024, bv: (21, 0, 3), cost: 10000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    Srt4 { name: ("IS", "SRT-4"), part_id: 188, mass: 2048, bv: (39, 0, 5), cost: 60000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    Srt6 { name: ("IS", "SRT-6"), part_id: 189, mass: 3072, bv: (59, 0, 7), cost: 80000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    ClanLrt5 { name: ("CL", "LRT-5"), part_id: 190, mass: 1024, bv: (55, 0, 7), cost: 30000, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 0, [7, 14, 21], 1, 24, 15) },
    ClanLrt10 { name: ("CL", "LRT-10"), part_id: 191, mass: 2560, bv: (109, 0, 14), cost: 100000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 0, [7, 14, 21], 1, 12, 20) },
    ClanLrt15 { name: ("CL", "LRT-15"), part_id: 192, mass: 3584, bv: (164, 0, 21), cost: 175000, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 0, [7, 14, 21], 2, 8, 25) },
    ClanLrt20 { name: ("CL", "LRT-20"), part_id: 193, mass: 5120, bv: (220, 0, 27), cost: 250000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 0, [7, 14, 21], 4, 6, 20) },
    ClanSrt2 { name: ("CL", "SRT-2"), part_id: 194, mass: 512, bv: (21, 0, 3), cost: 10000, skill: "Gunnery-Missile",
        profile: (2, 2, 2, 0, [3, 6, 9], 1, 50, 15) },
    ClanSrt4 { name: ("CL", "SRT-4"), part_id: 195, mass: 1024, bv: (39, 0, 5), cost: 60000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    ClanSrt6 { name: ("CL", "SRT-6"), part_id: 196, mass: 1536, bv: (59, 0, 7), cost: 80000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 1, 15, 15) },
    Lrm5 { name: ("IS", "LRM-5"), part_id: 130, mass: 2048, bv: (45, 9, 6), cost: 30000, skill: "Gunnery-Missile",
        profile: (2, 1, 5, 6, [7, 14, 21], 1, 24, 15) },
    Lrm10 { name: ("IS", "LRM-10"), part_id: 131, mass: 5120, bv: (90, 18, 11), cost: 100000, skill: "Gunnery-Missile",
        profile: (4, 1, 10, 6, [7, 14, 21], 2, 12, 20) },
    Lrm15 { name: ("IS", "LRM-15"), part_id: 132, mass: 7168, bv: (136, 27, 17), cost: 175000, skill: "Gunnery-Missile",
        profile: (5, 1, 15, 6, [7, 14, 21], 3, 8, 25) },
    Srm4 { name: ("IS", "SRM-4"), part_id: 135, mass: 2048, bv: (39, 8, 5), cost: 60000, skill: "Gunnery-Missile",
        profile: (3, 2, 4, 0, [3, 6, 9], 1, 25, 15) },
    Srm6 { name: ("IS", "SRM-6"), part_id: 136, mass: 3072, bv: (59, 12, 7), cost: 80000, skill: "Gunnery-Missile",
        profile: (4, 2, 6, 0, [3, 6, 9], 2, 15, 15) },
    Lrm20 { name: ("IS", "LRM-20"), part_id: 133, mass: 10240, bv: (181, 36, 23), cost: 250000, skill: "Gunnery-Missile",
        profile: (6, 1, 20, 6, [7, 14, 21], 5, 6, 30) },
    HeavyGaussRifle { name: ("IS", "HeavyGaussRifle"), part_id: 104, mass: 18432, bv: (346, 0, 43), cost: 500000, skill: "Gunnery-Ballistic",
        profile: (2, 25, 0, 4, [6, 13, 20], 11, 4, 30) },
    GaussRifle { name: ("IS", "GaussRifle"), part_id: 102, mass: 15360, bv: (320, 0, 40), cost: 300000, skill: "Gunnery-Ballistic",
        profile: (1, 15, 0, 2, [7, 15, 22], 7, 8, 30) },
    LightGaussRifle { name: ("IS", "LightGaussRifle"), part_id: 103, mass: 12288, bv: (159, 0, 20), cost: 275000, skill: "Gunnery-Ballistic",
        profile: (1, 8, 0, 3, [8, 17, 25], 5, 16, 20) },
    MagshotGaussRifle { name: ("IS", "MagshotGaussRifle"), part_id: 160, mass: 512, bv: (15, 0, 2), cost: 8500, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [3, 6, 9], 2, 50, 12) },
    Lbx2 { name: ("IS", "LB2-XAC"), part_id: 105, mass: 6144, bv: (42, 0, 5), cost: 150000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [9, 18, 27], 4, 45, 15) },
    Lbx5 { name: ("IS", "LB5-XAC"), part_id: 106, mass: 8192, bv: (83, 0, 10), cost: 250000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [7, 14, 21], 5, 20, 20) },
    Lbx10 { name: ("IS", "LB10-XAC"), part_id: 107, mass: 11264, bv: (148, 0, 19), cost: 400000, skill: "Gunnery-Ballistic",
        profile: (2, 10, 0, 0, [6, 12, 18], 6, 10, 25) },
    Lbx20 { name: ("IS", "LB20-XAC"), part_id: 108, mass: 14336, bv: (237, 0, 30), cost: 600000, skill: "Gunnery-Ballistic",
        profile: (6, 20, 0, 0, [4, 8, 12], 11, 5, 30) },
    Ac2 { name: ("IS", "AC/2"), part_id: 95, mass: 6144, bv: (37, 0, 5), cost: 75000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 4, [8, 16, 24], 1, 45, 12) },
    Ac5 { name: ("IS", "AC/5"), part_id: 96, mass: 8192, bv: (70, 0, 9), cost: 125000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 3, [6, 12, 18], 4, 20, 20) },
    Ac10 { name: ("IS", "AC/10"), part_id: 97, mass: 12288, bv: (123, 0, 15), cost: 200000, skill: "Gunnery-Ballistic",
        profile: (3, 10, 0, 0, [5, 10, 15], 7, 10, 25) },
    Ac20 { name: ("IS", "AC/20"), part_id: 98, mass: 14336, bv: (178, 0, 22), cost: 300000, skill: "Gunnery-Ballistic",
        profile: (7, 20, 0, 0, [3, 6, 9], 10, 5, 30) },
    UltraAc2 { name: ("IS", "UltraAC/2"), part_id: 111, mass: 7168, bv: (56, 0, 7), cost: 120000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 3, [8, 17, 25], 3, 45, 12) },
    UltraAc5 { name: ("IS", "UltraAC/5"), part_id: 112, mass: 9216, bv: (112, 0, 14), cost: 200000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 2, [6, 13, 20], 5, 20, 20) },
    UltraAc10 { name: ("IS", "UltraAC/10"), part_id: 113, mass: 13312, bv: (210, 0, 26), cost: 320000, skill: "Gunnery-Ballistic",
        profile: (4, 10, 0, 0, [6, 12, 18], 7, 10, 25) },
    UltraAc20 { name: ("IS", "UltraAC/20"), part_id: 114, mass: 15360, bv: (281, 0, 35), cost: 480000, skill: "Gunnery-Ballistic",
        profile: (8, 20, 0, 0, [3, 7, 10], 10, 5, 30) },
    RotaryAc2 { name: ("IS", "RotaryAC/2"), part_id: 109, mass: 8192, bv: (118, 0, 15), cost: 175000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [6, 12, 18], 3, 45, 15) },
    RotaryAc5 { name: ("IS", "RotaryAC/5"), part_id: 110, mass: 10240, bv: (247, 0, 31), cost: 275000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [5, 10, 15], 6, 20, 22) },
    ClanRotaryAc2 { name: ("CL", "RotaryAC/2"), part_id: 164, mass: 8192, bv: (161, 0, 20), cost: 175000, skill: "Gunnery-Ballistic",
        profile: (1, 2, 0, 0, [8, 17, 25], 4, 45, 10) },
    ClanRotaryAc5 { name: ("CL", "RotaryAC/5"), part_id: 165, mass: 10240, bv: (345, 0, 43), cost: 275000, skill: "Gunnery-Ballistic",
        profile: (1, 5, 0, 0, [7, 14, 21], 8, 20, 15) }
}

#[cfg(test)]
mod tests {
    use super::Weapon;
    use super::Weapon as W;

    /// The whole Battle Value column: weapon BV, one-shot BV (0 for none) and ammunition BV per
    /// ton, from the TechManual pp. 317-318 tables and the Tactical Operations: Advanced Units
    /// & Equipment pp. 194-197 addenda.
    #[test]
    fn battle_values_match_the_published_tables() {
        let expected = [
            (W::ArrowIv, 240, 0, 30),
            (W::ClanArrowIv, 240, 0, 30),
            (W::LongTom, 368, 0, 46),
            (W::Sniper, 85, 0, 11),
            (W::Thumper, 43, 0, 5),
            (W::LongTomCannon, 329, 0, 41),
            (W::SniperCannon, 77, 0, 10),
            (W::ThumperCannon, 41, 0, 5),
            (W::APod, 1, 0, 0),
            (W::ClanAPod, 1, 0, 0),
            (W::BPod, 2, 0, 0),
            (W::ClanBPod, 2, 0, 0),
            (W::INarcBeacon, 75, 15, 0),
            (W::NarcBeacon, 30, 6, 0),
            (W::ClanNarcBeacon, 30, 6, 0),
            (W::AntiMissileSystem, 32, 0, 11),
            (W::ClanAntiMissileSystem, 32, 0, 22),
            (W::LaserAms, 45, 0, 0),
            (W::ClanLaserAms, 45, 0, 0),
            (W::ClanLbx2, 47, 0, 6),
            (W::ClanLbx5, 93, 0, 12),
            (W::ClanLbx10, 148, 0, 19),
            (W::ClanLbx20, 237, 0, 30),
            (W::ClanUltraAc2, 62, 0, 8),
            (W::ClanUltraAc5, 122, 0, 15),
            (W::ClanUltraAc10, 210, 0, 26),
            (W::ClanUltraAc20, 335, 0, 42),
            (W::Mml3, 29, 0, 4),
            (W::Mml5, 45, 0, 6),
            (W::Mml7, 67, 0, 8),
            (W::Mml9, 86, 0, 11),
            (W::ClanAtm3, 53, 0, 14),
            (W::ClanAtm6, 105, 0, 26),
            (W::ClanAtm9, 147, 0, 36),
            (W::ClanAtm12, 212, 0, 52),
            (W::ClanLrm5, 55, 11, 7),
            (W::ClanLrm10, 109, 22, 14),
            (W::ClanLrm15, 164, 33, 21),
            (W::ClanLrm20, 220, 44, 27),
            (W::ClanSrm2, 21, 4, 3),
            (W::ClanSrm4, 39, 8, 5),
            (W::ClanSrm6, 59, 12, 7),
            (W::ClanStreakSrm2, 40, 8, 5),
            (W::ClanStreakSrm4, 79, 16, 10),
            (W::ClanStreakSrm6, 118, 24, 15),
            (W::ClanStreakLrm5, 86, 17, 11),
            (W::ClanStreakLrm10, 173, 35, 22),
            (W::ClanStreakLrm15, 259, 52, 32),
            (W::ClanStreakLrm20, 345, 69, 43),
            (W::ClanGaussRifle, 320, 0, 40),
            (W::ClanMachineGun, 5, 0, 1),
            (W::ClanLightMachineGun, 5, 0, 1),
            (W::ClanHeavyMachineGun, 6, 0, 1),
            (W::ClanErLargeLaser, 248, 0, 0),
            (W::ClanErMediumLaser, 108, 0, 0),
            (W::ClanErSmallLaser, 31, 0, 0),
            (W::ClanErMicroLaser, 7, 0, 0),
            (W::ClanErPpc, 412, 0, 0),
            (W::ClanFlamer, 6, 0, 0),
            (W::ClanHeavyLargeLaser, 244, 0, 0),
            (W::ClanHeavyMediumLaser, 76, 0, 0),
            (W::ClanHeavySmallLaser, 15, 0, 0),
            (W::ClanLargePulseLaser, 265, 0, 0),
            (W::ClanMediumPulseLaser, 111, 0, 0),
            (W::ClanSmallPulseLaser, 24, 0, 0),
            (W::ClanMicroPulseLaser, 12, 0, 0),
            (W::ClanErLargePulseLaser, 272, 0, 0),
            (W::ClanErMediumPulseLaser, 117, 0, 0),
            (W::ClanErSmallPulseLaser, 36, 0, 0),
            (W::ClanPlasmaCannon, 170, 0, 21),
            (W::Flamer, 6, 0, 0),
            (W::HeavyFlamer, 15, 0, 2),
            (W::VehicleFlamer, 5, 0, 1),
            (W::PlasmaRifle, 210, 0, 26),
            (W::Thunderbolt5, 64, 13, 8),
            (W::Thunderbolt10, 127, 25, 16),
            (W::Thunderbolt15, 229, 46, 29),
            (W::Thunderbolt20, 305, 61, 38),
            (W::HyperAc2, 53, 0, 7),
            (W::HyperAc5, 109, 0, 14),
            (W::HyperAc10, 158, 0, 20),
            (W::MachineGun, 5, 0, 1),
            (W::HeavyMachineGun, 6, 0, 1),
            (W::LightAc2, 30, 0, 4),
            (W::LightAc5, 62, 0, 8),
            (W::SmallLaser, 9, 0, 0),
            (W::MediumLaser, 46, 0, 0),
            (W::LargeLaser, 123, 0, 0),
            (W::Ppc, 176, 0, 0),
            (W::ErSmallLaser, 17, 0, 0),
            (W::ErMediumLaser, 62, 0, 0),
            (W::ErLargeLaser, 163, 0, 0),
            (W::ErPpc, 229, 0, 0),
            (W::SmallPulseLaser, 12, 0, 0),
            (W::MediumPulseLaser, 48, 0, 0),
            (W::LargePulseLaser, 119, 0, 0),
            (W::XSmallPulseLaser, 21, 0, 0),
            (W::XMediumPulseLaser, 71, 0, 0),
            (W::XLargePulseLaser, 178, 0, 0),
            (W::LightPpc, 88, 0, 0),
            (W::HeavyPpc, 317, 0, 0),
            (W::SnubNosedPpc, 165, 0, 0),
            (W::Srm2, 21, 4, 3),
            (W::Rocket10, 18, 0, 0),
            (W::Rocket15, 23, 0, 0),
            (W::Rocket20, 24, 0, 0),
            (W::Mrm10, 56, 11, 7),
            (W::Mrm20, 112, 22, 14),
            (W::Mrm30, 168, 34, 21),
            (W::Mrm40, 224, 45, 28),
            (W::StreakSrm2, 30, 6, 4),
            (W::StreakSrm4, 59, 12, 7),
            (W::StreakSrm6, 89, 18, 11),
            (W::Elrm5, 67, 13, 8),
            (W::Elrm10, 133, 27, 17),
            (W::Elrm15, 200, 40, 25),
            (W::Elrm20, 268, 54, 34),
            (W::Nlrm5, 52, 10, 7),
            (W::Nlrm10, 104, 21, 13),
            (W::Nlrm15, 157, 31, 20),
            (W::Nlrm20, 210, 42, 26),
            (W::Lrt5, 45, 0, 6),
            (W::Lrt10, 90, 0, 11),
            (W::Lrt15, 136, 0, 17),
            (W::Lrt20, 181, 0, 23),
            (W::Srt2, 21, 0, 3),
            (W::Srt4, 39, 0, 5),
            (W::Srt6, 59, 0, 7),
            (W::ClanLrt5, 55, 0, 7),
            (W::ClanLrt10, 109, 0, 14),
            (W::ClanLrt15, 164, 0, 21),
            (W::ClanLrt20, 220, 0, 27),
            (W::ClanSrt2, 21, 0, 3),
            (W::ClanSrt4, 39, 0, 5),
            (W::ClanSrt6, 59, 0, 7),
            (W::Lrm5, 45, 9, 6),
            (W::Lrm10, 90, 18, 11),
            (W::Lrm15, 136, 27, 17),
            (W::Srm4, 39, 8, 5),
            (W::Srm6, 59, 12, 7),
            (W::Lrm20, 181, 36, 23),
            (W::HeavyGaussRifle, 346, 0, 43),
            (W::GaussRifle, 320, 0, 40),
            (W::LightGaussRifle, 159, 0, 20),
            (W::MagshotGaussRifle, 15, 0, 2),
            (W::Lbx2, 42, 0, 5),
            (W::Lbx5, 83, 0, 10),
            (W::Lbx10, 148, 0, 19),
            (W::Lbx20, 237, 0, 30),
            (W::Ac2, 37, 0, 5),
            (W::Ac5, 70, 0, 9),
            (W::Ac10, 123, 0, 15),
            (W::Ac20, 178, 0, 22),
            (W::UltraAc2, 56, 0, 7),
            (W::UltraAc5, 112, 0, 14),
            (W::UltraAc10, 210, 0, 26),
            (W::UltraAc20, 281, 0, 35),
            (W::RotaryAc2, 118, 0, 15),
            (W::RotaryAc5, 247, 0, 31),
            (W::ClanRotaryAc2, 161, 0, 20),
            (W::ClanRotaryAc5, 345, 0, 43),
            (W::ClanApGaussRifle, 21, 0, 3),
            (W::ClanHag20, 267, 0, 33),
            (W::ClanHag30, 401, 0, 50),
            (W::ClanHag40, 535, 0, 67),
            (W::LightMachineGun, 5, 0, 1),
        ];
        let listed: std::collections::BTreeSet<_> = expected.iter().map(|row| row.0).collect();
        assert_eq!(listed.len(), Weapon::ALL.len());
        for (weapon, bv, one_shot, ammunition) in expected {
            assert_eq!(weapon.battle_value(), bv, "{weapon:?}");
            assert_eq!(
                weapon.one_shot_battle_value(),
                (one_shot > 0).then_some(one_shot),
                "{weapon:?}"
            );
            assert_eq!(weapon.ammunition_battle_value(), ammunition, "{weapon:?}");
        }
    }

    /// Recycle feedback follows each canonical equipment label with no namespace leakage.
    #[test]
    fn catalogue_recycle_feedback_matches_weapon_labels() {
        for &weapon in Weapon::ALL {
            let (_, label) = weapon.name().split_once('.').unwrap();
            assert_eq!(
                weapon.recycle_notice(),
                format!("{label} finished recycling.")
            );
        }
    }
}
