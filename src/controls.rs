//! Runtime admission and maintenance controls, independent of persistent configuration.
use crate::{
    flags,
    powers::Power,
    world::{ObjectId, World},
};

/// Implemented C global controls and their accepted spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Cleaning,
    IdleChecking,
    Queueing,
    Logins,
}
impl Control {
    pub const ALL: [Self; 4] = [
        Self::Cleaning,
        Self::IdleChecking,
        Self::Queueing,
        Self::Logins,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Cleaning => "cleaning",
            Self::IdleChecking => "idlechecking",
            Self::Queueing => "queueing",
            Self::Logins => "logins",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        Self::ALL.into_iter().find(|c| {
            name.len() >= if *c == Self::Logins { 3 } else { 2 } && c.name().starts_with(&name)
        })
    }
}
/// All controls reset to enabled on process startup, never on Lua reload.
pub struct Controls {
    values: [bool; 4],
}
impl Default for Controls {
    fn default() -> Self {
        Self { values: [true; 4] }
    }
}
impl Controls {
    pub fn enabled(&self, c: Control) -> bool {
        self.values[c as usize]
    }
    pub fn set(&mut self, c: Control, enabled: bool) {
        self.values[c as usize] = enabled;
    }
    pub fn status(&self) -> String {
        format!(
            "Global parameters: {}",
            Control::ALL
                .into_iter()
                .map(|c| format!(
                    "{}...{}",
                    c.name(),
                    if self.enabled(c) {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
    /// Evaluate elapsed times supplied by the caller, making timeout policy clock-independent.
    pub fn timeout(
        &self,
        world: &World,
        player: Option<ObjectId>,
        connection_age: std::time::Duration,
        idle_age: std::time::Duration,
        connection_limit: u64,
        idle_limit: u64,
    ) -> Option<&'static str> {
        if !self.enabled(Control::IdleChecking) {
            return None;
        }
        match player {
            None if connection_age > std::time::Duration::from_secs(connection_limit) => {
                Some("*** Login Timeout ***\r\n")
            }
            Some(p)
                if idle_age > std::time::Duration::from_secs(idle_limit) && !can_idle(world, p) =>
            {
                Some("*** Inactivity Timeout ***\r\n")
            }
            _ => None,
        }
    }
    /// Capacity counts authenticated sessions; verified privileged logins bypass admission limits.
    pub fn admission(&self, count: usize, maximum: i64, privileged: bool) -> Result<(), Admission> {
        if privileged {
            return Ok(());
        }
        if !self.enabled(Control::Logins) {
            return Err(Admission::Down);
        }
        if maximum >= 0 && count as u128 >= maximum as u128 {
            return Err(Admission::Full);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Down,
    Full,
}
/// Current Wizard authority or IDLE metadata exempts authenticated players only.
pub fn can_idle(world: &World, player: ObjectId) -> bool {
    flags::is_wizard(world, player)
        || world
            .objects
            .get(&player)
            .is_some_and(|o| o.powers.contains(Power::Idle))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_counts_sessions_and_uses_live_controls() {
        let mut c = Controls::default();
        assert!(c.admission(100, -1, false).is_ok());
        assert_eq!(c.admission(0, 0, false), Err(Admission::Full));
        assert_eq!(c.admission(2, 2, false), Err(Admission::Full));
        assert!(c.admission(1, 2, false).is_ok());
        c.set(Control::Logins, false);
        assert_eq!(c.admission(0, 2, false), Err(Admission::Down));
        assert!(c.admission(100, 0, true).is_ok());
        assert!(Controls::default().enabled(Control::Logins));
        for (name, control) in [
            ("CL", Control::Cleaning),
            ("id", Control::IdleChecking),
            ("qu", Control::Queueing),
            ("log", Control::Logins),
        ] {
            assert_eq!(Control::parse(name), Some(control));
        }
        assert_eq!(Control::parse("lo"), None);
    }
    #[test]
    fn timeout_switch_applies_to_both_classes_and_live_privilege() {
        use std::time::Duration;
        let mut c = Controls::default();
        let w = World::default();
        let age = Duration::from_secs(100);
        assert!(c.timeout(&w, None, age, age, 10, 10).is_some());
        assert!(c.timeout(&w, Some(ObjectId(9)), age, age, 10, 10).is_some());
        assert!(c.timeout(&w, Some(ObjectId(1)), age, age, 10, 10).is_none());
        c.set(Control::IdleChecking, false);
        assert!(c.timeout(&w, None, age, age, 10, 10).is_none());
        assert!(c.timeout(&w, Some(ObjectId(9)), age, age, 10, 10).is_none());
        c.set(Control::IdleChecking, true);
        assert!(c.timeout(&w, Some(ObjectId(9)), age, age, 10, 10).is_some());
    }
}
