//! Flow state-machine, callback boundary and VM-replacement regression tests.
use super::*;
use crate::{
    config::Config,
    lua::{Scripts, sources::Sources},
    world::ObjectId,
};
use std::{path::Path, sync::Arc};

async fn setup(source: &str, change: impl FnOnce(&mut Config)) -> (Config, Scripts) {
    let mut c =
        Config::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game")).unwrap();
    change(&mut c);
    let world = Rc::new(RefCell::new(
        crate::persistence::load(&c.database()).await.unwrap(),
    ));
    let mut sources = Sources::read(&c).unwrap();
    sources
        .files
        .insert("global_logic/probe.lua".into(), source.into());
    let s = Scripts::from_sources(
        &c,
        world,
        crate::help::HelpIndex::load(&c).unwrap(),
        Arc::new(sources),
        RuntimeMode::Live,
    )
    .unwrap();
    s.flows.sessions(
        [1, 2, 3]
            .into_iter()
            .map(|session| {
                (
                    session,
                    Identity {
                        player: ObjectId(1),
                        generation: s.world.borrow().objects[&ObjectId(1)].generation,
                    },
                )
            })
            .collect(),
    );
    (c, s)
}
fn start(s: &Scripts, session: u64, step: &str) -> anyhow::Result<()> {
    with_root(&s.lua, "global_logic/probe.lua", || {
        s.eval_callback(&format!(
            "mux.session.flow_start({session}, 'probe.lua', '{step}')"
        ))
    })
}
fn output(s: &Scripts) -> String {
    s.effects
        .drain_private()
        .into_iter()
        .map(|o| o.document.source().to_string())
        .collect::<Vec<_>>()
        .join("")
}

#[tokio::test]
async fn contexts_input_scratch_outcomes_and_order() {
    let (_, s) = setup(
        r#"return {flows={
      first=function(ctx)
        assert(ctx.scope=='flow' and ctx.enactor==1 and ctx.cause==1 and ctx.descriptor==1)
        assert(ctx.object==nil and ctx.input==nil)
        ctx.flow.number=42; ctx.flow.bytes=string.char(255)
        mux.world.pemit(1,'before')
        return {action='goto',step='second',message='transition'}
      end,
      second=function(ctx)
        assert(ctx.flow.number=='42' and ctx.flow.bytes==string.char(255))
        if ctx.input==nil then
          mux.world.pemit(1,'between')
          return {prompt='question ',message='ignored'}
        end
        if ctx.input=='' then return {} end
        assert(ctx.input=='  quit  ')
        return {action='done',message='finished'}
      end}}
    "#,
        |_| {},
    )
    .await;
    start(&s, 1, "first").unwrap();
    let pending = s.effects.drain_private();
    assert_eq!(pending.iter().map(|p| p.after).collect::<Vec<_>>(), [1, 2]);
    assert!(pending[0].document.contains("transition"));
    assert!(pending[1].document.contains("question"));
    assert!(!pending[1].document.contains("ignored"));
    s.effects.commit();
    s.outbox.borrow_mut().clear();
    s.flow_input(1, "").unwrap();
    assert!(output(&s).contains("question"));
    s.flow_input(1, "  quit  ").unwrap();
    assert_eq!(output(&s), "finished");
    assert!(!s.flows.active(1));
}

#[tokio::test]
async fn caught_nested_failure_restores_world_output_and_all_flow_effects() {
    let (_, s) = setup(
        r#"return {flows={
      good=function(ctx) return {prompt='good'} end,
      bad=function(ctx)
        mux.session.flow_start(2,'probe.lua','good')
        mux.world.object(1):state('flow'):set('bad',true)
        mux.world.pemit(1,'must disappear')
        ctx.flow.invalid={}
        return {prompt='bad'}
      end}}
    "#,
        |_| {},
    )
    .await;
    with_root(&s.lua, "global_logic/probe.lua", || {
        s.eval_callback::<()>(
            r#"
      local ok,e=pcall(mux.session.flow_start,1,'probe.lua','bad')
      assert(not ok and mux.error.is(e,'mux.runtime'))
      assert(not mux.world.object(1):state('flow'):has('bad'))
    "#,
        )
    })
    .unwrap();
    assert!(!s.flows.active(1) && !s.flows.active(2));
    assert!(s.outbox.borrow().is_empty());
    assert!(output(&s).is_empty());
    start(&s, 1, "good").unwrap();
    let err = start(&s, 1, "good").unwrap_err().to_string();
    assert!(err.contains("connection.unavailable"), "{err}");
    assert!(
        start(&s, 99, "good")
            .unwrap_err()
            .to_string()
            .contains("connection.invalid")
    );
}

