//! Typed candidate effects and savepoints for serialized world transactions.

use crate::{
    config::Config,
    state::Generation,
    text::Document,
    world::{ObjectId, World},
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// World ownership shared by native services and the scripting adapter.
pub type SharedWorld = Rc<RefCell<World>>;

/// Transaction-staged object-directed output.
pub type Outbox = Rc<RefCell<Vec<(ObjectId, Document)>>>;

/// Player incarnation associated with a descriptor flow.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FlowIdentity {
    pub player: ObjectId,
    pub generation: Generation,
}

/// Plain candidate flow data independent of Lua module tables.
#[derive(Clone)]
pub(crate) struct ActiveFlow {
    pub identity: FlowIdentity,
    pub module: String,
    pub step: String,
    pub scratch: BTreeMap<Vec<u8>, Vec<u8>>,
    pub prompt: String,
}

/// Private descriptor output anchored among ordinary notifications.
#[derive(Clone)]
pub struct PrivateOutput {
    pub after: usize,
    pub session: u64,
    pub document: Document,
}

/// A complete candidate-effect savepoint for nested rollback.
#[derive(Clone, Default)]
pub struct EffectBatch {
    /// Require durable persistence even when the world candidate is unchanged.
    pub save_requested: bool,
    /// Object-directed output at the savepoint.
    pub normal: Vec<(ObjectId, Document)>,
    pub(crate) flows: BTreeMap<u64, ActiveFlow>,
    /// Descriptor-private output at the savepoint.
    pub private: Vec<PrivateOutput>,
    /// Deferred logfile appends.
    pub logs: Vec<crate::logging::FileRequest>,
    /// Categorized diagnostics published only after world commit.
    pub records: Vec<crate::logging::Record>,
    /// Prepared map replacements, published only after world commit.
    pub map_writes: Vec<super::MapAssetWrite>,
    /// Merged semantic maintenance request.
    pub maintenance: Option<crate::dbck::DbCheckReport>,
}

#[derive(Default)]
struct State {
    sessions: BTreeMap<u64, FlowIdentity>,
    durable: BTreeMap<u64, ActiveFlow>,
    pending: EffectBatch,
}

/// Shared candidate effects with session-aware savepoint restoration.
#[derive(Clone)]
pub struct Effects {
    state: Rc<RefCell<State>>,
    outbox: Outbox,
    config: Config,
}

impl Effects {
    /// Create an empty candidate-effect owner.
    pub fn new(config: &Config, outbox: &Outbox) -> Self {
        Self {
            state: Default::default(),
            outbox: outbox.clone(),
            config: config.clone(),
        }
    }

    /// Share durable/candidate flow state with a host while retaining this runtime's output.
    pub fn hosted(&self, host: &Self) -> Self {
        Self {
            state: host.state.clone(),
            ..self.clone()
        }
    }

    /// Publish authoritative descriptor identities and discard stale flow effects.
    pub fn sessions(&self, sessions: BTreeMap<u64, FlowIdentity>) {
        let mut state = self.state.borrow_mut();
        state.sessions = sessions;
        let sessions = state.sessions.clone();
        state
            .durable
            .retain(|id, flow| sessions.get(id) == Some(&flow.identity));
        state
            .pending
            .flows
            .retain(|id, flow| sessions.get(id) == Some(&flow.identity));
        state
            .pending
            .private
            .retain(|output| sessions.contains_key(&output.session));
    }

    /// Capture all staged effects without capturing authoritative sessions.
    pub fn checkpoint(&self) -> EffectBatch {
        let mut batch = self.state.borrow().pending.clone();
        batch.normal = self.outbox.borrow().clone();
        batch
    }

    /// Restore a savepoint while respecting sessions detached since capture.
    pub fn restore(&self, mut batch: EffectBatch) {
        let mut state = self.state.borrow_mut();
        batch
            .flows
            .retain(|id, flow| state.sessions.get(id) == Some(&flow.identity));
        batch
            .private
            .retain(|output| state.sessions.contains_key(&output.session));
        *self.outbox.borrow_mut() = std::mem::take(&mut batch.normal);
        state.pending = batch;
    }

    /// Publish candidate flow state after persistence succeeds.
    pub fn commit(&self) {
        let mut state = self.state.borrow_mut();
        state.durable = state.pending.flows.clone();
        state.pending.save_requested = false;
    }

    /// Coalesce explicit save requests into the enclosing world transaction.
    pub fn request_save(&self) {
        self.state.borrow_mut().pending.save_requested = true;
    }

    /// Whether the host must persist an otherwise unchanged candidate.
    pub fn save_requested(&self) -> bool {
        self.state.borrow().pending.save_requested
    }

    /// Return flow candidates to their durable state and discard other effects.
    pub fn rollback(&self) {
        self.outbox.borrow_mut().clear();
        let mut state = self.state.borrow_mut();
        state.pending = EffectBatch {
            flows: state.durable.clone(),
            ..Default::default()
        };
    }

