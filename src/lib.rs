// lib.rs — exposes internal modules so binaries in src/bin/ can import them.
// main.rs is a private entry point, so anything src/bin/ needs must come through here.
pub mod fetch;
pub mod ws;
