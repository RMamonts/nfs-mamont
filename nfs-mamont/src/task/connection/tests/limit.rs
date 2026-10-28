//! Tests of the in-flight budget, see [`crate::task::connection::limit`].

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::task::connection::limit::{Budget, Limiter, Token};

const WAIT: Duration = Duration::from_millis(50);

fn budget(limit: usize) -> (Budget, Arc<Limiter>) {
    Budget::new(NonZeroUsize::new(limit).unwrap())
}

async fn take(budget: &mut Budget) -> Option<Token> {
    timeout(WAIT, budget.acquire()).await.expect("acquire should not wait")
}

#[tokio::test]
async fn tokens_are_issued_up_to_the_limit() {
    let (mut budget, _limiter) = budget(2);

    let _first = take(&mut budget).await.unwrap();
    let _second = take(&mut budget).await.unwrap();
    assert!(timeout(WAIT, budget.acquire()).await.is_err(), "third token must wait");
}

#[tokio::test]
async fn a_release_lets_the_next_acquire_through() {
    let (mut budget, limiter) = budget(1);
    let token = take(&mut budget).await.unwrap();

    let releaser = tokio::spawn(async move {
        tokio::time::sleep(WAIT).await;
        limiter.release(token);
    });
    let _token = timeout(WAIT * 10, budget.acquire()).await.expect("woken by the release");
    releaser.await.unwrap();
}

#[tokio::test]
async fn a_cancelled_token_frees_its_slot() {
    let (mut budget, _limiter) = budget(1);
    let token = take(&mut budget).await.unwrap();

    budget.cancel(token);
    let _token = take(&mut budget).await.unwrap();
}

#[tokio::test]
async fn a_dropped_token_frees_its_slot() {
    let (mut budget, limiter) = budget(1);
    let token = take(&mut budget).await.unwrap();

    drop(token);
    let _token = take(&mut budget).await.unwrap();
    assert_eq!(limiter.released(), 1);
}

#[tokio::test]
async fn a_token_dropped_elsewhere_wakes_a_waiting_acquire() {
    let (mut budget, _limiter) = budget(1);
    let token = take(&mut budget).await.unwrap();

    // What a panicking worker does with the request it holds.
    let dropper = tokio::spawn(async move {
        tokio::time::sleep(WAIT).await;
        drop(token);
    });
    let _token = timeout(WAIT * 10, budget.acquire()).await.expect("woken by the drop");
    dropper.await.unwrap();
}

#[tokio::test]
async fn close_stops_a_waiting_acquire() {
    let (mut budget, limiter) = budget(1);
    let _token = take(&mut budget).await.unwrap();

    let closer = tokio::spawn(async move {
        tokio::time::sleep(WAIT).await;
        limiter.close();
    });
    let next = timeout(WAIT * 10, budget.acquire()).await.expect("woken by the close");
    assert!(next.is_none());
    closer.await.unwrap();
}

#[tokio::test]
async fn releases_are_seen_without_a_wait() {
    let (mut budget, limiter) = budget(1);
    let token = take(&mut budget).await.unwrap();

    limiter.release(token);
    let _token = take(&mut budget).await.unwrap();
    assert_eq!(limiter.released(), 1);
}