#[tokio::test]
async fn scratch_and_transition_limits_are_atomic() {
    for mutation in [
        "ctx.flow[string.rep('k',32)]='x'",
        "ctx.flow.x=string.rep('v',8192)",
        "ctx.flow.x=0/0",
        "ctx.flow.x=true",
        "ctx.flow.x='x'..string.char(0)",
        "ctx.flow[1]='x'",
        "for i=1,17 do ctx.flow['k'..i]='v' end",
    ] {
        let source = format!(
            "return {{flows={{step=function(ctx) {mutation}; return {{prompt='bad'}} end}}}}"
        );
        let (_, s) = setup(&source, |_| {}).await;
        assert!(start(&s, 1, "step").is_err(), "{mutation}");
        assert!(!s.flows.active(1));
        assert!(output(&s).is_empty());
    }
    let (_, s) = setup(
        "return {flows={step=function(ctx) return {action='goto',step='step',message='loop'} end}}",
        |_| {},
    )
    .await;
    assert!(
        start(&s, 1, "step")
            .unwrap_err()
            .to_string()
            .contains("32 immediate")
    );
    assert!(output(&s).is_empty());
    let (_, s) = setup(
        "return {flows={step=function(ctx) while true do end end}}",
        |_| {},
    )
    .await;
    assert!(
        start(&s, 1, "step")
            .unwrap_err()
            .to_string()
            .contains("instruction budget")
    );
}

#[tokio::test]
async fn output_limits_include_messages_after_prompts() {
    let (_,s)=setup(r#"return {flows={
      second=function() return {prompt='1234567890'} end,
      first=function() mux.session.flow_start(2,'probe.lua','second'); mux.world.pemit(1,'abcdefghijklmnopqrst'); return {action='done'} end}}
    "#, |c| {
        let d=tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("stompymux.toml"),format!("include = [\"{}/stompymux.toml\"]\n[lua]\noutput_byte_limit=30\n", c.root.display())).unwrap();
        let root=c.root.clone(); *c=Config::load(d.path()).unwrap(); c.root=root;
    }).await;
    assert!(start(&s, 1, "first").is_err());
    assert!(!s.flows.active(2));
    assert!(output(&s).is_empty());
}

#[tokio::test]
async fn committed_retry_reload_and_disconnect_identity() {
    let (c, s) = setup(
        r#"return {flows={step=function(ctx)
      if ctx.input==nil then ctx.flow.counter=1; return {prompt='old'} end
      ctx.flow.counter=tonumber(ctx.flow.counter)+1; return {prompt=ctx.flow.counter..ctx.input}
    end}}"#,
        |_| {},
    )
    .await;
    start(&s, 1, "step").unwrap();
    s.effects.commit();
    output(&s);
    s.flow_input(1, "retry").unwrap();
    s.effects.rollback();
    s.flow_input(1, "retry").unwrap();
    assert!(output(&s).contains("2retry"));
    s.effects.rollback();
    let mut sources = (*s.sources).clone();
    sources.files.insert("global_logic/probe.lua".into(),r#"return {flows={step=function(ctx) assert(ctx.flow.counter=='1');return {action='done',message='new '..ctx.input} end}}"#.into());
    let next = Scripts::from_sources(
        &c,
        s.world.clone(),
        s.help.clone(),
        Arc::new(sources),
        RuntimeMode::Live,
    )
    .unwrap();
    next.effects.inherit(&s.effects);
    next.flow_input(1, "code").unwrap();
    assert_eq!(output(&next), "new code");
    let saved = s.effects.checkpoint();
    s.flows.sessions(Default::default());
    s.effects.restore(saved);
    s.effects.rollback();
    assert!(!s.flows.active(1));
}

#[tokio::test]
async fn removed_steps_and_initialization_validation() {
    let (c, s) = setup(
        "return {flows={step=function() return {prompt='ready'} end}}",
        |_| {},
    )
    .await;
    start(&s, 1, "step").unwrap();
    s.effects.commit();
    output(&s);
    let mut sources = (*s.sources).clone();
    sources.files.remove("global_logic/probe.lua");
    let next = Scripts::from_sources(
        &c,
        s.world.clone(),
        s.help.clone(),
        Arc::new(sources),
        RuntimeMode::Live,
    )
    .unwrap();
    next.effects.inherit(&s.effects);
    assert!(
        next.flow_input(1, "x")
            .unwrap_err()
            .to_string()
            .contains("probe.lua is unavailable")
    );
    for source in [
        "mux.session.flow_start(1,'probe.lua','step'); return {}",
        "return {flows={bad=42}}",
        "return {flows={[1]=function() end}}",
    ] {
        let mut sources = (*s.sources).clone();
        sources
            .files
            .insert("global_logic/probe.lua".into(), source.into());
        assert!(
            Scripts::from_sources(
                &c,
                s.world.clone(),
                s.help.clone(),
                Arc::new(sources),
                RuntimeMode::Live
            )
            .is_err()
        );
    }
    assert!(
        s.eval_callback::<()>("mux.session.flow_start(1,'../bad','step')")
            .is_err()
    );
}

