//! World preparation, bootstrap persistence, and listener startup.
use super::*;
use crate::Account;

pub async fn prepare(c: &Config) -> Result<Scripts> {
    c.validate_for_serve()?;
    let existing = c.database().exists();
    let world = if existing {
        let path = c.database();
        let timeout = c.database.busy_timeout_ms;
        let mut loaded = persistence::load_with_timeout(&path, timeout).await?;
        loaded.palette = std::sync::Arc::new(crate::text::Palette::from_config(c)?);
        loaded.validate(c)?;
        persistence::validate_lists(&path, &loaded, timeout).await?;
        loaded
    } else {
        World::default()
    };
    let world = Rc::new(RefCell::new(world));
    if !existing {
        let mut w = world.borrow_mut();
        for (dbref, entry) in &c.database.bootstrap.objects {
            w.next_id = dbref.0;
            let kind = match entry.r#type {
                crate::config::BootstrapKind::Room => Kind::Room,
                crate::config::BootstrapKind::Player => Kind::Player,
            };
            let id = w.create_with(
                c,
                entry.name.clone(),
                kind,
                crate::CreationContext::Bootstrap,
            )?;
            if kind == Kind::Player {
                let object = w.objects.get_mut(&id).unwrap();
                object.location = Some(ObjectId(c.start()));
                object.home = Some(ObjectId(c.home()));
                if entry.wizard {
                    object.flags.insert(crate::flags::Flag::Wizard);
                }
            }
        }
    }
    // Seed imported characters and assigned pilots before startup's commit admits gameplay.
    {
        let mut loaded = world.borrow_mut();
        let players: std::collections::BTreeSet<_> = loaded
            .btech
            .characters()
            .keys()
            .copied()
            .chain(
                loaded
                    .btech
                    .constructed_units()
                    .values()
                    .filter_map(|unit| unit.pilot()),
            )
            .filter(|id| {
                loaded.objects.get(id).is_some_and(|object| {
                    object.kind == Kind::Player && !object.flags.contains(crate::Flag::Going)
                })
            })
            .collect();
        for player in players {
            crate::prepare_battle_recovery(&mut loaded, player)?;
        }
    }
    let help_config = c.clone();
    let help =
        tokio::task::spawn_blocking(move || crate::help::HelpIndex::load(&help_config)).await??;
    let source_config = c.clone();
    let sources =
        tokio::task::spawn_blocking(move || crate::lua::sources::Sources::read(&source_config))
            .await??;
    let scripts = Scripts::from_sources(
        c,
        world,
        help,
        std::sync::Arc::new(sources),
        crate::lua::RuntimeMode::Live,
    )?;
    if !existing {
        let god = Zeroizing::new(accounts::random_password());
        let wizard = Zeroizing::new(accounts::random_password());
        let cc = c.clone();
        let g = god.clone();
        let z = wizard.clone();
        let (gh, zh) = tokio::task::spawn_blocking(move || -> Result<_> {
            Ok((accounts::hash(&g, &cc)?, accounts::hash(&z, &cc)?))
        })
        .await??;
        {
            let mut w = scripts.world.borrow_mut();
            w.accounts.insert(
                ObjectId(1),
                Account {
                    hash: Some(gh),
                    ..Default::default()
                },
            );
            w.accounts.insert(
                ObjectId(2),
                Account {
                    hash: Some(zh),
                    ..Default::default()
                },
            );
        }
        scripts.event("on_server_first_startup", None, None)?;
        scripts.world.borrow_mut().initialized = true;
        scripts.world.borrow().validate(c)?;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let credentials = c.path(&c.database.bootstrap.credentials_file);
        if let Some(parent) = credentials.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&credentials)
            .context("cannot create bootstrap credentials; move any stale file before retrying")?;
        f.write_all(format!("GOD: {}\nWizard: {}\n", god.as_str(), wizard.as_str()).as_bytes())?;
        f.sync_all()?;
        let path = c.database();
        let snapshot = scripts.world.borrow().clone();
        let busy_timeout_ms = c.database.busy_timeout_ms;
        if let Err(e) =
            persistence::initialize_with_timeout(&path, &snapshot, busy_timeout_ms).await
        {
            let _ = std::fs::remove_file(&credentials);
            return Err(e);
        }
        scripts.effects.drain_maintenance();
        for request in scripts.effects.drain_logs() {
            c.logger.submit(c, request);
        }
        (c).log(
            &[crate::logging::Category::Startup],
            "INI",
            "INFO",
            format!("Bootstrap credentials written to {}", credentials.display()),
        );
    }
    scripts.world.borrow().validate(c)?;
    scripts.event("on_server_startup", None, None)?;
    let after = scripts.world.borrow().clone();
    after.validate(c)?;
    after.validate_player_zone(c)?;
    // Unchanged durable fields are not rewritten.
    let saved = persistence::persist_effects(
        &c.database(),
        &after,
        c.database.busy_timeout_ms,
        scripts.effects.maintenance(),
        c.database.clock_save_interval,
    )
    .await?;
    scripts.world.borrow_mut().links = saved.links;
    if let Some(report) = scripts.effects.drain_maintenance() {
        for finding in report.findings {
            c.log(
                &[crate::logging::Category::Checkpoints],
                "DB",
                "CHECK",
                finding,
            );
        }
    }
    scripts.outbox.borrow_mut().clear();
    for request in scripts.effects.drain_logs() {
        c.logger.submit(c, request);
    }
    Ok(scripts)
}
pub async fn serve(c: Config, shutdown: impl Future<Output = ShutdownRequest>) -> Result<()> {
    let address = c.listener();
    let scripts = prepare(&c).await?;
    let listener = TcpListener::bind(address).await?;
    run(c, scripts, listener, shutdown).await
}
