//! Wire fixtures based on mux/network/telnet_handler.c and third_party/libtelnet's RFC 1143 handling.
use stompymux_rs::telnet::{
    CHARSET, Decoder, ECHO, Input, NAWS, TTYPE,
    q::{QState, Side},
};
/// Extract ordered wire effects, ignoring observable metadata changes.
fn wire(events: Vec<Input>) -> Vec<u8> {
    events
        .into_iter()
        .flat_map(|event| {
            if let Input::Reply(bytes) = event {
                bytes
            } else {
                Vec::new()
            }
        })
        .collect()
}
/// Feed a subnegotiation split at every byte boundary, escaping literal IAC payload bytes.
fn sub(d: &mut Decoder, option: u8, payload: &[u8]) -> Vec<u8> {
    let mut input = vec![255, 250, option];
    for &b in payload {
        input.push(b);
        if b == 255 {
            input.push(b);
        }
    }
    input.extend([255, 240]);
    input
        .into_iter()
        .flat_map(|b| wire(d.feed_byte(b).unwrap()))
        .collect()
}
#[test]
fn c_startup_directions_and_unsupported_options() {
    let mut d = Decoder::default();
    assert_eq!(
        wire(d.initial()),
        [
            255, 253, 24, 255, 253, 31, 255, 253, 39, 255, 251, 70, 255, 251, 86, 255, 251, 42,
            255, 251, 201
        ]
    );
    assert!(d.initial().is_empty());
    assert_eq!(
        wire(d.feed(&[255, 251, 24]).unwrap()),
        [255, 250, 24, 1, 255, 240]
    );
    assert!(wire(d.feed(&[255, 251, 24]).unwrap()).is_empty());
    assert_eq!(
        wire(d.feed(&[255, 253, 42]).unwrap()),
        b"\xff\xfa\x2a\x01;UTF-8\xff\xf0"
    );
    assert!(wire(d.feed(&[255, 253, 42]).unwrap()).is_empty());
    for (verb, option, response) in [
        (251, 42, 254),
        (253, 24, 252),
        (253, 31, 252),
        (251, 1, 254),
        (253, 1, 252),
        (251, 99, 254),
        (253, 255, 252),
    ] {
        assert_eq!(
            wire(d.feed(&[255, verb, option]).unwrap()),
            [255, response, option]
        );
    }
    assert_eq!(d.option_state(CHARSET, Side::Local), QState::Yes);
    assert_eq!(d.option_state(CHARSET, Side::Remote), QState::No);
}
#[test]
fn echo_reversals_refusal_and_delayed_acknowledgements() {
    let mut d = Decoder::default();
    assert!(d.echo(false).is_empty());
    assert_eq!(wire(d.echo(true)), [255, 251, 1]);
    assert!(d.echo(true).is_empty());
    assert!(wire(d.echo(false)).is_empty());
    assert!(wire(d.echo(true)).is_empty());
    assert!(wire(d.feed(&[255, 253, 1]).unwrap()).is_empty());
    assert_eq!(d.option_state(ECHO, Side::Local), QState::Yes);
    assert_eq!(wire(d.echo(false)), [255, 252, 1]);
    assert!(wire(d.echo(true)).is_empty());
    assert_eq!(wire(d.feed(&[255, 254, 1]).unwrap()), [255, 251, 1]);
    assert!(wire(d.feed(&[255, 254, 1]).unwrap()).is_empty());
    assert_eq!(d.option_state(ECHO, Side::Local), QState::No);
    // Neither refusal nor normal data retries; a later prompt is a new application stimulus.
    assert!(wire(d.feed(b"hello\r\n").unwrap()).is_empty());
    assert_eq!(wire(d.echo(true)), [255, 251, 1]);
    assert!(wire(d.echo(false)).is_empty());
    assert_eq!(wire(d.feed(&[255, 253, 1]).unwrap()), [255, 252, 1]);
    assert!(wire(d.feed(&[255, 254, 1]).unwrap()).is_empty());
}
#[test]
fn terminal_discovery_metadata_and_disabled_payloads() {
    let mut d = Decoder::default();
    assert!(sub(&mut d, NAWS, &[0, 120, 0, 40]).is_empty());
    sub(&mut d, TTYPE, b"\x00DUMB");
    assert!(d.ansi);
    assert_eq!((d.width, d.height), (80, 25));
    assert_eq!(
        wire(d.feed(&[255, 251, 24]).unwrap()),
        [255, 253, 24, 255, 250, 24, 1, 255, 240]
    );
    assert_eq!(
        sub(&mut d, TTYPE, b"\x00Mudlet"),
        [255, 250, 24, 1, 255, 240]
    );
    assert_eq!(
        sub(&mut d, TTYPE, b"\x00xterm"),
        [255, 250, 24, 1, 255, 240]
    );
    assert!(sub(&mut d, TTYPE, b"\x00MTTS 65").is_empty());
    assert!(!d.ansi);
    assert_eq!(wire(d.feed(&[255, 252, 24]).unwrap()), [255, 254, 24]);
    assert_eq!(d.terminal, "vt100");
    assert!(!d.ansi); // C retains color/MTTS metadata on WONT.
    sub(&mut d, TTYPE, b"\x00xterm");
    assert_eq!(d.terminal, "vt100");
    assert_eq!(wire(d.feed(&[255, 251, 31]).unwrap()), [255, 253, 31]);
    sub(&mut d, NAWS, &[0, 255, 0, 40]);
    assert_eq!((d.width, d.height), (255, 40));
    assert_eq!(wire(d.feed(&[255, 252, 31]).unwrap()), [255, 254, 31]);
    assert_eq!((d.width, d.height), (80, 25));
    sub(&mut d, NAWS, &[0, 120, 0, 40]);
    assert_eq!((d.width, d.height), (80, 25));
}

