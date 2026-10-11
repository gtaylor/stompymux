//! MXP negotiation, protocol replies, inline image markup and live delivery.
use std::{cell::Cell, rc::Rc};
use stompymux_rs::{
    ObjectId, ShutdownRequest, accounts, persistence,
    telnet::{Decoder, Input, MXP, mxp, q::Side},
    text::{self, Document, Palette, RenderOptions},
};

use crate::support;

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

/// Retain only complete command lines.
fn lines(events: &[Input]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            Input::Line(line) => Some(line.as_str()),
            _ => None,
        })
        .collect()
}

/// Rendering options for a client that advertised `+image`.
fn mxp_options() -> RenderOptions {
    RenderOptions {
        mxp: ["image".to_owned()].into_iter().collect(),
        ..Default::default()
    }
}

const ATLAS: &str = "https://cdn.example.com/mechs/atlas.png";

#[test]
fn negotiation_locks_the_client_and_swallows_protocol_replies() {
    let mut d = Decoder::default();
    let initial = wire(d.initial());
    assert!(initial.ends_with(&[255, 251, MXP]));
    assert_eq!(wire(d.feed(&[255, 253, MXP]).unwrap()), mxp::activation());
    assert!(d.option_state(MXP, Side::Local).enabled());
    assert!(wire(d.feed(&[255, 253, MXP]).unwrap()).is_empty());
    let events = d
        .feed(
            b"\x1b[1z<VERSION MXP=1.0 CLIENT=Mudlet VERSION=\"4.17.2\" REGISTERED=yes>\r\n\
              \x1b[1z<SUPPORTS +image +image.url +send -frame>\r\n\
              look\r\n",
        )
        .unwrap();
    assert_eq!(lines(&events), ["look"]);
    assert_eq!(d.mxp.name, "Mudlet");
    assert_eq!(d.mxp.version, "4.17.2");
    assert_eq!(d.mxp.mxp_version, "1.0");
    assert_eq!(d.mxp.registered, Some(true));
    assert!(d.mxp.supports("IMAGE") && d.mxp.supports("image.url"));
    assert!(!d.mxp.supports("frame"));
    // Unknown secure-line replies are still protocol data, never commands.
    let events = d.feed(b"\x1b[1z<FRAME name=x>\r\n").unwrap();
    assert!(lines(&events).is_empty());
    // The client turning MXP off forgets everything it reported.
    assert_eq!(wire(d.feed(&[255, 254, MXP]).unwrap()), [255, 252, MXP]);
    assert!(!d.option_state(MXP, Side::Local).enabled());
    assert_eq!(d.mxp, mxp::Client::default());
    let events = d.feed(b"\x1b[1z<VERSION>\r\n").unwrap();
    assert_eq!(lines(&events), ["\x1b[1z<VERSION>"]);
    // An unsolicited DO is accepted and restarts the handshake from scratch.
    assert_eq!(
        wire(d.feed(&[255, 253, MXP]).unwrap()),
        [vec![255, 251, MXP], mxp::activation()].concat()
    );
}

#[test]
fn diagnostics_report_the_mxp_client_with_escaping() {
    let mut d = Decoder::default();
    d.initial();
    d.feed(&[255, 253, MXP]).unwrap();
    d.feed(b"\x1b[1z<VERSION CLIENT=\"Evil\x1b[31m\" VERSION=1>\r\n\x1b[1z<SUPPORTS +image>\r\n")
        .unwrap();
    let mut report = stompymux_rs::telnet::diagnostics::Report::new(8192);
    stompymux_rs::telnet::diagnostics::telnet(&mut report, "Wizard", 1, 7, &d, &Default::default());
    let text = String::from_utf8(report.finish()).unwrap();
    assert!(text.contains("  MXP:\r\n    Negotiated: yes"), "{text}");
    assert!(
        text.contains("Client: \"Evil\\x1B[31m\" version \"1\""),
        "{text}"
    );
    assert!(
        text.contains("Replies: version yes; supports yes"),
        "{text}"
    );
    assert!(text.contains("Supported tags: image"), "{text}");
    assert!(!text.contains('\x1b'));
}

