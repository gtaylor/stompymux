//! Clock-relative storage for the whole-number values in a saved JSON record.
//!
//! Unit and vehicle records hold many values that count once per simulation second:
//! weapon recycle and stun countdowns, lock settling, overheat and stagger clocks, and so
//! on. Saving them as plain numbers would rewrite the record every tick. Instead, a value
//! that is counting in step with the simulation clock is stored as a clock form: the
//! second it reaches zero when counting down, the second it was zero when counting up,
//! or, for a count that wraps around, the second it was last zero and its cycle length.
//! Its place in the stored JSON holds a fixed placeholder, so the JSON text stays the same
//! while the count runs, and loading rebuilds the value from the form and the saved clock.
//!
//! Forms are chosen by observation rather than by per-field rules. A value that changed by
//! exactly one per elapsed second since the previous save gets a form, and a counting-up
//! value that falls back while its count runs on is taken to wrap; a stored form is kept
//! while it still decodes to the current value; anything else is stored as a plain
//! number. A count that pauses, stops or is changed by an event therefore costs a write,
//! and a count that runs costs none. Every choice decodes exactly, so a wrong guess can
//! only cost a write, never correctness.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The value stored in place of a number that is kept as a clock form.
const PLACEHOLDER: i64 = 0;

/// Clock forms keyed by the JSON pointer of the value they replace.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Clocks {
    /// Values counting down, stored as the simulation second they reach zero.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    down: BTreeMap<String, i64>,
    /// Values counting up, stored as the simulation second they were zero.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    up: BTreeMap<String, i64>,
    /// Values counting up and wrapping, stored as a second they were zero and the cycle
    /// length.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    cycle: BTreeMap<String, (i64, i64)>,
}

impl Clocks {
    /// Decode stored forms, where an empty column means none.
    pub(super) fn parse(text: &str) -> Result<Self> {
        if text.is_empty() {
            return Ok(Self::default());
        }
        let clocks: Self = serde_json::from_str(text).context("decoding saved clock forms")?;
        anyhow::ensure!(
            clocks.cycle.values().all(|&(_, length)| length > 0),
            "saved clock cycle has no length"
        );
        Ok(clocks)
    }

    /// Encode the forms for storage.
    pub(super) fn encode(&self) -> String {
        serde_json::to_string(self).expect("clock forms always serialize")
    }

    /// Whether no value is kept as a clock form.
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.down.is_empty() && self.up.is_empty() && self.cycle.is_empty()
    }

    /// Pointers of every value kept as a clock form.
    pub(super) fn paths(&self) -> impl Iterator<Item = &str> {
        self.down
            .keys()
            .chain(self.up.keys())
            .chain(self.cycle.keys())
            .map(String::as_str)
    }

    /// The value a stored form stands for at simulation second `now`.
    fn value_at(&self, path: &str, now: i64) -> Option<i64> {
        if let Some(&zero_at) = self.down.get(path) {
            return Some(zero_at - now);
        }
        if let Some(&zero_at) = self.up.get(path) {
            return Some(now - zero_at);
        }
        self.cycle
            .get(path)
            .map(|&(zero_at, length)| (now - zero_at).rem_euclid(length))
    }

    /// Copy the form stored for `path` in `from`, if any.
    fn keep(&mut self, from: &Clocks, path: String) {
        if let Some(&zero_at) = from.down.get(&path) {
            self.down.insert(path, zero_at);
        } else if let Some(&zero_at) = from.up.get(&path) {
            self.up.insert(path, zero_at);
        } else if let Some(&form) = from.cycle.get(&path) {
            self.cycle.insert(path, form);
        }
    }

    /// Put each form's value at `now` back into a record read from storage.
    pub(super) fn restore(&self, record: &mut Value, now: i64) -> Result<()> {
        for path in self.paths() {
            let value = self.value_at(path, now).expect("path has a form");
            let slot = record
                .pointer_mut(path)
                .with_context(|| format!("saved clock form {path} has no value"))?;
            *slot = value.into();
        }
        Ok(())
    }

    /// Replace every value kept as a form with the placeholder, giving the stored JSON.
    pub(super) fn blank(&self, part: &mut Value) {
        for path in self.paths() {
            if let Some(slot) = part.pointer_mut(path) {
                *slot = PLACEHOLDER.into();
            }
        }
    }

    /// Choose forms for the values of one part of a record saved at second `now`.
    ///
    /// `stored` holds the forms currently in the database, `before` the same part as
    /// last saved at second `then`, and `after` the part being saved. Forms for paths
    /// outside this part, as decided by `in_part`, are carried over unchanged. Values
    /// for which `may_count` is false are always stored as plain numbers.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_part(
        &mut self,
        stored: &Clocks,
        in_part: impl Fn(&str) -> bool,
        may_count: impl Fn(&str) -> bool,
        before: Option<&Value>,
        after: &Value,
        then: i64,
        now: i64,
    ) {
        self.down.retain(|path, _| !in_part(path));
        self.up.retain(|path, _| !in_part(path));
        self.cycle.retain(|path, _| !in_part(path));
        let elapsed = now - then;
        let mut values = Vec::new();
        integers(after, &mut String::new(), &mut values);
        for (path, value) in values {
            if !may_count(&path) {
                continue;
            }
            if stored.value_at(&path, now) == Some(value) {
                self.keep(stored, path);
                continue;
            }
            if elapsed <= 0 {
                continue;
            }
            let Some(previous) = before
                .and_then(|before| before.pointer(&path))
                .and_then(Value::as_i64)
            else {
                continue;
            };
            if previous - value == elapsed {
                self.down.insert(path, now + value);
            } else if value - previous == elapsed {
                self.up.insert(path, now - value);
            } else if (stored.up.contains_key(&path) || stored.cycle.contains_key(&path))
                && value < previous
            {
                // A running count that fell back wrapped; its cycle length is how far it
                // would have reached.
                let length = previous + elapsed - value;
                if length > previous {
                    self.cycle.insert(path, (now - value, length));
                }
            }
        }
    }
}

