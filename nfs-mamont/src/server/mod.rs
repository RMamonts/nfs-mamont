//! Server construction, shared resources and lifecycle management.
//!
//! [`ServerContext`] owns filesystem resources and backend registration.
//! [`Server`] runs the listener with [`ServerConfig`]; [`ServerHandle`] exposes
//! its state and shutdown channel. Bind a listener and create the context before
//! starting the actor with [`Server::spawn`].

mod actor;
mod context;

pub use actor::{Server, ServerCommand, ServerConfig, ServerHandle, ServerState};
pub use context::ServerContext;