#[test]
fn image_markup_renders_a_tag_a_link_or_fallback_text() {
    let p = Palette::default();
    let source = format!(
        "See [image=\"{ATLAS}\" width=200 height=150 align=right]the [bold]Atlas[/][/] now"
    );
    text::validate(&p, &source).unwrap();
    let document = Document::Styled(source.clone());
    let shown = String::from_utf8(document.telnet(&p, &mxp_options(), 4096)).unwrap();
    assert_eq!(
        shown,
        "See \x1b[4z<IMAGE atlas.png URL=\"https://cdn.example.com/mechs/\" W=200 H=150 ALIGN=right> now"
    );
    let plain = String::from_utf8(document.telnet(&p, &RenderOptions::default(), 4096)).unwrap();
    assert_eq!(plain, "See the Atlas now");
    let mut osc = RenderOptions::default();
    osc.capabilities.insert(String::new());
    let linked = String::from_utf8(document.telnet(&p, &osc, 4096)).unwrap();
    assert_eq!(
        linked,
        format!("See \x1b]8;;{ATLAS}\x1b\\the Atlas\x1b]8;;\x1b\\ now")
    );
    // Bytes stay within the message budget whichever way the image renders.
    for limit in 0..120 {
        for options in [mxp_options(), osc.clone(), RenderOptions::default()] {
            let bytes = document.telnet(&p, &options, limit);
            assert!(bytes.len() <= limit, "{limit}: {}", bytes.len());
            assert!(std::str::from_utf8(&bytes).is_ok());
        }
    }
    let html = document.html(&p, 4096).unwrap();
    assert!(
        html.contains(&format!(
            "<img src=\"{ATLAS}\" alt=\"the Atlas\" width=\"200\" height=\"150\" class=\"mux-align-right\">"
        )),
        "{html}"
    );
    assert_eq!(text::plain(&source), "See the Atlas now");
    assert_eq!(text::width(&source), 17);
    let truncated = text::truncate(&source, 7);
    assert_eq!(
        truncated,
        format!("See [image=\"{ATLAS}\" width=200 height=150 align=right]the[/]")
    );
    text::validate(&p, &truncated).unwrap();
}

#[test]
fn image_markup_validation_mirrors_links() {
    let p = Palette::default();
    for bad in [
        format!("[image=\"{ATLAS}\"][/]"),
        format!("[image={ATLAS}]x[/]"),
        "[image=\"https://cdn.example.com/\"]x[/]".to_owned(),
        "[image=\"https://cdn.example.com/a.png?v=1\"]x[/]".to_owned(),
        "[image=\"javascript:alert(1)\"]x[/]".to_owned(),
        format!("[image=\"{ATLAS}\"][image=\"{ATLAS}\"]x[/][/]"),
        format!("[send=\"look\"][image=\"{ATLAS}\"]x[/][/]"),
        format!("[image=\"{ATLAS}\"][send=\"look\"]x[/][/]"),
        format!("[image=\"{ATLAS}\" width=abc]x[/]"),
        format!("[image=\"{ATLAS}\" align=center]x[/]"),
        format!("[image=\"{ATLAS}\" bold]x[/]"),
        format!("[image=\"{ATLAS}\"]x"),
    ] {
        assert!(text::validate(&p, &bad).is_err(), "{bad}");
    }
    for good in [
        format!("[bold][image=\"{ATLAS}\"]x[/][/]"),
        format!("[image=\"{ATLAS}\"][fg=red]x[/][/]"),
        format!("[image=\"{ATLAS}\"][[[/]"),
        format!("[IMAGE=\"{ATLAS}\" WIDTH=1 Align=TOP]x[/]"),
    ] {
        text::validate(&p, &good).unwrap();
    }
    // Permissive display leaves a malformed image tag visible instead of hiding text.
    let shown = text::plain(&format!("[image=\"{ATLAS}\" nonsense]x[/]"));
    assert!(shown.contains("nonsense]x"), "{shown}");
}

