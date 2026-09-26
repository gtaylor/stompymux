//! Channel and paging model, policy, and services.

mod commands;
mod delivery;
mod membership;
mod model;
mod page;
mod policy;
mod service;
pub mod speech;

pub use crate::authority::is_wizard as wizard;
pub use commands::{addcom, admin, alias, allcom, clearcom, comlist, delcom, page};
pub use model::{
    ALIAS_LIMIT, Access, CHANNEL_NAME_LIMIT, Channel, ChannelAlias, ChannelFlag, ChannelFlags,
    ChannelId, ChannelMessage, HISTORY_LIMIT, Membership,
};
pub use policy::in_character;
pub use service::{HostCallbacks, LockOutcome, Service, ServiceConfig};

use crate::{
    config::Config,
    flags::Flag,
    world::{Kind, ObjectId},
};
use anyhow::{Result, ensure};
