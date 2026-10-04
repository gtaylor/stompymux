//! Map rule flags on live battlefield maps. The flags themselves, their bits and their names
//! live in `stompymux-map` as [`MapFlag`].
use super::{MapFlag, StoredBattleMap};

impl StoredBattleMap {
    /// Whether this map has `flag` switched on.
    pub fn has_flag(&self, flag: MapFlag) -> bool {
        flag.is_set(self.flags)
    }

    /// Switch `flag` on or off without touching other flags.
    pub fn set_flag(&mut self, flag: MapFlag, enabled: bool) {
        self.flags = flag.apply(self.flags, enabled);
    }
}