#[test]
fn markdown_images_follow_the_same_policy() {
    let p = Palette::default();
    let document = Document::markdown(
        format!(
            "Look: ![An Atlas]({ATLAS}) and ![](https://cdn.example.com/mechs/raven.png) and ![bad](javascript:x)\n"
        ),
        4096,
    )
    .unwrap();
    let wide = |mut options: RenderOptions| {
        options.width = 400;
        options
    };
    let shown = String::from_utf8(document.telnet(&p, &wide(mxp_options()), 4096)).unwrap();
    assert!(
        shown.contains(
            "Look: \x1b[4z<IMAGE atlas.png URL=\"https://cdn.example.com/mechs/\"> and \x1b[4z<IMAGE raven.png URL=\"https://cdn.example.com/mechs/\"> and bad"
        ),
        "{shown}"
    );
    let plain =
        String::from_utf8(document.telnet(&p, &wide(RenderOptions::default()), 4096)).unwrap();
    assert!(
        plain.contains(&format!(
            "Look: An Atlas ({ATLAS}) and raven.png (https://cdn.example.com/mechs/raven.png) and bad"
        )),
        "{plain}"
    );
    let mut osc = wide(RenderOptions::default());
    osc.capabilities.insert(String::new());
    let linked = String::from_utf8(document.telnet(&p, &osc, 4096)).unwrap();
    assert!(
        linked.contains(&format!("\x1b]8;;{ATLAS}\x1b\\An Atlas\x1b]8;;\x1b\\ and ")),
        "{linked}"
    );
    let html = document.html(&p, 4096).unwrap();
    assert!(
        html.contains(&format!("<img src=\"{ATLAS}\" alt=\"An Atlas\"")),
        "{html}"
    );
    assert!(!html.contains("javascript:"));
    assert!(html.contains("bad"));
}

/// A negotiated client receives the image tag and `@telnet` reports what it said;
/// a plain client on the same server still reads the fallback text.
#[tokio::test(flavor = "current_thread")]
async fn live_sessions_negotiate_and_render_per_client() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world) = support::isolated_world().await;
            for id in [1, 2] {
                world.accounts.get_mut(&ObjectId(id)).unwrap().hash =
                    Some(accounts::hash("secret", &config).unwrap());
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let (address, shutdown, task, _, _heartbeats) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            let mut wizard = support::Client::connect(address, 1).await;
            let mut plain = support::Client::connect(address, 2).await;
            use tokio::io::AsyncWriteExt;
            wizard.socket.write_all(&[255, 253, MXP]).await.unwrap();
            wizard.until("\x1b[7z\x1b[4z<VERSION>\x1b[4z<SUPPORT>").await;
            wizard
                .socket
                .write_all(
                    b"\x1b[1z<VERSION MXP=1.0 CLIENT=Mudlet VERSION=4.17>\r\n\x1b[1z<SUPPORTS +image +send>\r\n",
                )
                .await
                .unwrap();
            wizard.send("@telnet #1").await;
            let report = wizard.until("Supported tags: image send").await;
            assert!(report.contains("Client: \"Mudlet\" version \"4.17\""), "{report}");
            assert!(!report.contains("Huh?"), "{report}");
            wizard
                .send(&format!("@pemit #1=[image=\"{ATLAS}\"]Atlas[/] inbound"))
                .await;
            let shown = wizard.until(" inbound").await;
            assert!(
                shown.contains("\x1b[4z<IMAGE atlas.png URL=\"https://cdn.example.com/mechs/\"> inbound"),
                "{shown:?}"
            );
            assert!(!shown.contains("Atlas inbound"), "{shown:?}");
            wizard
                .send(&format!("@pemit #2=[image=\"{ATLAS}\"]Atlas[/] inbound"))
                .await;
            let fallback = plain.until("Atlas inbound").await;
            assert!(!fallback.contains("<IMAGE"), "{fallback:?}");
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
