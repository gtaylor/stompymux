//! Shared administrative target resolution.
use crate::world::{Kind, ObjectId};
use anyhow::{Context, Result};
/// Resolve explicit identities and exact visible local names without guessing.
pub(crate) fn admin_target(
    w: &crate::world::World,
    player: ObjectId,
    name: &str,
) -> Result<ObjectId> {
    use anyhow::{bail, ensure};
    let name = name.trim();
    let room = w.objects.get(&player).and_then(|o| o.location);
    let explicit = if name.eq_ignore_ascii_case("me") {
        Some(player)
    } else if name.eq_ignore_ascii_case("here") {
        room
    } else if let Some(n) = name.strip_prefix('#') {
        Some(ObjectId(n.parse().context("Invalid dbref.")?))
    } else {
        None
    };
    if let Some(id) = explicit {
        ensure!(
            w.objects
                .get(&id)
                .is_some_and(|o| o.kind != Kind::Garbage && w.visible(o, player)),
            "No such object."
        );
        return Ok(id);
    }
    let matches: Vec<_> = w
        .objects
        .values()
        .filter(|o| {
            o.kind != Kind::Garbage
                && w.visible(o, player)
                && (o.id == player
                    || Some(o.id) == room
                    || o.location == room
                    || o.location == Some(player))
                && if o.kind == Kind::Exit {
                    crate::text::plain_with(&w.palette, &o.name)
                        .split(';')
                        .any(|n| n.eq_ignore_ascii_case(name))
                } else {
                    crate::text::plain_with(&w.palette, &o.name).eq_ignore_ascii_case(name)
                }
        })
        .map(|o| o.id)
        .collect();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => bail!("No such object."),
        _ => bail!("I don't know which object you mean."),
    }
}
