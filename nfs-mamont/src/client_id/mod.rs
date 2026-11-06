//! Client registration and connection identity.
//!
//! [`ClientIdManager`] implements NFSv4 SETCLIENTID/CONFIRM.
//! [`Nfs3ClientIdManager`] provides explicit connection registration for NFSv3
//! locking, without DRC or confirmation. Share a [`ClientIdGenerator`] across
//! registries to allocate clientids from one server-wide sequence.

mod cache;
mod connection;
mod generator;
mod nfsv3;
mod nfsv4;
mod types;

pub use crate::task::global::client_id::{
    ClientIdCommand, ClientIdReceiver, ClientIdReplyReceiver, ClientIdReplySender, ClientIdSender,
    ClientIdTask,
};
pub use connection::{ConnectionGeneration, ConnectionGenerationGenerator, ConnectionInfo};
pub use generator::ClientIdGenerator;
pub use nfsv3::{Nfs3Client, Nfs3ClientIdManager};
pub use nfsv4::ClientIdManager;
pub use types::{
    Callback, ClientId, ClientIdentity, ClientVerifier, ConfirmationVerifier, Error, NetId,
    PendingClient, Registration, Result,
};
