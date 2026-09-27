//! Field-level relational changes and legacy containment-list maintenance.
use crate::{
    accounts::Account, communication::Channel, flags::Flag, state::Value as Scalar, world::*,
};
use anyhow::{Context, Result, ensure};
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// SQL values for explicitly owned columns; unknown columns never enter a write set.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Cell {
    Null,
    Integer(i64),
    Number(f64),
    Text(String),
    Blob(Vec<u8>),
}
impl Cell {
    /// Bind values without interpolating user-controlled SQL.
    fn bind(&self, q: &mut QueryBuilder<Sqlite>) {
        match self {
            Self::Null => {
                q.push_bind(None::<i64>);
            }
            Self::Integer(v) => {
                q.push_bind(*v);
            }
            Self::Number(v) => {
                q.push_bind(*v);
            }
            Self::Text(v) => {
                q.push_bind(v.clone());
            }
            Self::Blob(v) => {
                q.push_bind(v.clone());
            }
        }
    }
}
/// Known column/value projection of a row.
pub(super) type Fields = BTreeMap<String, Cell>;
/// Convert fixed, code-owned column names into a field map.
pub(super) fn fields(items: impl IntoIterator<Item = (&'static str, Cell)>) -> Fields {
    items
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect()
}
/// Encode the legacy negative-reference sentinel for newly written references.
fn reference(id: Option<ObjectId>) -> Cell {
    Cell::Integer(id.map_or(-1, |id| id.0))
}
/// Nullable legacy text fields.
fn text(value: &Option<String>) -> Cell {
    value.as_ref().map_or(Cell::Null, |s| Cell::Text(s.clone()))
}
/// Nullable timestamps.
fn integer(value: Option<i64>) -> Cell {
    value.map_or(Cell::Null, Cell::Integer)
}
/// Append primary-key predicates, using only schema-owned identifiers.
fn predicate(q: &mut QueryBuilder<Sqlite>, key: &Fields) {
    for (index, (name, value)) in key.iter().enumerate() {
        if index > 0 {
            q.push(" AND ");
        }
        q.push(name).push(" = ");
        value.bind(q);
    }
}
/// Insert new rows or update only changed supported fields. Never use REPLACE.
pub(super) async fn row(
    c: &mut SqliteConnection,
    table: &str,
    key: Fields,
    before: Option<&Fields>,
    after: &Fields,
) -> Result<bool> {
    if let Some(before) = before {
        let mut changed = after
            .iter()
            .filter(|(name, value)| before.get(*name) != Some(*value))
            .peekable();
        if changed.peek().is_none() {
            return Ok(false);
        }
        update(c, table, &key, changed).await?;
        return Ok(true);
    }
    let all: Vec<_> = key.iter().chain(after.iter()).collect();
    let mut query = QueryBuilder::new(format!("INSERT INTO {table} ("));
    for (index, (name, _)) in all.iter().enumerate() {
        if index > 0 {
            query.push(",");
        }
        query.push(name.as_str());
    }
    query.push(") VALUES (");
    for (index, (_, value)) in all.iter().enumerate() {
        if index > 0 {
            query.push(",");
        }
        value.bind(&mut query);
    }
    query.push(")");
    execute_one(c, table, &key, query).await?;
    Ok(true)
}
/// Update the given columns of one existing row.
pub(super) async fn update<'a>(
    c: &mut SqliteConnection,
    table: &str,
    key: &Fields,
    values: impl IntoIterator<Item = (&'a String, &'a Cell)>,
) -> Result<()> {
    let mut query = QueryBuilder::new(format!("UPDATE {table} SET "));
    for (index, (name, value)) in values.into_iter().enumerate() {
        if index > 0 {
            query.push(",");
        }
        query.push(name).push(" = ");
        value.bind(&mut query);
    }
    query.push(" WHERE ");
    predicate(&mut query, key);
    execute_one(c, table, key, query).await
}
/// Run a statement that must affect exactly one row.
async fn execute_one(
    c: &mut SqliteConnection,
    table: &str,
    key: &Fields,
    mut query: QueryBuilder<Sqlite>,
) -> Result<()> {
    let changed = query
        .build()
        .execute(&mut *c)
        .await
        .with_context(|| format!("writing {table} {key:?}"))?
        .rows_affected();
    ensure!(
        changed == 1,
        "expected one {table} row for {key:?}; found {changed}"
    );
    Ok(())
}
/// Delete a specifically removed owned key, never an entire table or object.
pub(super) async fn delete(c: &mut SqliteConnection, table: &str, key: Fields) -> Result<()> {
    let mut query = QueryBuilder::new(format!("DELETE FROM {table} WHERE "));
    predicate(&mut query, &key);
    query
        .build()
        .execute(c)
        .await
        .with_context(|| format!("removing {table} {key:?}"))?;
    Ok(())
}
/// Rows of one table keyed by their key column values, as used by [`sync_rows`].
pub(super) type Rows = BTreeMap<Vec<i64>, Fields>;

