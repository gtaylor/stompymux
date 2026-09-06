//! Lua-owned Markdown source accounting, released when the immutable handle is collected.
use crate::text::Document;

/// Lua-owned source memory is charged until the immutable handle is collected.
pub struct LuaDocument {
    /// Immutable source exposed through the Lua userdata handle.
    pub document: Document,
    /// Retained allocation charged to the Lua document budget.
    pub allocation: usize,
    /// Shared live allocation counter for this Lua runtime.
    pub usage: std::rc::Rc<std::cell::Cell<usize>>,
}

impl mlua::UserData for LuaDocument {}

impl Drop for LuaDocument {
    fn drop(&mut self) {
        self.usage
            .set(self.usage.get().saturating_sub(self.allocation));
    }
}
