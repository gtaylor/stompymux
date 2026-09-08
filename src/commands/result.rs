//! Command results separated into rendered reports and world-owner requests.

use crate::world::ObjectId;

/// Text produced by command dispatch, with the rendering contract retained for the server.
pub enum Report {
    /// Bounded private response without report rendering.
    Reply(String),
    /// Complete inspection text delivered through the help-style report path.
    Inspection(String),
    /// Literal text whose rows and whitespace must be preserved.
    Literal(String),
    /// Styled text rendered for the recipient's negotiated capabilities.
    Styled(String),
}

/// Work that must be completed by the serialized server owner.
pub enum ServerRequest {
    /// Administrative account hashing or session removal.
    AccountAdmin(crate::account_admin::Request),
    /// Read-only diagnostics for active sessions.
    Sessions(String),
    /// Administrative connection listing.
    Who(String),
    /// Collect platform resource usage outside the world borrow.
    ProcessReport,
    /// Read-only Telnet negotiation diagnostics.
    Telnet(String),
    /// Captured schedule metadata for the invoking session.
    LuaSchedules(String),
    /// Isolated Lua checks, source views, and atomic runtime replacement.
    LuaAdmin(crate::lua::AdminRequest),
    /// Session-local rendering preference.
    Color(String),
    /// Read-only help lookup for the invoking session.
    Help(String),
    /// Rebuild the immutable help metadata snapshot.
    HelpReload,
    /// Reload connection messages without touching world storage.
    ReadCache,
    /// Append to a permitted existing logfile outside world persistence.
    Log(crate::logging::FileRequest),
    /// Apply runtime-only configuration administration.
    ConfigAdmin(crate::config::administration::Request),
    /// Request common graceful shutdown.
    Shutdown,
    /// Run transactional database maintenance.
    DbCheck,
    /// Change or inspect runtime cleaning controls.
    GlobalControl(Option<(crate::controls::Control, bool)>),
    /// Disconnect the invoking session.
    Quit,
    /// Read persisted list pointers for a debug examination.
    ExamineDebug(ObjectId),
}

/// Result of dispatch before the server commits state or performs owner-only work.
pub enum Action {
    /// Commit callback changes and flush player-directed output.
    Continue,
    /// Commit mutations before delivering a confirmation.
    CommitReply(String),
    /// Transactional queue admission or cancellation.
    Queue(crate::commands::queue::Request),
    /// Rendered command output.
    Report(Report),
    /// Serialized work outside the command dispatcher.
    Server(ServerRequest),
}
