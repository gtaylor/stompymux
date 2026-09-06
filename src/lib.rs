//! Independent MUX server components.
pub mod accounts;
pub mod commands;
pub mod config;
pub mod lua;
pub mod persistence;
pub mod server;
pub mod sessions;
pub mod telnet;
pub mod text;
pub mod world;

pub mod flags;
pub mod movement;
pub mod powers;

pub mod find;

/// Transactional database checking and semantic repair.
pub mod dbck;

/// Startup-indexed Markdown help.
pub mod help;

/// Channels, communication policy and online paging.
pub mod communication;

/// Binary-safe typed object state and administrative operations.
pub mod state;

/// Captured Lua scheduling and clock-driven execution primitives.
pub use lua::schedules::{
    Catalog as ScheduleCatalog, Cron, Definition as ScheduleDefinition, Job as ScheduledJob,
    Queue as ScheduleQueue, jitter as schedule_jitter,
};
/// Embedded server entry point with a schedule-only test clock.
pub use server::run_with_schedule_clock;

/// Player-defined command templates and shared macro sets.
pub mod macros;
pub use macros::{MacroEntry, MacroModes, MacroSet, MacroSlots, PlayerMacros};
