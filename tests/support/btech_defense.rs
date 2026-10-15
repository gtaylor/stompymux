//! Representative AMS-equipped targets for shared firing and missile-flight acceptance.

/// Seven supported defender chassis include both Clan and Inner Sphere interception dice.
pub fn templates() -> Vec<String> {
    let tracked = include_str!("../../game/units/Goblin-58.toml");
    vec![
        include_str!("../../game/units/Daishi-A.toml").into(),
        include_str!("../../game/units/GOL-1H.toml").replace(
            "    { at = 4, item = \"Ammo_IS.MachineGun\", rounds = 200 },\n",
            "    { at = 4, item = \"Ammo_IS.MachineGun\", rounds = 200 },\n    { at = 11, item = \"IS.Anti-MissileSystem\" },\n    { at = 12, item = \"Ammo_IS.Anti-MissileSystem\", rounds = 12 },\n",
        ),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 4", "walk_mp = 0"),
        // A forward mount, so the defense covers a shooter the VTOL faces (Total Warfare p. 129).
        include_str!("../../game/units/Kestrel.toml").replace(
            "    { at = 3, item = \"Ammo_IS.MachineGun\", rounds = 100, modes = [\"Hotload\", \"Halfton\"] },\n",
            "    { at = 3, item = \"Ammo_IS.MachineGun\", rounds = 100, modes = [\"Hotload\", \"Halfton\"] },\n    { at = 4, item = \"IS.Anti-MissileSystem\" },\n    { at = 5, item = \"Ammo_IS.Anti-MissileSystem\", rounds = 12 },\n",
        ),
    ]
}
