//! Exact C-contract coverage for the BattleTech loose-part package.

use crate::support;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use stompymux_rs::{
    Scripts,
    lua::{RuntimeMode, sources::Sources},
    persistence,
};
use support::isolated_scripts;

const C_WEAPONS: &str = r#"[[1,"energy",0,0,0,1,1,1,1,0,30,1,0.5],[2,"missile",2,2,0,4,8,12,1,50,15,40,1.0],[3,"missile",3,2,0,4,8,12,1,25,15,79,2.0],[4,"missile",4,2,0,4,8,12,2,15,15,119,3.0],[5,"artillery",20,20,0,0,0,20,30,5,60,171,30.0],[6,"ballistic",0,32,0,24,48,72,1,30,10,1,0.0],[7,"ballistic",0,9,0,9,18,27,1,20,10,1,0.0],[8,"ballistic",0,9,0,22,44,66,1,50,10,1,0.0],[9,"ballistic",0,7,0,4,9,13,1,50,7,1,0.0],[10,"ballistic",0,14,0,30,60,90,1,15,10,1,0.0],[11,"ballistic",0,27,0,35,70,105,1,10,10,1,0.0],[12,"ballistic",0,9,0,40,80,120,1,45,10,1,0.0],[13,"ballistic",0,17,0,24,48,72,1,30,10,1,0.0],[14,"ballistic",0,11,0,5,10,15,1,50,10,1,0.0],[15,"ballistic",0,13,0,22,44,66,1,30,10,1,0.0],[16,"ballistic",0,10,0,7,14,20,1,50,5,1,0.0],[17,"ballistic",0,12,0,6,12,18,1,10,15,1,0.0],[18,"ballistic",0,16,0,7,14,20,1,50,10,1,0.0],[19,"ballistic",0,18,0,8,16,24,1,50,7,1,0.0],[20,"ballistic",0,21,0,19,38,57,1,30,10,1,0.0],[21,"missile",3,1,10,12,24,36,1,18,30,1000,6.0],[22,"missile",6,1,10,12,24,36,4,9,30,1000,8.0],[23,"missile",8,1,10,12,24,36,6,6,30,1000,12.0],[24,"missile",10,1,10,12,24,36,8,4,30,1000,18.0],[25,"missile",2,2,4,6,12,18,1,24,15,1000,2.0],[26,"missile",4,2,4,6,12,18,2,12,20,1000,5.0],[27,"missile",5,2,4,6,12,18,3,8,25,1000,7.0],[28,"missile",6,2,4,6,12,18,5,6,30,1000,10.0],[29,"missile",2,3,0,2,4,6,1,50,15,1000,1.0],[30,"missile",3,3,0,2,4,6,1,25,15,1000,2.0],[31,"missile",4,3,0,2,4,6,2,15,15,1000,3.0],[32,"ballistic",1,2,3,10,20,35,4,30,12,1000,8.0],[33,"ballistic",3,5,0,8,16,28,5,15,20,1000,12.0],[34,"ballistic",7,10,0,6,12,20,6,8,25,1000,14.0],[35,"energy",12,10,0,8,15,25,1,0,20,249,4.0],[36,"energy",5,7,0,5,10,15,1,0,15,108,1.0],[37,"energy",2,5,0,2,4,6,1,0,10,31,0.5],[38,"energy",1,2,0,1,2,4,1,0,15,7,0.25],[39,"energy",15,15,0,7,14,23,2,0,25,412,6.0],[40,"energy",3,2,0,1,2,3,1,0,10,6,0.5],[41,"energy",18,16,0,5,10,15,3,0,30,243,4.0],[42,"energy",7,10,0,3,6,9,2,0,25,76,1.0],[43,"energy",3,6,0,1,2,3,1,0,20,15,0.5],[44,"energy",10,10,0,6,14,20,2,0,23,265,6.0],[45,"energy",4,7,0,4,8,12,1,0,18,111,2.0],[46,"energy",2,3,0,2,4,6,1,0,13,24,1.0],[47,"energy",1,3,0,1,2,3,1,0,15,12,0.5],[48,"energy",13,10,0,7,15,23,3,0,30,271,6.0],[49,"energy",6,7,0,5,9,14,2,0,30,116,2.0],[50,"energy",3,5,0,2,4,6,1,0,30,36,1.5],[51,"missile",1,2,0,1,1,1,1,24,10,63,0.5],[52,"ballistic",1,15,2,7,15,22,6,8,30,321,12.0],[53,"ballistic",1,2,4,10,20,30,3,45,15,47,5.0],[54,"ballistic",1,5,3,8,15,24,4,20,20,93,7.0],[55,"ballistic",2,10,0,6,12,18,5,10,25,148,10.0],[56,"ballistic",6,20,0,4,8,12,9,5,30,237,12.0],[57,"ballistic",0,2,0,1,2,3,1,200,7,5,0.25],[58,"ballistic",0,1,0,2,4,6,1,200,7,5,0.25],[59,"ballistic",0,3,0,1,2,3,1,100,7,6,0.5],[60,"ballistic",1,2,2,9,18,27,2,45,12,62,5.0],[61,"ballistic",1,5,0,7,14,21,3,20,20,123,7.0],[62,"ballistic",3,10,0,6,12,18,4,10,25,211,10.0],[63,"ballistic",7,20,0,4,8,12,8,5,30,337,12.0],[64,"artillery",10,20,0,0,0,6,12,5,60,171,12.0],[65,"missile",2,2,4,5,10,15,2,20,15,53,1.5],[66,"missile",4,2,4,5,10,15,3,10,20,105,3.5],[67,"missile",6,2,4,5,10,15,4,7,25,147,5.0],[68,"missile",8,2,4,5,10,15,5,5,30,212,7.0],[69,"missile",2,1,0,7,14,21,1,24,15,55,1.0],[70,"missile",4,1,0,7,14,21,1,12,20,109,2.5],[71,"missile",5,1,0,7,14,21,2,8,25,164,3.5],[72,"missile",6,1,0,7,14,21,4,6,20,220,5.0],[73,"missile",1,4,0,4,8,12,1,6,30,30,2.0],[74,"missile",2,2,0,3,6,9,1,50,15,21,0.5],[75,"missile",3,2,0,3,6,9,1,25,15,39,1.0],[76,"missile",4,2,0,3,6,9,1,15,15,59,1.5],[77,"energy",10,10,3,6,12,18,3,0,30,176,7.0],[78,"energy",8,8,0,5,10,15,2,0,25,124,5.0],[79,"energy",3,5,0,3,6,9,1,0,20,46,1.0],[80,"energy",1,3,0,1,2,3,1,0,15,9,0.5],[81,"energy",3,2,0,1,2,3,1,0,10,6,1.0],[82,"energy",15,10,0,7,14,23,3,0,30,229,7.0],[83,"energy",12,8,0,7,14,19,2,0,25,163,5.0],[84,"energy",5,5,0,4,8,12,1,0,20,62,1.0],[85,"energy",2,3,0,2,4,5,1,0,15,17,0.5],[86,"energy",10,9,0,3,7,10,2,0,25,119,7.0],[87,"energy",4,6,0,2,4,6,1,0,20,48,2.0],[88,"energy",2,3,0,1,2,3,1,0,15,12,1.0],[89,"energy",14,9,0,5,10,15,2,0,27,178,7.0],[90,"energy",6,6,0,3,6,9,1,0,22,71,2.0],[91,"energy",3,3,0,2,4,5,1,0,17,21,1.0],[92,"energy",10,10,0,9,13,15,2,0,30,165,6.0],[93,"energy",5,5,3,6,12,18,2,0,30,88,3.0],[94,"energy",15,15,3,6,12,18,4,0,30,317,10.0],[95,"ballistic",1,2,4,8,16,24,1,45,12,37,6.0],[96,"ballistic",1,5,3,6,12,18,4,20,20,70,8.0],[97,"ballistic",3,10,0,5,10,15,7,10,25,124,12.0],[98,"ballistic",7,20,0,3,6,9,10,5,30,178,14.0],[99,"ballistic",3,2,0,1,2,3,1,20,10,5,0.5],[100,"ballistic",0,2,0,1,2,3,1,200,7,5,0.5],[101,"missile",1,2,0,1,1,1,1,12,10,32,0.5],[102,"ballistic",1,15,2,7,15,22,7,8,30,321,15.0],[103,"ballistic",1,8,3,8,17,25,5,16,20,159,12.0],[104,"ballistic",2,25,4,6,13,20,11,4,30,346,18.0],[105,"ballistic",1,2,4,9,18,27,4,45,15,42,6.0],[106,"ballistic",1,5,3,7,14,21,5,20,20,83,8.0],[107,"ballistic",2,10,0,6,12,18,6,10,25,148,11.0],[108,"ballistic",6,20,0,4,8,12,11,5,30,237,14.0],[109,"ballistic",1,2,0,6,12,18,3,45,15,118,8.0],[110,"ballistic",1,5,0,5,10,15,6,20,22,247,10.0],[111,"ballistic",1,2,4,8,17,25,3,45,12,56,7.0],[112,"ballistic",1,5,2,6,13,20,5,20,20,113,9.0],[113,"ballistic",4,10,0,6,12,18,7,10,25,253,13.0],[114,"ballistic",8,20,0,3,7,10,10,5,30,282,15.0],[115,"artillery",10,20,0,0,0,5,15,5,60,171,15.0],[116,"artillery",10,10,0,0,0,12,20,10,60,86,20.0],[117,"artillery",6,5,0,0,0,14,15,20,60,40,15.0],[118,"ballistic",5,4,0,2,4,6,1,10,15,20,1.0],[119,"ballistic",1,2,0,6,12,18,1,45,12,30,4.0],[120,"ballistic",1,5,0,5,10,15,2,20,20,62,5.0],[121,"artillery",20,20,4,6,13,20,15,5,30,348,20.0],[122,"artillery",10,10,2,4,8,12,10,10,25,115,15.0],[123,"artillery",6,5,3,4,9,14,7,20,25,58,10.0],[124,"ballistic",0,2,0,2,4,6,1,100,7,6,1.0],[125,"ballistic",10,10,0,5,10,15,2,10,30,210,6.0],[126,"missile",2,1,0,3,6,9,2,33,30,29,1.5],[127,"missile",3,1,0,3,6,9,3,20,30,45,3.0],[128,"missile",4,1,0,3,6,9,4,14,30,67,4.5],[129,"missile",5,1,0,3,6,9,5,11,30,86,6.0],[130,"missile",2,1,6,7,14,21,1,24,15,45,2.0],[131,"missile",4,1,6,7,14,21,2,12,20,90,5.0],[132,"missile",5,1,6,7,14,21,3,8,25,136,7.0],[133,"missile",6,1,6,7,14,21,5,6,30,181,10.0],[134,"missile",2,2,0,3,6,9,1,50,15,21,1.0],[135,"missile",3,2,0,3,6,9,1,25,15,39,2.0],[136,"missile",4,2,0,3,6,9,2,15,15,59,3.0],[137,"missile",4,1,0,3,8,15,2,24,20,56,3.0],[138,"missile",6,1,0,3,8,15,3,12,30,112,7.0],[139,"missile",10,1,0,3,8,15,5,8,30,168,10.0],[140,"missile",12,1,0,3,8,15,7,6,30,224,12.0],[141,"missile",1,4,0,3,6,9,2,6,30,30,3.0],[142,"missile",1,6,0,4,9,15,3,4,30,75,5.0],[143,"missile",2,2,0,3,6,9,1,50,15,30,1.5],[144,"missile",3,2,0,3,6,9,1,25,15,59,3.0],[145,"missile",4,2,0,3,6,9,2,15,15,89,4.5],[146,"missile",3,1,0,5,11,18,1,0,30,18,0.5],[147,"missile",4,1,0,4,9,15,2,0,30,23,1.0],[148,"missile",5,1,0,3,7,12,3,0,30,24,1.5],[149,"missile",3,5,5,6,12,18,1,12,20,64,3.0],[150,"missile",5,10,5,6,12,18,2,6,20,127,7.0],[151,"missile",7,15,5,6,12,18,3,4,30,229,11.0],[152,"missile",8,20,5,6,12,18,5,3,30,305,15.0],[153,"melee",0,5,0,1,1,1,1,0,3,1,0.0],[154,"melee",0,7,0,1,1,1,1,0,3,1,0.0],[155,"missile",2,1,6,7,14,21,1,24,15,87,2.0],[156,"missile",4,1,6,7,14,21,2,12,20,173,5.0],[157,"missile",5,1,6,7,14,21,3,8,25,260,7.0],[158,"missile",6,1,6,7,14,21,5,6,30,346,10.0],[159,"energy",0,0,0,1,1,1,1,0,30,1,0.5],[160,"ballistic",0,2,0,3,6,9,2,50,12,15,0.5],[161,"ballistic",0,3,0,1,2,3,1,25,15,15,1.0],[162,"ballistic",3,3,0,1,2,3,2,10,25,30,1.5],[163,"ballistic",5,4,0,2,4,6,1,20,10,20,1.0],[164,"ballistic",1,2,2,9,18,27,4,45,10,75,7.0],[165,"ballistic",1,5,0,7,14,21,5,20,15,150,10.0],[166,"ballistic",3,10,0,6,12,18,7,10,20,250,14.0],[167,"ballistic",7,20,0,4,8,12,10,5,25,400,16.0],[168,"energy",15,10,0,7,14,22,2,0,25,400,6.0],[169,"energy",1,2,0,1,1,1,1,24,25,105,0.5],[170,"energy",12,2,0,1,1,1,1,24,25,105,0.5],[171,"ballistic",0,1,0,1,2,2,1,20,7,5,0.5],[172,"ballistic",0,1,0,1,2,3,1,10,10,7,0.75],[173,"ballistic",0,2,0,1,2,4,1,5,15,9,1.0],[174,"ballistic",0,1,0,1,2,3,1,20,5,8,1.0],[175,"energy",1,2,0,1,2,3,1,0,10,9,0.75],[176,"energy",1,1,0,1,2,3,1,0,10,5,0.75],[177,"missile",1,1,0,2,4,6,1,2,20,12,1.0],[178,"missile",1,1,4,6,9,12,1,1,20,22,1.0]]"#;

