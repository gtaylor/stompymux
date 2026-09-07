//! Bounded, runtime-only command lists with monotonic deadlines and fair executor rotation.
use super::{Action, CommandContext, CommandInput, ExecutionContext, InputOrigin};
use crate::{
    flags::{self, Flag},
    state::Generation,
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, VecDeque};
use tokio::time::{Duration, Instant};

/// Hard ceiling for retained command text across ready and delayed entries.
pub const MAX_QUEUE_BYTES: usize = 16 * 1024 * 1024;

/// Transactional admission or cancellation, interpreted by the world owner.
#[derive(Clone, Debug)]
pub enum Request {
    /// Admit a list using the executor's own capacity and authority.
    Add {
        /// Object that will execute each command.
        executor: ObjectId,
        /// Actor responsible for the queued work.
        cause: ObjectId,
        /// Signed seconds; nonpositive values are immediately ready.
        seconds: i32,
        /// Unexpanded literal command list.
        text: String,
    },
    /// Cancel one executor, or all executors when the target is absent.
    Halt {
        /// Exact executing object; ownership does not broaden cancellation.
        target: Option<ObjectId>,
    },
}

/// Convert command syntax/policy failures to private replies without mutating the world.
fn request(work: impl FnOnce() -> Result<Request>) -> Result<Action> {
    Ok(match work() {
        Ok(r) => Action::Queue(r),
        Err(e) => Action::Reply(e.to_string()),
    })
}

/// Force a controlled object; never execute it recursively on the caller's stack.
pub fn force(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    request(|| {
        let (target, text) = input
            .args
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Usage: @force <target>=<commands>"))?;
        let w = ctx.scripts.world.borrow();
        let executor = super::target::builder_target(&w, ctx.player, target)?;
        ensure!(
            flags::controls(&w, ctx.player, executor),
            "Permission denied."
        );
        Ok(Request::Add {
            executor,
            cause: ctx.player,
            seconds: 0,
            text: text.into(),
        })
    })
}

/// Queue a literal list after an integer number of seconds; nonpositive waits are ready immediately.
pub fn wait(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    request(|| {
        let (delay, text) = input
            .args
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Usage: @wait <seconds>=<commands>"))?;
        let seconds = delay
            .trim()
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Wait time must be a number."))?;
        let text = text.trim();
        let text = text
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(text);
        Ok(Request::Add {
            executor: ctx.player,
            cause: ctx.cause,
            seconds,
            text: text.into(),
        })
    })
}

/// Cancel ready and delayed entries, including unexecuted portions of a list.
pub fn halt(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    request(|| {
        if let Some(switch) = &input.switch {
            ensure!(
                !switch.is_empty() && "all".starts_with(switch),
                "Unsupported command switch."
            );
            ensure!(
                input.args.trim().is_empty(),
                "Can't specify a target and /all"
            );
            return Ok(Request::Halt { target: None });
        }
        let target = if input.args.trim().is_empty() {
            ctx.player
        } else {
            super::target::builder_target(&ctx.scripts.world.borrow(), ctx.player, &input.args)?
        };
        Ok(Request::Halt {
            target: Some(target),
        })
    })
}

/// An admitted command list; incarnation prevents execution against replacement identities.
#[derive(Clone)]
struct Entry {
    executor: ObjectId,
    generation: Generation,
    cause: ObjectId,
    due: Instant,
    text: String,
}

/// A single attempted command, consumed even if callbacks or persistence fail.
pub struct Work {
    /// Captured identities, with no connection assigned.
    pub execution: ExecutionContext,
    /// One literal command extracted from the admitted list.
    pub text: String,
}

/// Runtime queue; clone a candidate when preparing transactional additions/cancellations.
#[derive(Clone, Default)]
pub struct Queue {
    entries: BTreeMap<u64, Entry>,
    rotation: VecDeque<ObjectId>,
    next: u64,
    bytes: usize,
}

