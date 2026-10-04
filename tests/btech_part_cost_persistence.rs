use crate::support;
use sqlx::{Connection, Row, SqliteConnection};
use support::isolated_world;

#[tokio::test(flavor = "current_thread")]
async fn part_costs_use_canonical_c_names_and_preserve_unknown_rows() {
    let (_directory, config, _) = isolated_world().await;
    let catalogue = stompymux_rs::btech::part_catalogue();
    for form in catalogue {
        assert_eq!(
            form.very_long_name,
            stompymux_rs::Part::from_id(form.part_id).unwrap().name
        );
    }
    let canonical = stompymux_rs::Part::from_id(6).unwrap().name;
    let mut connection =
        SqliteConnection::connect(&format!("sqlite://{}", config.database().display()))
            .await
            .unwrap();
    sqlx::query("INSERT INTO btech_economy_costs(item_name,cost) VALUES(?,?),(?,?)")
        .bind(&canonical)
        .bind("41")
        .bind("future-part-row")
        .bind("999")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();

    let mut world = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert_eq!(stompymux_rs::btech::part_cost(&world, 6).unwrap(), 41);
    stompymux_rs::btech::set_part_cost(&mut world, 6, 73).unwrap();
    stompymux_rs::persistence::save(&config.database(), &world)
        .await
        .unwrap();
    let reloaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert_eq!(stompymux_rs::btech::part_cost(&reloaded, 6).unwrap(), 73);

    let mut connection =
        SqliteConnection::connect(&format!("sqlite://{}", config.database().display()))
            .await
            .unwrap();
    let rows = sqlx::query("SELECT item_name,cost FROM btech_economy_costs ORDER BY item_name")
        .fetch_all(&mut connection)
        .await
        .unwrap();
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("item_name") == canonical && row.get::<String, _>("cost") == "73"
    }));
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("item_name") == "future-part-row"
            && row.get::<String, _>("cost") == "999"
    }));
}
