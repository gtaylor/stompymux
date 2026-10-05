//! Standard adversarial armor diagrams share cockpit artwork without disclosing numeric protection.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Every silhouette has a fixed three-column legend and identical native/Lua disclosure after restart.
#[tokio::test]
async fn all_scan_silhouettes_use_symbols_and_preserve_owned_layouts() {
    use std::{cell::RefCell, rc::Rc};
    for template in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/SHD-2H.toml"),
        include_str!("../game/units/WHM-6R.toml"),
        include_str!("../game/units/AS7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Savannah_Master.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, world, source, target, _) =
            firing::fixture_with_target(include_str!("../game/units/JR7-D.toml"), None, template)
                .await;
        let before = serde_json::to_value(&world.btech).unwrap();
        let scan = scan_battle_unit(&world, source, ObjectId(1), target, "A").unwrap();
        let plain = text::plain(&scan);
        let rows: Vec<_> = plain
            .lines()
            .skip_while(|line| !line.contains("FRONT"))
            .collect();
        assert!(matches!(rows.len(), 7 | 10));
        assert!(
            rows.iter()
                .all(|line| !line.chars().any(|c| c.is_ascii_digit()))
        );
        assert!(rows.iter().any(|line| line.starts_with("Key")));
        for symbol in ["** ", "XX ", "xx ", "oo ", "OO "] {
            assert!(
                rows.iter().any(|line| line.starts_with(symbol)),
                "Missing {symbol} in {plain}"
            );
        }
        let owned = battle_unit_status(&world, target, "A").unwrap();
        let owned_plain = text::plain(&owned);
        let owned_rows: Vec<_> = owned_plain
            .lines()
            .skip_while(|line| !line.contains("FRONT"))
            .filter(|line| !line.trim().is_empty())
            .collect();
        assert_eq!(rows.len(), owned_rows.len());
        for (scan_row, owned_row) in rows.iter().zip(&owned_rows) {
            assert_eq!(scan_row.len(), owned_row.len() + 3);
        }
        assert!(
            owned_rows
                .iter()
                .any(|row| row.chars().any(|c| c.is_ascii_digit()))
        );
        let info = scan_battle_unit(&world, source, ObjectId(1), target, "I").unwrap();
        assert!(!info.lines().any(|line| line.starts_with("Key")));
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            scan_battle_unit(&restored, source, ObjectId(1), target, "A").unwrap(),
            scan
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.scan({},1,{},'A')",
                source.0, target.0
            ))
            .unwrap();
        assert_eq!(lua, scan);
        scripts.drain_outbox();
        let native = support::run_text_for_player(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("scan #{} A", target.0),
        );
        assert_eq!(native, scan);
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            before
        );
    }
}
