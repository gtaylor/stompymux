//! Wizard diagnostics expose actual Rust scheduler state and explicitly bounded storage measurements.
use crate::{Config, ObjectId, World};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, io::Write};

/// Live counts and measured representation sizes; no allocator or historical event totals are inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleRuntimeStats {
    pub simulation_pending: bool,
    pub scanner_observers: usize,
    pub reactor_startup_remaining: u8,
    pub artillery_shots: usize,
    pub station_locks: usize,
    pub maps: usize,
    pub mechs: usize,
    pub vehicles: usize,
    pub stations: usize,
    pub registration_kinds: BTreeMap<String, usize>,
    /// Inline sizes of the state root and live map/unit/station records, excluding their heap storage.
    pub inline_record_bytes: usize,
    /// Exact compact JSON encoding size; this is a representation size, not process memory usage.
    pub encoded_state_bytes: u64,
}

/// Count serialized bytes without allocating a second full state representation.
#[derive(Default)]
struct ByteCounter(u64);

impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Inspect the same live state used by the one-second simulation scheduler without advancing it.
pub fn runtime_stats(
    world: &World,
    config: &Config,
    actor: ObjectId,
) -> Result<BattleRuntimeStats> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    let scanner_observers = crate::battle_contact_observers(world).len();
    let state = &world.btech;
    let mut counter = ByteCounter::default();
    serde_json::to_writer(&mut counter, state)?;
    let mut registration_kinds = BTreeMap::new();
    for kind in state.registrations().values() {
        *registration_kinds.entry(kind.clone()).or_insert(0) += 1;
    }
    Ok(BattleRuntimeStats {
        simulation_pending: super::simulation_pending::pending(
            world,
            config,
            scanner_observers != 0,
        ),
        scanner_observers,
        reactor_startup_remaining: state.reactor.startup_remaining,
        artillery_shots: state
            .maps()
            .values()
            .map(|map| map.artillery_shots.len())
            .sum(),
        station_locks: state
            .gunner_stations()
            .values()
            .filter(|station| station.lock_remaining > 0)
            .count(),
        maps: state.maps().len(),
        mechs: state.constructed_units().len(),
        vehicles: state.vehicles().len(),
        stations: state.gunner_stations().len(),
        registration_kinds,
        inline_record_bytes: std::mem::size_of_val(state)
            + state
                .maps()
                .values()
                .map(std::mem::size_of_val)
                .sum::<usize>()
            + state
                .constructed_units()
                .values()
                .map(std::mem::size_of_val)
                .sum::<usize>()
            + state
                .vehicles()
                .values()
                .map(std::mem::size_of_val)
                .sum::<usize>()
            + state
                .gunner_stations()
                .values()
                .map(std::mem::size_of_val)
                .sum::<usize>(),
        encoded_state_bytes: counter.0,
    })
}

/// Render native diagnostics as literal text; LONG adds saved registration categories.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let memory = input.name == "memstats";
        let long = input.args.trim().eq_ignore_ascii_case("long");
        ensure!(
            input.args.trim().is_empty() || memory && long,
            "Usage: EVENTSTATS or MEMSTATS [LONG]"
        );
        let stats = runtime_stats(&ctx.scripts.world(), ctx.config, ctx.player)?;
        if !memory {
            return Ok(format!(
                "BattleTech simulation interval: 1 second\nSimulation work pending: {}\nScanner observers: {}\nArtillery shots in flight: {}\nStation locks settling: {}\nReactor startup grace: {} seconds",
                stats.simulation_pending,
                stats.scanner_observers,
                stats.artillery_shots,
                stats.station_locks,
                stats.reactor_startup_remaining
            ));
        }
        let mut text = format!(
            "BattleTech records: {} maps, {} Mechs, {} vehicles, {} stations\nInline record bytes (heap excluded): {}\nEncoded state bytes (JSON): {}\nAllocator totals: unavailable",
            stats.maps,
            stats.mechs,
            stats.vehicles,
            stats.stations,
            stats.inline_record_bytes,
            stats.encoded_state_bytes
        );
        if long {
            for (kind, count) in stats.registration_kinds {
                text.push_str(&format!("\n{kind}: {count} registrations"));
            }
        }
        Ok(text)
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Reply(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}
