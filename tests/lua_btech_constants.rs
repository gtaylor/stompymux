//! Complete C-derived BattleTech typed-constant contract coverage.
use crate::support;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use stompymux_rs::{
    Scripts,
    lua::{RuntimeMode, sources::Sources},
};
use support::isolated_scripts;

const EXPECTED: &str = r#"{"btech.unit.types":["MECH","VEHICLE","VTOL","NAVAL","SPHEROID_DROPSHIP","AERO_FIGHTER","MECHWARRIOR","AERODYNE_DROPSHIP","BATTLESUIT"],"btech.unit.movement_types":["BIPED","TRACK","WHEEL","HOVER","VTOL","HULL","FOIL","FLY","QUAD","SUB","NONE"],"btech.unit.sections":["FRONT_LEFT_LEG","FRONT_RIGHT_LEG","LEFT_TORSO","RIGHT_TORSO","CENTER_TORSO","REAR_LEFT_LEG","REAR_RIGHT_LEG","HEAD","LEFT_ARM","RIGHT_ARM","LEFT_LEG","RIGHT_LEG","SUIT_1","SUIT_2","SUIT_3","SUIT_4","SUIT_5","SUIT_6","SUIT_7","SUIT_8","LEFT_SIDE","RIGHT_SIDE","FRONT_SIDE","AFT_SIDE","TURRET","ROTOR","NOSE","LEFT_WING","RIGHT_WING","LEFT_REAR_WING","RIGHT_REAR_WING","AFT","FRONT_RIGHT_SIDE","FRONT_LEFT_SIDE","REAR_LEFT_SIDE","REAR_RIGHT_SIDE"],"btech.unit.technology":["TRIPLE_STRENGTH_MYOMER","CLAN_ANTI_MISSILE","INNER_SPHERE_ANTI_MISSILE","DOUBLE_HEAT_SINKS","MASC","CLAN","FLIPPABLE_ARMS","C3_MASTER","C3_SLAVE","ARTEMIS_IV","ECM","BEAGLE_PROBE","SALVAGE","CARGO","SEARCH_LIGHT","LIGHT_ACTIVE_PROBE","ANTI_AIRCRAFT","NO_SENSORS","SIXTH_SENSE","FERRO_FIBROUS","ENDO_STEEL","XL_ENGINE","ICE_ENGINE","SINGLE_HEAT_SINKS","LIGHT_ENGINE","XXL_ENGINE","COMPACT_ENGINE","REINFORCED_INTERNAL","COMPOSITE_INTERNAL","HARDENED_ARMOR","CRITICAL_PROOF","STEALTH_ARMOR","HEAVY_FERRO_FIBROUS","LASER_REFLECTIVE_ARMOR","REACTIVE_ARMOR","NULL_SIGNATURE_SYSTEM","C3I","SUPERCHARGER","IMPROVED_JUMP_JETS","MECHANICAL_JUMP_JETS","COMPACT_HEAT_SINKS","LASER_HEAT_SINKS","BLOODHOUND_PROBE","ANGEL_ECM","WATCHDOG","LIGHT_FERRO_FIBROUS","TAG","OMNIMECH","ARTEMIS_V","CAMOUFLAGE","CARRIER","WATERPROOF","XL_GYRO","HEAVY_DUTY_GYRO","COMPACT_GYRO","TARGETING_COMPUTER","SMALL_COCKPIT","SWARM_ATTACK","MOUNT_FRIENDS","ANTI_LEG_ATTACK","PURIFIER_STEALTH","KAGE_STEALTH","ACHILEUS_STEALTH","INFILTRATOR_STEALTH","INFILTRATOR_II_STEALTH","MUST_JETTISON_PACK","CAN_JETTISON_PACK"],"btech.unit.technology_groups":["UNIT","INFANTRY","ALL"],"btech.unit.fire_modes":["DESTROYED","DISABLED","BROKEN","DAMAGED","TARGETING_COMPUTER","REAR_MOUNT","HOTLOAD","HALF_TON","ONE_SHOT","ONE_SHOT_USED","ULTRA","RAPID_FIRE","GATLING","ROTARY_TWO_SHOT","ROTARY_FOUR_SHOT","ROTARY_SIX_SHOT","HEAT","BACKPACK","JETTISONED","OMNI_BASE","ROCKET_FIRED"],"btech.unit.ammunition_modes":["LBX_CLUSTER","ARTEMIS_MINE","NARC_SMOKE","CLUSTER","MINE","SMOKE","INFERNO","SWARM","SWARM_1","INARC_EXPLOSIVE","INARC_HAYWIRE","INARC_ECM","INARC_NEMESIS","ARMOR_PIERCING","FLECHETTE","INCENDIARY","PRECISION","STINGER","CASELESS","SEMI_GUIDED","EXTENDED_RANGE","HIGH_EXPLOSIVE","MML_LRM"],"btech.autopilot.orders":["MOVE","HOLD","FOLLOW","PATROL","ATTACK","ATTACK_MOVE"],"btech.autopilot.submission_modes":["APPEND","REPLACE"],"btech.autopilot.fire_modes":["HOLD","ASSIGNED_TARGET","OPPORTUNISTIC"],"btech.map.light_levels":["NIGHT","TWILIGHT","DAY"],"btech.repair.operations":["REATTACH","REPAIR_PART","REPAIR_WEAPON_TEMPORARY","REPAIR_ENHANCEMENT","REPAIR_FOCUS","REPAIR_CRYSTAL","REPAIR_BARREL","REPAIR_AMMO_FEED","REPAIR_RANGING","REPAIR_AMMO_MOUNT","REPLACE_WEAPON","RELOAD","REPAIR_ARMOR","REPAIR_REAR_ARMOR","REPAIR_INTERNAL","DETACH","SCRAP_PART","SCRAP_WEAPON","UNLOAD","RESEAL","REPLACE_SUIT"]}"#;