impl Queue {
    /// Apply to a candidate queue/world; the caller publishes only after persistence succeeds.
    pub fn apply(
        &mut self,
        request: Request,
        world: &mut World,
        limit: usize,
        line_limit: usize,
        now: Instant,
    ) -> Result<Option<String>> {
        match request {
            Request::Halt { target } => {
                let n = self.cancel(target);
                Ok(Some(format!("{n} queue entries removed.")))
            }
            Request::Add {
                executor,
                cause,
                seconds,
                text,
            } => {
                ensure!(
                    text.len() <= line_limit,
                    "Queued command exceeds the input limit."
                );
                let object = world
                    .objects
                    .get(&executor)
                    .ok_or_else(|| anyhow::anyhow!("No such object."))?;
                ensure!(
                    object.kind != Kind::Garbage && !object.flags.contains(Flag::Going),
                    "Object is being destroyed."
                );
                if object.flags.contains(Flag::Halted) {
                    return Ok(None);
                }
                if self
                    .entries
                    .values()
                    .filter(|e| e.executor == executor)
                    .count()
                    >= limit
                {
                    self.cancel(Some(executor));
                    world
                        .objects
                        .get_mut(&executor)
                        .unwrap()
                        .flags
                        .insert(Flag::Halted);
                    return Ok(Some(
                        "Run away objects: too many commands queued.  Halted.".into(),
                    ));
                }
                ensure!(
                    self.bytes.saturating_add(text.len()) <= MAX_QUEUE_BYTES,
                    "Command queue text limit exceeded."
                );
                let due = now
                    .checked_add(Duration::from_secs(seconds.max(0) as u64))
                    .ok_or_else(|| anyhow::anyhow!("Wait time is out of range."))?;
                let sequence = self
                    .next
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("Command queue sequence exhausted."))?;
                self.next = sequence;
                self.bytes += text.len();
                self.entries.insert(
                    sequence,
                    Entry {
                        executor,
                        generation: object.generation,
                        cause,
                        due,
                        text,
                    },
                );
                Ok(None)
            }
        }
    }

    /// Cancel a single executor or all executors. Counts lists rather than individual commands.
    pub fn cancel(&mut self, target: Option<ObjectId>) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| target.is_some_and(|t| e.executor != t));
        self.recount();
        before - self.entries.len()
    }

    /// Remove dead identities after successful purge; flags are rechecked when a list is ready.
    pub fn reconcile(&mut self, world: &World) {
        self.entries.retain(|_, e| {
            world
                .objects
                .get(&e.executor)
                .is_some_and(|o| o.generation == e.generation && o.kind != Kind::Garbage)
        });
        self.recount();
    }

    fn recount(&mut self) {
        self.bytes = self.entries.values().map(|e| e.text.len()).sum();
        self.rotation
            .retain(|p| self.entries.values().any(|e| e.executor == *p));
    }

    /// Earliest ready/delayed list, used by the world owner's monotonic timer.
    pub fn deadline(&self) -> Option<Instant> {
        self.entries.values().map(|e| e.due).min()
    }

    /// Next delayed wakeup, including when other lists are already ready but lack processing credit.
    pub fn next_wakeup(&self, now: Instant) -> Option<Instant> {
        self.entries
            .values()
            .map(|e| e.due)
            .filter(|due| *due > now)
            .min()
    }

    /// Check readiness without consuming a list or rotating executors.
    pub fn ready(&self, now: Instant) -> bool {
        self.deadline().is_some_and(|due| due <= now)
    }

    /// Consume one command, retaining its list position across executor rotation.
    pub fn take(&mut self, now: Instant, world: &World, compress: bool) -> Option<Work> {
        self.reconcile(world);
        let mut ready: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.due <= now)
            .map(|(id, e)| (e.due, *id, e.executor))
            .collect();
        ready.sort();
        for (_, _, p) in &ready {
            if !self.rotation.contains(p) {
                self.rotation.push_back(*p);
            }
        }
        self.rotation
            .retain(|p| ready.iter().any(|(_, _, q)| q == p));
        let p = self.rotation.pop_front()?;
        let id = ready.iter().find(|(_, _, q)| *q == p)?.1;
        let object = &world.objects[&p];
        if object.flags.contains(Flag::Going) || object.flags.contains(Flag::Halted) {
            self.entries.remove(&id);
            self.recount();
            return None;
        }
        let e = self.entries.get_mut(&id).unwrap();
        let (text, rest) = split_command(&e.text, compress);
        let execution = ExecutionContext {
            executor: e.executor,
            cause: e.cause,
            session: None,
            origin: InputOrigin::Queued,
        };
        if let Some(rest) = rest {
            e.text = rest;
        } else {
            self.entries.remove(&id);
        }
        if self
            .entries
            .values()
            .any(|e| e.executor == p && e.due <= now)
        {
            self.rotation.push_back(p);
        }
        self.recount();
        Some(Work { execution, text })
    }
}

