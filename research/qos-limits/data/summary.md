### E1 getattr c1

| server | op | conns | depth | ops/s | MiB/s | p50 | p99 | server CPU | RSS peak MiB | in-flight waits | pending waits | data waits | allocator waits | DRR rotations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| main | getattr | 1 | 1 | 15.3k | 0 | 51 µs | 110 µs | 60 % | 4.2 | - | - | - | - | - |
| inflight | getattr | 1 | 1 | 16.3k | 0 | 47 µs | 100 µs | 60 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| budgets | getattr | 1 | 1 | 15.9k | 0 | 49 µs | 96 µs | 60 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| drr | getattr | 1 | 1 | 20.3k | 0 | 36 µs | 80 µs | 42 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| main | getattr | 1 | 32 | 74.2k | 0 | 408 µs | 800 µs | 113 % | 4.4 | - | - | - | - | - |
| inflight | getattr | 1 | 32 | 71.5k | 0 | 424 µs | 848 µs | 115 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| budgets | getattr | 1 | 32 | 68.8k | 0 | 432 µs | 896 µs | 113 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| drr | getattr | 1 | 32 | 69.9k | 0 | 440 µs | 880 µs | 115 % | 4.6 | 0 | 0 | 0 | 0 | 0 |
| main | getattr | 1 | 256 | 85.4k | 0 | 2.88 ms | 5.25 ms | 119 % | 4.5 | - | - | - | - | - |
| inflight | getattr | 1 | 256 | 111.2k | 0 | 2.18 ms | 4.22 ms | 122 % | 4.5 | 3335 | 0 | 0 | 0 | 0 |
| budgets | getattr | 1 | 256 | 106.0k | 0 | 2.37 ms | 4.61 ms | 120 % | 4.7 | 3695 | 0 | 0 | 0 | 0 |
| drr | getattr | 1 | 256 | 112.8k | 0 | 2.11 ms | 3.90 ms | 120 % | 4.8 | 3928 | 0 | 0 | 0 | 14704 |
| main | getattr | 1 | 1024 | 84.1k | 0 | 11.52 ms | 19.97 ms | 119 % | 4.9 | - | - | - | - | - |
| inflight | getattr | 1 | 1024 | 142.9k | 0 | 6.53 ms | 13.57 ms | 122 % | 4.7 | 6611 | 0 | 0 | 0 | 0 |
| budgets | getattr | 1 | 1024 | 135.4k | 0 | 6.91 ms | 12.54 ms | 122 % | 4.6 | 6363 | 0 | 0 | 0 | 0 |
| drr | getattr | 1 | 1024 | 131.8k | 0 | 6.78 ms | 16.38 ms | 118 % | 4.7 | 6300 | 0 | 0 | 0 | 18389 |

### E2 getattr c4

| server | op | conns | depth | ops/s | MiB/s | p50 | p99 | server CPU | RSS peak MiB | in-flight waits | pending waits | data waits | allocator waits | DRR rotations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| main | getattr | 4 | 64 | 299.4k | 0 | 784 µs | 2.18 ms | 173 % | 4.5 | - | - | - | - | - |
| inflight | getattr | 4 | 64 | 291.0k | 0 | 816 µs | 2.18 ms | 174 % | 4.6 | 0 | 0 | 0 | 0 | 0 |
| budgets | getattr | 4 | 64 | 259.0k | 0 | 896 µs | 2.43 ms | 170 % | 4.7 | 0 | 0 | 0 | 0 | 0 |
| drr | getattr | 4 | 64 | 275.3k | 0 | 880 µs | 2.30 ms | 178 % | 4.7 | 0 | 0 | 0 | 0 | 25829 |
| main | getattr | 4 | 512 | 285.9k | 0 | 6.78 ms | 13.06 ms | 173 % | 5.4 | - | - | - | - | - |
| inflight | getattr | 4 | 512 | 324.9k | 0 | 5.89 ms | 13.57 ms | 186 % | 5.0 | 15158 | 0 | 0 | 0 | 0 |
| budgets | getattr | 4 | 512 | 302.9k | 0 | 6.66 ms | 13.31 ms | 188 % | 5.3 | 13885 | 139 | 0 | 0 | 0 |
| drr | getattr | 4 | 512 | 324.9k | 0 | 6.02 ms | 13.31 ms | 188 % | 5.3 | 15094 | 163 | 0 | 0 | 46079 |

