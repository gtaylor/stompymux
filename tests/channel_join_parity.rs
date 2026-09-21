//! Focused C-parity checks for native channel join policy and diagnostics.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    Config, Flag, ObjectId, Scripts,
    commands::{self, Action},
};

use crate::support;
use support::isolated_world;

/// Load an isolated world with the ordinary test player connected.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (directory, config, mut world) = isolated_world().await;
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
        .drain_outbox()
        .into_iter()
        .filter(|(recipient, _)| *recipient == player)
        .map(|(_, document)| document.source().to_owned())
        .collect::<Vec<_>>();
    if let Action::Report(commands::Report::Reply(message))
    | Action::CommitReply(message)
    | Action::Report(commands::Report::Inspection(message)) = action
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
            .world()
            .channel_aliases
            .get(&player)
            .is_none_or(|aliases| aliases.iter().all(|alias| alias.alias != "c"))
    );
    assert!(
        scripts.world().channels["Closed"]
            .users
            .iter()
            .all(|member| member.who != player)
    );

    // A missing channel-object JOIN lock grants access independently of closed flags.
    scripts
        .world_mut()
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
