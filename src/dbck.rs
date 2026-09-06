//! Deterministic semantic repair plans, separate from transaction execution and sessions.
use crate::{
    config::Config,
    flags::{self, Flag},
    world::*,
};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// Legacy contents head, exits head (or exit source), and next member.
pub type Links = BTreeMap<ObjectId, [i64; 3]>;
/// A reference or flag adjustment made by maintenance.
#[derive(Debug)]
pub struct FieldRepair {
    /// Object being repaired.
    pub object: ObjectId,
    /// Supported field changed.
    pub field: &'static str,
}
/// Occupant relocation requiring movement callbacks before commit.
#[derive(Debug)]
pub struct Relocation {
    /// Moved occupant.
    pub object: ObjectId,
    /// Original immediate container, possibly invalid.
    pub source: Option<ObjectId>,
    /// Safe replacement container.
    pub destination: ObjectId,
}
/// Reviewable operations approved before destructive SQL is issued.
#[derive(Debug, Default)]
pub struct RepairPlan {
    /// Reference and flag repairs.
    pub fields: Vec<FieldRepair>,
    /// Player/thing transitions in ascending dbref order.
    pub relocations: Vec<Relocation>,
    /// Complete repaired legacy list slots; only differences are written.
    pub links: Links,
    /// Rows with changed list slots.
    pub list_changes: Vec<ObjectId>,
    /// Objects converted to retained garbage tombstones.
    pub purges: BTreeSet<ObjectId>,
    /// Player identities whose sessions must detach after commit.
    pub detachments: BTreeSet<ObjectId>,
}
/// Diagnostic results, including observations that require no mutation.
#[derive(Debug, Default)]
pub struct DbCheckReport {
    /// Detailed findings for server diagnostics.
    pub findings: Vec<String>,
    /// Transactional changes made by the checker.
    pub plan: RepairPlan,
}
impl DbCheckReport {
    /// Reserve the completion marker even when the configured output budget is small.
    pub fn response(&self, limit: usize) -> Vec<u8> {
        const FOOTER: &[u8] = b"Done.\r\n";
        if limit < FOOTER.len() {
            return crate::find::bounded_error("Done.", limit);
        }
        let summary = self.summary();
        let summary = summary.strip_suffix("\nDone.").unwrap_or(&summary);
        let mut bytes = crate::find::bounded_error(summary, limit - FOOTER.len());
        // A truncated summary still needs a line break before the completion marker.
        if !bytes.is_empty() && !bytes.ends_with(b"\r\n") {
            bytes = crate::find::bounded_error(summary, limit.saturating_sub(FOOTER.len() + 2));
            if limit >= FOOTER.len() + 2 {
                bytes.extend_from_slice(b"\r\n");
            }
        }
        bytes.extend_from_slice(FOOTER);
        bytes
    }
    /// Compact response, encoded and bounded by the session owner.
    pub fn summary(&self) -> String {
        format!(
            "Database check: {} findings, {} field repairs, {} relocations, {} list repairs, {} purges.\nDone.",
            self.findings.len(),
            self.plan.fields.len(),
            self.plan.relocations.len(),
            self.plan.list_changes.len(),
            self.plan.purges.len()
        )
    }
}
/// Whether an object is a surviving container.
fn container(w: &World, id: ObjectId) -> bool {
    w.objects.get(&id).is_some_and(|o| {
        matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing) && !o.flags.contains(Flag::Going)
    })
}
/// A destination chain must reach a room without passing through the moved object.
fn safe(w: &World, object: ObjectId, dest: ObjectId) -> bool {
    w.containment_chain(Some(dest)).is_ok_and(|chain| {
        !chain.contains(&object)
            && chain.iter().all(|id| container(w, *id))
            && chain
                .last()
                .is_some_and(|id| w.objects[id].kind == Kind::Room)
    })
}
/// Ordered legacy replacement-home policy with containment safety.
fn home(w: &World, c: &Config, object: ObjectId) -> Result<ObjectId> {
    let o = &w.objects[&object];
    o.location
        .into_iter()
        .chain(o.home)
        .filter(|id| flags::controls(w, object, *id))
        .chain([
            ObjectId(c.mux.default_home),
            ObjectId(c.home()),
            ObjectId(c.start()),
        ])
        .find(|id| safe(w, object, *id))
        .context(format!("no safe replacement home for #{}", object.0))
}
/// Walk the usable prefix of a persisted list; a bad member cannot hide earlier valid members.
fn chain(w: &World, raw: &Links, owner: ObjectId, exits: bool) -> Vec<ObjectId> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut next = raw.get(&owner).map_or(-1, |r| r[usize::from(exits)]);
    while next >= 0 {
        let id = ObjectId(next);
        if !seen.insert(id) {
            break;
        }
        let Some(o) = w.objects.get(&id) else {
            break;
        };
        if if exits {
            o.kind != Kind::Exit
        } else {
            !matches!(o.kind, Kind::Player | Kind::Thing)
        } {
            break;
        }
        result.push(id);
        next = raw.get(&id).map_or(-1, |r| r[2]);
    }
    result
}
/// Rebuild lists, keeping surviving members in their existing order then appending by dbref.
pub fn rebuild_links(w: &World, raw: &Links) -> Links {
    let mut links: Links = w.objects.keys().map(|id| (*id, [-1; 3])).collect();
    for o in w.objects.values() {
        if o.kind == Kind::Exit {
            links.get_mut(&o.id).unwrap()[1] = o.location.map_or(-1, |id| id.0);
        }
        if !matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing) {
            continue;
        }
        for exits in [false, true] {
            let members: BTreeSet<_> = w
                .objects
                .values()
                .filter(|v| {
                    v.location == Some(o.id)
                        && if exits {
                            v.kind == Kind::Exit
                        } else {
                            matches!(v.kind, Kind::Player | Kind::Thing)
                        }
                })
                .map(|v| v.id)
                .collect();
            let mut order = chain(w, raw, o.id, exits);
            order.retain(|id| members.contains(id));
            let retained: BTreeSet<_> = order.iter().copied().collect();
            order.extend(members.difference(&retained).copied());
            links.get_mut(&o.id).unwrap()[usize::from(exits)] = order.first().map_or(-1, |id| id.0);
            for (i, id) in order.iter().enumerate() {
                links.get_mut(id).unwrap()[2] = order.get(i + 1).map_or(-1, |id| id.0);
            }
        }
    }
    links
}
/// Compute semantic repairs without executing callbacks, SQL or session detachments.
pub fn plan(before: &World, raw: &Links, c: &Config) -> Result<(World, DbCheckReport)> {
    before.validate_accounts()?;
    let mut w = before.clone();
    let mut report = DbCheckReport::default();
    ensure!(
        w.objects
            .get(&ObjectId(1))
            .is_some_and(|o| o.kind == Kind::Player),
        "GOD #1 is missing or not a player"
    );
    let protected: BTreeSet<_> = [1, c.start(), c.home(), c.mux.default_home]
        .into_iter()
        .filter(|id| *id >= 0)
        .map(ObjectId)
        .collect();
    for id in &protected {
        let o = w
            .objects
            .get_mut(id)
            .with_context(|| format!("missing protected object #{}; cannot synthesize it", id.0))?;
        ensure!(
            o.kind != Kind::Garbage,
            "protected object #{} is garbage",
            id.0
        );
        if o.flags.remove(Flag::Going) {
            report
                .findings
                .push(format!("Protected #{}: cleared GOING", id.0));
        }
    }
    let ids: Vec<_> = w.objects.keys().copied().collect();
    // Resolve competing chains: the claimed location wins if it actually contains the member.
    let mut claims: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
    for id in &ids {
        if container(&w, *id) {
            for exits in [false, true] {
                for member in chain(&w, raw, *id, exits) {
                    claims.entry(member).or_default().push(*id);
                }
            }
        }
    }
    for id in &ids {
        let o = &w.objects[id];
        if o.kind == Kind::Garbage || o.flags.contains(Flag::Going) {
            continue;
        }
        if matches!(o.kind, Kind::Exit | Kind::Player | Kind::Thing) {
            let location = claims.get(id).and_then(|owners| {
                o.location
                    .filter(|loc| owners.contains(loc))
                    .or_else(|| owners.first().copied())
            });
            if location != o.location {
                w.objects.get_mut(id).unwrap().location = location;
            }
        }
    }
    // Exits with invalid endpoints are doomed. Exit destinations can be any surviving container.
    for id in &ids {
        let o = &w.objects[id];
        if o.kind == Kind::Exit
            && (!o.location.is_some_and(|d| container(&w, d))
                || !o.destination.is_some_and(|d| container(&w, d)))
        {
            w.objects.get_mut(id).unwrap().flags.insert(Flag::Going);
        }
    }
    report.plan.purges = w
        .objects
        .values()
        .filter(|o| o.kind != Kind::Garbage && o.flags.contains(Flag::Going))
        .map(|o| o.id)
        .collect();
    let live: BTreeSet<_> = w
        .objects
        .values()
        .filter(|o| o.kind != Kind::Garbage && !report.plan.purges.contains(&o.id))
        .map(|o| o.id)
        .collect();
    for id in &ids {
        if !live.contains(id) {
            continue;
        }
        let o = w.objects.get_mut(id).unwrap();
        if o.zone.is_some_and(|id| !live.contains(&id)) {
            o.zone = None;
        }
        if o.affiliation.is_some_and(|id| !live.contains(&id)) {
            o.affiliation = None;
        }
        if o.kind == Kind::Room {
            o.location = None;
            o.home = None;
        }
        if o.kind == Kind::Exit {
            o.home = None;
        }
        if o.dropto.is_some_and(|d| !live.contains(&d)) {
            o.dropto = None;
        }
        if matches!(o.kind, Kind::Player | Kind::Thing) {
            if !o.home.is_some_and(|d| safe(&w, *id, d)) {
                let dest = home(&w, c, *id)?;
                w.objects.get_mut(id).unwrap().home = Some(dest);
            }
            if !w.objects[id].location.is_some_and(|d| safe(&w, *id, d)) {
                let dest = home(&w, c, *id)?;
                w.objects.get_mut(id).unwrap().location = Some(dest);
            }
        }
    }
    for id in &report.plan.purges {
        let o = w.objects.get_mut(id).unwrap();
        if o.kind == Kind::Player {
            report.plan.detachments.insert(*id);
        }
        o.kind = Kind::Garbage;
        o.name = "Garbage".into();
        o.flags = [Flag::Going].into_iter().collect();
        o.powers = Default::default();
        o.location = None;
        o.home = None;
        o.destination = None;
        o.dropto = None;
        o.zone = None;
        o.affiliation = None;
        o.description = None;
        o.internal_description = None;
        o.lua_parent.clear();
        o.state.clear();
        w.accounts.remove(id);
        report.findings.push(format!("Purged #{}", id.0));
    }
    w.macros.purge(&report.plan.purges);
    w.channel_aliases.retain(|id, _| live.contains(id));
    w.last_pages.retain(|id, _| live.contains(id));
    for recipients in w.last_pages.values_mut() {
        recipients.retain(|id| live.contains(id));
    }
    for ch in w.channels.values_mut() {
        ch.users.retain(|u| live.contains(&u.who));
        if ch.object.is_some_and(|id| !live.contains(&id)) {
            ch.object = None;
        }
    }
    for id in &ids {
        let old = &before.objects[id];
        let new = &w.objects[id];
        for (field, changed) in [
            ("location", old.location != new.location),
            ("home", old.home != new.home),
            ("zone", old.zone != new.zone),
            ("affiliation", old.affiliation != new.affiliation),
            ("dropto", old.dropto != new.dropto),
            ("flags", old.flags != new.flags),
        ] {
            if changed {
                report.plan.fields.push(FieldRepair { object: *id, field });
                report.findings.push(format!("Repaired #{}, {field}", id.0));
            }
        }
        if matches!(new.kind, Kind::Player | Kind::Thing) && old.location != new.location {
            report.plan.relocations.push(Relocation {
                object: *id,
                source: old.location,
                destination: new.location.context("repaired occupant has no location")?,
            });
        }
        if flags::is_wizard(&w, *id) {
            report.findings.push(format!("Wizard object #{}", id.0));
        }
        if !flags::is_wizard(&w, *id) && new.location.is_some_and(|loc| flags::is_wizard(&w, loc)) {
            report
                .findings
                .push(format!("Non-Wizard #{} inside Wizard container", id.0));
        }
    }
    let mut reachable = BTreeSet::from([ObjectId(c.start())]);
    loop {
        let old = reachable.len();
        for o in w.objects.values().filter(|o| o.kind == Kind::Exit) {
            if o.location.is_some_and(|id| reachable.contains(&id))
                && let Some(dest) = o.destination
            {
                reachable.insert(dest);
            }
        }
        if old == reachable.len() {
            break;
        }
    }
    for o in w.objects.values().filter(|o| {
        o.kind == Kind::Room && !o.flags.contains(Flag::Floating) && !reachable.contains(&o.id)
    }) {
        report
            .findings
            .push(format!("Unreachable room #{} lacks FLOATING", o.id.0));
    }
    report.plan.links = rebuild_links(&w, raw);
    report.plan.list_changes = report
        .plan
        .links
        .iter()
        .filter(|(id, links)| raw.get(id) != Some(*links))
        .map(|(id, _)| *id)
        .collect();
    for id in &report.plan.list_changes {
        report
            .findings
            .push(format!("Rebuilt list slots at #{}", id.0));
    }
    w.validate(c)?;
    Ok((w, report))
}
