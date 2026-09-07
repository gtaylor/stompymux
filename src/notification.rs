//! Bounded C-compatible notification traversal, independent of sockets and rendering.
use crate::{
    config::Config,
    flags::Flag,
    lua::Outbox,
    text::Document,
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, ensure};

/// Routing directives model only branches implemented by this fork's C notifier.
#[derive(Clone, Copy)]
pub struct Policy(u16);
const ME: u16 = 1;
const INV: u16 = 2;
const EXITS: u16 = 4;
const NBR_A: u16 = 8;
const LOC_A: u16 = 16;
const NEIGHBOR_EXITS_A: u16 = 32;
const INSIDE: u16 = 64;
impl Policy {
    pub const DIRECT: Self = Self(ME | EXITS);
    pub const ROOM: Self = Self(ME | EXITS | INV | NBR_A | LOC_A | NEIGHBOR_EXITS_A | INSIDE);
    const ROOM_EXCLUDED: Self = Self(ME | EXITS | NBR_A | LOC_A | NEIGHBOR_EXITS_A | INSIDE);
}
/// One logical message; exclusions apply at the initial location and contents, as in C.
pub struct Request {
    pub target: ObjectId,
    pub sender: ObjectId,
    pub document: Document,
    pub policy: Policy,
    pub exclusions: Option<Vec<ObjectId>>,
}
struct Visit {
    target: ObjectId,
    document: Document,
    policy: Policy,
    depth: usize,
}

/// Traverse into a temporary batch: resource failure cannot leave partial output even under Lua pcall.
pub fn send(world: &World, outbox: &Outbox, config: &Config, request: Request) -> Result<()> {
    ensure!(
        !request.document.source().contains('\0'),
        "message contains an embedded NUL byte"
    );
    let mut stack = Vec::new();
    if let Some(exclusions) = request.exclusions {
        for o in world.objects.values().filter(|o| {
            o.location == Some(request.target) && !matches!(o.kind, Kind::Garbage | Kind::Exit)
        }) {
            if !exclusions.contains(&o.id) {
                ensure!(
                    stack.len() < config.lua.output_entry_limit
                        && stack
                            .iter()
                            .map(|v: &Visit| v.document.len())
                            .sum::<usize>()
                            .saturating_add(request.document.len())
                            <= config.lua.output_byte_limit,
                    "Notification traversal limit exceeded"
                );
                stack.push(Visit {
                    target: o.id,
                    document: request.document.clone(),
                    policy: Policy(ME),
                    depth: 1,
                });
            }
        }
        if !exclusions.contains(&request.target) {
            stack.insert(
                0,
                Visit {
                    target: request.target,
                    document: request.document,
                    policy: Policy::ROOM_EXCLUDED,
                    depth: 1,
                },
            );
        }
        stack.reverse();
    } else {
        stack.push(Visit {
            target: request.target,
            document: request.document,
            policy: request.policy,
            depth: 1,
        });
    }
    let existing = outbox.borrow();
    let mut bytes = existing.iter().map(|(_, d)| d.len()).sum::<usize>();
    let existing_count = existing.len();
    drop(existing);
    let mut staged = Vec::new();
    let mut visits = 0usize;
    while let Some(v) = stack.pop() {
        if v.document.is_empty() || v.depth as i64 >= config.mux.notify_recursion_limit {
            continue;
        }
        let Some(o) = world
            .objects
            .get(&v.target)
            .filter(|o| !matches!(o.kind, Kind::Garbage | Kind::Exit))
        else {
            continue;
        };
        visits += 1;
        ensure!(
            visits <= config.lua.output_entry_limit,
            "Notification traversal limit exceeded"
        );
        ensure!(
            v.document.len() <= config.runtime.output_message_limit,
            "Lua output limit exceeded"
        );
        let key = v.policy.0;
        let audible = o.flags.contains(Flag::Audible);
        if key & ME != 0 && o.kind == Kind::Player {
            bytes = bytes.saturating_add(v.document.len());
            ensure!(
                staged.len() + existing_count < config.lua.output_entry_limit
                    && bytes <= config.lua.output_byte_limit,
                "Lua output limit exceeded"
            );
            staged.push((o.id, v.document.clone()));
        }
        let mut next = Vec::new();
        let mut queued_bytes = stack.iter().map(|v| v.document.len()).sum::<usize>();
        let mut add = |target, document: Document, policy| -> Result<()> {
            queued_bytes = queued_bytes.saturating_add(document.len());
            ensure!(
                next.len() + stack.len() < config.lua.output_entry_limit
                    && queued_bytes <= config.lua.output_byte_limit,
                "Notification traversal limit exceeded"
            );
            next.push(Visit {
                target,
                document,
                policy: Policy(policy),
                depth: v.depth + 1,
            });
            Ok(())
        };
        if key & EXITS != 0 {
            for exit in world.objects.values().filter(|e| {
                e.kind == Kind::Exit && e.location == Some(o.id) && e.flags.contains(Flag::Audible)
            }) {
                if let Some(to) = exit.destination.filter(|to| *to != o.id) {
                    add(
                        to,
                        v.document.prefixed("From a distance, "),
                        ME | NBR_A | LOC_A | INV | INSIDE,
                    )?;
                }
            }
        }
        let forwarded = if key & INSIDE != 0 {
            v.document.prefixed(&format!("From {}, ", o.name))
        } else {
            v.document.clone()
        };
        if let Some(location) = o.location
            && key & NEIGHBOR_EXITS_A != 0
            && audible
        {
            for exit in world.objects.values().filter(|e| {
                e.kind == Kind::Exit
                    && e.location == Some(location)
                    && e.flags.contains(Flag::Audible)
            }) {
                if let Some(to) = exit.destination.filter(|to| *to != location && *to != o.id) {
                    add(
                        to,
                        forwarded.prefixed("From a distance, "),
                        ME | NBR_A | LOC_A | INV | INSIDE,
                    )?;
                }
            }
        }
        if key & INV != 0 {
            for child in world.objects.values().filter(|c| {
                c.location == Some(o.id)
                    && c.id != o.id
                    && !matches!(c.kind, Kind::Garbage | Kind::Exit)
            }) {
                add(child.id, v.document.clone(), ME)?;
            }
        }
        if let Some(location) = o.location {
            if key & NBR_A != 0 && audible {
                for neighbor in world.objects.values().filter(|n| {
                    n.location == Some(location)
                        && n.id != o.id
                        && n.id != location
                        && !matches!(n.kind, Kind::Garbage | Kind::Exit)
                }) {
                    add(neighbor.id, forwarded.clone(), ME)?;
                }
            }
            if key & LOC_A != 0 && audible {
                add(location, forwarded, ME | NBR_A | LOC_A | INSIDE)?;
            }
        }
        stack.extend(next.into_iter().rev());
        ensure!(
            stack.len() <= config.lua.output_entry_limit,
            "Notification traversal limit exceeded"
        );
    }
    outbox.borrow_mut().extend(staged);
    Ok(())
}

/// Broadcast delivery bypasses routing but uses the same staging budgets.
pub fn direct(outbox: &Outbox, config: &Config, id: ObjectId, document: Document) -> Result<()> {
    let mut out = outbox.borrow_mut();
    ensure!(
        document.len() <= config.runtime.output_message_limit
            && out.len() < config.lua.output_entry_limit
            && out
                .iter()
                .map(|(_, d)| d.len())
                .sum::<usize>()
                .saturating_add(document.len())
                <= config.lua.output_byte_limit,
        "Lua output limit exceeded"
    );
    out.push((id, document));
    Ok(())
}
