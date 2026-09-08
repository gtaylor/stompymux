//! Independent MUX server components.
pub mod accounts;
pub use accounts::{Account, Login};
pub mod commands;
pub use commands::{
    Action as CommandAction, CommandContext, CommandDefinition, CommandHandler, CommandInput,
    CommandMatcher, CommandPermissions, CommandRegistry, CommandScope, ExecutionContext,
    InputOrigin, Report as CommandReport, ServerRequest, SwitchDefinition,
};
pub mod config;
pub use config::Config;
mod destruction;
pub mod lua;
pub use lua::sources::Sources as LuaSources;
pub use lua::{AdminRequest as LuaAdminRequest, AppearanceMode, RuntimeMode};
pub use lua::{Scripts, WorldInspection};
pub mod persistence;
pub mod server;
pub use server::{ShutdownRequest, prepare as prepare_server, serve};
pub mod sessions;
pub mod telnet;
pub mod text;
pub use text::{ColorDepth, Document, Palette, RenderOptions};
mod world;
pub use world::{CreationContext, Kind, LinkSlots, Links, Object, ObjectId, World};

pub mod flags;
pub use flags::{Flag, FlagSet};
pub mod movement;
pub mod powers;

pub mod find;
pub mod reports;
pub mod runtime;
pub mod search;
pub use runtime::{EffectBatch, Effects, Outbox, PrivateOutput, SharedWorld};

/// Transactional database checking and semantic repair.
pub mod dbck;

/// Startup-indexed Markdown help.
pub mod help;

/// Channels, communication policy and online paging.
pub mod communication;
pub use communication::Channel;

/// Binary-safe typed object state and administrative operations.
pub mod state;
pub use state::Value as StateValue;

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
mod locks;
pub use locks::{LOCKS, LockInvocation, LockOutcome, LockType};

/// Live-world Lua suite execution and bounded reports.
pub use lua::testing::{Report as LuaTestReport, Request as LuaTestRequest};

/// Wizard account requests and login-history inspection.
pub mod account_admin;

/// Bounded container and audible-exit message routing.
pub mod notification;

pub mod cleaning;

pub mod controls;
pub mod message_cache;

/// Ordered connection access policies.
pub mod sites;

/// Shared command and list access evaluation.
pub mod access;

pub mod logging;

/// Read-only operational reports and platform resource snapshots.
pub mod operations;

/// Object authority independent of flag mutation and lock evaluation.
pub mod authority;
/// Wall-clock timestamps, separate from monotonic runtime deadlines.
pub mod clock;
