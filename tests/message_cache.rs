//! File-cache publication, bounded reads and last-good reload behavior on temporary content.
use stompymux_rs::{
    config::Config,
    message_cache::{File, MessageCache},
};
#[test]
fn cache_refresh_preserves_failed_entries_and_normalizes_text() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("text")).unwrap();
    std::fs::create_dir(d.path().join("banners")).unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        "mux.connect_dir='banners'\nlua.output_byte_limit=512",
    )
    .unwrap();
    for file in ["connect", "badsite", "down", "full", "quit"] {
        std::fs::write(
            d.path().join(format!("text/{file}.txt")),
            format!("{file}\r\n\0"),
        )
        .unwrap();
    }
    for (file, text) in [
        ("b.txt", "second"),
        ("a.txt.extra", "first"),
        (".hidden.txt", "hidden"),
        ("ignore.md", "ignored"),
    ] {
        std::fs::write(d.path().join("banners").join(file), text).unwrap();
    }
    let c = Config::load(d.path()).unwrap();
    let (old, _) = MessageCache::default().reload(&c);
    assert_eq!(old.text(File::Connect), "connect\n");
    assert_eq!(old.banner_count(), 2);
    assert_eq!(old.welcome(0), "first");
    assert_eq!(old.welcome(1), "second");
    std::fs::write(d.path().join("text/down.txt"), [255]).unwrap();
    std::fs::write(d.path().join("text/full.txt"), "new full").unwrap();
    std::fs::remove_file(d.path().join("banners/b.txt")).unwrap();
    let (new, report) = old.reload(&c);
    assert_eq!(old.text(File::Full), "full\n");
    assert_eq!(new.text(File::Full), "new full");
    assert_eq!(new.text(File::Down), "down\n");
    assert!(report.iter().any(|r| r.contains("invalid UTF-8")));
    assert_eq!(new.banner_count(), 1);
    std::fs::remove_dir_all(d.path().join("banners")).unwrap();
    let (retained, _) = new.reload(&c);
    assert_eq!(retained.banner_count(), 1);
    std::fs::write(d.path().join("text/full.txt"), "x".repeat(600)).unwrap();
    let (bounded, _) = retained.reload(&c);
    assert_eq!(bounded.text(File::Full), "new full");
    for file in ["connect", "badsite", "down", "full", "quit"] {
        std::fs::write(d.path().join(format!("text/{file}.txt")), "x".repeat(150)).unwrap();
    }
    let (bounded, report) = retained.reload(&c);
    assert_eq!(bounded.text(File::Full), "new full");
    assert!(report.iter().any(|r| r.contains("aggregate")));
}