#[tokio::test]
async fn hosted_tests_background_calls_and_immutable_error_codes() {
    let (c, s) = setup(
        r#"return {flows={step=function(ctx)
        if ctx.input==nil then return {prompt='hosted'} end
        return {action='done',message=ctx.input}
    end}}"#,
        |_| {},
    )
    .await;
    let mut testing = Scripts::from_sources(
        &c,
        s.world.clone(),
        s.help.clone(),
        s.sources.clone(),
        RuntimeMode::Testing,
    )
    .unwrap();
    assert!(
        start(&testing, 2, "step")
            .unwrap_err()
            .to_string()
            .contains("unavailable.checking")
    );
    testing.host_effects(&s);
    // No dynamically scoped descriptor: the explicitly named authenticated connection is sufficient.
    start(&testing, 2, "step").unwrap();
    assert!(s.flows.active(2));
    assert!(output(&testing).contains("hosted"));
    s.effects.commit();
    s.flow_input(2, "resumed on active VM").unwrap();
    assert_eq!(output(&s), "resumed on active VM");
    s.eval_callback::<()>(
        r#"
      local codes=mux.error.codes.connection
      assert(tostring(codes.invalid)=='mux.connection.invalid')
      local invalid=codes.invalid
      codes.invalid='fake';assert(codes.invalid=='fake');codes.invalid=invalid
      assert(not pcall(function() codes.missing='fake' end))
      local ok,e=pcall(mux.session.flow_start,99,'probe.lua','step')
      assert(not ok and mux.error.is(e,codes.invalid))
    "#,
    )
    .unwrap();
}

#[tokio::test]
async fn nested_starts_invalid_outcomes_and_prompt_limits() {
    let (_, s) = setup(
        r#"return {flows={step=function(ctx)
        mux.session.flow_start(ctx.descriptor+1,'probe.lua','step')
        return {prompt='must disappear'}
    end}}"#,
        |_| {},
    )
    .await;
    let identity = s.effects.session(1).unwrap();
    s.flows
        .sessions((1..=40).map(|id| (id, identity)).collect());
    assert!(
        start(&s, 1, "step")
            .unwrap_err()
            .to_string()
            .contains("nested-start limit")
    );
    assert!((1..=40).all(|session| !s.flows.active(session)));
    for outcome in [
        "nil",
        "{action='unknown'}",
        "{action='goto',step='absent'}",
        "{action='goto',step=string.rep('s',64)}",
        "{prompt=string.rep('p',8192)}",
        "{message=false}",
    ] {
        let (_, s) = setup(
            &format!("return {{flows={{step=function() return {outcome} end}}}}"),
            |_| {},
        )
        .await;
        assert!(start(&s, 1, "step").is_err(), "{outcome}");
        assert!(!s.flows.active(1));
    }
}

#[tokio::test]
async fn object_root_resolution_dynamic_steps_and_shutdown() {
    let (c, s) = setup(
        "return {flows={step=function() return {prompt='global'} end}}",
        |_| {},
    )
    .await;
    let mut sources = (*s.sources).clone();
    sources.files.insert(
        "object_logic/probe.lua".into(),
        r#"local m={};m.flows={step=function(ctx)
        if ctx.input==nil then
            m.flows.step=function() return {action='done',message='dynamic'} end
            return {prompt='object'}
        end
        error('stale function')
    end};return m"#
            .into(),
    );
    let object = Scripts::from_sources(
        &c,
        s.world.clone(),
        s.help.clone(),
        Arc::new(sources),
        RuntimeMode::Live,
    )
    .unwrap();
    object.effects.inherit(&s.effects);
    let f = object
        .lua
        .load("mux.session.flow_start(1,'probe.lua','step')")
        .set_name("object_logic/launcher.lua")
        .into_function()
        .unwrap();
    object.call::<()>(&f, ()).unwrap();
    assert!(output(&object).contains("object"));
    object.flow_input(1, "x").unwrap();
    assert_eq!(output(&object), "dynamic");
    start(&object, 2, "step").unwrap();
    object.effects.commit();
    object.flows.stop();
    object.effects.rollback();
    assert!(!object.flows.active(2));
    assert!(start(&object, 1, "step").is_err());
}