/// Read one column of a fetched row as whichever SQL value it holds.
fn read_cell(entry: &sqlx::sqlite::SqliteRow, name: &str) -> Result<Cell> {
    use sqlx::{TypeInfo, ValueRef};
    let raw = entry.try_get_raw(name)?;
    if raw.is_null() {
        return Ok(Cell::Null);
    }
    let kind = raw.type_info().name().to_owned();
    Ok(match kind.as_str() {
        "INTEGER" => Cell::Integer(entry.try_get(name)?),
        "REAL" => Cell::Number(entry.try_get(name)?),
        "TEXT" => Cell::Text(entry.try_get(name)?),
        "BLOB" => Cell::Blob(entry.try_get(name)?),
        other => anyhow::bail!("unsupported SQL value type {other} in column {name}"),
    })
}

/// Make the rows of `table` within `scope` match `desired`.
///
/// `scope` fixes leading key columns, such as the owning object, and may be empty to
/// cover the whole table. Rows within it are identified by `keys`; `desired` maps each
/// row's key values to its `columns`. Keys must be integers; columns may hold any SQL
/// value. Stored rows are read back first, so only changed columns are updated, new rows are
/// inserted and rows absent from `desired` are deleted. Returns whether anything was
/// written.
pub(super) async fn sync_rows(
    c: &mut SqliteConnection,
    table: &str,
    scope: &[(&'static str, i64)],
    keys: &[&'static str],
    columns: &[&'static str],
    desired: &Rows,
) -> Result<bool> {
    let selected: Vec<&str> = keys.iter().chain(columns).copied().collect();
    let mut query = QueryBuilder::new(format!("SELECT {} FROM {table}", selected.join(",")));
    let scope_key: Fields = scope
        .iter()
        .map(|(name, value)| ((*name).into(), Cell::Integer(*value)))
        .collect();
    if !scope_key.is_empty() {
        query.push(" WHERE ");
        predicate(&mut query, &scope_key);
    }
    let mut stored = BTreeMap::new();
    for entry in query.build().fetch_all(&mut *c).await? {
        let key = keys
            .iter()
            .map(|name| entry.try_get::<i64, _>(*name))
            .collect::<Result<Vec<_>, _>>()?;
        let mut values = Fields::new();
        for name in columns {
            values.insert((*name).into(), read_cell(&entry, name)?);
        }
        stored.insert(key, values);
    }
    let row_key = |values: &[i64]| {
        let mut key = scope_key.clone();
        key.extend(
            keys.iter()
                .zip(values)
                .map(|(name, value)| ((*name).into(), Cell::Integer(*value))),
        );
        key
    };
    let mut changed = false;
    for (key, values) in desired {
        let previous = stored.remove(key);
        changed |= row(c, table, row_key(key), previous.as_ref(), values).await?;
    }
    for key in stored.keys() {
        delete(c, table, row_key(key)).await?;
        changed = true;
    }
    Ok(changed)
}

/// [`sync_rows`], skipped without touching the database when `desired` equals
/// `previous`, the rows produced from the last saved world.
pub(super) async fn sync_changed_rows(
    c: &mut SqliteConnection,
    table: &str,
    scope: &[(&'static str, i64)],
    keys: &[&'static str],
    columns: &[&'static str],
    previous: Option<&Rows>,
    desired: &Rows,
) -> Result<bool> {
    if previous == Some(desired) {
        return Ok(false);
    }
    sync_rows(c, table, scope, keys, columns, desired).await
}

/// Supported object fields; relationship-list slots are supplied from durable rows.
fn object(o: &Object, links: LinkSlots) -> Fields {
    let mut result = fields([
        ("name", Cell::Text(o.name.clone())),
        ("type", Cell::Integer(o.kind.code())),
        (
            "location",
            reference(if o.kind == Kind::Exit {
                o.destination
            } else if o.kind == Kind::Room {
                o.dropto
            } else {
                o.location
            }),
        ),
        ("zone", reference(o.zone)),
        ("affiliation", reference(o.affiliation)),
        ("link", reference(o.home)),
        ("lua_parent", Cell::Text(o.lua_parent.clone())),
        ("description", text(&o.description)),
        ("internal_description", text(&o.internal_description)),
        ("contents", Cell::Integer(links.contents)),
        (
            "exits",
            if o.kind == Kind::Exit {
                reference(o.location)
            } else {
                Cell::Integer(links.exits)
            },
        ),
        ("next", Cell::Integer(links.next)),
        (
            "has_idle_power",
            Cell::Integer(i64::from(o.powers.contains(crate::powers::Power::Idle))),
        ),
    ]);
    for flag in crate::flags::ALL {
        result.insert(
            format!("has_{}_flag", flag.world_name().to_ascii_lowercase()),
            Cell::Integer(i64::from(flag != Flag::Connected && o.flags.contains(flag))),
        );
    }
    result
}
/// Account columns exclude all deferred player and BattleTech state.
fn account(a: &Account) -> Fields {
    fields([
        ("password_hash", text(&a.hash)),
        ("alias", text(&a.alias)),
        ("last_login", integer(a.last_login)),
        ("last_site", text(&a.last_site)),
        ("successful_login_count", Cell::Integer(a.successes)),
        ("failed_login_count", Cell::Integer(a.failures)),
        (
            "unreported_failed_login_count",
            Cell::Integer(a.unreported_failures),
        ),
    ])
}
/// Preserve legacy numeric type tags; newly written strings use byte-preserving blobs.
fn scalar(s: &Scalar) -> Result<Fields> {
    let (tag, value) = match s {
        Scalar::Boolean(v) => (2, Cell::Integer(i64::from(*v))),
        Scalar::Integer(v) => (3, Cell::Integer(*v)),
        Scalar::Number(v) => {
            ensure!(v.is_finite(), "Lua state number must be finite");
            (4, Cell::Number(*v))
        }
        Scalar::String(v) => (1, Cell::Blob(v.clone())),
    };
    Ok(fields([
        ("value_type", Cell::Integer(tag)),
        ("value", value),
    ]))
}
/// Flatten only supported Lua keys, retaining no deferred data in the Rust world model.
fn state<'a>(
    objects: impl Iterator<Item = &'a Object>,
) -> BTreeMap<(i64, String, String), &'a Scalar> {
    objects
        .flat_map(|o| {
            o.state.iter().flat_map(move |(ns, values)| {
                values
                    .iter()
                    .map(move |(key, value)| ((o.id.0, ns.clone(), key.clone()), value))
            })
        })
        .collect()
}
/// Primary key of a Lua scalar row.
fn state_key(key: &(i64, String, String)) -> Fields {
    fields([
        ("object_dbref", Cell::Integer(key.0)),
        ("namespace", Cell::Text(key.1.clone())),
        ("key", Cell::Text(key.2.clone())),
    ])
}
/// Legacy history is newest-first separately for successes and failures.
fn history(a: &Account) -> BTreeMap<(i64, i64), Fields> {
    let mut result = BTreeMap::new();
    for (success, capacity) in [(true, 4), (false, 3)] {
        for (index, login) in a
            .history
            .iter()
            .rev()
            .filter(|l| l.success == success)
            .take(capacity)
            .enumerate()
        {
            result.insert(
                (i64::from(!success), index as i64),
                fields([
                    ("occurred_at", Cell::Integer(login.at)),
                    ("host", Cell::Text(login.host.clone())),
                ]),
            );
        }
    }
    result
}
/// Primary key of a bounded history slot.
fn history_key(id: ObjectId, key: (i64, i64)) -> Fields {
    fields([
        ("player_dbref", Cell::Integer(id.0)),
        ("outcome", Cell::Integer(key.0)),
        ("position", Cell::Integer(key.1)),
    ])
}
/// Validate and update only containment lists whose membership actually changed.
fn relationships(before: &World, after: &World, raw: &Links) -> Result<Links> {
    let mut links = raw.clone();
    let mut affected = BTreeSet::new();
    for o in after.objects.values() {
        let old = before.objects.get(&o.id);
        if old.is_some_and(|v| v.kind == o.kind && v.location == o.location) {
            continue;
        }
        if let Some(old) = old
            && let Some(container) = old.location
        {
            affected.insert((container, old.kind == Kind::Exit));
        }
        if let Some(container) = o.location {
            affected.insert((container, o.kind == Kind::Exit));
        }
        links.entry(o.id).or_default().next = -1;
    }
    for (container, exits) in affected {
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        let mut next = raw.get(&container).map_or(-1, |slots| slots.head(exits));
        while next >= 0 {
            let id = ObjectId(next);
            ensure!(
                seen.insert(id),
                "cycle in {} list of #{}",
                if exits { "exit" } else { "contents" },
                container.0
            );
            let o = before
                .objects
                .get(&id)
                .with_context(|| format!("missing linked object #{next}"))?;
            ensure!(
                o.location == Some(container) && (o.kind == Kind::Exit) == exits,
                "inconsistent list member #{next} in #{}",
                container.0
            );
            ordered.push(id);
            next = raw.get(&id).context("linked object row missing")?.next;
        }
        let expected: BTreeSet<_> = before
            .objects
            .values()
            .filter(|o| o.location == Some(container) && (o.kind == Kind::Exit) == exits)
            .map(|o| o.id)
            .collect();
        ensure!(
            seen == expected,
            "incomplete containment list at #{}",
            container.0
        );
        let members: BTreeSet<_> = after
            .objects
            .values()
            .filter(|o| o.location == Some(container) && (o.kind == Kind::Exit) == exits)
            .map(|o| o.id)
            .collect();
        ordered.retain(|id| members.contains(id));
        let retained: BTreeSet<_> = ordered.iter().copied().collect();
        ordered.extend(members.difference(&retained).copied());
        *links.entry(container).or_default().head_mut(exits) =
            ordered.first().map_or(-1, |id| id.0);
        for (index, id) in ordered.iter().enumerate() {
            links.entry(*id).or_default().next = ordered.get(index + 1).map_or(-1, |id| id.0);
        }
    }
    Ok(links)
}
/// Write changed object rows, including the containment-list links that moves change.
///
/// An object still shared with `before` is unchanged, but its row also holds list links
/// that a sibling's move can change, so it is skipped only when those links are
/// unchanged too. Maintenance rewrites lists wholesale and checks every row.
async fn save_objects(
    c: &mut SqliteConnection,
    before: &World,
    after: &World,
    maintenance: Option<&crate::dbck::RepairPlan>,
) -> Result<bool> {
    if maintenance.is_none() && before.objects.ptr_eq(&after.objects) {
        return Ok(false);
    }
    // A new cycle must pass through an object whose location changed.
    for o in after.objects.values() {
        if maintenance.is_none() && before.objects.shares_entry(&after.objects, &o.id) {
            continue;
        }
        let chain = after.containment_chain(o.location)?;
        ensure!(!chain.contains(&o.id), "containment cycle at #{}", o.id.0);
    }
    let mut raw = BTreeMap::new();
    let mut connected = BTreeSet::new();
    for r in sqlx::query("SELECT dbref,contents,exits,next,has_connected_flag FROM objects")
        .fetch_all(&mut *c)
        .await?
    {
        let id = ObjectId(r.try_get("dbref")?);
        raw.insert(
            id,
            LinkSlots {
                contents: r.try_get("contents")?,
                exits: r.try_get("exits")?,
                next: r.try_get("next")?,
            },
        );
        if r.try_get::<i64, _>("has_connected_flag")? != 0 {
            connected.insert(id);
        }
    }
    let links = match maintenance {
        Some(plan) => plan.links.clone(),
        None => relationships(before, after, &raw)?,
    };
    let mut changed = false;
    for (id, o) in &after.objects {
        let stored = raw.get(id).copied().unwrap_or_default();
        let linked = links.get(id).copied().unwrap_or_default();
        if maintenance.is_none()
            && before.objects.shares_entry(&after.objects, id)
            && stored == linked
            && !connected.contains(id)
        {
            continue;
        }
        let mut old = before.objects.get(id).map(|o| object(o, stored));
        if connected.contains(id)
            && let Some(old) = old.as_mut()
        {
            old.insert("has_connected_flag".into(), Cell::Integer(1));
        }
        changed |= row(
            c,
            "objects",
            fields([("dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &object(o, linked),
        )
        .await?;
    }
    Ok(changed)
}
/// Apply a supported projection delta to an already-open write transaction, reporting
/// whether any row changed. The simulation clock is written alongside other changes,
/// or alone once it is `clock_interval` seconds ahead of the stored clock (never when
/// zero).
pub(super) async fn apply(
    c: &mut SqliteConnection,
    before: &World,
    after: &World,
    clock_interval: u64,
) -> Result<bool> {
    apply_changes(c, before, after, None, clock_interval).await
}
/// Explicit maintenance writes may repair lists and remove only approved accounts.
pub(super) async fn apply_changes(
    c: &mut SqliteConnection,
    before: &World,
    after: &World,
    maintenance: Option<&crate::dbck::RepairPlan>,
    clock_interval: u64,
) -> Result<bool> {
    super::btech::validate_changes(before, after, maintenance.map(|plan| &plan.purges))?;
    ensure!(
        before.objects.keys().all(|k| after.objects.contains_key(k)),
        "object deletion is not supported; deferred rows must be preserved"
    );
    ensure!(
        before
            .accounts
            .keys()
            .all(|k| after.accounts.contains_key(k)
                || maintenance.is_some_and(|plan| plan.purges.contains(k))),
        "account deletion is not supported"
    );
    let mut changed = save_objects(c, before, after, maintenance).await?;
    for (id, a) in &after.accounts {
        let old = before.accounts.get(id).map(account);
        changed |= row(
            c,
            "player_state",
            fields([("object_dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &account(a),
        )
        .await?;
        if before
            .accounts
            .get(id)
            .is_some_and(|old| old.history == a.history)
        {
            continue;
        }
        // Compare actual legacy slots, not a reconstructed ordering of the loaded history.
        let mut old = BTreeMap::new();
        for r in sqlx::query("SELECT outcome,position,occurred_at,host FROM player_login_history WHERE player_dbref=?1").bind(id.0).fetch_all(&mut *c).await? {
            old.insert((r.try_get::<i64,_>("outcome")?,r.try_get::<i64,_>("position")?),fields([("occurred_at",Cell::Integer(r.try_get("occurred_at")?)),("host",Cell::Text(r.try_get("host")?))]));
        }
        let new = history(a);
        for (key, values) in &new {
            changed |= row(
                c,
                "player_login_history",
                history_key(*id, *key),
                old.get(key),
                values,
            )
            .await?;
        }
        for key in old.keys().filter(|k| !new.contains_key(k)) {
            delete(c, "player_login_history", history_key(*id, *key)).await?;
            changed = true;
        }
    }

    // An object still shared with the baseline carries the baseline's own Lua state.
    let edited: Vec<ObjectId> = after
        .objects
        .keys()
        .filter(|id| !before.objects.shares_entry(&after.objects, id))
        .copied()
        .collect();
    let old = state(edited.iter().filter_map(|id| before.objects.get(id)));
    let new = state(edited.iter().filter_map(|id| after.objects.get(id)));
    for (key, value) in &new {
        if old.get(key) == Some(value) {
            continue;
        }
        let prior = old.get(key).map(|v| scalar(v)).transpose()?;
        changed |= row(
            c,
            "object_state",
            state_key(key),
            prior.as_ref(),
            &scalar(value)?,
        )
        .await?;
    }
    for key in old.keys().filter(|k| !new.contains_key(k)) {
        delete(c, "object_state", state_key(key)).await?;
        changed = true;
    }
    for (name, ch) in &after.channels {
        let channel = |ch: &Channel| {
            fields([
                ("type", Cell::Integer(ch.flags.0)),
                ("num_messages", Cell::Integer(ch.messages)),
                ("chan_obj", reference(ch.object)),
            ])
        };
        let old = before.channels.get(name).map(channel);
        changed |= row(
            c,
            "comsys_channels",
            fields([("name", Cell::Text(name.clone()))]),
            old.as_ref(),
            &channel(ch),
        )
        .await?;
    }
    changed |= super::communication::save(c, before, after).await?;
    changed |= super::macros::save(c, before, after).await?;
    changed |= super::btech::save(c, before, after).await?;
    // The simulation clock goes last, since it is written alongside any other change.
    changed |= super::btech_turn_clock::save(c, after, changed, clock_interval).await?;
    let next = after
        .next_id
        .max(before.next_id)
        .max(after.objects.keys().next_back().map_or(0, |id| id.0 + 1));
    changed |= next != before.next_id || after.record_players != before.record_players;
    if changed {
        sqlx::query(
            "UPDATE snapshot SET db_top=max(db_top,?1),record_players=?2,dump_time=?3 WHERE id=1",
        )
        .bind(next)
        .bind(i64::try_from(after.record_players)?)
        .bind(crate::clock::wall_time())
        .execute(&mut *c)
        .await?;
    }
    Ok(changed)
}
