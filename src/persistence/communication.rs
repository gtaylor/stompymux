//! Selective schema-32 channel, alias and last-page persistence; unrelated macro data is preserved.
use super::write::{Cell, Fields, delete, row};
use crate::{
    communication::{ChannelAlias, ChannelMessage, Membership},
    world::{ObjectId, World},
};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

/// Decode the newly owned relational records in their legacy position order.
pub(super) async fn load(c: &mut SqliteConnection, w: &mut World) -> Result<()> {
    for r in sqlx::query("SELECT * FROM comsys_channel_users ORDER BY channel_name,position")
        .fetch_all(&mut *c)
        .await?
    {
        let name: String = r.try_get("channel_name")?;
        let who = ObjectId(r.try_get("who")?);
        let on: i64 = r.try_get("is_on")?;
        ensure!(
            on == 0 || on == 1,
            "channel {name}: invalid listening value {on}"
        );
        let ch = w
            .channels
            .get_mut(&name)
            .with_context(|| format!("membership references missing channel {name}"))?;
        ensure!(
            w.objects.contains_key(&who),
            "channel {name}: invalid member #{}",
            who.0
        );
        ensure!(
            !ch.users.iter().any(|u| u.who == who),
            "channel {name}: duplicate member #{}",
            who.0
        );
        ch.users.push(Membership {
            who,
            listening: on != 0,
        });
        ch.max_users = ch.users.len().div_ceil(10) * 10;
    }
    for r in sqlx::query("SELECT * FROM comsys_channel_messages ORDER BY channel_name,position")
        .fetch_all(&mut *c)
        .await?
    {
        let name: String = r.try_get("channel_name")?;
        w.channels
            .get_mut(&name)
            .with_context(|| format!("history references missing channel {name}"))?
            .history
            .push(ChannelMessage {
                at: r.try_get("sent_at")?,
                message: r.try_get("message")?,
            });
    }
    for r in sqlx::query("SELECT * FROM commac_aliases ORDER BY who,position")
        .fetch_all(&mut *c)
        .await?
    {
        let who = ObjectId(r.try_get("who")?);
        let alias: String = r.try_get("alias")?;
        let channel: String = r.try_get("channel_name")?;
        ensure!(
            w.objects.contains_key(&who),
            "channel alias {alias}: invalid owner #{}",
            who.0
        );
        let entries = w.channel_aliases.entry(who).or_default();
        ensure!(
            !entries.iter().any(|a| a.alias.eq_ignore_ascii_case(&alias)),
            "duplicate channel alias {alias} for #{}",
            who.0
        );
        entries.push(ChannelAlias { alias, channel });
    }
    for r in sqlx::query("SELECT * FROM player_last_page_recipients ORDER BY player_dbref,position")
        .fetch_all(&mut *c)
        .await?
    {
        let who = ObjectId(r.try_get("player_dbref")?);
        ensure!(
            w.accounts.contains_key(&who),
            "last-page owner #{} missing",
            who.0
        );
        w.last_pages
            .entry(who)
            .or_default()
            .push(ObjectId(r.try_get("recipient_dbref")?));
    }
    Ok(())
}

/// Project ordered records onto their existing compound keys for field-level updates.
fn records(w: &World, table: &str) -> BTreeMap<(String, i64), Fields> {
    let mut out = BTreeMap::new();
    match table {
        "comsys_channel_users" => {
            for (name, c) in &w.channels {
                for (i, u) in c.users.iter().enumerate() {
                    out.insert(
                        (name.clone(), i as i64),
                        Fields::from([
                            ("who", Cell::Integer(u.who.0)),
                            ("is_on", Cell::Integer(i64::from(u.listening))),
                        ]),
                    );
                }
            }
        }
        "comsys_channel_messages" => {
            for (name, c) in &w.channels {
                for (i, m) in c.history.iter().enumerate() {
                    out.insert(
                        (name.clone(), i as i64),
                        Fields::from([
                            ("sent_at", Cell::Integer(m.at)),
                            ("message", Cell::Text(m.message.clone())),
                        ]),
                    );
                }
            }
        }
        "commac_aliases" => {
            for (who, aliases) in &w.channel_aliases {
                for (i, a) in aliases.iter().enumerate() {
                    out.insert(
                        (who.0.to_string(), i as i64),
                        Fields::from([
                            ("alias", Cell::Text(a.alias.clone())),
                            ("channel_name", Cell::Text(a.channel.clone())),
                        ]),
                    );
                }
            }
        }
        "player_last_page_recipients" => {
            for (who, recipients) in &w.last_pages {
                for (i, id) in recipients.iter().enumerate() {
                    out.insert(
                        (who.0.to_string(), i as i64),
                        Fields::from([("recipient_dbref", Cell::Integer(id.0))]),
                    );
                }
            }
        }
        _ => unreachable!(),
    }
    out
}

