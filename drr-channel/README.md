# drr-channel

An async channel with one queue per sender, served by deficit round robin.

A pool of workers that takes its work from one ordinary channel serves whoever queued
first: a source that queues a thousand items makes everyone else wait behind them.
`drr-channel` gives every source a queue of its own, a *flow*, and hands the items out in
turn, so each flow gets its share of the workers however much the others have queued.

It is built on `tokio::sync` and was written for the VFS worker pool of
[`nfs-mamont`](https://crates.io/crates/nfs-mamont), where a flow is a client connection;
nothing in it is specific to NFS.

[API documentation](https://docs.rs/drr-channel)

## Installation

```sh
cargo add drr-channel
```

## Usage

```rust
use std::num::{NonZeroU32, NonZeroUsize};

use drr_channel::{channel, Cost};

#[derive(Debug)]
struct Job(u32);

impl Cost for Job {
    fn cost(&self) -> u32 {
        1
    }
}

#[tokio::main]
async fn main() {
    let capacity = NonZeroUsize::new(64).unwrap();
    let quantum = NonZeroU32::new(2).unwrap();
    let (sender, receiver) = channel(capacity, quantum);

    // One flow per source of work.
    let (busy, quiet) = (sender.flow(), sender.flow());
    for id in 0..4 {
        busy.send(Job(id)).await.unwrap();
    }
    quiet.send(Job(100)).await.unwrap();

    // Prints 0, 1, 100, 2, 3: the busy flow spends its quantum, then the quiet one
    // gets its turn.
    for _ in 0..5 {
        let (job, slot) = receiver.recv().await.unwrap();
        println!("{}", job.0);
        // The item counts against the capacity until its slot is dropped.
        drop(slot);
    }
}
```

## Contents

| Item | Role |
|------|------|
| `channel` | Creates a channel with a capacity and a quantum |
| `Sender` | Opens flows; dropping it closes the channel |
| `FlowSender` | Queues the items of one flow, waiting for a free slot of the capacity |
| `Receiver` | Takes items from all flows in deficit round robin order; clone it once per worker |
| `Slot` | Place of a taken item in the capacity, freed when dropped |
| `Cost` | How much of the receivers' work an item takes |

## Behaviour

- A flow alone gets every receiver; backlogged flows get equal shares of the cost.
- A light flow waits for at most one quantum of each backlogged one.
- A flow that runs empty loses its unspent credit, so an idle flow banks none.
- At most `capacity` items are queued or held by receivers at once; senders wait for a
  slot in the order they came.
- Dropping the `Sender` closes the channel: the queued items are still handed out, then
  `Receiver::recv` returns `None`.

## License

This project is licensed under the [MIT License](LICENSE).