fn verify(scripts: &Scripts) {
    let expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set(
            "expected_btech_constants",
            mlua::LuaSerdeExt::to_value(&scripts.inspect_lua(), &expected).unwrap(),
        )
        .unwrap();
    scripts.eval_callback::<()>(r#"
        assert(require('btech') == btech)
        assert(btech.error.codes == btech.errors)
        assert(tostring(btech.error.codes.operation.failed) == 'btech.operation.failed')
        local ammunition=btech.unit.ammunition_modes
        assert(ammunition.LBX_CLUSTER~=ammunition.CLUSTER)
        assert(ammunition.ARTEMIS_MINE~=ammunition.MINE)
        assert(ammunition.NARC_SMOKE~=ammunition.SMOKE)
        local first
        for path,names in pairs(expected_btech_constants) do
            local namespace=_G
            for part in path:gmatch('[^.]+') do namespace=namespace[part] end
            assert(type(namespace)=='userdata',path)
            assert(getmetatable(namespace)=='protected BattleTech constant namespace metatable',path)
            for _,name in ipairs(names) do
                local value=namespace[name]
                assert(type(value)=='userdata','type '..path..'.'..name)
                assert(tostring(value)==name,'tostring '..path..'.'..name)
                assert(value==namespace[name],'identity '..path..'.'..name)
                assert(getmetatable(value)=='protected BattleTech constant metatable','metatable '..path..'.'..name..' '..tostring(getmetatable(value)))
                if first and path~='btech.unit.types' then assert(value~=first,'catalog '..path..'.'..name) end
            end
            if path=='btech.unit.types' then first=namespace.MECH end
            local ok,err=mux.error.pcall(function() return namespace.UNKNOWN end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2,path)
            ok,err=mux.error.pcall(function() return namespace[1] end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2,path)
            ok,err=mux.error.pcall(function() namespace.EXTRA=namespace[names[1]] end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail==nil,path)
            ok,err=mux.error.pcall(function() namespace[names[1]].extra=true end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail==nil,path)
        end
        for path, expected in pairs({
            ['btech.unit.detection_channels'] = { SENSORS='sensors', SIGHT='sight', RADAR='radar', PROBE='probe' },
            ['btech.autopilot.states'] = { PAUSED='paused', IDLE='idle', EXECUTING='executing', BLOCKED='blocked' },
            ['btech.autopilot.order_states'] = { QUEUED='queued', RUNNING='running', SUCCEEDED='succeeded', FAILED='failed', CANCELED='canceled' },
            ['btech.autopilot.reasons'] = { MANUAL_TAKEOVER='manual_takeover', CONTACT_LOST='contact_lost', STUCK='stuck', UNREACHABLE='unreachable', INVALIDATED='invalidated', RESOURCE_LIMIT='resource_limit', CONGESTED='congested', INVALID_TARGET='invalid_target', UNIT_UNAVAILABLE='unit_unavailable', MAP_CHANGED='map_changed', UNSUPPORTED='unsupported', STALE_REVISION='stale_revision' },
        }) do
            local namespace = _G
            for part in path:gmatch('[^.]+') do namespace=namespace[part] end
            assert(type(namespace)=='userdata',path)
            for name, value in pairs(expected) do
                assert(namespace[name]==value,path..'.'..name)
            end
            local ok,err=mux.error.pcall(function() namespace.EXTRA='extra' end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail==nil,path)
        end
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn all_constant_catalogs_are_typed_strict_and_available_while_checking() {
    let (_directory, config, live) = isolated_scripts().await;
    verify(&live);
    let sources = Arc::new(Sources::read(&config).unwrap());
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(live.world().clone())),
        live.help().clone(),
        sources,
        RuntimeMode::Checking,
    )
    .unwrap();
    verify(&checking);
}
