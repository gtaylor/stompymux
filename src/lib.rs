//! Independent MUX server components.
pub mod accounts;
pub mod commands;
pub mod config;
pub mod lua;
pub use lua::sources::Sources as LuaSources;
pub use lua::{AdminRequest as LuaAdminRequest, AppearanceMode, RuntimeMode};
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

/// Shared object and channel lock catalog.
pub mod locks;
pub use locks::{LockInvocation, LockOutcome, LockType};

/// Live-world Lua suite execution and bounded reports.
pub use lua::testing::{Report as LuaTestReport, Request as LuaTestRequest};

/// Wizard account requests and login-history inspection.
pub mod account_admin;
