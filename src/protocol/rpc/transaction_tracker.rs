//! Transaction tracking for RPC idempotency as described in RFC 5531 (previously RFC 1057).
//!
//! This module implements the idempotency requirements for RPC by tracking
//! transaction state using transaction IDs (XIDs) and client addresses.
//! It ensures that:
//!
//! - Duplicate requests due to network retransmissions are properly identified
//! - Only one instance of a given RPC request is processed
//! - Transaction state is maintained for a configurable period to handle delayed retransmissions
//! - Server resources are managed efficiently by cleaning up expired transaction records
//!
//! The transaction tracking system is essential for maintaining the at-most-once
//! semantics required by NFS and other RPC-based protocols, where duplicate
//! operations (like file writes) could cause data corruption.
#![cfg(feature = "udp")]
use std::time::Duration;
use dashmap::DashSet;
use moka::sync::Cache;

/// Tracks RPC transactions to detect and handle retransmissions
///
/// Implements idempotency for RPC operations by tracking transaction state
/// using a combination of transaction ID (XID) and client address.
/// Helps prevent duplicate processing of retransmitted requests
/// and maintains transaction state for a configurable retention period.
pub struct TransactionTracker {
    in_progress_transactions: DashSet<(u32, String)>,
    completed_transactions: Cache<(u32, String), ()>,
}

impl TransactionTracker {
    /// Creates a new transaction tracker with specified retention period
    ///
    /// Initializes a transaction tracker that will maintain transaction state
    /// for the given duration. This helps balance memory usage with the ability
    /// to detect retransmissions over time.
    pub fn new(retention_period: Duration) -> Self {
        let cache = Cache::builder().time_to_live(retention_period).build();

        Self { in_progress_transactions: DashSet::new(), completed_transactions: cache }
    }

    /// Checks if a transaction is a retransmission
    ///
    /// Identifies whether the transaction with given XID and client address
    /// has been seen before. If it's a new transaction, marks it as in-progress.
    /// Returns true for retransmissions, false for new transactions.
    pub fn is_retransmission(&self, xid: u32, client_addr: &str) -> bool {
        let key = (xid, client_addr.to_string());

        self.completed_transactions.get(&key).is_some()
            || !self.in_progress_transactions.insert(key)
    }

    /// Marks a transaction as successfully processed
    ///
    /// Moves a transaction to completed cache with TTL
    /// Called after a transaction has been fully processed and responded to.
    pub fn mark_processed(&self, xid: u32, client_addr: &str) {
        let key = (xid, client_addr.to_string());

        if self.in_progress_transactions.remove(&key).is_some() {
            self.completed_transactions.insert(key, ());
        }
    }
}