/// Collect every whole-number value in `value` with its JSON pointer.
fn integers(value: &Value, path: &mut String, found: &mut Vec<(String, i64)>) {
    match value {
        Value::Number(number) => {
            if let Some(number) = number.as_i64().filter(|_| !number.is_f64()) {
                found.push((path.clone(), number));
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let length = path.len();
                path.push('/');
                path.push_str(&index.to_string());
                integers(item, path, found);
                path.truncate(length);
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                let length = path.len();
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                integers(item, path, found);
                path.truncate(length);
            }
        }
        _ => {}
    }
}

/// The record field a pointer starts in.
pub(super) fn field(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default()
        .replace("~1", "/")
        .replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Choose forms for a whole record and return them with the stored JSON.
    fn save(
        stored: &Clocks,
        before: Option<&Value>,
        after: &Value,
        then: i64,
        now: i64,
    ) -> (Clocks, Value) {
        let mut clocks = stored.clone();
        clocks.update_part(stored, |_| true, |_| true, before, after, then, now);
        let mut text = after.clone();
        clocks.blank(&mut text);
        (clocks, text)
    }

    /// A running countdown keeps its stored JSON and form, and restores exactly.
    #[test]
    fn running_counts_are_stored_once() {
        let first = json!({"recycle": {"3": 10}, "phase": 4, "name": "Atlas"});
        let (clocks, _) = save(&Clocks::default(), None, &first, 100, 100);
        assert!(clocks.is_empty());

        let second = json!({"recycle": {"3": 9}, "phase": 5, "name": "Atlas"});
        let (clocks, stored) = save(&clocks, Some(&first), &second, 100, 101);
        assert_eq!(clocks.down["/recycle/3"], 110);
        assert_eq!(clocks.up["/phase"], 96);

        let third = json!({"recycle": {"3": 8}, "phase": 6, "name": "Atlas"});
        let (next, again) = save(&clocks, Some(&second), &third, 101, 102);
        assert_eq!(
            (next.clone(), again.clone()),
            (clocks.clone(), stored.clone())
        );

        let mut loaded = again;
        next.restore(&mut loaded, 102).unwrap();
        assert_eq!(loaded, third);
    }

    /// A counting-up value that wraps is stored as a cycle, and later wraps cost nothing.
    #[test]
    fn wrapping_counts_become_cycles() {
        let mut clocks = Clocks::default();
        clocks.up.insert("/phase".into(), 0);
        let (clocks, _) = save(
            &clocks,
            Some(&json!({"phase": 29})),
            &json!({"phase": 0}),
            29,
            30,
        );
        assert_eq!(clocks.cycle["/phase"], (30, 30));
        for (second, phase) in [(31, 1), (59, 29), (60, 0), (61, 1)] {
            assert_eq!(clocks.value_at("/phase", second), Some(phase));
        }
        // A plain value reset to zero is not taken for a wrap.
        let (plain, _) = save(
            &Clocks::default(),
            Some(&json!({"count": 7})),
            &json!({"count": 0}),
            1,
            2,
        );
        assert!(plain.is_empty());
    }

    /// A paused or edited count goes back to a plain number.
    #[test]
    fn paused_and_edited_counts_are_plain() {
        let before = json!({"stun": 5});
        let (clocks, _) = save(
            &Clocks::default(),
            Some(&json!({"stun": 6})),
            &before,
            99,
            100,
        );
        assert_eq!(clocks.down["/stun"], 105);
        let (paused, stored) = save(&clocks, Some(&before), &before, 100, 101);
        assert!(paused.is_empty());
        assert_eq!(stored, before);
        let edited = json!({"stun": 10});
        let (clocks, stored) = save(&clocks, Some(&before), &edited, 100, 101);
        assert!(clocks.is_empty());
        assert_eq!(stored, edited);
    }

    /// Saves between ticks keep valid forms and never invent new ones.
    #[test]
    fn saves_without_elapsed_time_keep_forms() {
        let mut clocks = Clocks::default();
        clocks.down.insert("/lock".into(), 8);
        let value = json!({"lock": 3, "other": 1});
        let (next, _) = save(&clocks, Some(&json!({"lock": 3, "other": 2})), &value, 5, 5);
        assert_eq!(next, clocks);
    }

    /// Forms for other parts survive, excluded values stay plain, and pointers escape
    /// map keys.
    #[test]
    fn parts_exclusions_and_pointer_escaping() {
        let mut stored = Clocks::default();
        stored.down.insert("/core_timer".into(), 50);
        let before = json!({"a/b": 3, "dice": 4});
        let after = json!({"a/b": 2, "dice": 5});
        let mut clocks = stored.clone();
        clocks.update_part(
            &stored,
            |path| field(path) != "core_timer",
            |path| field(path) != "dice",
            Some(&before),
            &after,
            10,
            11,
        );
        assert_eq!(clocks.down["/core_timer"], 50);
        assert_eq!(clocks.down["/a~1b"], 13);
        assert!(!clocks.up.contains_key("/dice"));
        assert_eq!(field("/a~1b"), "a/b");
    }

    /// Fractional values are never treated as counts.
    #[test]
    fn floats_are_plain() {
        let (clocks, _) = save(
            &Clocks::default(),
            Some(&json!({"heat": 2.0})),
            &json!({"heat": 1.0}),
            1,
            2,
        );
        assert!(clocks.is_empty());
    }
}
