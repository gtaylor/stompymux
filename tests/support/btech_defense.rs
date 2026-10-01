//! Representative AMS-equipped targets for shared firing and missile-flight acceptance.

/// Seven supported defender chassis include both Clan and Inner Sphere interception dice.
pub fn templates() -> Vec<String> {
    let tracked = include_str!("../../game/mechs/Goblin-58.toml");
    vec![
        include_str!("../../game/mechs/Daishi-A.toml").into(),
        include_str!("../../game/mechs/GOL-1H.toml").replace(
            "    { at = 4, item = \"Ammo_IS.MachineGun\", rounds = 200 },\n",
            "    { at = 4, item = \"Ammo_IS.MachineGun\", rounds = 200 },\n    { at = 11, item = \"IS.Anti-MissileSystem\" },\n    { at = 12, item = \"Ammo_IS.Anti-MissileSystem\", rounds = 12 },\n",
        ),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 6", "walk_mp = 0"),
        include_str!("../../game/mechs/Kestrel.toml").replace(
            "[sections.aft_side]\n",
            "[sections.aft_side]\nslots = [\n    { at = 1, item = \"IS.Anti-MissileSystem\" },\n    { at = 2, item = \"Ammo_IS.Anti-MissileSystem\", rounds = 12 },\n]\n",
        ),
    ]
}
