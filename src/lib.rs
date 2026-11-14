//! gitview: a read-only git browser TUI built with cascivo-tui.
//!
//! [`git`] is the domain layer (I/O behind the [`git::GitRepo`] trait), [`app`] the UI,
//! [`fuzzy`] the list filter.
pub mod app;
pub mod fuzzy;
pub mod git;
