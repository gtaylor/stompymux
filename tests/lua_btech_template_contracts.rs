//! Focused C-shape coverage for the btech.template Lua namespace.
use crate::support;
use support::isolated_scripts;

/// The PARITY fixture uses branded criticals exclusively, so its projections
/// match the C reference values captured by the template_catalog and
/// template_inspection probes.
const LOADABLE: &str = include_str!("fixtures/btech/mechs/PARITY.toml");

fn seed(config: &stompymux_rs::Config) {
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(root.join("stock")).unwrap();
    std::fs::write(root.join("PARITY.toml"), LOADABLE).unwrap();
    // Stock templates name parts without a manufacturer and still load.
    std::fs::write(
        root.join("stock/JR7-D.toml"),
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
    )
    .unwrap();
    std::fs::write(root.join("BROKEN.toml"), "not a template at all").unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn template_callables_match_c_argument_and_error_shapes() {
    let (_directory, config, scripts) = isolated_scripts().await;
    seed(&config);
    let failures: Vec<String> = scripts
        .eval_callback(
            r#"
      local out={}
      local function record(f,...)
        local ok,e=mux.error.pcall(f,...)
        out[#out+1]=ok and 'ok' or (e.code..' | '..e.message)
        return ok,e
      end
      -- Extra arguments are ignored: the C handlers read fixed stack slots.
      assert(btech.template.exists('PARITY','extra'))
      assert(type(btech.template.engine('PARITY','extra').rating)=='number')
      assert(type(btech.template.base_cost('PARITY','extra'))=='number')
      assert(type(btech.template.payload('PARITY','extra'))=='table')
      assert(type(btech.template.installed_parts('PARITY','extra'))=='table')
      assert(type(btech.template.technologies('PARITY','extra'))=='table')
      assert(type(btech.template.armor('PARITY',nil,'extra').armor)=='table')
      assert(type(btech.template.weapons('PARITY',nil,'extra'))=='table')
      local player=mux.world.object(1)
      assert(select('#',btech.template.show_status('PARITY',player,'extra'))==0)
      assert(select('#',btech.template.show_weapon_specs('PARITY',player,'extra'))==0)
      assert(select('#',btech.template.show_critical_status('PARITY',player,btech.unit.sections.HEAD,'extra'))==0)
      -- battle_value is the one exact-arity callable.
      record(btech.template.battle_value)
      record(btech.template.battle_value,'PARITY','extra')
      -- Reference coercion and validation.
      record(btech.template.exists,42)
      record(btech.template.exists,false)
      record(btech.template.exists)
      record(btech.template.exists,'')
      record(btech.template.exists,'../PARITY')
      record(btech.template.exists,'sub/PARITY')
      record(btech.template.exists,'PARITY\\x')
      record(btech.template.exists,'missing-template')
      record(btech.template.exists,'BROKEN')
      record(btech.template.exists,'JR7-D')
      record(btech.template.engine,42)
      record(btech.template.base_cost,'missing-template')
      -- Section validation.
      record(btech.template.armor,'PARITY',false)
      record(btech.template.armor,'PARITY',42)
      record(btech.template.armor,'PARITY',btech.unit.fire_modes.DESTROYED)
      record(btech.template.armor,'PARITY',btech.unit.sections.TURRET)
      record(btech.template.critical_slots,'PARITY')
      record(btech.template.critical_slots,'PARITY',nil)
      record(btech.template.critical_slots,'PARITY',btech.unit.sections.TURRET)
      record(btech.template.weapons,'PARITY',btech.unit.sections.TURRET)
      -- Player display recipients.
      record(btech.template.show_status,'PARITY',mux.world.object(0))
      record(btech.template.show_status,'PARITY',0)
      record(btech.template.show_weapon_specs,'PARITY',mux.world.object(0))
      record(btech.template.show_critical_status,'PARITY',player)
      record(btech.template.show_critical_status,'PARITY',player,nil)
      record(btech.template.show_critical_status,'PARITY',player,btech.unit.sections.TURRET)
      record(btech.template.show_critical_status,'missing-template',player,btech.unit.sections.HEAD)
      return out
    "#,
        )
        .unwrap();
    let expected = [
        "mux.arg.invalid | expected exactly 1 argument",
        "mux.arg.invalid | expected exactly 1 argument",
        "mux.arg.invalid | bad argument #1 to '?' (reference must be a string)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must be a string)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must be a string)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must not be empty)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must not contain path components)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must not contain path components)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must not contain path components)",
        "ok",
        "btech.template.invalid | bad argument #1 to '?' (existing template is malformed)",
        "ok",
        "mux.arg.invalid | bad argument #1 to '?' (reference must be a string)",
        "btech.template.not_found | bad argument #1 to '?' (template was not found)",
        "mux.arg.invalid | bad argument #2 to '?' (section must be a btech.unit.sections constant from this runtime)",
        "mux.arg.invalid | bad argument #2 to '?' (section must be a btech.unit.sections constant from this runtime)",
        "mux.arg.invalid | bad argument #2 to '?' (section must be a btech.unit.sections constant from this runtime)",
        "mux.arg.invalid | bad argument #2 to '?' (section is not valid for this unit)",
        "mux.arg.invalid | bad argument #2 to '?' (section is required)",
        "mux.arg.invalid | bad argument #2 to '?' (section is required)",
        "mux.arg.invalid | bad argument #2 to '?' (section is not valid for this unit)",
        "mux.arg.invalid | bad argument #2 to '?' (section is not valid for this unit)",
        "mux.object.invalid | bad argument #2 to '?' (display recipient must be a player)",
        "mux.object.invalid | bad argument #2 to '?' (display recipient must be a player)",
        "mux.object.invalid | bad argument #2 to '?' (display recipient must be a player)",
        "mux.arg.invalid | bad argument #3 to '?' (section is required)",
        "mux.arg.invalid | bad argument #3 to '?' (section is required)",
        "mux.arg.invalid | bad argument #3 to '?' (section is not valid for this unit)",
        "btech.template.not_found | bad argument #1 to '?' (template was not found)",
    ];
    assert_eq!(failures.len(), expected.len());
    for (index, (failure, expected)) in failures.iter().zip(&expected).enumerate() {
        assert_eq!(failure, expected, "template rejection {index}");
    }
}

/// Pristine projections carry the C reference values captured differentially
/// by the template_inspection probe for the identical PARITY template body.
#[tokio::test(flavor = "current_thread")]
async fn loadable_template_projects_exact_reference_rows() {
    let (_directory, config, scripts) = isolated_scripts().await;
    seed(&config);
    scripts
        .eval_callback::<()>(
            r#"
      local player=mux.world.object(1)
      assert(btech.template.exists('parity') and not btech.template.exists('missing-template'))
      local engine=btech.template.engine('parity')
      assert(engine.rating==300 and engine.suspension_factor==0,'engine')
      assert(btech.template.base_cost('parity')==8356250,'cost')
      local bv=btech.template.battle_value('parity')
      assert(math.abs(bv.total-849.20001220703125)<0.000001,'total')
      assert(bv.offensive==251 and math.abs(bv.defensive-598.20001220703125)<0.000001,'bv')
      local armor=btech.template.armor('parity')
      assert(armor.armor.current==104 and armor.internal.current==114 and armor.rear_armor.current==12,'armor all')
      assert(armor.section==nil,'no section field when aggregated')
      local head=btech.template.armor('parity',btech.unit.sections.HEAD)
      assert(head.section==btech.unit.sections.HEAD and head.armor.current==9 and head.internal.current==3,'head')
      local slots=btech.template.critical_slots('parity',btech.unit.sections.HEAD)
      assert(#slots==12 and slots[1].kind=='empty' and slots[12].slot==12,'head slots')
      local arm=btech.template.critical_slots('parity',btech.unit.sections.LEFT_ARM)
      assert(arm[1].kind=='weapon' and arm[1].part.id==77 and arm[1].part.brand==5,'arm slots')
      local weapons=btech.template.weapons('parity')
      assert(#weapons==1 and weapons[1].number==0 and weapons[1].first_slot==1,'weapons')
      assert(weapons[1].part.id==77 and weapons[1].part.brand==5 and weapons[1].operational,'weapon row')
      local left=btech.template.weapons('parity',btech.unit.sections.LEFT_ARM)
      assert(#left==1 and left[1].part.id==77,'filtered weapons')
      local installed=btech.template.installed_parts('parity')
      assert(#installed==1 and installed[1].part.id==77 and installed[1].part.brand==5 and installed[1].quantity==1,'installed')
      local payload=btech.template.payload('parity')
      assert(#payload==1 and payload[1].part.id==77,'payload')
      local technologies=btech.template.technologies('parity')
      assert(#technologies>=1,'technologies')
      assert(select('#',btech.template.show_status('parity',player))==0)
      assert(select('#',btech.template.show_weapon_specs('parity',player))==0)
      assert(select('#',btech.template.show_critical_status('parity',player,btech.unit.sections.HEAD))==0)
      assert(select('#',btech.template.exists('parity'))==1)
      assert(select('#',btech.template.battle_value('parity'))==1)
    "#,
        )
        .unwrap();
}