### E4 data

| server | op | conns | depth | ops/s | MiB/s | p50 | p99 | server CPU | RSS peak MiB | in-flight waits | pending waits | data waits | allocator waits | DRR rotations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| main | read:64k | 1 | 32 | 22.2k | 1389 | 1.34 ms | 2.43 ms | 109 % | 4.3 | - | - | - | - | - |
| inflight | read:64k | 1 | 32 | 21.8k | 1363 | 1.34 ms | 2.43 ms | 111 % | 4.4 | 0 | 0 | 0 | 0 | 0 |
| budgets | read:64k | 1 | 32 | 21.7k | 1357 | 1.38 ms | 2.50 ms | 111 % | 4.6 | 0 | 0 | 0 | 4 | 0 |
| drr | read:64k | 1 | 32 | 22.6k | 1413 | 1.38 ms | 2.37 ms | 110 % | 4.5 | 0 | 0 | 0 | 7 | 1990 |
| main | read:1m | 1 | 16 | 3.2k | 3165 | 4.74 ms | 8.06 ms | 99 % | 4.3 | - | - | - | - | - |
| inflight | read:1m | 1 | 16 | 3.0k | 3006 | 4.99 ms | 8.96 ms | 99 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| budgets | read:1m | 1 | 16 | 3.0k | 3026 | 4.99 ms | 8.19 ms | 99 % | 4.4 | 0 | 0 | 0 | 0 | 0 |
| drr | read:1m | 1 | 16 | 3.1k | 3078 | 4.86 ms | 8.70 ms | 99 % | 4.6 | 0 | 0 | 0 | 0 | 7662 |
| main | write:64k | 1 | 32 | 23.2k | 1449 | 1.22 ms | 2.24 ms | 108 % | 8.6 | - | - | - | - | - |
| inflight | write:64k | 1 | 32 | 20.6k | 1286 | 1.44 ms | 2.43 ms | 110 % | 8.7 | 0 | 0 | 0 | 30 | 0 |
| budgets | write:64k | 1 | 32 | 20.2k | 1265 | 1.41 ms | 2.88 ms | 108 % | 8.7 | 0 | 0 | 0 | 69 | 0 |
| drr | write:64k | 1 | 32 | 22.1k | 1381 | 1.28 ms | 2.37 ms | 104 % | 8.9 | 0 | 0 | 0 | 0 | 250 |
| main | read:4k | 4 | 64 | 177.5k | 693 | 1.15 ms | 4.74 ms | 169 % | 4.4 | - | - | - | - | - |
| inflight | read:4k | 4 | 64 | 178.8k | 698 | 1.15 ms | 4.61 ms | 171 % | 4.5 | 0 | 0 | 0 | 92401 | 0 |
| budgets | read:4k | 4 | 64 | 218.7k | 854 | 1.01 ms | 3.26 ms | 189 % | 4.7 | 0 | 0 | 54092 | 7 | 0 |
| drr | read:4k | 4 | 64 | 217.3k | 849 | 1.01 ms | 3.26 ms | 188 % | 4.6 | 0 | 0 | 56964 | 4 | 40721 |

### E5 non-reading getattr

| server | accepted requests | RSS final MiB | RSS peak MiB | RSS timeline MiB (every 4 s) | write timeouts |
|---|---|---|---|---|---|
| main | 5010944 | 1475.9 | 1475.9 | 120 → 583 → 944 → 1296 | - |
| inflight | 95180 | 4.5 | 4.5 | 4 → 4 → 4 → 4 | 0 |
| budgets | 94685 | 4.6 | 4.7 | 4 → 4 → 4 → 4 | 0 |
| budgets --write-timeout 5s | 95180 | 4.6 | 4.6 | 4 → 4 → 4 → 4 | 1 |

