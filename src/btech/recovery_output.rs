//! Recovery feedback uses the committed check, keeping private rolls out of cockpit broadcasts.
use super::{CharacterNotice, MessageTarget};
use crate::Scripts;
use anyhow::Result;

/// Publish the complete recovery result; a failure rolls back the enclosing heartbeat transaction.
pub(super) fn publish(scripts: &Scripts, notice: CharacterNotice) -> Result<()> {
    messages(notice)
        .into_iter()
        .try_for_each(|(recipient, text)| super::notify_message(scripts, recipient, &text))
}

/// Build pilot and cockpit feedback from the resolved recovery check.
fn messages(notice: CharacterNotice) -> Vec<(MessageTarget, String)> {
    let pilot = MessageTarget::Player(notice.player);
    let mut messages = vec![
        (pilot, "You attempt to regain consciousness!".into()),
        (
            pilot,
            format!(
                "Regain Consciousness on: {}  \tRoll: {}",
                notice.check.target, notice.check.roll
            ),
        ),
    ];
    if let Some(unit) = notice.unit {
        if notice.check.conscious {
            messages.push((
                MessageTarget::Unit(unit),
                "The pilot regains consciousness!".into(),
            ));
        }
    } else {
        messages.push((
            pilot,
            if notice.check.conscious {
                "You regain consciousness!"
            } else {
                "You fail to regain consciousness."
            }
            .into(),
        ));
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConsciousnessCheck, ObjectId};

    /// Literal reference strings and audiences distinguish private checks from public recovery.
    #[test]
    fn recovery_feedback_preserves_privacy_and_failure_silence() {
        let notice = CharacterNotice {
            player: ObjectId(1),
            unit: Some(ObjectId(9)),
            check: ConsciousnessCheck {
                target: 7,
                roll: 8,
                conscious: true,
            },
        };
        assert_eq!(
            messages(notice.clone()),
            vec![
                (
                    MessageTarget::Player(ObjectId(1)),
                    "You attempt to regain consciousness!".into()
                ),
                (
                    MessageTarget::Player(ObjectId(1)),
                    "Regain Consciousness on: 7  \tRoll: 8".into()
                ),
                (
                    MessageTarget::Unit(ObjectId(9)),
                    "The pilot regains consciousness!".into()
                ),
            ]
        );
        let mut failed = notice.clone();
        failed.check.roll = 6;
        failed.check.conscious = false;
        let feedback = messages(failed.clone());
        assert_eq!(feedback.len(), 2);
        assert!(
            feedback
                .iter()
                .all(|(target, _)| *target == MessageTarget::Player(ObjectId(1)))
        );
        assert_eq!(feedback[1].1, "Regain Consciousness on: 7  \tRoll: 6");
        failed.unit = None;
        assert_eq!(
            messages(failed).last().unwrap().1,
            "You fail to regain consciousness."
        );
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    use crate::{Config, ConsciousnessCheck, Kind, ObjectId, persistence};
    use std::{cell::RefCell, rc::Rc};

    /// Real notification fan-out delivers the roll only to the pilot and success to both occupants.
    #[tokio::test]
    async fn cockpit_delivery_keeps_roll_private_and_does_not_change_world() {
        let config = Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        // The fixture database is read-only; all report setup remains in this detached world.
        let mut world = persistence::load(&config.database()).await.unwrap();
        let unit = world.create(&config, "Recovery cockpit".into(), Kind::Thing);
        for id in [ObjectId(1), ObjectId(2)] {
            world.objects.get_mut(&id).unwrap().location = Some(unit);
        }
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let notice = CharacterNotice {
            player: ObjectId(1),
            unit: Some(unit),
            check: ConsciousnessCheck {
                target: 7,
                roll: 8,
                conscious: true,
            },
        };
        publish(&scripts, notice.clone()).unwrap();
        let outbox = scripts.drain_outbox();
        let pilot: Vec<_> = outbox
            .iter()
            .filter(|(id, _)| *id == ObjectId(1))
            .map(|(_, text)| crate::text::plain(text.source()))
            .collect();
        let passenger: Vec<_> = outbox
            .iter()
            .filter(|(id, _)| *id == ObjectId(2))
            .map(|(_, text)| crate::text::plain(text.source()))
            .collect();
        assert_eq!(
            pilot,
            [
                "You attempt to regain consciousness!",
                "Regain Consciousness on: 7  \tRoll: 8",
                "The pilot regains consciousness!"
            ]
        );
        assert_eq!(passenger, ["The pilot regains consciousness!"]);
        let mut failed = notice;
        failed.check.conscious = false;
        failed.check.roll = 6;
        publish(&scripts, failed).unwrap();
        assert!(
            scripts
                .drain_outbox()
                .iter()
                .all(|(id, _)| *id == ObjectId(1))
        );
        assert_eq!(scripts.world().btech, before);
    }
}
