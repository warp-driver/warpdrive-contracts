use soroban_sdk::contractclient;

use super::warpdrive::WarpDriveInterface;

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "ProjectRootClient")]
pub trait ProjectRootInterface: WarpDriveInterface {}