#[tokio::test(flavor = "current_thread")]
async fn catalogue_resolution_and_c_weapon_projection_are_exact() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let expected: serde_json::Value = serde_json::from_str(C_WEAPONS).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set(
            "c_weapon_catalogue",
            mlua::LuaSerdeExt::to_value(&scripts.inspect_lua(), &expected).unwrap(),
        )
        .unwrap();
    scripts.eval_callback::<()>(r#"
        local categories=btech.parts.categories()
        local expected_codes={'weapon','ammunition','bomb','special','cargo','other'}
        local expected_names={'Weapons','Ammunition','Bombs','Special Equipment','Cargo','Other'}
        assert(#categories==6)
        for index,row in ipairs(categories) do
            assert(row.code==expected_codes[index] and row.name==expected_names[index])
        end

        local all=btech.parts.list()
        assert(#all==510,'catalogue '..#all)
        local counted=0
        local registered_weapons={}
        local names={}
        local ambiguous
        for _,code in ipairs(expected_codes) do
            local rows=btech.parts.list(code:upper())
            counted=counted+#rows
            for _,part in ipairs(rows) do assert(part.category==code) end
        end
        assert(counted==#all)
        for index,part in ipairs(all) do
            assert(type(part.id)=='number' and part.id>=0 and part.id<1024)
            assert(type(part.brand)=='number' and part.brand>=0 and part.brand<=5)
            assert(part.packed_id==part.brand*1024+part.id)
            assert(type(part.short_name)=='string' and type(part.long_name)=='string' and type(part.very_long_name)=='string')
            assert(type(part.weight_tons)=='number' and type(part.cost)=='number')
            local by_number=btech.parts.resolve(part.packed_id)
            local by_record=btech.parts.resolve({id=part.id,brand=part.brand,ignored_projection_field=true})
            assert(by_number.id==part.id and by_number.brand==part.brand)
            assert(by_record.id==part.id and by_record.brand==part.brand)
            assert(part.brand>0 and part.category=='weapon')
            registered_weapons[part.id]=part
            if part.category=='weapon' then assert(type(part.weapon)=='table') else assert(part.weapon==nil) end
            if index>1 then assert(all[index-1].short_name<=part.short_name) end
            for _,name in ipairs({part.short_name,part.long_name,part.very_long_name}) do
                local key=name:lower()
                if names[key] and names[key]~=part.packed_id then ambiguous=name end
                names[key]=part.packed_id
            end
        end

        local registered_ids=0
        for _,expected in ipairs(c_weapon_catalogue) do
            local id,kind,heat,damage,minimum,short,medium,long,slots,ammo,recycle,bv,weight=unpack(expected)
            local part=registered_weapons[id]
            if part then
            registered_ids=registered_ids+1
            local weapon=part.weapon
            assert(weapon.kind==kind,id..' kind '..tostring(weapon.kind)..' '..kind)
            assert(weapon.heat==heat and weapon.damage==damage and weapon.minimum_range==minimum,id..' core '..weapon.heat..','..weapon.damage..','..weapon.minimum_range..' expected '..heat..','..damage..','..minimum)
            assert(weapon.short_range==short and weapon.medium_range==medium and weapon.long_range==long,id..' ranges')
            assert(weapon.critical_slots==slots and weapon.ammunition_per_ton==ammo,id..' installation')
            assert(weapon.recycle_time==recycle and weapon.battle_value==bv,id..' values')
            assert(part.weight_tons==weight,id..' weight '..part.weight_tons..' '..weight)
            end
        end
        assert(registered_ids==98,'registered ids '..registered_ids)

        assert(ambiguous,'expected a source-catalogue ambiguous name')
        local ok,err=mux.error.pcall(function() btech.parts.resolve(ambiguous) end)
        assert(not ok and err.code=='btech.part.ambiguous' and err.detail.argument==1)

        local first=all[1]
        assert(btech.parts.resolve(first.short_name..'\0ignored').packed_id==first.packed_id)
        assert(#btech.parts.search(first.short_name..'\0ignored')>=1)
        assert(#btech.parts.list('weapon\0ignored')==#btech.parts.list('weapon'))
        assert(btech.parts.resolve(256*1024+first.id)==nil)
        assert(btech.parts.resolve((256*1024+first.id)+0.0)==nil)
        ok,err=mux.error.pcall(function() btech.parts.search('\0ignored') end)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        for _,bad in ipairs({-1,1.5,0/0,math.huge}) do
            ok,err=mux.error.pcall(function() btech.parts.resolve(bad) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        end
        ok,err=mux.error.pcall(function() btech.parts.resolve({id=1,brand=6}) end)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        assert(btech.parts.resolve({id=79,brand=0})==nil)
        assert(btech.parts.resolve('IS.MediumLaser')==nil)
        ok,err=mux.error.pcall(function() btech.parts.store_quantity(1,{id=0,brand=0}) end)
        assert(not ok and err.code=='btech.part.not_found' and err.detail.argument==2)
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn stores_costs_rollback_persistence_and_checking_match_contract() {
    let (_directory, config, scripts) = isolated_scripts().await;
    scripts.eval_callback::<()>(r#"
        local part=btech.parts.list('weapon')[1]
        store_part=part
        assert(btech.parts.store_quantity(1,part)==0)
        assert(select('#',btech.parts.adjust_stores(1,part,3))==0)
        assert(btech.parts.store_quantity(1,part)==3)
        local stores=btech.parts.stores(1)
        local found=false
        for _,row in ipairs(stores) do if row.part.packed_id==part.packed_id then assert(row.quantity==3); found=true end end
        assert(found)
        btech.parts.adjust_stores(1,part,-2)
        assert(btech.parts.store_quantity(1,part)==1)
        local ok,err=mux.error.pcall(function() btech.parts.adjust_stores(1,part,-2) end)
        assert(not ok and err.code=='btech.operation.failed' and err.detail.reason=='store_capacity_exceeded')
        ok,err=mux.error.pcall(function() btech.parts.adjust_stores(1,part,0) end)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==3)

        local branded
        for _,candidate in ipairs(btech.parts.list('weapon')) do if candidate.brand>0 then branded=candidate break end end
        assert(branded)
        assert(select('#',btech.parts.set_cost(branded,9007199254740991))==0)
        assert(btech.parts.resolve(branded).cost==9007199254740991)
        ok,err=mux.error.pcall(function() btech.parts.set_cost(branded,9007199254740992) end)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
        persisted_part=branded.id
    "#).unwrap();

    let before = scripts
        .eval_callback::<i64>("return btech.parts.store_quantity(1,store_part)")
        .unwrap();
    assert!(
        scripts
            .eval_callback::<()>("btech.parts.adjust_stores(1,store_part,4); error('rollback')")
            .is_err()
    );
    assert_eq!(
        scripts
            .eval_callback::<i64>("return btech.parts.store_quantity(1,store_part)")
            .unwrap(),
        before
    );

    let world_snapshot = scripts.world().clone();
    persistence::save_with_timeout(
        &config.database(),
        &world_snapshot,
        config.database.busy_timeout_ms,
    )
    .await
    .unwrap();
    let reloaded = persistence::load(&config.database()).await.unwrap();
    let persisted_part = scripts
        .inspect_lua()
        .globals()
        .get::<i32>("persisted_part")
        .unwrap();
    assert_eq!(
        stompymux_rs::btech::part_cost(&reloaded, persisted_part).unwrap(),
        9_007_199_254_740_991
    );

    let sources = Arc::new(Sources::read(&config).unwrap());
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(reloaded)),
        scripts.help().clone(),
        sources,
        RuntimeMode::Checking,
    )
    .unwrap();
    checking.eval_callback::<()>(r#"
        for _,name in ipairs({'categories','list','search','resolve','stores','store_quantity','adjust_stores','set_cost'}) do
            local ok,err=mux.error.pcall(function() btech.parts[name]() end)
            assert(not ok and err.code=='mux.unavailable.checking',name)
        end
    "#).unwrap();
}
