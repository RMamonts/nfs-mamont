use crate::allocator::multilevel::alloc::Level;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod alloc;
pub mod slice;

pub const MULTIPLICITY: usize = 8;

pub fn allocator_constructor(
    block_size: NonZeroUsize,
    low_level_amount: NonZeroUsize,
    current_level: usize,
    levels: usize,
) -> Option<Level> {
    if current_level == levels {
        return None;
    }

    let block_amount =
        low_level_amount.get().checked_mul(current_level)?.checked_mul(MULTIPLICITY)?;

    let blocks = NonZeroUsize::new(block_amount)?;

    let next = allocator_constructor(block_size, low_level_amount, current_level + 1, levels)
        .map(|lvl| Arc::new(Mutex::new(lvl)));

    Some(Level::new(block_size, blocks, next))
}
