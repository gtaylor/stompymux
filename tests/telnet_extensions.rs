//! Extended protocol fixtures derived from C telnet_handler.c and telnet_environment.c.
use stompymux_rs::telnet::{
    self, Decoder, Input,
    environment::{Environment, Kind},
    q::Side,
};
/// Encode one escaped Telnet subnegotiation.
fn sub(option: u8, payload: &[u8]) -> Vec<u8> {
    match Decoder::sub_reply(option, payload) {
        Input::Reply(v) => v,
        _ => unreachable!(),
    }
}
/// Retain only ordered wire output.
fn wire(events: Vec<Input>) -> Vec<u8> {
    events
        .into_iter()
        .flat_map(|e| {
            if let Input::Reply(v) = e {
                v
            } else {
                Vec::new()
            }
        })
        .collect()
}
#[test]
fn environment_is_info_escaping_and_limits_are_atomic() {
    let mut e = Environment::default();
    e.update(&[0, 0, b'A', 1, b'x', 3, b'A', 1, 2, 0, 2, 1, 2, 2, 2, 3, 255])
        .unwrap();
    assert_eq!(e.0[&(Kind::Var, b"A".to_vec())], b"x");
    assert_eq!(e.0[&(Kind::UserVar, b"A".to_vec())], [0, 1, 2, 3, 255]);
    e.update(&[2, 0, b'A', 1]).unwrap();
    assert!(e.0[&(Kind::Var, b"A".to_vec())].is_empty());
    e.update(&[2, 0, b'A']).unwrap();
    assert!(!e.0.contains_key(&(Kind::Var, b"A".to_vec())));
    let original = e.0.clone();
    for malformed in [
        vec![1],
        vec![2, 0, b'A', 1, 2],
        [vec![2, 0], vec![b'n'; 257], vec![1]].concat(),
        [vec![2, 0, b'A', 1], vec![b'x'; 4097]].concat(),
    ] {
        assert!(e.update(&malformed).is_err());
        assert_eq!(e.0, original);
    }
    let mut maximum = vec![0];
    for n in 0..64 {
        maximum.extend([0, b'a' + n / 26, b'a' + n % 26, 1, b'v']);
    }
    e.update(&maximum).unwrap();
    assert_eq!(e.0.len(), 64);
    let original = e.0.clone();
    assert!(e.update(&[2, 0, b'Z', 1, b'v']).is_err());
    assert_eq!(e.0, original);
    let mut bytes = vec![0];
    for n in 0..16 {
        bytes.extend([0, b'a' + n, 1]);
        bytes.extend(vec![b'x'; 4095]);
    }
    e.update(&bytes).unwrap(); // 16 * (1+4095) = 65536
    let original = e.0.clone();
    assert!(e.update(&[2, 0, b'Z', 1, b'v']).is_err());
    assert_eq!(e.0, original);
    e.update(&[0]).unwrap();
    assert!(e.0.is_empty());
}
#[test]
fn options_direction_reenable_and_gmcp_c_behavior() {
    let mut d = Decoder::default();
    d.initial();
    for (option, verb) in [(39, 251), (70, 253), (86, 253), (201, 253)] {
        let events = d.feed(&[255, verb, option]).unwrap();
        match option {
            39 => assert_eq!(wire(events), sub(39, &[1])),
            70 => assert!(events.iter().any(|e| matches!(e, Input::StatusRequest))),
            86 => assert!(events.iter().any(|e| matches!(e, Input::StartCompression))),
            _ => assert!(wire(events).is_empty()),
        }
        assert!(d.feed(&[255, verb, option]).unwrap().is_empty());
        let opposite = if verb == 251 { 253 } else { 251 };
        assert_eq!(
            wire(d.feed(&[255, opposite, option]).unwrap()),
            [255, if opposite == 253 { 252 } else { 254 }, option]
        );
    }
    d.feed(&sub(39, b"\x00\x00USER\x01client")).unwrap();
    assert_eq!(d.environment.0.len(), 1);
    assert!(
        d.feed(&sub(39, b"\x02\x00USER\x01\x02"))
            .unwrap()
            .iter()
            .any(|e| matches!(e, Input::Diagnostic(_)))
    );
    assert_eq!(d.environment.0.len(), 1);
    for payload in [b"Core.Ping".as_slice(), b"Core.Ping {}"] {
        assert_eq!(
            wire(d.feed(&sub(201, payload)).unwrap()),
            sub(201, b"Core.Ping")
        );
    }
    for payload in [b"core.Ping".as_slice(), b"Core.PingPong", b"Core.Hello {}"] {
        assert!(wire(d.feed(&sub(201, payload)).unwrap()).is_empty());
    }
    d.feed(&[255, 254, 201]).unwrap();
    assert!(wire(d.feed(&sub(201, b"Core.Ping")).unwrap()).is_empty());
    d.feed(&[255, 252, 39]).unwrap();
    assert!(d.environment.0.is_empty());
    assert_eq!(
        wire(d.feed(&[255, 251, 39]).unwrap()),
        [vec![255, 253, 39], sub(39, &[1])].concat()
    );
    assert!(d.option_state(telnet::NEW_ENVIRON, Side::Remote).enabled());
}
/// MCCP2 wraps exactly the bytes after its marker in one persistent, finishable zlib stream.
#[tokio::test]
async fn compression_marker_stream_flush_and_finish() {
    use std::io::Read;
    use telnet::transport::{Stats, Writer};
    let stats = Stats::default();
    let mut writer = Writer::default();
    let mut bytes = Vec::new();
    writer.bytes(&mut bytes, &stats, b"plain").await.unwrap();
    writer.start(&mut bytes, &stats).await.unwrap();
    assert_eq!(bytes, b"plain\xff\xfa\x56\xff\xf0");
    writer.bytes(&mut bytes, &stats, b"prompt> ").await.unwrap();
    // A sync-flushed prompt can be decompressed before closing the connection.
    let mut partial = flate2::Decompress::new(true);
    let mut decoded = [0; 128];
    partial
        .decompress(&bytes[10..], &mut decoded, flate2::FlushDecompress::Sync)
        .unwrap();
    assert_eq!(&decoded[..partial.total_out() as usize], b"prompt> ");
    writer.start(&mut bytes, &stats).await.unwrap();
    writer
        .bytes(&mut bytes, &stats, &[255, 252, 86])
        .await
        .unwrap();
    let mut seed = 1u32;
    let payload: Vec<u8> = (0..65536)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 24) as u8
        })
        .collect();
    writer.bytes(&mut bytes, &stats, &payload).await.unwrap();
    writer.finish(&mut bytes, &stats).await.unwrap();
    let mut decoded = Vec::new();
    flate2::read::ZlibDecoder::new(&bytes[10..])
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(
        decoded,
        [b"prompt> ".to_vec(), vec![255, 252, 86], payload].concat()
    );
    assert_eq!(stats.snapshot().wire_output, bytes.len() as u64);
    assert_eq!(stats.snapshot().compression, 2);
}
#[tokio::test]
async fn compression_socket_failure_does_not_claim_activation() {
    let (mut writer, reader) = tokio::io::duplex(16);
    drop(reader);
    let mut compression = telnet::transport::Writer::default();
    let stats = telnet::transport::Stats::default();
    assert!(compression.start(&mut writer, &stats).await.is_err());
    assert_ne!(stats.snapshot().compression, 2);
}
#[test]
fn diagnostics_escape_and_bound_hostile_environment() {
    use telnet::diagnostics::{Report, escape};
    assert_eq!(escape(&[27, 255, b'"', b'\\']), "\\x1B\\xFF\\\"\\\\");
    let mut d = Decoder::default();
    d.environment.update(b"\x00\x00X\x01\x1b[31m").unwrap();
    for limit in [0, 8, 32, 128, 4096] {
        let mut report = Report::new(limit);
        telnet::diagnostics::telnet(&mut report, "Wizard", 2, 1, &d, &Default::default());
        let bytes = report.finish();
        assert!(bytes.len() <= limit);
        assert!(!bytes.contains(&27));
        if limit == 128 {
            assert!(String::from_utf8_lossy(&bytes).contains("truncated"));
        }
    }
}

