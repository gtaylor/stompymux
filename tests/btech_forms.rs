//! Catalogue forms share selection identities and reach clients through paced report delivery.
use crate::support;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
use stompymux_rs::*;

#[tokio::test]
async fn forms_cover_stock_and_share_native_lua_names_without_mutation() {
    let (_dir, config, world) = support::isolated_world().await;
    let before = world.btech.clone();
    let forms = battle_part_forms(&world, ObjectId(1)).unwrap();
    assert!(forms.len() > 1024);
    assert!(forms.windows(2).all(|pair| {
        let key = |f: &BattlePartForm| (f.short_name.clone(), f.brand_id, f.part_id);
        key(&pair[0]) < key(&pair[1])
    }));
    let base: BTreeSet<_> = forms
        .iter()
        .filter(|f| f.brand_id == 0)
        .map(|f| f.part_id)
        .collect();
    assert_eq!(base.len(), 619);
    for id in base {
        let part = BattlePart::from_id(id).unwrap();
        assert!(
            forms
                .iter()
                .any(|f| f.brand_id == 0 && f.part_id == id && f.very_long_name == part.name)
        );
    }
    let magna = forms
        .iter()
        .find(|f| f.very_long_name == "Magna.IS.SmallLaser")
        .unwrap();
    assert_eq!(magna.short_name, "Ma.SL");
    assert_eq!(magna.long_name, "Magna.SmallLaser");
    assert!(
        forms
            .iter()
            .any(|f| f.short_name == magna.short_name && f.brand_id != magna.brand_id)
    );
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "listforms ignored");
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines[0], "Listing of forms:");
    assert_eq!(lines.len(), forms.len() + 1);
    for (i, form) in forms.iter().enumerate() {
        assert_eq!(
            lines[i + 1],
            format!(
                "{i:3} {:<20} {:<25} {}",
                form.short_name, form.long_name, form.very_long_name
            )
        );
    }
    let names: Vec<String> = scripts.eval_callback("local r={};for _,f in ipairs(btech.inventory.forms(1)) do r[#r+1]=f.very_long_name end;return r").unwrap();
    assert_eq!(
        names,
        forms
            .iter()
            .map(|f| f.very_long_name.clone())
            .collect::<Vec<_>>()
    );
    scripts
        .eval_callback::<()>("local f=btech.inventory.forms(1);f[1].very_long_name='changed'")
        .unwrap();
    assert_eq!(
        battle_part_forms(&scripts.world(), ObjectId(1)).unwrap(),
        forms
    );
    assert!(
        scripts
            .eval_callback::<()>("btech.inventory.forms(4)")
            .is_err()
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(4), 4, "listforms")
            .contains("Permission denied")
    );
    assert!(
        !support::run_text(&scripts, &config, ObjectId(1), 1, "listforms/invalid")
            .contains("Listing of forms:")
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn complete_catalogue_is_delivered_over_tcp() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world) = support::isolated_world().await;
            let forms = battle_part_forms(&world, ObjectId(1)).unwrap();
            world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
                Some(accounts::hash("secret", &config).unwrap());
            persistence::save(&config.database(), &world).await.unwrap();
            let (address, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
            let mut client = support::Client::connect(address, 1).await;
            client.send("listforms").await;
            client.until("Listing of forms:").await;
            for (i, form) in forms.iter().enumerate() {
                client.until(&format!("{i:3} {}", form.short_name)).await;
            }
            client.until(&forms.last().unwrap().very_long_name).await;
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
