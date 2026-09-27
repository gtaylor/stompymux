//! Read the supported projection of schema-32 storage without touching deferred data.
use crate::{
    accounts::{Account, Login},
    communication::Channel,
    state::Value as Scalar,
    world::*,
};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, VecDeque};
/// Convert the legacy negative-reference sentinel.
fn id(value: i64) -> Option<ObjectId> {
    (value >= 0).then_some(ObjectId(value))
}
/// Decode legacy rows with checked SQLite storage classes for Lua scalar values.
pub(super) async fn read(c: &mut SqliteConnection) -> Result<World> {
    let (version, next, record): (i64, i64, i64) =
        sqlx::query_as("SELECT schema_version,db_top,record_players FROM snapshot WHERE id=1")
            .fetch_one(&mut *c)
            .await?;
    ensure!(
        version == 32,
        "unsupported legacy schema {version}; expected 32"
    );
    let mut w = World {
        next_id: next,
        record_players: usize::try_from(record).context("invalid record_players")?,
        initialized: true,
        ..Default::default()
    };
    for r in sqlx::query("SELECT * FROM objects")
        .fetch_all(&mut *c)
        .await?
    {
        let object_id = ObjectId(r.try_get("dbref")?);
        let kind = Kind::from_code(r.try_get("type")?)?;
        let link = id(r.try_get("link")?);
        let mut flags = crate::flags::FlagSet::default();
        let mut powers = crate::powers::PowerSet::default();
        for flag in crate::flags::ALL {
            let column = flag.column();
            let value: i64 = r
                .try_get(column)
                .with_context(|| format!("object #{}: {column}", object_id.0))?;
            ensure!(
                value == 0 || value == 1,
                "object #{}: invalid {} value {}",
                object_id.0,
                column,
                value
            );
            if value == 1 && flag != crate::flags::Flag::Connected {
                flags.insert(flag);
            }
        }
        let idle: i64 = r.try_get("has_idle_power")?;
        ensure!(
            idle == 0 || idle == 1,
            "object #{}: invalid has_idle_power",
            object_id.0
        );
        if idle == 1 {
            powers.insert(crate::powers::Power::Idle);
        }
        w.objects.insert(
            object_id,
            Object {
                generation: Default::default(),
                pending_destroyer: None,
                id: object_id,
                name: r.try_get("name")?,
                kind,
                location: if kind == Kind::Room {
                    None
                } else {
                    id(r.try_get(if kind == Kind::Exit {
                        "exits"
                    } else {
                        "location"
                    })?)
                },
                dropto: if kind == Kind::Room {
                    id(r.try_get("location")?)
                } else {
                    None
                },
                zone: id(r.try_get("zone")?),
                affiliation: id(r.try_get("affiliation")?),
                home: if kind != Kind::Exit { link } else { None },
                destination: if kind == Kind::Exit {
                    id(r.try_get("location")?)
                } else {
                    None
                },
                description: r.try_get("description")?,
                internal_description: r.try_get("internal_description")?,
                lua_parent: r.try_get("lua_parent")?,
                flags,
                powers,
                state: Default::default(),
            },
        );
    }
    for r in sqlx::query("SELECT * FROM player_state")
        .fetch_all(&mut *c)
        .await?
    {
        w.accounts.insert(
            ObjectId(r.try_get("object_dbref")?),
            Account {
                hash: r.try_get("password_hash")?,
                alias: r.try_get("alias")?,
                last_login: r.try_get("last_login")?,
                last_site: r.try_get("last_site")?,
                successes: r.try_get("successful_login_count")?,
                failures: r.try_get("failed_login_count")?,
                unreported_failures: r.try_get("unreported_failed_login_count")?,
                history: Vec::new(),
            },
        );
    }
    let mut histories: BTreeMap<ObjectId, (VecDeque<Login>, VecDeque<Login>)> = BTreeMap::new();
    for r in sqlx::query(
        "SELECT * FROM player_login_history ORDER BY player_dbref,outcome,position DESC",
    )
    .fetch_all(&mut *c)
    .await?
    {
        let player = ObjectId(r.try_get("player_dbref")?);
        ensure!(
            w.accounts.contains_key(&player),
            "login history account missing"
        );
        let login = Login {
            success: r.try_get::<i64, _>("outcome")? == 0,
            at: r.try_get("occurred_at")?,
            host: r.try_get("host")?,
        };
        let queues = histories.entry(player).or_default();
        if login.success {
            queues.0.push_back(login);
        } else {
            queues.1.push_back(login);
        }
    }
    for (player, (mut successes, mut failures)) in histories {
        let history = &mut w.accounts.get_mut(&player).unwrap().history;
        // Positions define order within an outcome. Merge the two queues by time where the
        // legacy schema retains enough information, without reordering either queue.
        while !successes.is_empty() || !failures.is_empty() {
            let success = match (successes.front(), failures.front()) {
                (Some(success), Some(failure)) => success.at <= failure.at,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            history.push(if success {
                successes.pop_front().unwrap()
            } else {
                failures.pop_front().unwrap()
            });
        }
    }
    for r in sqlx::query("SELECT *, typeof(value) AS storage_type FROM object_state")
        .fetch_all(&mut *c)
        .await?
    {
        let kind: i64 = r.try_get("value_type")?;
        let storage: String = r.try_get("storage_type")?;
        let object_id = ObjectId(r.try_get("object_dbref")?);
        let namespace: String = r.try_get("namespace")?;
        let key: String = r.try_get("key")?;
        let value = (|| -> Result<Scalar> {
            crate::state::address(&namespace, Some(&key))?;
            Ok(match (kind, storage.as_str()) {
                (3, "integer") => Scalar::Integer(r.try_get("value")?),
                (2, "integer") => {
                    let value: i64 = r.try_get("value")?;
                    ensure!(value == 0 || value == 1, "boolean must be 0 or 1");
                    Scalar::Boolean(value == 1)
                }
                (4, "real") => {
                    let value: f64 = r.try_get("value")?;
                    ensure!(value.is_finite(), "number must be finite");
                    Scalar::Number(value)
                }
                (4, "integer") => Scalar::Number(r.try_get::<i64, _>("value")? as f64),
                (1, "text" | "blob") => Scalar::String(r.try_get("value")?),
                _ => anyhow::bail!("unsupported state encoding {kind} ({storage})"),
            })
        })()
        .with_context(|| format!("object #{} state {namespace}/{key}", object_id.0))?;
        w.objects
            .get_mut(&object_id)
            .context("state object missing")?
            .state
            .entry(namespace)
            .or_default()
            .insert(key, value);
    }
    for r in sqlx::query("SELECT * FROM comsys_channels")
        .fetch_all(&mut *c)
        .await?
    {
        let name: String = r.try_get("name")?;
        w.channels.insert(
            name.clone(),
            Channel {
                name: name.clone(),
                object: id(r.try_get("chan_obj")?),
                flags: crate::communication::ChannelFlags(r.try_get("type")?),
                messages: r.try_get("num_messages")?,
                ..Channel::new(name)
            },
        );
    }
    super::communication::load(c, &mut w).await?;
    super::macros::load(c, &mut w).await?;
    w.btech = super::btech::load(c).await?;
    super::btech_player_configuration::normalize(&mut w);
    super::btech_unit_configuration::normalize(&mut w);
    w.btech.validate(&w)?;
    w.next_id = w
        .next_id
        .max(w.objects.keys().next_back().map_or(0, |id| id.0 + 1));
    w.links = std::sync::Arc::new(super::maintenance::links(c).await?);
    Ok(w)
}
