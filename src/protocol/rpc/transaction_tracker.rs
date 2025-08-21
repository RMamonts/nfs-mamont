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

use std::sync::atomic::AtomicU64;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use tracing::info;

/// Tracks RPC transactions to detect and handle retransmissions
///
/// Implements idempotency for RPC operations by tracking transaction state
/// using a combination of transaction ID (XID) and client address.
/// Helps prevent duplicate processing of retransmitted requests
/// and maintains transaction state for a configurable retention period.
pub struct TransactionTracker {
    retention_period: Duration,
    transactions: DashMap<(u32, String), TransactionState>,
    /// Counter to track operations for periodic cleanup
    operation_count: AtomicU64,
    /// How often to run cleanup (every N operations)
    cleanup_threshold: u64,
}

impl TransactionTracker {
    /// Creates a new transaction tracker with specified retention period
    ///
    /// Initializes a transaction tracker that will maintain transaction state
    /// for the given duration. This helps balance memory usage with the ability
    /// to detect retransmissions over time.
    pub fn new(retention_period: Duration) -> Self {
        Self {
            retention_period,
            transactions: DashMap::new(),
            operation_count: AtomicU64::new(0),
            cleanup_threshold: 10_000, // Run cleanup every 1000 operations
        }
    }

    /// Checks if a transaction is a retransmission
    ///
    /// Identifies whether the transaction with given XID and client address
    /// has been seen before. If it's a new transaction, marks it as in-progress.
    /// Returns true for retransmissions, false for new transactions.
    ///
    /// Optimized to minimize locking and reduce overhead on the hot path.
    pub fn is_retransmission(&self, xid: u32, client_addr: &str) -> bool {
        // Fast path: check if transaction already exists without acquiring lock
        if self.transactions.contains_key(&(xid, client_addr.to_string())) {
            return true;
        }

        // Increment operation count and check if cleanup is needed
        let should_cleanup = {
            let count = self.operation_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            info!("should_cleanup: count {}", count);
            count % self.cleanup_threshold == 0
        };

        // Perform cleanup periodically to avoid accumulation
        if should_cleanup {
            housekeeping(&self.transactions, self.retention_period);

            // TODO: reset self.operation_count
        }

        // Insert new transaction
        self.transactions.insert((xid, client_addr.to_string()), TransactionState::InProgress);

        false
    }

    /// Marks a transaction as successfully processed
    ///
    /// Updates the state of a transaction from in-progress to completed,
    /// recording the completion time for retention period calculations.
    /// Called after a transaction has been fully processed and responded to.
    pub fn mark_processed(&self, xid: u32, client_addr: &str) {
        let key = (xid, client_addr.to_string());
        let completion_time = SystemTime::now();
        if let Some(mut tx) = self.transactions.get_mut(&key) {
            *tx = TransactionState::Completed(completion_time);
        }
    }
}

/// Removes expired transactions from the tracking map
///
/// Cleans up completed transactions that have exceeded the maximum retention age.
/// Keeps in-progress transactions regardless of age to prevent processing duplicates.
/// Called during transaction checks to maintain memory efficiency.
fn housekeeping(transactions: &DashMap<(u32, String), TransactionState>, max_age: Duration) {
    let mut cutoff = SystemTime::now() - max_age;
    transactions.retain(|_, v| match v {
        TransactionState::InProgress => true,
        TransactionState::Completed(completion_time) => completion_time >= &mut cutoff,
    });
}

/// Represents the current state of an RPC transaction
///
/// Either in-progress (currently being processed) or
/// completed (successfully processed with timestamp).
/// Used for tracking transaction lifecycle and retransmission detection.
enum TransactionState {
    InProgress,
    Completed(SystemTime),
}
