//! Representative AMS-equipped targets for shared firing and missile-flight acceptance.

/// Seven supported defender chassis include both Clan and Inner Sphere interception dice.
pub fn templates() -> Vec<String> {
    let tracked = include_str!("../../game/mechs/Goblin-58");
    vec![
        include_str!("../../game/mechs/Daishi-A").into(),
        include_str!("../../game/mechs/GOL-1H").replace(
            "Left_Torso\n",
            "Left_Torso\n CRIT_11 { IS.Anti-MissileSystem - - }\n CRIT_12 { Ammo_IS.Anti-MissileSystem 12 - }\n",
        ),
        tracked.into(),
        tracked.replace("{ Track }", "{ Wheel }"),
        tracked.replace("{ Track }", "{ Hover }"),
        tracked.replace("{ Track }", "{ None }").replace("{ 64.50 }", "{ 0 }"),
        include_str!("../../game/mechs/Kestrel").replace(
            "Aft_Side\n",
            "Aft_Side\n CRIT_1 { IS.Anti-MissileSystem - - }\n CRIT_2 { Ammo_IS.Anti-MissileSystem 12 - }\n",
        ),
    ]
}