    pub(crate) fn active(&self, session: u64) -> bool {
        self.state.borrow().pending.flows.contains_key(&session)
    }

    /// Clone one candidate flow for script step execution.
    pub(crate) fn flow(&self, session: u64) -> Option<ActiveFlow> {
        self.state.borrow().pending.flows.get(&session).cloned()
    }

    /// Replace a descriptor's candidate flow after a successful step.
    pub(crate) fn insert_flow(&self, session: u64, flow: ActiveFlow) {
        self.state.borrow_mut().pending.flows.insert(session, flow);
    }

    /// Remove and return a completed or cancelled candidate flow.
    pub(crate) fn remove_flow(&self, session: u64) -> Option<ActiveFlow> {
        self.state.borrow_mut().pending.flows.remove(&session)
    }

    /// Read an authoritative descriptor identity.
    pub(crate) fn session(&self, session: u64) -> Option<FlowIdentity> {
        self.state.borrow().sessions.get(&session).copied()
    }

    /// Abandon a flow and its private output without invoking completion steps.
    pub fn cancel(&self, session: u64) {
        let mut state = self.state.borrow_mut();
        state.pending.flows.remove(&session);
        state.durable.remove(&session);
        state
            .pending
            .private
            .retain(|output| output.session != session);
    }

    /// Stage a bounded logfile append in the current transaction.
    pub fn stage_log(&self, request: crate::logging::FileRequest, config: &Config) -> bool {
        let mut state = self.state.borrow_mut();
        let pending = &mut state.pending.logs;
        let bytes = pending
            .iter()
            .map(|entry| entry.filename.len() + entry.message.len())
            .sum::<usize>();
        if pending.len() >= config.lua.output_entry_limit
            || bytes.saturating_add(request.filename.len() + request.message.len())
                > config.lua.output_byte_limit
        {
            return false;
        }
        pending.push(request);
        true
    }

    /// Admit a categorized diagnostic under the aggregate transaction output budget.
    pub fn stage_record(&self, record: crate::logging::Record) -> anyhow::Result<()> {
        let before = self.checkpoint();
        self.state.borrow_mut().pending.records.push(record);
        if let Err(error) = self.validate() {
            self.restore(before);
            return Err(error);
        }
        Ok(())
    }

    /// Consume categorized diagnostics after successful persistence.
    pub fn drain_records(&self) -> Vec<crate::logging::Record> {
        std::mem::take(&mut self.state.borrow_mut().pending.records)
    }

    /// Stage a map replacement under the same savepoint and aggregate limits as other effects.
    pub fn stage_map_write(&self, request: super::MapAssetWrite) -> anyhow::Result<()> {
        let before = self.checkpoint();
        self.state.borrow_mut().pending.map_writes.push(request);
        if let Err(error) = self.validate() {
            self.restore(before);
            return Err(error);
        }
        Ok(())
    }

    /// Consume map replacements after the host has durably committed the world candidate.
    pub fn drain_map_writes(&self) -> Vec<super::MapAssetWrite> {
        std::mem::take(&mut self.state.borrow_mut().pending.map_writes)
    }

    /// Stop all interactive work and discard staged effects.
    pub fn stop(&self) {
        self.outbox.borrow_mut().clear();
        *self.state.borrow_mut() = State::default();
    }

    /// Read the currently staged maintenance plan.
    pub fn maintenance(&self) -> Option<crate::dbck::DbCheckReport> {
        self.state.borrow().pending.maintenance.clone()
    }

    /// Merge a maintenance plan into the bounded candidate effects.
    pub fn stage_maintenance(&self, mut report: crate::dbck::DbCheckReport) {
        let mut state = self.state.borrow_mut();
        if let Some(old) = state.pending.maintenance.take() {
            report.findings.splice(0..0, old.findings);
            report.plan.purges.extend(old.plan.purges);
            report.plan.detachments.extend(old.plan.detachments);
        }
        let mut bytes = 0usize;
        let mut entries = 0usize;
        report.findings.retain(|finding| {
            bytes = bytes.saturating_add(finding.len());
            entries += 1;
            entries <= self.config.lua.output_entry_limit
                && bytes <= self.config.lua.output_byte_limit
        });
        state.pending.maintenance = Some(report);
    }

    /// Consume the staged maintenance request after commit.
    pub fn drain_maintenance(&self) -> Option<crate::dbck::DbCheckReport> {
        self.state.borrow_mut().pending.maintenance.take()
    }

    /// Consume deferred logfile appends after commit.
    pub fn drain_logs(&self) -> Vec<crate::logging::FileRequest> {
        std::mem::take(&mut self.state.borrow_mut().pending.logs)
    }

    /// Consume private descriptor output after commit.
    pub fn drain_private(&self) -> Vec<PrivateOutput> {
        std::mem::take(&mut self.state.borrow_mut().pending.private)
    }

