//! Command handlers: the dispatch logic for running individual CLI subcommands.
//!
//! Each subcommand has a handler struct here with a `new(args)` constructor and
//! a `dispatch()` entrypoint; [`Command::dispatch`](crate::config::Command)
//! delegates to them. Handlers are wired up but not yet implemented — the
//! corresponding pipeline tickets fill them in.

pub use generate::Generate;
pub use serve::Serve;
pub use train::Train;

mod generate;
mod serve;
mod train;
