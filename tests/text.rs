//! C-derived styled text compatibility and Unicode/Markdown renderer regressions.
use serde::Deserialize;
use stompymux_rs::text::{self, ColorDepth, Document, Palette, RenderOptions};

#[derive(Deserialize)]
struct Fixture {
    reference: String,
    mode: String,
    source: String,
    expected: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

fn palette() -> Palette {
    Palette::from_config(
        &stompymux_rs::Config::load(crate::support::repository_root().join("tests/fixtures/game"))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn c_validation_catalog() {
    let mut p = palette();
    p.colors.insert("brand-blue".into(), [32, 96, 192]);
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/text/c_compatibility.json")).unwrap();
    let mut errors = Vec::new();
    for f in fixtures
        .iter()
        .filter(|f| f.mode == "invalid" || f.mode == "valid" || f.mode == "compile")
    {
        let result = text::validate(&p, &f.source);
        if (f.mode == "invalid") == result.is_ok() {
            errors.push(format!("{} {}: {:?}", f.reference, f.source, result));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn c_render_visible_text() {
    let mut p = palette();
    p.colors.insert("brand-blue".into(), [32, 96, 192]);
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/text/c_compatibility.json")).unwrap();
    for f in fixtures.iter().filter(|f| f.mode.starts_with("render")) {
        let options = RenderOptions {
            capabilities: f.capabilities.iter().cloned().collect(),
            ..Default::default()
        };
        let actual = Document::Styled(f.source.clone()).telnet(&p, &options, 65536);
        let actual = text::plain(std::str::from_utf8(&actual).unwrap());
        let expected = text::plain(f.expected.as_ref().unwrap());
        assert_eq!(actual, expected, "{}", f.reference);
    }
}

#[test]
fn unicode_boundaries_styles_and_limits() {
    let p = Palette::default();
    for s in ["[fg=red]e[/]\u{301}x", "👩‍👩‍👧‍👦x", "🇨🇦x", "界x"] {
        let width = if s.starts_with('[') { 1 } else { 2 };
        let truncated = text::truncate(s, width);
        assert_eq!(text::width(&truncated), width);
        assert!(!text::plain(&truncated).ends_with('x'));
        let d = Document::Styled(s.into());
        for limit in 0..70 {
            let b = d.telnet(
                &p,
                &RenderOptions {
                    color: ColorDepth::Truecolor,
                    ..Default::default()
                },
                limit,
            );
            assert!(b.len() <= limit);
            assert!(std::str::from_utf8(&b).is_ok());
        }
    }
    assert!(Document::markdown("*".repeat(100), 10).is_err());
}

#[test]
fn markdown_formats_stay_separate() {
    let p = Palette::default();
    let d=Document::markdown("# Heading\n\n**bold** and `[fg=red]example[/]`\n\n- [x] item\n\n[bad](javascript:alert)\n\n<script>alert(1)</script>\n".into(),4096).unwrap();
    let output = String::from_utf8(d.telnet(
        &p,
        &RenderOptions {
            color: ColorDepth::Ansi16,
            width: 80,
            ..Default::default()
        },
        8192,
    ))
    .unwrap();
    assert!(output.contains("[fg=red]example[/]"));
    assert!(output.contains("\x1b[1m"));
    assert!(!output.contains("\x1b[91m"));
    assert!(!output.contains("alert(1)"));
    let html = d.html(&p, 8192).unwrap();
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<code>[fg=red]example[/]</code>"));
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("<script>"));
}

/// Compare OSC protocol data semantically; JSON key ordering is not observable behavior.
fn osc(s: &str) -> Vec<(String, Option<serde_json::Value>)> {
    s.split("\x1b]8;;")
        .skip(1)
        .filter_map(|s| {
            let uri = s.split("\x1b\\").next().unwrap();
            if uri.is_empty() {
                return None;
            }
            if let Some((base, config)) = uri.split_once("config=") {
                let end = config.find(['&', '#']).unwrap_or(config.len());
                let mut bytes = Vec::new();
                let b = &config.as_bytes()[..end];
                let mut i = 0;
                while i < b.len() {
                    if b[i] == b'%' {
                        bytes.push(
                            u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap(), 16)
                                .unwrap(),
                        );
                        i += 3;
                    } else {
                        bytes.push(b[i]);
                        i += 1;
                    }
                }
                if let Ok(v) = serde_json::from_slice(&bytes) {
                    return Some((format!("{base}config={}{}", "{}", &config[end..]), Some(v)));
                }
            }
            Some((uri.into(), None))
        })
        .collect()
}

#[test]
fn c_osc_payloads() {
    let mut p = palette();
    p.colors.insert("brand-blue".into(), [32, 96, 192]);
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/text/c_compatibility.json")).unwrap();
    let mut failures = Vec::new();
    for f in fixtures.iter().filter(|f| f.mode == "render_options") {
        let options = RenderOptions {
            capabilities: f.capabilities.iter().cloned().collect(),
            ..Default::default()
        };
        let actual =
            String::from_utf8(Document::Styled(f.source.clone()).telnet(&p, &options, 65536))
                .unwrap();
        if osc(&actual) != osc(f.expected.as_ref().unwrap()) {
            failures.push(format!(
                "{}\nactual {:?}\nexpected {:?}",
                f.reference,
                osc(&actual),
                osc(f.expected.as_ref().unwrap())
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn markdown_tables_code_wrapping_and_html_actions() {
    let p = Palette::default();
    let d=Document::markdown("| Name | Notes |\n| --- | --- |\n| 界 | several words |\n\n```text\n  [red]  spaces\n```\n\n[Topic](flags/ansi.md)".into(),4096).unwrap();
    let render = |width| {
        String::from_utf8(d.telnet(
            &p,
            &RenderOptions {
                width,
                ..Default::default()
            },
            8192,
        ))
        .unwrap()
    };
    let wide = render(80);
    assert!(wide.contains("Name |"));
    assert!(wide.contains("界"));
    let narrow = render(12);
    assert!(narrow.contains("Name: 界"));
    assert!(narrow.contains("Notes:"));
    assert!(narrow.contains("  [red]  spaces"));
    let html = d.html(&p, 8192).unwrap();
    assert!(html.contains("<table>"));
    assert!(html.contains("data-mux-command=\"help flags/ansi\""));
    assert!(d.html(&p, 8).is_err());
    let styled = Document::Styled(
        "[send=\"say \\\"hi\\\"\" tooltip=\"<script>\" color=red]Action[/]".into(),
    );
    let html = styled.html(&p, 8192).unwrap();
    assert!(html.contains("data-mux-action=\"send\""));
    assert!(html.contains("color:#ff0000"));
    assert!(!html.contains("<script>"));
}

#[test]
fn preset_merging_capability_fallback_and_escaping() {
    let p = palette();
    let d = Document::Styled("[send=\"look\" preset=\"osc8-demo-button\" bg=blue]Look[/]".into());
    let o = RenderOptions::all();
    let text = String::from_utf8(d.telnet(&p, &o, 8192)).unwrap();
    let links = osc(&text);
    assert!(links[0].0.contains("preset=osc8-demo-button"));
    assert_eq!(links[0].1.as_ref().unwrap()["s"]["bg"], "#0000ff");
    let mut o = o;
    o.capabilities.remove("PRESETS");
    o.capabilities.remove("COMPACT");
    let text = String::from_utf8(d.telnet(&p, &o, 8192)).unwrap();
    let links = osc(&text);
    let config = links[0].1.as_ref().unwrap();
    assert_eq!(config["style"]["bg"], "#0000ff");
    assert_eq!(config["style"]["bold"], true);
    let d = Document::Styled(
        "[link=\"https://example.com/?config=old&preset=old#x\" color=red]x[/]".into(),
    );
    let text = String::from_utf8(d.telnet(&p, &RenderOptions::all(), 8192)).unwrap();
    assert!(text.contains("%63%6F%6E%66%69%67=old"));
    assert!(text.contains("%70%72%65%73%65%74=old"));
}

#[tokio::test]
async fn lua_documents_are_immutable_bounded_and_palette_aware() {
    use std::{cell::RefCell, rc::Rc};
    let c =
        stompymux_rs::Config::load(crate::support::repository_root().join("tests/fixtures/game"))
            .unwrap();
    let world = stompymux_rs::persistence::load(&c.database())
        .await
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("stompymux.toml"),
        format!(
            "include = [{:?}]\n[lua]\ndirectory = {:?}\noutput_byte_limit = 256\n",
            c.root.join("stompymux.toml").to_str().unwrap(),
            c.lua_dir().to_str().unwrap()
        ),
    )
    .unwrap();
    let c = stompymux_rs::Config::load(temp.path()).unwrap();
    let s = stompymux_rs::Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
    s.inspect_lua().load(r#"
      local d=mux.text.markdown('**value**')
      assert(not pcall(function() d.source='changed' end))
      mux.world.pemit(mux.world.object(2),d)
      assert(mux.text.strip_style('[fg=red]red[/]')=='red')
      -- C styled_text_width measures plain-render bytes: U+754C is three bytes.
      assert(mux.text.width('[bold]界[/]')==3)
      assert(mux.text.markup('[bold]ok[/]')=='[bold]ok[/]')
      assert(not pcall(mux.text.markup,'[bad]x[/]'))
      assert(not pcall(mux.text.markup,string.char(255)))
      assert(not mux.text.is_printable_ascii('界'))
      local docs={}; local failed=false
      for i=1,10 do local ok,v=pcall(mux.text.markdown,string.rep('x',80));if not ok then failed=true;break end;docs[i]=v end
      assert(failed)
    "#).exec().unwrap();
    assert!(matches!(&s.outbox()[0].1, Document::Markdown(_)));
}

#[test]
fn ansi_depths_and_word_boundaries() {
    let p = Palette::default();
    for (depth, fg, bg) in [
        (ColorDepth::Ansi16, "\x1b[91m", "\x1b[104m"),
        (ColorDepth::Ansi256, "\x1b[38;5;9m", "\x1b[48;5;12m"),
        (
            ColorDepth::Truecolor,
            "\x1b[38;2;255;0;0m",
            "\x1b[48;2;0;0;255m",
        ),
    ] {
        let options = RenderOptions {
            color: depth,
            ..Default::default()
        };
        let bytes = Document::Styled("[fg=red bg=blue]R[/]".into()).telnet(&p, &options, 1024);
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains(fg));
        assert!(s.contains(bg));
        assert!(s.ends_with("\x1b[0m"));
        let raw =
            String::from_utf8(Document::Styled("\x1b[31mR".into()).telnet(&p, &options, 1024))
                .unwrap();
        assert!(raw.contains("\x1b[31m"));
    }
    let d = Document::markdown("abc def ghi".into(), 128).unwrap();
    let bytes = d.telnet(
        &p,
        &RenderOptions {
            width: 3,
            ..Default::default()
        },
        1024,
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "abc\r\ndef\r\nghi\r\n\r\n"
    );
}

#[tokio::test]
async fn rendered_unicode_survives_mccp2_streaming() {
    use std::io::Read;
    use tokio::io::AsyncReadExt;
    let doc = Document::Styled("[send=\"look\" fg=red]界é👩‍👩‍👧‍👦[/]".into());
    let expected = doc.telnet(&Palette::default(), &RenderOptions::all(), 4096);
    let (mut server, mut client) = tokio::io::duplex(8192);
    let stats = stompymux_rs::telnet::transport::Stats::default();
    let mut writer = stompymux_rs::telnet::transport::Writer::default();
    writer.start(&mut server, &stats).await.unwrap();
    writer.bytes(&mut server, &stats, &expected).await.unwrap();
    writer.bytes(&mut server, &stats, &expected).await.unwrap();
    writer.finish(&mut server, &stats).await.unwrap();
    drop(server);
    let mut wire = Vec::new();
    client.read_to_end(&mut wire).await.unwrap();
    assert_eq!(&wire[..5], b"\xff\xfa\x56\xff\xf0");
    let mut plain = Vec::new();
    flate2::read::ZlibDecoder::new(&wire[5..])
        .read_to_end(&mut plain)
        .unwrap();
    assert_eq!(plain, [expected.clone(), expected].concat());
}

#[test]
fn help_index_visibility_order_and_relative_topics() {
    let temp = tempfile::tempdir().unwrap();
    let source = std::fs::canonicalize(
        crate::support::repository_root().join("tests/fixtures/game/stompymux.toml"),
    )
    .unwrap();
    std::fs::write(
        temp.path().join("stompymux.toml"),
        format!("include=[{:?}]\n", source.to_str().unwrap()),
    )
    .unwrap();
    let help = temp.path().join("help");
    std::fs::create_dir(&help).unwrap();
    std::fs::write(help.join("index.md"),"+++\ntitle='Index'\ndescription='Index'\nkeywords=['help']\nshow_index_for_article_tags=['root']\n+++\n# Index\n").unwrap();
    for (path, keyword, weight, private) in [
        ("one", "First", 1, false),
        ("two", "Second", 2, false),
        ("secret", "Secret", 0, true),
    ] {
        std::fs::write(help.join(format!("{path}.md")),format!("+++\ntitle='{keyword}'\ndescription='{keyword} description'\nkeywords=['{keyword}']\narticle_tags=['root']\nweight={weight}\nwizard_only={private}\n+++\n{keyword} body\n")).unwrap();
    }
    let config = stompymux_rs::Config::load(temp.path()).unwrap();
    let index = stompymux_rs::help::HelpIndex::load(&config).unwrap();
    let lookup = |topic: &str, wizard: bool| {
        index
            .lookup(topic, wizard)
            .unwrap()
            .spans(&RenderOptions::default())
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>()
    };
    let regular = lookup("", false);
    assert!(!regular.as_str().contains("Secret"));
    assert!(regular.as_str().find("First").unwrap() < regular.as_str().find("Second").unwrap());
    let wizard = lookup("", true);
    assert!(wizard.as_str().find("Secret").unwrap() < wizard.as_str().find("First").unwrap());
    assert!(lookup("FIRST", false).as_str().contains("First body"));
    assert!(lookup("one", false).as_str().contains("First body"));
    assert!(lookup("secret", false).as_str().contains("No help found"));
    assert!(lookup("sec", false).as_str().contains("No exact match")); // Second remains visible.
    assert!(!lookup("sec", false).as_str().contains("secret"));
}

#[test]
fn invalid_palette_has_configuration_source_and_limits() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("stompymux.toml");
    std::fs::write(&root, "[colors]\nred=[1,2,3]\n").unwrap();
    let error = stompymux_rs::Config::load(temp.path()).unwrap_err();
    assert!(format!("{error:#}").contains("stompymux.toml"));
    assert!(format!("{error:#}").contains("colors.red"));
    std::fs::write(&root, "[osc8.presets]\nbutton='mystery=true'\n").unwrap();
    let error = stompymux_rs::Config::load(temp.path()).unwrap_err();
    assert!(format!("{error:#}").contains("osc8.presets.button"));
    let p = Palette::default();
    for s in [
        "[fg=rgb(+1,2,3)]x[/]",
        "[send=\"x\" visibility.action=conceal visibility.delay=+1]x[/]",
        "[send=\"x\" blink=true]x[/]",
    ] {
        assert!(text::validate(&p, s).is_err(), "{s}");
    }
    assert!(text::validate(&p, "[UNDERLINE=WAVY BOLD=TRUE]x[/]").is_ok());
}

#[test]
fn native_styled_preserves_redundant_and_textless_controls() {
    let document = Document::NativeStyled("[fg=black bold][fg=black bold]x[reset]".to_owned());
    let output = document.telnet(
        &Palette::default(),
        &RenderOptions {
            color: ColorDepth::Ansi16,
            ..Default::default()
        },
        1024,
    );
    assert_eq!(
        output,
        b"\x1b[0m\x1b[1m\x1b[30m\x1b[0m\x1b[1m\x1b[30mx\x1b[0m\x1b[0m"
    );
    assert_eq!(
        document.html(&Palette::default(), 1024).unwrap(),
        Document::Styled(document.source().to_owned())
            .html(&Palette::default(), 1024)
            .unwrap()
    );
}

#[test]
fn native_styled_truncation_reserves_the_final_reset() {
    let document = Document::NativeStyled("[fg=red]abcdef".to_owned());
    let options = RenderOptions {
        color: ColorDepth::Ansi16,
        ..Default::default()
    };
    let output = document.telnet(&Palette::default(), &options, 14);
    assert_eq!(output, b"\x1b[0m\x1b[91ma\x1b[0m");
    assert!(output.len() <= 14);
    assert!(output.ends_with(b"\x1b[0m"));
}

#[test]
fn native_styled_forwarding_preserves_explicit_controls() {
    let document = Document::native_styled("[bold][bold]report[/]".into(), 64)
        .unwrap()
        .prefixed("From here, ");
    assert_eq!(
        String::from_utf8(document.telnet(
            &Palette::default(),
            &RenderOptions {
                color: ColorDepth::Ansi16,
                ..Default::default()
            },
            128,
        ))
        .unwrap(),
        "From here, \x1b[0m\x1b[1m\x1b[0m\x1b[1mreport\x1b[0m\x1b[1m\x1b[0m"
    );
}

#[test]
fn native_styled_rejects_oversized_sources_at_construction() {
    assert!(Document::native_styled("12345".into(), 4).is_err());
}

#[test]
fn native_styled_forwarding_closes_prefix_style_before_plain_body() {
    let document = Document::native_styled("report".into(), 64)
        .unwrap()
        .prefixed("[bold]From here, ");
    let ansi = document.telnet(
        &Palette::default(),
        &RenderOptions {
            color: ColorDepth::Ansi16,
            ..Default::default()
        },
        128,
    );
    assert_eq!(ansi, b"\x1b[0m\x1b[1mFrom here, \x1b[0mreport\x1b[0m");
    assert_eq!(
        document.telnet(&Palette::default(), &RenderOptions::default(), 128),
        b"From here, report"
    );
}