#[test]
fn oversized_environment_drains_without_replacing_state() {
    let config = stompymux_rs::config::RuntimeConfig {
        telnet_subnegotiation_limit: 6,
        ..Default::default()
    };
    let mut d = Decoder::new(&config);
    d.feed(&[255, 251, 39]).unwrap();
    d.feed(&sub(39, b"\x00\x00X\x01Y")).unwrap();
    let saved = d.environment.0.clone();
    let payload = [sub(39, b"\x00\x00X\x01toolong"), b"ok\r\n".to_vec()].concat();
    let events = d.feed(&payload).unwrap();
    assert!(events.iter().any(|e| matches!(e, Input::Diagnostic(_))));
    assert!(events.iter().any(|e| matches!(e,Input::Line(s) if s=="ok")));
    assert_eq!(d.environment.0, saved);
    d.feed(&sub(39, b"\x02\x00X")).unwrap();
    assert!(d.environment.0.is_empty());
}
#[test]
fn queue_counters_and_starting_compression_are_observable() {
    use stompymux_rs::sessions::{LoginFlow, Output, Session};
    let (output, mut rx) = tokio::sync::mpsc::channel(1);
    let now = std::time::Instant::now();
    let s = Session {
        output,
        stats: Default::default(),
        palette: Default::default(),
        color_override: Default::default(),
        presets_emitted: Default::default(),
        peer: "127.0.0.1".parse().unwrap(),
        site: Default::default(),
        player: None,
        flow: LoginFlow::Name,
        connected: now,
        active: now,
        decoder: Default::default(),
        quota: 1,
        quota_at: now,
        failed: Default::default(),
        output_message_limit: 64,
    };
    s.protocol(vec![Input::StartCompression]);
    assert_eq!(s.stats.snapshot().compression, 1);
    assert!(matches!(rx.try_recv().unwrap(), Output::StartCompression));
    s.protocol(vec![Input::StartCompression]);
    assert!(rx.try_recv().is_err());
    assert!(s.raw(b"abc".to_vec()));
    assert!(!s.raw(b"xy".to_vec()));
    assert!(s.raw(Vec::new()));
    assert_eq!(s.stats.snapshot().output, [3, 2, 5]);
    assert!(s.failed.get());
    assert_eq!(telnet::diagnostics::connected_time(90060), "1d 01:01");
    assert_eq!(telnet::diagnostics::idle_time(600), "0s");
    assert_eq!(telnet::diagnostics::idle_time(601), "10m");
}
#[tokio::test]
async fn compressed_slow_writer_is_bounded_by_timeout() {
    let (mut socket, _reader) = tokio::io::duplex(16);
    let mut writer = telnet::transport::Writer::default();
    let stats = telnet::transport::Stats::default();
    writer.start(&mut socket, &stats).await.unwrap();
    let payload: Vec<_> = (0..65536).map(|n| (n % 251) as u8).collect();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(10),
            writer.bytes(&mut socket, &stats, &payload)
        )
        .await
        .is_err()
    );
    assert_eq!(stats.snapshot().compression, 2);
    assert!(stats.snapshot().wire_output <= 16);
}