### E6 allocator hog

| server | victim alone ops/s | victim with hog ops/s | victim p50 | note | data waits | conn data waits | allocator waits |
|---|---|---|---|---|---|---|---|
| main | 61.9k | 0 | - | stalled: no result within 15s | - | - | - |
| inflight | 57.3k | 0 | - | stalled: no result within 15s | 0 | 0 | 7 |
| budgets | 59.6k | 58.9k | 248 µs |  | 0 | 3 | 0 |
| drr | 59.8k | 60.6k | 240 µs |  | 0 | 50 | 0 |

### E7 heavy+light

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main | 3 | 56.8k | 0 | 34.82 ms | 28 | 0 | 34.82 ms | 41.98 ms | - | - | - |
| inflight | 3 | 57.1k | 0 | 34.82 ms | 211 | 0 | 4.48 ms | 7.94 ms | 0 | 0 | 0 |
| inflight --max-in-flight 64 | 3 | 46.2k | 0 | 43.01 ms | 683 | 0 | 1.28 ms | 3.20 ms | 0 | 0 | 0 |
| budgets | 3 | 57.3k | 0 | 34.82 ms | 218 | 0 | 4.35 ms | 7.55 ms | 42872 | 0 | 0 |
| drr | 3 | 57.0k | 0 | 34.82 ms | 534 | 0 | 1.82 ms | 3.78 ms | 43846 | 24970 | 0 |

### E8 c256+probe

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main | 3 | 57.8k | 0 | 69.63 ms | 14 | 0 | 69.63 ms | 75.78 ms | - | - | - |
| inflight | 3 | 58.0k | 0 | 69.63 ms | 14 | 0 | 69.63 ms | 77.82 ms | 0 | 0 | 0 |
| budgets | 3 | 57.4k | 0 | 69.63 ms | 108 | 0 | 8.96 ms | 12.29 ms | 807428 | 0 | 0 |
| drr | 3 | 58.1k | 0 | 69.63 ms | 89 | 0 | 11.01 ms | 13.82 ms | 815222 | 0 | 0 |

### E8 c1024+probe

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main | 3 | 58.1k | 0 | 278.53 ms | 4 | 0 | 278.53 ms | 300.96 ms | - | - | - |
| inflight | 3 | 58.1k | 0 | 278.53 ms | 4 | 0 | 278.53 ms | 301.78 ms | 0 | 0 | 0 |
| budgets | 3 | 58.0k | 0 | 278.53 ms | 45 | 0 | 22.02 ms | 26.11 ms | 828636 | 0 | 0 |
| drr | 3 | 58.4k | 0 | 278.53 ms | 45 | 0 | 21.50 ms | 26.62 ms | 834281 | 0 | 0 |

### E9 fast+slow

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main | 3 | 54.5k | 0 | 4.48 ms | 436 | 0 | 4.48 ms | 6.91 ms | - | - | - |
| inflight | 3 | 57.4k | 0 | 4.35 ms | 455 | 0 | 4.22 ms | 6.14 ms | 0 | 0 | 0 |
| budgets | 3 | 57.6k | 0 | 4.35 ms | 453 | 0 | 4.22 ms | 6.27 ms | 45172 | 0 | 0 |
| drr | 3 | 56.8k | 0 | 4.35 ms | 1.1k | 0 | 1.66 ms | 3.39 ms | 46661 | 17698 | 0 |