/// C parse_to-style literal delimiter handling; escapes remain literal and nesting protects semicolons.
pub fn split_command(source: &str, compress: bool) -> (String, Option<String>) {
    let mut braces = 0usize;
    let mut stack = Vec::new();
    let mut escaped = false;
    let mut end = source.len();
    for (i, c) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        match c {
            '{' => braces += 1,
            '}' if braces > 0 => braces -= 1,
            '(' | '[' if braces == 0 && stack.len() < 32 => {
                stack.push(if c == '(' { ')' } else { ']' })
            }
            ')' | ']' if braces == 0 && stack.last() == Some(&c) => {
                stack.pop();
            }
            ';' if braces == 0 && stack.is_empty() => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    let input = &source[..end];
    let text = if compress {
        let mut output = String::new();
        let mut escaped = false;
        for c in input.trim().chars() {
            if !escaped && c == ' ' && output.ends_with(' ') {
                continue;
            }
            output.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            }
        }
        output
    } else {
        input.into()
    };
    (text, (end < source.len()).then(|| source[end + 1..].into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn world() -> World {
        let c = Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        let mut w = World {
            next_id: 1,
            ..Default::default()
        };
        w.create(&c, "Player".into(), Kind::Player);
        w.create(&c, "Thing".into(), Kind::Thing);
        w
    }

    fn add(q: &mut Queue, w: &mut World, who: i64, delay: i32, text: &str, now: Instant) {
        assert!(
            q.apply(
                Request::Add {
                    executor: ObjectId(who),
                    cause: ObjectId(1),
                    seconds: delay,
                    text: text.into()
                },
                w,
                100,
                8192,
                now
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn literal_c_parser_fixtures() {
        // command_parser.c parse_to: brackets, braces, parentheses and escapes protect separators.
        for (source, first, rest) in [
            ("say one;say two", "say one", Some("say two")),
            ("say {a;b};say c", "say {a;b}", Some("say c")),
            ("say [a;(b;c)];say d", "say [a;(b;c)]", Some("say d")),
            (r"say a\;b;say c", r"say a\;b", Some("say c")),
            ("say {unfinished;tail", "say {unfinished;tail", None),
            (";", "", Some("")),
            ("say x;", "say x", Some("")),
            ("  say   hi  ", "say hi", None),
        ] {
            let result = split_command(source, true);
            assert_eq!(result, (first.into(), rest.map(str::to_owned)), "{source}");
        }
        assert_eq!(
            split_command("  say   hi  ;x", false),
            ("  say   hi  ".into(), Some("x".into()))
        );
    }

    #[test]
    fn monotonic_timing_fifo_and_executor_fairness() {
        let mut w = world();
        let mut q = Queue::default();
        let now = Instant::now();
        add(&mut q, &mut w, 1, 2, "later", now);
        add(&mut q, &mut w, 1, 0, "first;second", now);
        add(&mut q, &mut w, 2, -2, "other;again", now);
        add(&mut q, &mut w, 1, 0, "third", now);
        for expected in ["first", "other", "second", "again", "third"] {
            let work = q.take(now, &w, true).unwrap();
            assert_eq!(work.text, expected);
            assert_eq!(work.execution.session, None);
            assert_eq!(work.execution.cause, ObjectId(1));
        }
        assert!(!q.ready(now));
        assert!(q.take(now + Duration::from_secs(1), &w, true).is_none());
        assert_eq!(
            q.take(now + Duration::from_secs(2), &w, true).unwrap().text,
            "later"
        );
        assert!(q.entries.is_empty());
        assert_eq!(q.bytes, 0);
    }

    #[test]
    fn halt_cancels_all_ready_delayed_and_partial_lists() {
        let mut w = world();
        let mut q = Queue::default();
        let now = Instant::now();
        add(&mut q, &mut w, 1, 0, "a;b", now);
        add(&mut q, &mut w, 2, 0, "c", now);
        add(&mut q, &mut w, 1, 30, "d", now);
        assert_eq!(q.take(now, &w, true).unwrap().text, "a");
        assert_eq!(q.cancel(None), 3);
        assert!(q.take(now + Duration::from_secs(60), &w, true).is_none());
        assert_eq!(q.bytes, 0);
    }

    #[test]
    fn nonplayers_have_capacity_and_overflow_halts_only_their_queue() {
        let mut w = world();
        let mut q = Queue::default();
        let now = Instant::now();
        add(&mut q, &mut w, 1, 0, "safe", now);
        add(&mut q, &mut w, 2, 0, "thing", now);
        let result = q
            .apply(
                Request::Add {
                    executor: ObjectId(2),
                    cause: ObjectId(1),
                    seconds: 0,
                    text: "excess".into(),
                },
                &mut w,
                1,
                8192,
                now,
            )
            .unwrap();
        assert!(result.unwrap().contains("Halted"));
        assert!(w.objects[&ObjectId(2)].flags.contains(Flag::Halted));
        assert_eq!(q.take(now, &w, true).unwrap().text, "safe");
        assert!(!q.ready(now));
        w.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Halted);
        assert!(
            q.apply(
                Request::Add {
                    executor: ObjectId(2),
                    cause: ObjectId(1),
                    seconds: 0,
                    text: "zero".into()
                },
                &mut w,
                0,
                8192,
                now
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    fn stale_going_halted_and_purged_executors_never_run() {
        for mode in 0..4 {
            let mut w = world();
            let mut q = Queue::default();
            let now = Instant::now();
            add(&mut q, &mut w, 2, 0, "never", now);
            let o = w.objects.get_mut(&ObjectId(2)).unwrap();
            match mode {
                0 => o.generation = Default::default(),
                1 => {
                    o.flags.insert(Flag::Going);
                }
                2 => {
                    o.flags.insert(Flag::Halted);
                }
                _ => o.kind = Kind::Garbage,
            }
            assert!(q.take(now, &w, true).is_none());
            assert_eq!(q.bytes, 0);
        }
    }

    #[test]
    fn input_and_global_byte_caps_reject_without_halting() {
        let mut w = world();
        let mut q = Queue::default();
        let now = Instant::now();
        let request = || Request::Add {
            executor: ObjectId(2),
            cause: ObjectId(1),
            seconds: 0,
            text: "abc".into(),
        };
        assert!(q.apply(request(), &mut w, 100, 2, now).is_err());
        q.apply(
            Request::Add {
                executor: ObjectId(2),
                cause: ObjectId(1),
                seconds: 0,
                text: "x".repeat(MAX_QUEUE_BYTES - 2),
            },
            &mut w,
            100,
            MAX_QUEUE_BYTES,
            now,
        )
        .unwrap();
        assert!(
            q.apply(request(), &mut w, 100, 8192, now)
                .unwrap_err()
                .to_string()
                .contains("text limit")
        );
        assert!(!w.objects[&ObjectId(2)].flags.contains(Flag::Halted));
        assert_eq!(
            q.take(now, &w, false).unwrap().text.len(),
            MAX_QUEUE_BYTES - 2
        );
        assert_eq!(q.bytes, 0);
    }
    #[test]
    fn flags_are_checked_when_delayed_work_becomes_ready() {
        let mut w = world();
        let mut q = Queue::default();
        let now = Instant::now();
        add(&mut q, &mut w, 2, 60, "later", now);
        add(&mut q, &mut w, 1, 0, "ready", now);
        assert_eq!(q.next_wakeup(now), Some(now + Duration::from_secs(60)));
        w.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Halted);
        assert_eq!(q.take(now, &w, true).unwrap().text, "ready");
        w.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Halted);
        assert_eq!(
            q.take(now + Duration::from_secs(60), &w, true)
                .unwrap()
                .text,
            "later"
        );
    }
}
