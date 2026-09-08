//! Shared authorization and state change for deferred object destruction.
use crate::{
    config::Config,
    flags::Flag,
    world::{Kind, ObjectId, World},
};
use anyhow::{Context, Result, ensure};
/// Foundational identities cannot be destroyed through commands or trusted Lua.
pub(crate) fn protected(c: &Config, id: ObjectId) -> bool {
    [0, 1, c.start(), c.home(), c.mux.default_home].contains(&id.0)
}
/// Schedule without emitting text; callers choose native-command or silent Lua output.
pub(crate) fn schedule(
    w: &mut World,
    c: &Config,
    actor: ObjectId,
    id: ObjectId,
    override_safe: bool,
) -> Result<Kind> {
    ensure!(
        crate::authority::controls(w, actor, id),
        "Permission denied."
    );
    let o = w
        .objects
        .get(&id)
        .filter(|o| o.kind != Kind::Garbage)
        .context("object does not exist")?;
    ensure!(
        !o.flags.contains(Flag::Safe) || override_safe,
        "Sorry, that object is protected. Use @destroy/override to destroy it."
    );
    ensure!(!protected(c, id), "You can't destroy that!");
    let noun = match o.kind {
        Kind::Room => "room",
        Kind::Exit => "exit",
        Kind::Player => "player",
        _ => "object",
    };
    ensure!(
        !o.flags.contains(Flag::Going),
        "No sense beating a dead {noun}."
    );
    ensure!(
        o.kind != Kind::Player || !o.flags.contains(Flag::Wizard),
        "You may not destroy Wizards!"
    );
    let kind = o.kind;
    let object = w.objects.get_mut(&id).unwrap();
    object.flags.insert(Flag::Going);
    if kind == Kind::Player {
        object.pending_destroyer = Some(actor);
    }
    Ok(kind)
}
