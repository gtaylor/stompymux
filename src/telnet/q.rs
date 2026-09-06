//! RFC 1143 section 7 state machine, independent of framing and option-specific behavior.
/// The endpoint whose option is being negotiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Local,
    Remote,
}
/// Telnet negotiation verbs; values are protocol constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Verb {
    Will = 251,
    Wont = 252,
    Do = 253,
    Dont = 254,
}
impl Verb {
    /// Decode a framing-validated negotiation verb.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            251 => Some(Self::Will),
            252 => Some(Self::Wont),
            253 => Some(Self::Do),
            254 => Some(Self::Dont),
            _ => None,
        }
    }
}
/// Stable states have no queue; pending states retain one opposite request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QState {
    #[default]
    No,
    Yes,
    WantNo {
        opposite: bool,
    },
    WantYes {
        opposite: bool,
    },
}
impl QState {
    /// Negotiating is distinct from enabled, as required by RFC 1143.
    pub fn enabled(self) -> bool {
        self == Self::Yes
    }
    /// Application request; repetitions are harmless and reversals stay off the wire until acknowledged.
    fn request(self, enable: bool) -> (Self, Option<bool>) {
        use QState::*;
        match (self, enable) {
            (No, true) => (WantYes { opposite: false }, Some(true)),
            (Yes, false) => (WantNo { opposite: false }, Some(false)),
            (WantNo { .. }, enable) => (WantNo { opposite: enable }, None),
            (WantYes { .. }, enable) => (WantYes { opposite: !enable }, None),
            _ => (self, None),
        }
    }
    /// Positive/negative peer acknowledgement or unsolicited request. Unexpected replies use RFC recovery.
    fn receive(self, positive: bool, accept: bool) -> (Self, Option<bool>) {
        use QState::*;
        match (self, positive) {
            (No, true) => (if accept { Yes } else { No }, Some(accept)),
            (Yes, true) | (No, false) => (self, None),
            (Yes, false) => (No, Some(false)),
            (WantNo { opposite: false }, _) => (No, None),
            (WantNo { opposite: true }, true) => (Yes, None),
            (WantNo { opposite: true }, false) => (WantYes { opposite: false }, Some(true)),
            (WantYes { opposite: false }, true) => (Yes, None),
            (WantYes { opposite: true }, true) => (WantNo { opposite: false }, Some(false)),
            (WantYes { .. }, false) => (No, None),
        }
    }
}
/// Observable state transition, emitted only when state actually changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    /// Telnet option number.
    pub option: u8,
    /// Endpoint whose state changed.
    pub side: Side,
    /// State before processing this input.
    pub before: QState,
    /// State already installed in the engine.
    pub after: QState,
}
/// Ordered negotiation effects; the engine updates its state before returning these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Send(Verb, u8),
    Changed(Change),
}
/// Independent state for both endpoints of every possible option, with constant memory use.
#[derive(Debug, Clone)]
pub struct Negotiator {
    states: [[QState; 2]; 256],
}
impl Default for Negotiator {
    fn default() -> Self {
        Self {
            states: [[QState::No; 2]; 256],
        }
    }
}
impl Negotiator {
    /// Read negotiated or pending state without conflating directions.
    pub fn state(&self, option: u8, side: Side) -> QState {
        self.states[option as usize][side as usize]
    }
    /// Request an option change on either endpoint.
    pub fn request(&mut self, option: u8, side: Side, enable: bool) -> Vec<Event> {
        let before = self.state(option, side);
        self.apply(option, side, before.request(enable))
    }
    /// Receive a wire verb, consulting policy only for unsolicited enablement.
    pub fn receive(&mut self, option: u8, verb: Verb, accept: bool) -> Vec<Event> {
        let side = match verb {
            Verb::Will | Verb::Wont => Side::Remote,
            _ => Side::Local,
        };
        let positive = matches!(verb, Verb::Will | Verb::Do);
        self.apply(
            option,
            side,
            self.state(option, side).receive(positive, accept),
        )
    }
    /// Emit wire output before transition notifications, allowing option handlers to follow with subnegotiation.
    fn apply(
        &mut self,
        option: u8,
        side: Side,
        (after, send): (QState, Option<bool>),
    ) -> Vec<Event> {
        let before = self.state(option, side);
        self.states[option as usize][side as usize] = after;
        let mut events = Vec::new();
        if let Some(enable) = send {
            let verb = match (side, enable) {
                (Side::Local, true) => Verb::Will,
                (Side::Local, false) => Verb::Wont,
                (Side::Remote, true) => Verb::Do,
                (Side::Remote, false) => Verb::Dont,
            };
            events.push(Event::Send(verb, option));
        }
        if before != after {
            events.push(Event::Changed(Change {
                option,
                side,
                before,
                after,
            }));
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    const STATES: [QState; 6] = [
        QState::No,
        QState::Yes,
        QState::WantNo { opposite: false },
        QState::WantNo { opposite: true },
        QState::WantYes { opposite: false },
        QState::WantYes { opposite: true },
    ];
    /// Independent expected tables transcribed from RFC 1143 section 7, not computed from engine branches.
    #[test]
    fn exhaustive_transition_tables_in_both_directions() {
        // State indexes follow STATES. Send values mean positive/negative response in the selected direction.
        let request_yes = [
            (4, Some(true)),
            (1, None),
            (3, None),
            (3, None),
            (4, None),
            (4, None),
        ];
        let request_no = [
            (0, None),
            (2, Some(false)),
            (2, None),
            (2, None),
            (5, None),
            (5, None),
        ];
        let receive_yes = [
            (1, Some(true)),
            (1, None),
            (0, None),
            (1, None),
            (1, None),
            (2, Some(false)),
        ];
        let receive_no = [
            (0, None),
            (0, Some(false)),
            (0, None),
            (4, Some(true)),
            (0, None),
            (0, None),
        ];
        for option in 0..=255 {
            for side in [Side::Local, Side::Remote] {
                for (i, state) in STATES.into_iter().enumerate() {
                    for (receive, enable, table) in [
                        (false, true, request_yes),
                        (false, false, request_no),
                        (true, true, receive_yes),
                        (true, false, receive_no),
                    ] {
                        for accept in [false, true] {
                            let mut q = Negotiator::default();
                            q.states[option as usize][side as usize] = state;
                            let input = match (side, enable) {
                                (Side::Local, true) => Verb::Do,
                                (Side::Local, false) => Verb::Dont,
                                (Side::Remote, true) => Verb::Will,
                                (Side::Remote, false) => Verb::Wont,
                            };
                            let events = if receive {
                                q.receive(option, input, accept)
                            } else {
                                q.request(option, side, enable)
                            };
                            let (index, send) = if receive && enable && i == 0 && !accept {
                                (0, Some(false))
                            } else {
                                table[i]
                            };
                            assert_eq!(
                                q.state(option, side),
                                STATES[index],
                                "{side:?} {state:?} receive={receive} positive={enable} accept={accept}"
                            );
                            let expected_verb = send.map(|positive| match (side, positive) {
                                (Side::Local, true) => Verb::Will,
                                (Side::Local, false) => Verb::Wont,
                                (Side::Remote, true) => Verb::Do,
                                (Side::Remote, false) => Verb::Dont,
                            });
                            let actual: Vec<_> = events
                                .iter()
                                .filter_map(|e| {
                                    if let Event::Send(v, o) = e {
                                        assert_eq!(*o, option);
                                        Some(*v)
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            assert_eq!(actual, expected_verb.into_iter().collect::<Vec<_>>());
                            assert_eq!(
                                events
                                    .iter()
                                    .filter(|e| matches!(e, Event::Changed(_)))
                                    .count(),
                                usize::from(state != STATES[index])
                            );
                            let other = if side == Side::Local {
                                Side::Remote
                            } else {
                                Side::Local
                            };
                            assert_eq!(q.state(option, other), QState::No);
                        }
                    }
                }
            }
        }
    }
    /// Crossed requests and several application reversals still converge without unbounded wire exchanges.
    #[test]
    fn paired_peers_converge_after_crossed_requests() {
        let mut peers = [Negotiator::default(), Negotiator::default()];
        let mut wire = VecDeque::new();
        let mut seed = 7u32;
        for _ in 0..500 {
            // Queue application changes on both peers before delivering acknowledgements.
            for _ in 0..8 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let peer = (seed >> 16) as usize % 2;
                let side = if seed & 128 == 0 {
                    Side::Local
                } else {
                    Side::Remote
                };
                for e in peers[peer].request(24, side, seed & 1024 != 0) {
                    if let Event::Send(v, o) = e {
                        wire.push_back((1 - peer, v, o));
                    }
                }
            }
            let mut steps = 0;
            while let Some((peer, verb, option)) = wire.pop_front() {
                steps += 1;
                assert!(steps < 100, "negotiation loop");
                for e in peers[peer].receive(option, verb, true) {
                    if let Event::Send(v, o) = e {
                        wire.push_back((1 - peer, v, o));
                    }
                }
            }
            for (a, b) in [(Side::Local, Side::Remote), (Side::Remote, Side::Local)] {
                assert!(matches!(peers[0].state(24, a), QState::Yes | QState::No));
                assert_eq!(peers[0].state(24, a), peers[1].state(24, b));
            }
        }
    }
}
