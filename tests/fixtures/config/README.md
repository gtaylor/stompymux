# Legacy configuration contract

`legacy-catalog.json` inventories the 182 mappings from the sibling legacy tree's
`src/mux/server/configuration_toml.c`. Registry types come from
`configuration_registry.c`; compiled defaults come from `configuration.c` and
its foundational object defaults. This is a data contract fixture, not translated
server implementation. `complete.toml` exercises every mapping and each value
shape. Tests compare the inventory against the Rust catalog and effective defaults.

Two mappings have no registry entry: `mux.connect_reg_file` and
`mux.unowned_safe`. They are retained as an empty path and false respectively;
the inventory records this explicitly. The intentional default exception is
`database.game_database`: Rust uses `data/stompymux-rs.db` instead of the legacy
empty string. New Rust-only settings are tested separately and are not counted
among the 182 legacy mappings.