#[test]
fn mtts_matches_c_strtol_and_preserves_invalid_bytes() {
    fn discovered(name: &[u8]) -> Decoder {
        let mut decoder = Decoder::default();
        decoder.initial();
        decoder.feed(&[255, 251, TTYPE]).unwrap();
        sub(&mut decoder, TTYPE, b"\x00AuditClient");
        sub(&mut decoder, TTYPE, b"\x00XTERM");
        let mut payload = vec![0];
        payload.extend_from_slice(name);
        sub(&mut decoder, TTYPE, &payload);
        decoder
    }
    for name in [
        b"MTTS  265".as_slice(),
        b"MTTS \t265",
        b"MTTS \x0B265",
        b"MTTS +265",
        b"MTTS 4294967560",
    ] {
        let decoder = discovered(name);
        assert_eq!(decoder.terminal_raw, b"XTERM");
        assert_eq!(decoder.color_depth, 24);
    }
    for name in [b"MTTS ".as_slice(), b"MTTS -0"] {
        let decoder = discovered(name);
        assert_eq!(decoder.terminal_raw, b"XTERM");
        assert_eq!(decoder.color_depth, 0);
    }
    for name in [b"MTTS 265 ".as_slice(), b"MTTS   ", b"MTTS -1"] {
        let decoder = discovered(name);
        assert_eq!(decoder.terminal_raw, name);
        assert_eq!(decoder.color_depth, 16);
    }
    let decoder = discovered(b"MTTS 265\0ignored");
    assert_eq!(decoder.terminal_raw, b"XTERM");
    assert_eq!(decoder.color_depth, 24);

    let decoder = discovered(b"MTTS 9223372036854775807");
    assert_eq!(decoder.terminal_raw, b"XTERM");
    assert_eq!(decoder.color_depth, 24);
    assert!(decoder.screen_reader);
    let overflow = b"MTTS 9223372036854775808";
    let decoder = discovered(overflow);
    assert_eq!(decoder.terminal_raw, overflow);

    let mut long = vec![b'X'; 70];
    long.extend_from_slice(b"TRUECOLOR");
    let decoder = discovered(&long);
    assert_eq!(decoder.terminal_raw, vec![b'X'; 63]);
    assert_eq!(decoder.color_depth, 24);

    let decoder = discovered(b"bad\xFFtype");
    assert_eq!(decoder.terminal_raw, b"bad\xFFtype");
    let mut report = stompymux_rs::telnet::diagnostics::Report::new(8192);
    stompymux_rs::telnet::diagnostics::telnet(
        &mut report,
        "viewer",
        1,
        1,
        &decoder,
        &Default::default(),
    );
    assert!(
        String::from_utf8(report.finish())
            .unwrap()
            .contains("bad\\xFFtype")
    );
}

#[test]
fn ttype_response_count_continues_after_discovery_requests_stop() {
    let mut decoder = Decoder::default();
    decoder.initial();
    decoder.feed(&[255, 251, TTYPE]).unwrap();
    for _ in 0..300 {
        sub(&mut decoder, TTYPE, b"\x00XTERM");
    }
    assert_eq!(decoder.ttype_responses, 300);
}
#[test]
fn charset_pending_collisions_and_utf8_only_policy() {
    let mut d = Decoder::default();
    assert!(sub(&mut d, CHARSET, b"\x01;UTF-8").is_empty());
    d.initial();
    d.feed(&[255, 253, 42]).unwrap();
    assert_eq!(
        sub(&mut d, CHARSET, b"\x01;UTF-8"),
        [255, 250, 42, 3, 255, 240]
    );
    sub(&mut d, CHARSET, b"\x02UTF-8");
    assert!(d.charset_utf8);
    assert_eq!(
        sub(&mut d, CHARSET, b"\x01;ASCII;utf-8"),
        b"\xff\xfa\x2a\x02UTF-8\xff\xf0"
    );
    assert_eq!(
        sub(&mut d, CHARSET, b"\x01;ASCII"),
        [255, 250, 42, 3, 255, 240]
    );
    let payload = match Decoder::sub_reply(CHARSET, b"\x02LATIN1") {
        Input::Reply(payload) => payload,
        _ => unreachable!(),
    };
    let events = d.feed(&payload).unwrap();
    assert!(!d.charset_utf8);
    assert!(events.iter().any(
        |event| matches!(event, Input::Problem("CHARSET", message) if message.contains("unsupported charset"))
    ));
    assert!(matches!(
        d.feed(&[0xfe, b'\n']).unwrap().as_slice(),
        [Input::InvalidUtf8]
    ));
    d.feed(&[255, 254, 42]).unwrap();
    assert!(sub(&mut d, CHARSET, b"\x01;UTF-8").is_empty());
    d.feed(&[255, 253, 42]).unwrap();
    sub(&mut d, CHARSET, &[3]);
    assert_eq!(
        sub(&mut d, CHARSET, b"\x01;UTF-8"),
        b"\xff\xfa\x2a\x02UTF-8\xff\xf0"
    );
}
#[test]
fn inactive_subnegotiation_still_has_a_payload_limit() {
    let c = stompymux_rs::config::RuntimeConfig {
        telnet_subnegotiation_limit: 2,
        ..Default::default()
    };
    let mut d = Decoder::new(&c);
    assert!(d.feed(&[255, 250, 99, 1, 2, 3]).is_err());
}