### E10 meta+stream

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main | 3 | 2.0k | 2039 | 15.62 ms | 40.7k | 0 | 1.38 ms | 4.99 ms | - | - | - |
| inflight | 3 | 2.0k | 2044 | 15.62 ms | 41.6k | 0 | 1.34 ms | 4.35 ms | 0 | 0 | 0 |
| budgets | 3 | 2.0k | 2042 | 15.62 ms | 41.9k | 0 | 1.34 ms | 4.35 ms | 0 | 0 | 428 |
| drr | 3 | 2.0k | 2043 | 15.62 ms | 44.3k | 0 | 1.31 ms | 3.52 ms | 0 | 17109 | 437 |

### E2b getattr c4

| server | op | conns | depth | ops/s | MiB/s | p50 | p99 | server CPU | RSS peak MiB | in-flight waits | pending waits | data waits | allocator waits | DRR rotations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| inflight | getattr | 4 | 64 | 274.2k | 0 | 848 µs | 2.43 ms | 169 % | 4.6 | 0 | 0 | 0 | 0 | 0 |
| pending | getattr | 4 | 64 | 275.7k | 0 | 864 µs | 2.43 ms | 173 % | 4.6 | 0 | 0 | 0 | 0 | 0 |
| wtimeout | getattr | 4 | 64 | 267.3k | 0 | 896 µs | 2.37 ms | 177 % | 4.5 | 0 | 0 | 0 | 0 | 0 |
| databudget | getattr | 4 | 64 | 274.0k | 0 | 880 µs | 2.30 ms | 174 % | 4.6 | 0 | 0 | 0 | 0 | 0 |
| drr-only | getattr | 4 | 64 | 289.9k | 0 | 816 µs | 2.24 ms | 176 % | 4.6 | 0 | 0 | 0 | 0 | 27282 |
| inflight | getattr | 4 | 512 | 321.6k | 0 | 5.89 ms | 13.82 ms | 184 % | 5.1 | 14861 | 0 | 0 | 0 | 0 |
| pending | getattr | 4 | 512 | 332.9k | 0 | 5.76 ms | 13.57 ms | 186 % | 5.0 | 15272 | 157 | 0 | 0 | 0 |
| wtimeout | getattr | 4 | 512 | 311.3k | 0 | 6.14 ms | 13.82 ms | 186 % | 5.1 | 14461 | 0 | 0 | 0 | 0 |
| databudget | getattr | 4 | 512 | 328.5k | 0 | 5.89 ms | 13.06 ms | 186 % | 5.0 | 15190 | 0 | 0 | 0 | 0 |
| drr-only | getattr | 4 | 512 | 333.0k | 0 | 6.02 ms | 12.80 ms | 188 % | 5.3 | 15379 | 0 | 0 | 0 | 46916 |

### E7b heavy+light quantum

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| drr --scheduler drr:8 | 3 | 57.0k | 0 | 34.82 ms | 636 | 0 | 1.38 ms | 3.39 ms | 52435 | 100191 | 0 |
| drr --scheduler drr:1 | 3 | 56.6k | 0 | 35.84 ms | 647 | 0 | 1.31 ms | 3.78 ms | 49874 | 792262 | 0 |

### E10b meta+deep stream

| server | runs | heavy ops/s | heavy MiB/s | heavy p50 | light ops/s | light MiB/s | light p50 | light p99 | pending waits | DRR rotations | conn data waits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| main --concurrency 32 | 3 | 2.0k | 2044 | 124.93 ms | 576 | 0 | 110.59 ms | 118.78 ms | - | - | - |
| inflight --concurrency 32 | 3 | 2.0k | 2046 | 124.93 ms | 586 | 0 | 108.54 ms | 118.78 ms | 0 | 0 | 0 |
| budgets --max-data-per-conn 512M --concurrency 32 | 3 | 2.0k | 2048 | 124.93 ms | 790 | 0 | 79.87 ms | 88.06 ms | 22773 | 0 | 0 |
| drr --max-data-per-conn 512M --concurrency 32 | 3 | 2.0k | 2048 | 124.93 ms | 4.3k | 0 | 15.87 ms | 30.21 ms | 49401 | 15800 | 0 |