/// Touch only changed supported records, preserving unknown columns on retained positions.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for table in [
        "comsys_channel_users",
        "comsys_channel_messages",
        "commac_aliases",
        "player_last_page_recipients",
    ] {
        let old = records(before, table);
        let mut new = records(after, table);
        let affected = old
            .keys()
            .chain(new.keys())
            .filter(|k| old.get(*k) != new.get(*k))
            .map(|k| k.0.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if affected.is_empty() {
            continue;
        }
        // Read actual slots only for changed owners. Untouched sparse legacy positions stay intact.
        let old = stored_records(c, table)
            .await?
            .into_iter()
            .filter(|(k, _)| affected.contains(&k.0))
            .collect::<BTreeMap<_, _>>();
        new.retain(|k, _| affected.contains(&k.0));
        let key = |(owner, position): &(String, i64)| {
            let column = match table {
                "commac_aliases" => "who",
                "player_last_page_recipients" => "player_dbref",
                _ => "channel_name",
            };
            Fields::from([
                (
                    column,
                    if column == "channel_name" {
                        Cell::Text(owner.clone())
                    } else {
                        Cell::Integer(owner.parse().expect("typed dbref"))
                    },
                ),
                ("position", Cell::Integer(*position)),
            ])
        };
        for (k, value) in &new {
            changed |= row(c, table, key(k), old.get(k), value)
                .await
                .with_context(|| format!("writing {table} {k:?}"))?;
        }
        for k in old.keys().filter(|k| !new.contains_key(k)) {
            delete(c, table, key(k)).await?;
            changed = true;
        }
    }
    // A commac identity owns macro slots as well as aliases: initialize only missing identities.
    for who in after
        .channel_aliases
        .keys()
        .filter(|who| !before.channel_aliases.contains_key(who))
    {
        sqlx::query("INSERT INTO commac_entries(who,curmac,macro_slot_0,macro_slot_1,macro_slot_2,macro_slot_3,macro_slot_4) SELECT ?,-1,-1,-1,-1,-1,-1 WHERE NOT EXISTS(SELECT 1 FROM commac_entries WHERE who=?)").bind(who.0).bind(who.0).execute(&mut *c).await?;
    }
    for name in before
        .channels
        .keys()
        .filter(|name| !after.channels.contains_key(*name))
    {
        check_dependencies(c, name).await?;
        delete(
            c,
            "comsys_channels",
            Fields::from([("name", Cell::Text(name.clone()))]),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}

/// Read supported values with original composite keys, never reflecting unknown columns.
async fn stored_records(
    c: &mut SqliteConnection,
    table: &str,
) -> Result<BTreeMap<(String, i64), Fields>> {
    let mut result = BTreeMap::new();
    // table comes only from the fixed catalog in save().
    for r in sqlx::query(sqlx::AssertSqlSafe(format!("SELECT * FROM {table}")))
        .fetch_all(c)
        .await?
    {
        let owner = match table {
            "commac_aliases" => r.try_get::<i64, _>("who")?.to_string(),
            "player_last_page_recipients" => r.try_get::<i64, _>("player_dbref")?.to_string(),
            _ => r.try_get("channel_name")?,
        };
        let value = match table {
            "comsys_channel_users" => Fields::from([
                ("who", Cell::Integer(r.try_get("who")?)),
                ("is_on", Cell::Integer(r.try_get("is_on")?)),
            ]),
            "comsys_channel_messages" => Fields::from([
                ("sent_at", Cell::Integer(r.try_get("sent_at")?)),
                ("message", Cell::Text(r.try_get("message")?)),
            ]),
            "commac_aliases" => Fields::from([
                ("alias", Cell::Text(r.try_get("alias")?)),
                ("channel_name", Cell::Text(r.try_get("channel_name")?)),
            ]),
            "player_last_page_recipients" => Fields::from([(
                "recipient_dbref",
                Cell::Integer(r.try_get("recipient_dbref")?),
            )]),
            _ => unreachable!(),
        };
        result.insert((owner, r.try_get("position")?), value);
    }
    Ok(result)
}

/// Unknown declared dependencies are preserved by refusing channel destruction.
async fn check_dependencies(c: &mut SqliteConnection, name: &str) -> Result<()> {
    fn quote(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&mut *c)
            .await?;
    for table in tables {
        for fk in sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA foreign_key_list({})",
            quote(&table)
        )))
        .fetch_all(&mut *c)
        .await?
        {
            let target: String = fk.try_get("table")?;
            if target != "comsys_channels" {
                continue;
            }
            let column: String = fk.try_get("from")?;
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {} WHERE {}=?",
                quote(&table),
                quote(&column)
            )))
            .bind(name)
            .fetch_one(&mut *c)
            .await?;
            ensure!(
                count == 0,
                "channel {name}: dependency {table}.{column} prevents destruction"
            );
        }
    }
    Ok(())
}
