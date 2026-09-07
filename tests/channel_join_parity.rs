//! Focused C-parity checks for native channel join policy and diagnostics.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action},
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence,
    world::ObjectId,
};

/// Copy the checked-in fixture so the test never writes to the production game.
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

/// Load an isolated world with the ordinary test player connected.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let directory = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        directory.path(),
    );
    let config = Config::load(directory.path()).unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    (directory, config, scripts)
}

/// Return command text, including the command-private error response.
fn run(scripts: &Scripts, config: &Config, player: ObjectId, command: &str) -> String {
    let action = commands::run(scripts, config, player, 1, command).unwrap();
    let mut output = scripts
        .outbox
        .borrow_mut()
        .drain(..)
        .filter(|(recipient, _)| *recipient == player)
        .map(|(_, document)| document.source().to_owned())
        .collect::<Vec<_>>();
    if let Action::Reply(message) | Action::CommitReply(message) | Action::Report(message) = action
    {
        output.push(message);
    }
    output.join("\n")
}

#[tokio::test(flavor = "current_thread")]
async fn native_addcom_uses_c_closed_channel_denial_and_preserves_access_bypasses() {
    let (_directory, config, scripts) = fixture().await;
    let wizard = ObjectId(1);
    let player = ObjectId(2);

    run(&scripts, &config, wizard, "@chan/create Closed");
    run(&scripts, &config, wizard, "@chan/pflags Closed=!join");

    assert_eq!(
        run(&scripts, &config, player, "addcom c=Closed"),
        "Sorry, this channel type does not allow you to join."
    );
    assert!(
        scripts
            .world
            .borrow()
            .channel_aliases
            .get(&player)
            .is_none_or(|aliases| aliases.iter().all(|alias| alias.alias != "c"))
    );
    assert!(
        scripts.world.borrow().channels["Closed"]
            .users
            .iter()
            .all(|member| member.who != player)
    );

    // A missing channel-object JOIN lock grants access independently of closed flags.
    scripts
        .world
        .borrow_mut()
        .channels
        .get_mut("Closed")
        .unwrap()
        .object = Some(wizard);
    assert!(
        run(&scripts, &config, player, "addcom c=Closed")
            .contains("Channel Closed added with alias c.")
    );

    run(&scripts, &config, wizard, "@chan/create WizardClosed");
    run(&scripts, &config, wizard, "@chan/pflags WizardClosed=!join");
    assert!(
        run(&scripts, &config, wizard, "addcom wc=WizardClosed")
            .contains("Channel WizardClosed added with alias wc.")
    );

    config.logger.shutdown(&config).await.unwrap();
}