    /// Carry committed flows into a replacement runtime while retaining its loading effects.
    pub fn inherit(&self, previous: &Self) {
        let previous = previous.state.borrow();
        let mut state = self.state.borrow_mut();
        let logs = std::mem::take(&mut state.pending.logs);
        let records = std::mem::take(&mut state.pending.records);
        let map_writes = std::mem::take(&mut state.pending.map_writes);
        let maintenance = state.pending.maintenance.take();
        state.sessions = previous.sessions.clone();
        state.durable = previous.durable.clone();
        state.pending = EffectBatch {
            flows: state.durable.clone(),
            logs,
            records,
            map_writes,
            maintenance,
            ..Default::default()
        };
    }

    /// Enforce the aggregate allowance for notifications, private output, logs and map assets.
    pub fn validate(&self) -> anyhow::Result<()> {
        let normal = self.outbox.borrow();
        let state = self.state.borrow();
        let pending = &state.pending;
        let bytes = pending
            .map_writes
            .iter()
            .map(|request| request.bytes())
            .sum::<usize>()
            .saturating_add(
                pending
                    .records
                    .iter()
                    .map(|record| record.text.len())
                    .sum::<usize>(),
            )
            .saturating_add(
                normal
                    .iter()
                    .map(|(_, document)| document.len())
                    .sum::<usize>()
                    .saturating_add(
                        pending
                            .private
                            .iter()
                            .map(|output| output.document.len())
                            .sum::<usize>(),
                    )
                    .saturating_add(
                        pending
                            .logs
                            .iter()
                            .map(|entry| entry.filename.len() + entry.message.len())
                            .sum::<usize>(),
                    ),
            );
        if normal
            .len()
            .saturating_add(pending.private.len())
            .saturating_add(pending.logs.len())
            .saturating_add(pending.records.len())
            .saturating_add(pending.map_writes.len())
            > self.config.lua.output_entry_limit
            || bytes > self.config.lua.output_byte_limit
        {
            anyhow::bail!("Lua output limit exceeded");
        }
        Ok(())
    }

    /// Stage private output at its current ordinary-output position.
    pub fn output(&self, session: u64, document: Document) -> anyhow::Result<()> {
        let normal = self.outbox.borrow();
        let mut state = self.state.borrow_mut();
        let bytes = normal
            .iter()
            .map(|(_, document)| document.len())
            .sum::<usize>()
            + state
                .pending
                .private
                .iter()
                .map(|output| output.document.len())
                .sum::<usize>();
        if document.len() > self.config.runtime.output_message_limit
            || normal.len() + state.pending.private.len() >= self.config.lua.output_entry_limit
            || bytes.saturating_add(document.len()) > self.config.lua.output_byte_limit
        {
            anyhow::bail!("Flow output limit exceeded");
        }
        state.pending.private.push(PrivateOutput {
            after: normal.len(),
            session,
            document,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"))
            .unwrap()
    }

    #[test]
    fn nested_restore_retains_authoritative_session_removal() {
        let config = config();
        let outbox = Outbox::default();
        let effects = Effects::new(&config, &outbox);
        effects.sessions(BTreeMap::from([(
            7,
            FlowIdentity {
                player: ObjectId(1),
                generation: Default::default(),
            },
        )]));
        effects.output(7, "prompt".into()).unwrap();
        let checkpoint = effects.checkpoint();
        effects.sessions(BTreeMap::new());
        effects.restore(checkpoint);
        assert!(effects.drain_private().is_empty());
    }

    #[test]
    fn nested_restore_recovers_normal_private_log_and_flow_candidates() {
        let config = config();
        let outbox = Outbox::default();
        let effects = Effects::new(&config, &outbox);
        let identity = FlowIdentity {
            player: ObjectId(1),
            generation: Default::default(),
        };
        effects.sessions(BTreeMap::from([(7, identity)]));
        effects.insert_flow(
            7,
            ActiveFlow {
                identity,
                module: "global_logic/test.lua".into(),
                step: "start".into(),
                scratch: Default::default(),
                prompt: "prompt".into(),
            },
        );
        outbox.borrow_mut().push((ObjectId(1), "before".into()));
        effects.output(7, "private before".into()).unwrap();
        assert!(effects.stage_log(
            crate::logging::FileRequest::new("test", "before").unwrap(),
            &config,
        ));
        let checkpoint = effects.checkpoint();

        effects.remove_flow(7);
        outbox.borrow_mut().push((ObjectId(1), "after".into()));
        effects.output(7, "private after".into()).unwrap();
        assert!(effects.stage_log(
            crate::logging::FileRequest::new("test", "after").unwrap(),
            &config,
        ));
        effects.restore(checkpoint);

        assert!(effects.active(7));
        assert_eq!(outbox.borrow().len(), 1);
        assert_eq!(effects.drain_private().len(), 1);
        assert_eq!(effects.drain_logs().len(), 1);
    }
}
