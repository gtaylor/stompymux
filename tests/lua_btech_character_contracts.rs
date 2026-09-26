use crate::support;
use std::{cell::RefCell, rc::Rc};
use support::isolated_scripts;

#[tokio::test(flavor = "current_thread")]
async fn character_contract_uses_object_identity_catalog_codes_and_zero_return_mutations() {
    let (_d, config, s) = isolated_scripts().await;
    stompymux_rs::set_battle_character(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(1),
        stompymux_rs::BattleCharacter {
            bruise: 2,
            lethal: 1,
            build: 5,
            reflexes: 6,
            intuition: 7,
            learn: 8,
            charisma: 9,
        },
    )
    .unwrap();
    s.eval_callback::<()>(r#"
      local function argument_error(e,n,name,detail)
        local suffix="bad argument #"..n.." to '"..name.."' ("..detail..")"
        assert(e.message:sub(-#suffix)==suffix,e.message)
      end
      local c=mux.world.object(1)
      local rows=btech.character.catalog('Char_attribute'); assert(#rows==5 and rows[1].code==34 and rows[1].name=='Build')
      assert(#btech.character.catalog('attributes')==0)
      local v=btech.character.value(c,'Refle');assert(v.amount==6 and v.definition.kind=='Char_attribute')
      assert(select('#',btech.character.set_value(c,'Bruise',9))==0);assert(btech.character.value(c,6).amount==9)
      assert(select('#',btech.character.set_skill_target(c,'PilBip',4))==0);assert(btech.character.value(c,'Piloting-Biped').target==4)
      assert(select('#',btech.character.set_skill_experience(c,'PilBip',17))==0)
      assert(btech.character.experience_threshold('PilBip')==3000)
      local before=btech.character.value(c,'PilBip').experience
      assert(select('#',btech.character.add_skill_experience(c,'PilBip',3))==0)
      assert(btech.character.value(c,'PilBip').experience==before+3)
      local ok,e=mux.error.pcall(function() btech.character.value(1,'Build') end);assert(not ok and e.code=='mux.object.invalid')
      ok,e=mux.error.pcall(function() btech.character.value(c,false) end);assert(not ok and e.code=='mux.arg.invalid' and e.detail.argument==2)
      argument_error(e,2,'value','value must be an integer')
      for _,bad in ipairs({false,'1',{},function() end}) do
        ok,e=mux.error.pcall(function() btech.character.set_value(c,'Build',bad) end)
        assert(not ok and e.code=='mux.arg.invalid' and e.detail.argument==3)
        argument_error(e,3,'set_value','amount must be an integer')
      end
      ok,e=mux.error.pcall(function() btech.character.set_value(c,'Build') end)
      assert(not ok); argument_error(e,3,'set_value','amount must be an integer')
      ok,e=mux.error.pcall(function() btech.character.set_value(c,'Build',1.5) end)
      assert(not ok); argument_error(e,3,'set_value','amount must be a ranged integer')
      ok,e=mux.error.pcall(function() btech.character.value(c,1.5) end);assert(not ok and e.code=='mux.arg.invalid' and e.detail.argument==2)
      argument_error(e,2,'value','value must be a ranged integer')
      assert(btech.character.value(c,'Build\0ignored').amount==5)
      assert(#btech.character.catalog('Char_skill\0ignored')==78)
      assert(btech.character.experience_threshold('Piloting-Biped\0ignored')==3000)
      ok,e=mux.error.pcall(function() btech.character.value(btech.unit.types.MECH,'Build') end);assert(not ok and e.code=='mux.object.invalid')
      ok,e=mux.error.pcall(function() btech.character.set_skill_target(c,'Build',4) end);assert(not ok,e and e.code or 'no error')
      local filtered=btech.character.catalog('Char_skill',c);local found=false;for _,row in ipairs(filtered) do if row.name=='Piloting-Biped' then found=true end end;assert(found)
    "#).unwrap();

    s.eval_callback::<()>("saved_character_object=mux.world.object(1)")
        .unwrap();
    s.world_mut()
        .objects
        .get_mut(&stompymux_rs::ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Going);
    s.eval_callback::<()>(r#"
      local ok,e=mux.error.pcall(function() btech.character.value(saved_character_object,'Build') end)
      local suffix="bad argument #1 to 'value' (object is going away)"
      assert(not ok and e.code=='mux.object.unavailable' and e.message:sub(-#suffix)==suffix,e.message)
    "#).unwrap();
    let mut removed = s
        .world_mut()
        .objects
        .remove(&stompymux_rs::ObjectId(1))
        .unwrap();
    s.eval_callback::<()>(r#"
      local ok,e=mux.error.pcall(function() btech.character.value(saved_character_object,'Build') end)
      local suffix="bad argument #1 to 'value' (object no longer exists)"
      assert(not ok and e.code=='mux.object.unavailable' and e.message:sub(-#suffix)==suffix,e.message)
    "#).unwrap();
    removed.flags.remove(stompymux_rs::Flag::Going);
    s.world_mut()
        .objects
        .insert(stompymux_rs::ObjectId(1), removed);

    let saved = s.world().clone();
    stompymux_rs::persistence::save(&config.database(), &saved)
        .await
        .unwrap();
    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let reloaded = stompymux_rs::Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    reloaded
        .eval_callback::<()>(
            r#"
      local c=mux.world.object(1)
      assert(btech.character.value(c,'Bruise').amount==9)
      assert(btech.character.value(c,'PilBip').experience==20)
    "#,
        )
        .unwrap();
}
