# Количественное сравнение исходного кода nfs-mamont и Rust-конкурентов (статический анализ; замеры от 2026-10-30)

Все числа ниже получены одними и теми же скриптами (`rs_metrics.py`, `deps_audit.py`, `ci_scan.py`, `make_tables.py`; полный текст в последнем разделе), применёнными к git-клонам на зафиксированных SHA. Сторонний код не собирался и не запускался: только чтение файлов как текста/TOML/YAML (`python3 -I`, рабочий каталог вне загрузок). `cargo` запускался только для nfs-mamont (с `--manifest-path` и `CARGO_TARGET_DIR` во временном каталоге; репозиторий не изменялся, target-каталог удалён после замеров).

## 1. Что именно измерено: кодовые базы, источники, версии и SHA

### Takeaway
Восемь кодовых баз измерены по git-клонам (основной источник) на фиксированных коммитах; tarball'ы crates.io использованы для сверки. Для трёх репозиториев HEAD новее опубликованной версии, но по метрикам кода совпадает с релизом (кроме nfs-mamont); fractal-nfs отсутствует в HEAD своего репозитория и измерен по тегу релиза `fractal-nfs-0.1.0`.

### Cited Findings
| Кодовая база (что входит в замер) | Источник | Измеренный коммит (дата) | Опубликованная версия → коммит публикации из `.cargo_vcs_info.json` | Код замера = релиз? |
|---|---|---|---|---|
| nfs-mamont (workspace-крейт `nfs-mamont/`) | локальный checkout `/home/user/nfs-mamont` | `5eae9a536f4ee82ff79c49366f0e9a62d3841b7f` (2026-10-29) | 0.2.0 → `d0f2317a5a84` | нет: HEAD новее (prod code 5512 vs 5504 строк, 336 vs 333 тестов) |
| nfsserve | github.com/huggingface/nfsserve (полный depth-1 clone) | `327b45530f04664da678297ce2568c47bf3fbf81` (2026-10-14) | 0.11.0 → `af4f1176dd70` | метрики src идентичны tarball 0.11.0 |
| zerofs_nfsserve | github.com/barre/nfsserve (URL из crates.io) | `d61b08456ae66108666978e29524a47d2209f68d` (2026-03-02) | 0.19.1 → `d61b08456ae6` | да (HEAD = релиз) |
| nfs3_server + nfs3_types + nfs3_macros (+ тестовый крейт `nfs3_tests`, учтён целиком как test code) | github.com/Vaiz/nfs3 | `8179e9d66d655242a4aadd2153a9ef8ba8b1c083` (2026-10-21) | 0.11.1 / 0.5.3 / 0.5.1 → `8179e9d66d65` | да |
| embednfs + embednfs-proto | github.com/PeronGH/embednfs | `ac1a01641247790c5df414192f979e42c6cdaffb` (2026-03-10) | 0.4.1 / 0.4.1 → `168a11f80cfa` | метрики кода идентичны |
| nessie-nfsserve | github.com/Gilamonster-Foundation/nessie-store, `crates/nessie-nfsserve` (sparse, blob-less clone) | `0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b` (2026-10-26) | 0.3.1 → `805ef33b2efa` | метрики кода идентичны; `rust-version` в HEAD = 1.96 (crates.io для 0.3.1 сообщает 1.88) |
| fractal-nfs | github.com/fractalbits-labs/fractalbits-main, `crates/fs_server/fractal-nfs`, тег `fractal-nfs-0.1.0` | `86200e912bafc6f0d29a02733974ae9eddac9080` (2026-05-21) | 0.1.0 → `86200e912baf` | да; в HEAD `main` (`caa883bc28a2`, 2026-09-18) каталога крейта нет |
| nfs-v3 (семейство NFSWolf) | github.com/StrongWind1/NFSWolf, `crates/nfs-v3` (sparse) | `e067096480626c2a7134fa45e47e0ce8bca4b9f6` (2026-09-12) | 1.2.0 → `4b1e035e62b6` | метрики кода идентичны |
| RustSec advisory-db (данные) | github.com/rustsec/advisory-db | `2295466c4e9df26647fd67d8fa1c705ac2fc7f81` (2026-10-30 12:12 +01:00) | — | 1375 advisories из `crates/` |

Источники: [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d), [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve), [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3), [advisory-db@2295466](https://github.com/rustsec/advisory-db/tree/2295466c4e9df26647fd67d8fa1c705ac2fc7f81); SHA публикаций — из `.cargo_vcs_info.json` в tarball'ах `https://crates.io/api/v1/crates/<name>/<version>/download`.

- Время замеров: 2026-10-30, ~19:55–20:45 UTC. Окружение: Linux 6.18, 4 vCPU, Python 3.13.17 (PyYAML 6.0.1 из системы, `tomllib`), rustc/cargo 1.97.0, clippy 0.1.97, rustfmt 1.9.0 — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f) (замер, лог `results/cargo/summary.txt`).
- Во время замеров в локальный репозиторий nfs-mamont параллельными агентами были добавлены только docs-коммиты (`12f7cda`, `72634d0` с файлами `research_notes/…`); код совпадает с `5eae9a5` (`git diff --stat 5eae9a5..HEAD` показывает только 2 `.md`-файла) — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f).
- История релизов по crates.io API (на 2026-10-30): nfs-mamont — 3 версии, первая 2026-10-09, последняя 0.2.0 от 2026-10-26, 80 загрузок; nfsserve — 3 версии с 2023-06-05, 0.11.0 от 2026-04-01, 1 415 987 загрузок (1 132 218 за 90 дней); zerofs_nfsserve — 13 версий (2 yanked) с 2025-07-18, 0.19.1 от 2026-03-02, 53 245; nfs3_server — 15 версий с 2025-02-09, 0.11.1 от 2026-10-21, 43 752 (nfs3_types 61 536); embednfs — 13 версий за 2026-03-08…2026-03-10, 2 434; nessie-nfsserve — 1 версия (0.3.1, 2026-06-26), 760; fractal-nfs — 1 версия (0.1.0, 2026-05-21), 44; nfs-v3 — 3 версии 2026-08-08…2026-08-18, 147 — [crates.io API nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont), [nfsserve](https://crates.io/api/v1/crates/nfsserve), [zerofs_nfsserve](https://crates.io/api/v1/crates/zerofs_nfsserve), [nfs3_server](https://crates.io/api/v1/crates/nfs3_server), [embednfs](https://crates.io/api/v1/crates/embednfs), [nessie-nfsserve](https://crates.io/api/v1/crates/nessie-nfsserve), [fractal-nfs](https://crates.io/api/v1/crates/fractal-nfs), [nfs-v3](https://crates.io/api/v1/crates/nfs-v3).
- Сопоставимость: embednfs — «Embeddable NFSv4.1 server library» (другой протокол) — [crates.io embednfs](https://crates.io/api/v1/crates/embednfs); nfs-v3 — «A complete NFSv3 client» (клиент, не сервер) — [nfs-v3 lib.rs](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/src/lib.rs).
- Родословная: nessie-nfsserve — «vendored, hardened fork of nfsserve (v0.11.0)» — [nessie-nfsserve README](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve/README.md); zerofs_nfsserve содержит код «original nfsserve project by XetData» (BSD-3), модификации под AGPL-3.0 — [LICENSE-MODIFICATIONS](https://github.com/barre/nfsserve/blob/d61b08456ae66108666978e29524a47d2209f68d/LICENSE-MODIFICATIONS); среди авторов nfs3_server указан «Yucheng Low <ylow@xethub.com>» (он же в authors zerofs_nfsserve) — [nfs3_server Cargo.toml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/crates/nfs3_server/Cargo.toml); nfs-v3 «contains code derived from the Vaiz/nfs3 project» (nfs3_types/src/nfs3.rs → src/wire/types.rs и др.) — [nfs-v3 NOTICE](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/NOTICE).

### Inferences
- Конкуренты образуют две «линии»: семейство nfsserve (nfsserve, zerofs_nfsserve, nessie-nfsserve, и в переработанном виде nfs3_server) и независимые реализации (nfs-mamont, embednfs, fractal-nfs, nfs-v3-клиент, производный от nfs3_types). Метрики внутри семейства nfsserve коррелированы (почти одинаковые счётчики unwrap, `#[allow]`, doc-ratio), поэтому их стоит читать как «одну кодовую базу в трёх вариантах», а не как три независимых наблюдения.
- Для прямого сравнения с nfs-mamont (async NFSv3-сервер-библиотека) ближе всего nfsserve-семейство, nfs3_server и fractal-nfs; embednfs (NFSv4.1) и nfs-v3 (клиент) — контекст для «планки качества», а не прямые аналоги.

### Gaps
- Клоны с `--depth 1` (а у nfs-mamont локально shallow-история из 56 коммитов): метрики частоты коммитов, числа контрибьюторов, скорости закрытия issues не считались.
- По условию задачи код конкурентов не собирался и не запускался, поэтому для них нет pass/fail тестов, покрытия, времени сборки и предупреждений компилятора/clippy.

## 2. Размер кода: LOC (code/comment/blank), число .rs-файлов, крупнейшие файлы

### Takeaway
nfs-mamont — средний по объёму продакшн-кода (5512 строк code; больше только embednfs 9613 и nfs3 5861), но с самым дробным устройством (153 prod-файла, в среднем 36 строк кода на файл против 172–261 у остальных; крупнейший prod-файл — 400 строк против 975–1795 у конкурентов) и с самой высокой плотностью комментариев (45.6 строк комментариев/док-комментариев на 100 строк кода).

### Cited Findings
Таблица размера (prod = код, достижимый из lib/bin, вне `#[cfg(test)]`/тест-атрибутов; test = `tests/`, файлы, подключённые только через `#[cfg(test)] mod`, и cfg(test)/#[test]-регионы; dead = файлы, не достижимые ни из одного target root, т.е. никогда не компилируемые):

| Кодовая база | .rs всего | prod-файлов | in-src test-файлов | файлов в tests/ (+ тест-крейт) | examples | benches | dead | prod code | prod comment | prod doc-comment | prod blank | test code | all code (все .rs) | dead code |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 240 | 153 | 87 | 0 | 0 | 0 | 0 | 5512 | 77 | 2436 | 1074 | 5027 | 10539 | 0 |
| nfsserve | 19 | 16 | 0 | 0 | 2 | 0 | 1 | 3272 | 792 | 375 | 329 | 0 | 4119 | 0 |
| zerofs_nfsserve | 19 | 16 | 0 | 0 | 2 | 0 | 1 | 4173 | 809 | 394 | 365 | 0 | 5269 | 0 |
| nfs3_server+types+macros | 64 | 34 | 0 | 25 (11 в `tests/` + 14 src-файлов `nfs3_tests`) | 4 | 1 | 0 | 5861 | 144 | 399 | 795 | 6223 | 13294 | 0 |
| embednfs+proto | 85 | 41 | 3 | 41 | 0 | 0 | 0 | 9613 | 68 | 264 | 890 | 9634 | 19247 | 0 |
| nessie-nfsserve | 17 | 16 | 0 | 0 | 0 | 0 | 1 | 3576 | 806 | 418 | 344 | 22 | 3598 | 0 |
| fractal-nfs | 11 | 10 | 0 | 1 | 0 | 0 | 0 | 2220 | 63 | 133 | 304 | 182 | 2402 | 0 |
| nfs-v3 | 10 | 7 | 0 | 0 | 0 | 0 | 3 | 1750 | 7 | 408 | 279 | 453 | 2587 | 384 |

Источники: репозитории на SHA из раздела 1; скрипт `rs_metrics.py` (раздел 11) — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d), [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve), [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3).

Производные показатели (prod): среднее число строк кода на prod-файл и комментарии (comment + doc-comment) на 100 строк кода — nfs-mamont 36 / 45.6; nfsserve 204 / 35.7; zerofs_nfsserve 261 / 28.8; nfs3 172 / 9.3; embednfs 234 / 3.5; nessie-nfsserve 224 / 34.2; fractal-nfs 222 / 8.8; nfs-v3 250 / 23.7. Число `fn` в prod-коде (из них `pub fn`): nfs-mamont 381 (241), nfsserve 146 (55), zerofs 164 (60), nfs3 395 (96), embednfs 356 (34), nessie 153 (57), fractal 146 (110), nfs-v3 120 (80) — тот же скрипт, те же SHA.

Крупнейшие файлы (code lines; категория):

| Кодовая база | #1 | #2 | #3 |
|---|---|---|---|
| nfs-mamont | `src/parser/tests/parser_struct.rs` 906 (test) | `src/parser/parser_struct.rs` 400 (prod) | `src/allocator/tests/slice/iter_mut.rs` 390 (test); крупнейшие prod далее: `serializer/server/serialize_struct.rs` 355, `task/global/vfs.rs` 283 |
| nfsserve | `src/nfs_handlers.rs` 1311 (prod) | `examples/mirrorfs.rs` 529 | `src/nfs.rs` 409 |
| zerofs_nfsserve | `src/nfs_handlers.rs` 1795 (prod) | `examples/mirrorfs.rs` 698 | `src/nfs.rs` 647 |
| nfs3 | `nfs3_types/src/nfs3.rs` 975 (prod) | `nfs3_tests/src/bin/test_mirror_fs/readonly.rs` 867 (test) | `nfs3_server/src/nfs_handlers.rs` 855 (prod) |
| embednfs | `embednfs/tests/file_ops_cases/attrs_state.rs` 1043 (test) | `embednfs-proto/src/types/codec.rs` 975 (prod) | `embednfs/src/server/compound.rs` 689 (prod) |
| nessie-nfsserve | `src/nfs_handlers.rs` 1454 (prod) | `src/nfs.rs` 417 | `src/rpc.rs` 399 |
| fractal-nfs | `src/nfs3_wire.rs` 507 (prod) | `src/nfs3_types.rs` 391 | `src/dispatch.rs` 358 |
| nfs-v3 | `src/wire/types.rs` 1357 (prod) | `src/wire/mount.rs` 301 (dead) | `src/api/ops.rs` 287 |

Источник: те же SHA, `rs_metrics.py` (поле `largest`).

- Мёртвые файлы: в nfsserve, zerofs_nfsserve и nessie-nfsserve файл `src/config.rs` (1 строка) не объявлен в `lib.rs` — [nfsserve lib.rs](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/src/lib.rs); в nfs-v3 три файла (`src/error_mount.rs`, `src/mount.rs`, `src/wire/mount.rs`, 384 строки кода) не подключены ни одним `mod` (MOUNT-типы реэкспортируются из `nfs_mount` через inline `pub mod mount { pub use nfs_mount::wire::{…} }`) и при этом входят в опубликованный tarball 1.2.0 — [nfs-v3 wire/mod.rs](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/src/wire/mod.rs).
- 27 из 87 test-файлов nfs-mamont лежат не в `tests/`-каталогах, а в `src/serializer/client/**`: модуль подключён как `#[allow(dead_code)] #[cfg(test)] pub mod client;` (клиентские сериализаторы аргументов только для тестов) — [serializer/mod.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/serializer/mod.rs).
- Валидация лексера: если повторить правило crates.io (исключить каталоги `tests`/`benches`/`examples` и не считать doc-комментарии), наш счётчик воспроизводит поле `linecounts` crates.io байт-в-байт для 10 из 11 tarball'ов (nfs-mamont 0.2.0: 5977 code / 79 comment / 179 файлов; nfsserve 0.11.0: 3272/792/17; zerofs 0.19.1: 4173/809/17; nfs3_server 0.11.1: 4015/130/20; nfs3_types 0.5.3: 1907/20/13; nfs3_macros 0.5.1: 382/2/1; embednfs 0.4.1: 7808/66/35; embednfs-proto 0.4.1: 3265/2/9; nessie-nfsserve 0.3.1: 3598/809/17; fractal-nfs 0.1.0: 2334/67/10), для nfs-v3 1.2.0 расхождение +2 строки code (2587 vs 2585), comment совпадает (113) — [crates.io API nfs-mamont 0.2.0](https://crates.io/api/v1/crates/nfs-mamont/0.2.0), [nfsserve 0.11.0](https://crates.io/api/v1/crates/nfsserve/0.11.0), [nfs-v3 1.2.0](https://crates.io/api/v1/crates/nfs-v3/1.2.0) (скрипт `crosscheck_linecounts.py`).
- Сверка с опубликованными tarball'ами тем же `rs_metrics.py`: nfs-mamont 0.2.0 — 239 файлов, prod code 5504, test code 4867, 333 тест-функции (HEAD: 240 / 5512 / 5027 / 336); nfsserve 0.11.0, zerofs 0.19.1, nessie 0.3.1, fractal 0.1.0, nfs-v3 1.2.0, embednfs(+proto) 0.4.1 — prod/test-строки совпадают с git-замером; tarball'ы nfsserve/zerofs/nfs3_server не содержат `examples/` и `tests/` (`include = ["src/**/*", …]`), тест-крейт `nfs3_tests` не публикуется — [nfsserve Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/Cargo.toml), [nfs3_server Cargo.toml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/crates/nfs3_server/Cargo.toml).

### Inferences
- Дробность nfs-mamont (один файл на процедуру на каждом слое: parser / serializer / vfs / service) снижает «god-file»-риск, характерный для семейства nfsserve (`nfs_handlers.rs` 1311–1795 строк), и упрощает ревью; цена — 240 файлов и большая поверхность модулей.
- Высокая плотность doc-комментариев (2436 строк на 5512 строк кода) — сильная сторона nfs-mamont; у nfsserve-семейства много обычных (не doc) комментариев, у nfs3/embednfs/fractal комментариев почти нет.
- Совпадение с `linecounts` crates.io (10/11 точно) говорит, что классификация строк у нашего лексера эквивалентна той, что использует crates.io, и цифры воспроизводимы независимо.

### Gaps
- Метрики сложности (цикломатическая сложность, вложенность, длина функций) не считались — для этого нужен полноценный парсер Rust (syn/rust-analyzer), что вне ограничений «только свои Python-скрипты».
- `crates.io` не документирует (в найденных нами источниках) точное правило исключения каталогов; вывод «исключаются tests/benches/examples и doc-комментарии» — реконструкция по точному совпадению чисел.

## 3. Тесты: количество, интеграционные тесты, property/fuzz/bench, отношение test/code

### Takeaway
По числу и плотности тестов nfs-mamont лидирует (336 тест-функций, 61 на 1K строк prod-кода; следующие — nfs3 28.2, embednfs 26.9, nfs-v3 22.3), а отношение test/code 0.91 сопоставимо с лучшими (embednfs 1.00, nfs3 1.06 с отдельным тест-крейтом). Но все тесты nfs-mamont — модульные внутри `src/`: нет `tests/`, end-to-end-проверок с реальным NFS-клиентом, property-тестов, fuzz-таргетов (fuzz-workflow есть, но целей нет и он ни разу не запускался) и бенчмарков. nfsserve и zerofs_nfsserve не имеют ни одного теста, nessie-nfsserve — 2.

### Cited Findings
| Кодовая база | test fns | `#[test]` | `#[tokio::test]` | `#[ignore]` | файлов в `tests/` (test targets) | in-src test-module файлов | doc-test rust-блоков (doctest) | proptest/quickcheck | cargo-fuzz targets | benches | test code / prod code | test fns на 1K prod LOC |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 336 | 270 | 66 | 0 | 0 | 87 | 2 (on) | нет | нет (есть `fuzz.yml`, нет `fuzz/`) | нет | 0.912 | 61.0 |
| nfsserve | 0 | 0 | 0 | 0 | 0 | 0 | 0 (`doctest = false`) | нет | нет | нет | 0.0 | 0.0 |
| zerofs_nfsserve | 0 | 0 | 0 | 0 | 0 | 0 | 0 (`doctest = false`) | нет | нет | нет | 0.0 | 0.0 |
| nfs3_server+types+macros | 165 (88 из них в `nfs3_tests`) | 76 | 89 | 0 | 11 (`nfs3_server` 1, `nfs3_types` 5, `nfs3_tests` 5) + 14 src-файлов тест-крейта | 0 | 1 (`nfs3_server`: off; types/macros: on) | нет | нет | 1 файл, criterion (`nfs3_tests/benches/perf_bench.rs`) | 1.062 (0.245 без `nfs3_tests`) | 28.2 (13.1 без `nfs3_tests`) |
| embednfs+proto | 259 | 24 | 235 | 2 | 41 (10 test targets) | 3 | 0 (on) | нет | нет | нет | 1.002 (embednfs 1.41; proto 0.10) | 26.9 |
| nessie-nfsserve | 2 | 2 | 0 | 0 | 0 | 0 | 0 (`doctest = false`) | нет | нет | нет | 0.006 | 0.6 |
| fractal-nfs | 12 | 12 | 0 | 0 | 1 | 0 | 0 + 1 rust-блок в README (`#![doc = include_str!`) | нет | нет | нет | 0.082 | 5.4 |
| nfs-v3 | 39 (+11 в мёртвых файлах) | 39 | 0 | 0 | 0 | 0 | 0 (on) | нет | нет | нет | 0.259 | 22.3 |

Источники: репозитории на SHA из раздела 1 — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d), [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve), [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3); `rs_metrics.py`.

- Статический счётчик совпал с фактическим запуском у nfs-mamont: 336 тест-функций и 2 doc-test-блока статически ↔ `cargo test`: «336 passed; 0 failed; 0 ignored» (unittests src/lib.rs) и «Doc-tests nfs_mamont … 2 passed» — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f) (лог `results/cargo/test.log`).
- Тесты nfs-mamont не трогают файловую систему: в `src/` нет `std::fs`/`tokio::fs`/tempfile; сетевые тесты rpcbind поднимают `TcpListener::bind("127.0.0.1:0")` в процессе — [rpcbind/tests/mod.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/rpcbind/tests/mod.rs).
- nfs-mamont: workflow `fuzz.yml` (только `workflow_dispatch`) вызывает `cargo +nightly fuzz run "${{ inputs.target }}"`, но каталога `fuzz/` в дереве нет — [fuzz.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/workflows/fuzz.yml); GitHub API: у `fuzz.yml` 0 запусков (`total_count: 0`) — [Actions nfs-mamont](https://github.com/RMamonts/nfs-mamont/actions/workflows/fuzz.yml).
- nfs3: интеграционные тесты в отдельном крейте `nfs3_tests` (5 файлов `tests/`, бинарь `test_mirror_fs`, бенчмарк на criterion), CI выполняет `cargo run -p nfs3_tests --bin test_mirror_fs` на 4 целевых платформах — [rust.yml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/.github/workflows/rust.yml).
- embednfs: 10 интеграционных test target'ов (compound_session, create_special, directory, file_ops, filehandle, lock_ops, nfs_rs_smoke, transport_fragmentation, nfs4j_smoke, nfs4j_stress); два последних помечены `#[ignore = "requires external nfs4j harness"]`; dev-зависимость — git-клиент `nfs-rs` (NetAppLabs) на фиксированной ревизии — [embednfs tests/](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb/crates/embednfs/tests), [embednfs Cargo.toml](https://github.com/PeronGH/embednfs/blob/ac1a01641247790c5df414192f979e42c6cdaffb/crates/embednfs/Cargo.toml).
- Ни в одной из измеряемых кодовых баз нет `fuzz/`-каталогов с cargo-fuzz и использования proptest/quickcheck/arbitrary/bolero/loom/kani; в монорепо такие артефакты есть только вне измеряемых крейтов: `crates/nessie-backend-core/tests/proptest_roundtrip.rs` (nessie-store) и `tests/integration/xdr_fuzz_test.rs` + `proptest` в dev-deps корневого крейта `nfswolf` (не `nfs-v3`) — [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b), [NFSWolf Cargo.toml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/Cargo.toml).

### Inferences
- nfs-mamont по тестовой дисциплине (плотность, test/code, все тесты проходят) — верхняя группа вместе с nfs3 и embednfs; семейство nfsserve в этом измерении фактически не тестируется (0–2 теста на 3.3–4.2K строк), что объясняет, почему nessie пришлось «hardening»-форкать upstream.
- Главный тестовый риск nfs-mamont — не количество, а вид тестов: отсутствуют интероп/E2E-тесты (реальный клиент Linux/macOS или библиотека-клиент, как `nfs-rs` у embednfs или `nfs3_client`/`test_mirror_fs` у nfs3) и fuzz/property-тесты для рукописного XDR/RPC-парсера, который принимает недоверенный сетевой ввод. Наличие неработающего `fuzz.yml` создаёт ложное впечатление, что фаззинг есть.
- Отношение test/code у nfs3 сильно зависит от методики (1.06 с тест-крейтом vs 0.25 без), поэтому для честного сравнения важно включать тест-крейты; мы их включили.

### Gaps
- Покрытие кода (llvm-cov/tarpaulin) ни у кого не публикуется в CI, а запускать тесты конкурентов нельзя — сравнение покрытия невозможно.
- Число «assert на тест» и качество ассертов не оценивалось.

## 4. Безопасность и робастность: unsafe, паникующие вызовы, линты

### Takeaway
По паникующим вызовам в prod-коде nfs-mamont — в лучшей группе (8 сайтов, 1.45 на 1K LOC; лучше только embednfs 0.21 и fractal-nfs 0.90; у nfsserve-семейства 7.4–9.2, у nfs3 8.2). Зато по `unsafe` nfs-mamont — самый «небезопасный» в наборе (8 `unsafe {}`, 1 `unsafe fn`, 4 `unsafe impl Send/Sync`, всё в zero-copy аллокаторе), без единого `// SAFETY:`-комментария, без lint-ограничений (`[lints]`, `#![forbid/deny]`) и без Miri в CI.

### Cited Findings
Абсолютные значения в prod-коде (токенный счёт: строки/комментарии не считаются; `.unwrap()` = `.` `unwrap` `(` `)`):

| Кодовая база | `unsafe {}` | `unsafe fn` | `unsafe impl` | `// SAFETY:` | `.unwrap()` | `.expect(` | `panic!` | `unreachable!` | `todo!` | `unimplemented!` | Σ паникующих | атрибутов `#[allow]` / `#[expect]` (prod) | TODO/FIXME (весь код) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 8 | 1 | 4 | 0 | 2 | 4 | 1 | 1 | 0 | 0 | 8 | 8 / 0 (все `#[allow(dead_code)]`) | 13 |
| nfsserve | 1 | 0 | 0 | 0 | 27 | 2 | 0 | 1 | 0 | 0 | 30 | 84 / 0 | 3 |
| zerofs_nfsserve | 0 | 0 | 0 | 0 | 28 | 2 | 0 | 0 | 0 | 1 | 31 | 98 / 0 | 3 |
| nfs3_server+types+macros | 0 | 0 | 0 | 0 | 3 | 33 | 10 | 2 | 0 | 0 | 48 | 27 / 2 | 4 |
| embednfs+proto | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 2 | 0 / 10 (с `reason = …`) | 0 |
| nessie-nfsserve | 1 | 0 | 0 | 0 | 28 | 2 | 0 | 1 | 0 | 0 | 31 | 86 / 0 | 3 |
| fractal-nfs | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 2 | 4 / 0 | 0 |
| nfs-v3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 3 | 0 / 7 (с `reason = …`) | 0 |

На 1000 строк prod-кода:

| Кодовая база | prod code | unsafe blocks | unwrap | expect | panic! | unreachable! | todo!+unimplemented! | Σ паникующих / KLOC |
|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 5512 | 1.45 | 0.36 | 0.73 | 0.18 | 0.18 | 0.00 | 1.45 |
| nfsserve | 3272 | 0.31 | 8.25 | 0.61 | 0.00 | 0.31 | 0.00 | 9.17 |
| zerofs_nfsserve | 4173 | 0.00 | 6.71 | 0.48 | 0.00 | 0.00 | 0.24 | 7.43 |
| nfs3_server+types+macros | 5861 | 0.00 | 0.51 | 5.63 | 1.71 | 0.34 | 0.00 | 8.19 |
| embednfs+proto | 9613 | 0.00 | 0.00 | 0.21 | 0.00 | 0.00 | 0.00 | 0.21 |
| nessie-nfsserve | 3576 | 0.28 | 7.83 | 0.56 | 0.00 | 0.28 | 0.00 | 8.67 |
| fractal-nfs | 2220 | 0.00 | 0.90 | 0.00 | 0.00 | 0.00 | 0.00 | 0.90 |
| nfs-v3 | 1750 | 0.00 | 0.00 | 0.00 | 1.71 | 0.00 | 0.00 | 1.71 |

Источники: SHA из раздела 1, `rs_metrics.py` (`_count_safety`) — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d), [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve), [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3).

- Разбивка nfs3 по крейтам: nfs3_server — 32 `expect`, 3 `unwrap`, 2 `unreachable!`; nfs3_types — 3 `panic!` (в unwrap-подобных хелперах `Nfs3Result`/`Nfs3Option`), 1 `expect`; nfs3_macros (proc-macro) — 7 `panic!` (ошибки разбора derive-ввода на этапе компиляции) — [nfs3_types/src/nfs3.rs](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/crates/nfs3_types/src/nfs3.rs), [nfs3_macros/src/lib.rs](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/crates/nfs3_macros/src/lib.rs). Те же 3 `panic!` (с теми же текстами) есть в nfs-v3 `src/wire/types.rs` под `#[expect(clippy::panic)]` — [nfs-v3 wire/types.rs](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/src/wire/types.rs).
- В nfsserve 18 из 27 `unwrap()` находятся в `nfs_handlers.rs` и имеют вид «guarded unwrap» (`if let Err(stat) = id { …; return Ok(()); } let id = id.unwrap();`) — [nfsserve nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/src/nfs_handlers.rs); аналогичный шаблон `iter.unwrap()` после проверки `Err` есть в nfs3_server — [nfs3_server nfs_handlers.rs](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/crates/nfs3_server/src/nfs_handlers.rs).
- nfs-mamont: весь `unsafe` — в `src/allocator/mod.rs` (`unsafe impl Send/Sync for AllocatorState`, `alloc::alloc_zeroed`/`dealloc`, `libc::mlock` под feature `mlock`, `current_ptr.add`) и `src/allocator/buffer.rs` (`pub unsafe fn from_raw_parts`, `unsafe impl Send/Sync for UnownedBuffer`, `std::slice::from_raw_parts(_mut)` в `Deref`/`DerefMut`); из комментариев о безопасности есть только один `/// # Safety` у `from_raw_parts`; паникующие сайты prod: 4 `expect` и 1 `panic!` («mlock failed») и 1 `unreachable!` — тоже в аллокаторе, плюс 2 `unwrap`, оба невозможные по построению: `parser_struct.rs:658` `NonZeroUsize::new(args.count as usize).unwrap()` стоит в ветке `else` после `if args.count == 0 { A::Buffer::empty() }`, а `read_buffer.rs:113` — `try_into()` среза длины ровно `RMS_HEADER_SIZE` в массив `[u8; RMS_HEADER_SIZE]` после `ensure_buffered(RMS_HEADER_SIZE)` — [read_buffer.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/parser/read_buffer.rs), [allocator/mod.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/allocator/mod.rs), [allocator/buffer.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/allocator/buffer.rs), [parser_struct.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/parser/parser_struct.rs). Токенные счётчики сверены с `grep` построчно (совпадение 8/1/4/4/1/1/2).
- `#[allow]` у nfsserve-семейства — в основном `#[allow(non_camel_case_types)]` (64/78/66 шт.) на XDR-типах с RFC-именами — тот же скрипт (`allow_attrs_prod`).

Линты и конфигурация:

| Кодовая база | `#![..]` в корне крейта | `[lints]` | ключевые уровни | конфиги |
|---|---|---|---|---|
| nfs-mamont | нет (`lib.rs` начинается с одной строки `//!`) | нет | — | `.rustfmt.toml` (`use_small_heuristics = "Max"`), `.github/dependabot.yml` |
| nfsserve | `#![cfg_attr(feature = "strict", deny(warnings))]` | нет | — | `rustfmt.toml` (unstable опции), dependabot |
| zerofs_nfsserve | `#![cfg_attr(feature = "strict", deny(warnings))]` | нет | — | — |
| nfs3 | `#![cfg_attr(docsrs, feature(doc_cfg))]` | `workspace` | `clippy::pedantic = deny`, `clippy::nursery = deny`, `clippy::unwrap_used = deny`, `enum_glob_use = deny` | `rustfmt.toml`, dependabot, `.github/rust-toolchain.env` (1.97.0 + nightly rustfmt), CHANGELOG'и |
| embednfs | — | `workspace` | `clippy::{unwrap_used, expect_used, panic, todo, unimplemented, unreachable, indexing_slicing, panic_in_result_fn, undocumented_unsafe_blocks, allow_attributes(_without_reason)} = warn`, `rust::{unreachable_pub, unused_results} = warn` | — |
| nessie-nfsserve | — | нет (у крейта) | — | `rust-toolchain.toml` (1.96.1), `justfile` |
| fractal-nfs | — | нет | — | `.rustfmt.toml`, `justfile`, `flake.nix` |
| nfs-v3 | — | `workspace` | `rust::unsafe_code = deny`, `rust::missing_docs = warn`, ~100 rustc-линтов на forbid/deny/warn, `clippy::{all, pedantic, cargo} = deny`, `clippy::{unwrap_used, expect_used, panic, indexing_slicing, undocumented_unsafe_blocks} = warn`, `todo/unimplemented/dbg_macro = deny` | `deny.toml`, `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml` (1.95.0), `SECURITY.md`, `Makefile` |

Источники: [nfs-mamont Cargo.toml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/Cargo.toml), [nfs-mamont lib.rs](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/nfs-mamont/src/lib.rs), [nfsserve lib.rs](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/src/lib.rs), [nfs3 Cargo.toml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/Cargo.toml), [embednfs Cargo.toml](https://github.com/PeronGH/embednfs/blob/ac1a01641247790c5df414192f979e42c6cdaffb/Cargo.toml), [NFSWolf Cargo.toml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/Cargo.toml), [nessie rust-toolchain.toml](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/rust-toolchain.toml).

### Inferences
- Низкий счёт паникующих вызовов nfs-mamont — реальное достоинство при обработке сетевого ввода: в коде разбора/сериализации паник почти нет, исключения сосредоточены в инициализации аллокатора (`expect("size overflow")`, `panic!("mlock failed …")`), т.е. срабатывают при старте, а не на запросе клиента. Оба `unwrap` в парсере защищены предшествующими проверками, так что на путях обработки запросов у nfs-mamont фактически нет паникующих вызовов, выявляемых этим счётчиком (индексация срезов `[a..b]` счётчиком не учитывается).
- Сравнение unwrap-счётчиков с nfsserve-семейством нужно делать осторожно: большая часть их `unwrap()` «защищена» предшествующей проверкой `Err`, т.е. фактический риск паники ниже, чем 8–9/KLOC. У nfs3 «unwrap → expect» — следствие `clippy::unwrap_used = deny`: формально это те же паникующие вызовы, но с сообщением.
- `unsafe`-аллокатор — главный технический риск nfs-mamont в этом срезе: `unsafe impl Send/Sync` для структур с сырыми указателями и выдача `&mut [u8]` из общего пула корректны только при соблюдении инвариантов, которые нигде не задокументированы `// SAFETY:`-комментариями и не проверяются ни Miri, ни линтами (`clippy::undocumented_unsafe_blocks`, `unsafe_op_in_unsafe_fn`). У конкурентов unsafe практически отсутствует (0–1 блок), а у nfs-v3 запрещён (`unsafe_code = deny`).
- Отсутствие `[lints]`-таблицы — разрыв с «лучшей практикой» набора (nfs3, embednfs, NFSWolf): nfs-mamont полагается только на дефолтный clippy с `-D warnings` в CI.

### Gaps
- Достижимость каждого паникующего сайта из сетевого ввода не анализировалась (нужен data-flow-анализ).
- Не проверялось, используют ли зависимости `unsafe` (cargo-geiger запрещён ограничением «не устанавливать инструменты»).

## 5. Зависимости, MSRV, edition, лицензия, lockfile и RustSec

### Takeaway
nfs-mamont — самый «лёгкий» по дереву зависимостей среди кодовых баз с lockfile (9 прямых normal-зависимостей, 35 пакетов в `Cargo.lock`, 34 внешних пакета в normal+build-замыкании против 36–39 у конкурентов) и единственный с MSRV 1.75 (остальные 1.88–1.96 или не указан). Сверка с advisory-db: в lockfile nfs-mamont совпадает одно informational-предупреждение (tokio 1.53.1, RUSTSEC-2026-0386, unsound, `AsyncFd`), которое затрагивает и nfs3, embednfs, nessie, nfs-v3; ни одна кодовая база не использует `AsyncFd`.

### Cited Findings
| Кодовая база | прямые normal / dev / build (внешние, объединение) | optional | Cargo.lock | пакетов в lock (registry/git/local) | normal+build замыкание измеряемых крейтов (внешние пакеты) | замыкание с dev | дубли имён в замыкании | MSRV | edition | license |
|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 9 / 0 / 0 | 1 (`libc`) | есть | 35 (34/0/1) | 34 | 34 | `syn` (2.0.119 и 3.0.4) | 1.75.0 | 2021 | MIT |
| nfsserve | 10 / 0 / 0 | 2 | нет (в `.gitignore`), только прямые зависимости | — | — | — | — | не указан | 2021 | BSD-3-Clause |
| zerofs_nfsserve | 15 / 0 / 0 | 2 | нет, только прямые | — | — | — | — | не указан | 2021 | AGPL-3.0 |
| nfs3_server+types+macros | 7 / 3 / 0 | 1 | есть (workspace) | 181 (175/0/6) | 36 | 45 | `syn`, `windows-sys` | 1.88 | 2024 | BSD-3-Clause; Unlicense |
| embednfs+proto | 8 / 1 / 0 | 0 | есть | 186 (177/6/3) | 38 | 173 | — | 1.88 | 2024 | MIT |
| nessie-nfsserve | 8 / 0 / 0 | 0 | есть (монорепо) | 482 (466/0/16) | 39 | 39 | `syn` | 1.96 (HEAD) | 2021 | BSD-3-Clause AND (MIT OR Apache-2.0) |
| fractal-nfs | 10 / 0 / 0 | 5 (`tokio` и 4× `compio-*`) | нет на коммите релиза, только прямые | — | — | — | — | не указан | 2024 | Apache-2.0 |
| nfs-v3 | 1 / 0 / 0 (+3 внутренних: `onc-xdr`, `onc-rpc-client`, `nfs-mount`) | 0 | есть (workspace) | 199 (190/0/9) | 37 | 58 | `syn` | 1.95 | 2024 | Apache-2.0 |

Прямые normal-зависимости (как в Cargo.toml):
- **nfs-mamont**: async-channel 2.3.0, byteorder 1.5.0, crossbeam-queue 0.3.12, libc 0.2.186 [opt, feature `mlock`], num-derive 0.4.2, num-traits 0.2.19, tokio 1.51 (features rt, net, io-util, sync, time, macros, signal), tracing 0.1, trait-variant 0.1.2 — [Cargo.toml (workspace)](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/Cargo.toml).
- **nfsserve**: anyhow 1, async-trait 0.1.9, byteorder 1.4, filetime 0.2, num-derive 0.4, num-traits 0.2, tokio 1, tracing 0.1.31, [opt, только demo] tracing-subscriber 0.3, intaglio 1.6 — [Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/Cargo.toml).
- **zerofs_nfsserve**: как nfsserve + bytestream 0.4, futures 0.3.21, smallvec 1.10.0, tokio-util 0.7, tracing-attributes 0.1 — [Cargo.toml](https://github.com/barre/nfsserve/blob/d61b08456ae66108666978e29524a47d2209f68d/Cargo.toml).
- **nfs3**: anyhow 1.0.98, tokio 1.44.0, tracing 0.1.41, filetime 0.2.25 [opt]; nfs3_macros: proc-macro2, quote, syn 3.0.3; dev: intaglio 1.10, tracing-subscriber 0.3.20 — [Cargo.toml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/Cargo.toml).
- **embednfs**: async-trait 0.1.89, bytes 1.11.1, dashmap 6.1.0, thiserror 2.0.18, tokio 1.50.0 (features `full`), tracing 0.1.44; proto: bytes, num-derive, num-traits; dev: nfs-rs (git, rev 483ef9f…) — [embednfs Cargo.toml](https://github.com/PeronGH/embednfs/blob/ac1a01641247790c5df414192f979e42c6cdaffb/crates/embednfs/Cargo.toml).
- **nessie-nfsserve**: anyhow 1, async-trait 0.1, byteorder 1.4, filetime 0.2, num-derive 0.4, num-traits 0.2, tokio 1 (workspace features `full`), tracing 0.1 — [nessie-store Cargo.toml](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/Cargo.toml).
- **fractal-nfs**: bytes 1.11, libc 0.2, socket2 0.6, tokio-util 0.7, tracing 0.1, [opt] tokio 1.40, compio-runtime/net 0.11, compio-io 0.9, compio-buf 0.8 — [fractal-nfs Cargo.toml](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs/Cargo.toml).
- **nfs-v3**: tracing 0.1 + внутренние onc-xdr, onc-rpc-client, nfs-mount — [nfs-v3 Cargo.toml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/Cargo.toml).
- Lockfile nfs-mamont (format v3, 35 пакетов): async-channel 2.5.0, bytes 1.12.1, crossbeam-queue 0.3.13, tokio 1.53.1, tokio-macros 2.7.2 (→ syn 3.0.4), num-derive 0.4.2 / tracing-attributes 0.1.31 / trait-variant 0.1.3 (→ syn 2.0.119), windows-sys 0.61.2 и др. — [Cargo.lock](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/Cargo.lock).

RustSec (своя сверка версий из `Cargo.lock` с `versions.patched/unaffected` в advisory-db@2295466; withdrawn исключены):

| Кодовая база | в normal-замыкании измеряемых крейтов | только через dev-зависимости | в других крейтах workspace-lock |
|---|---|---|---|
| nfs-mamont | tokio 1.53.1 — RUSTSEC-2026-0386 (unsound) | — | — |
| nfsserve / zerofs_nfsserve / fractal-nfs | n/a (lockfile нет) | — | — |
| nfs3 | anyhow 1.0.101 — RUSTSEC-2026-0190 (unsound); tokio 1.49.0 — RUSTSEC-2026-0386 (unsound) | intaglio 1.12.0 — RUSTSEC-2026-0078 (vulnerability) | crossbeam-epoch 0.9.18 — RUSTSEC-2026-0204 (vulnerability); event-listener 5.4.1 — RUSTSEC-2026-0221 (unsound) |
| embednfs | tokio 1.50.0 — RUSTSEC-2026-0386 (unsound) | anyhow 1.0.102 (RUSTSEC-2026-0190), failure 0.1.8 (RUSTSEC-2020-0036 unmaintained, RUSTSEC-2019-0036 unsound), paste 1.0.15 (RUSTSEC-2024-0436 unmaintained), rand 0.9.2 (RUSTSEC-2026-0097 unsound) — всё через git dev-dep `nfs-rs` | — |
| nessie-nfsserve | anyhow 1.0.102 — RUSTSEC-2026-0190; tokio 1.52.3 — RUSTSEC-2026-0386 | — | 19 совпадений: rustls 0.23.40 (4 vuln), quinn-proto 0.11.16 (5 vuln), h2 0.4.15, pyo3 0.26.0 (2), rsa 0.9.10 (RUSTSEC-2023-0071), rustls-webpki 0.103.13, webpki-roots 1.0.8, webpki-root-certs 1.0.7, crypto-bigint 0.5.5, ctutils 0.4.2, rustls-pemfile 2.2.0 (unmaintained) |
| nfs-v3 | tokio 1.53.1 — RUSTSEC-2026-0386 (unsound) | — | — |

Источники: [advisory-db@2295466](https://github.com/rustsec/advisory-db/tree/2295466c4e9df26647fd67d8fa1c705ac2fc7f81), lockfiles на SHA из раздела 1, `deps_audit.py`.

- RUSTSEC-2026-0386 (tokio, 2026-10-13): informational = "unsound", категория memory-corruption, «`AsyncFd::new` and related constructors violate I/O safety»; patched `>= 1.51.6, < 1.52.0` и `>= 1.53.3`; unaffected `< 0.3.2` — [RUSTSEC-2026-0386.md](https://github.com/rustsec/advisory-db/blob/2295466c4e9df26647fd67d8fa1c705ac2fc7f81/crates/tokio/RUSTSEC-2026-0386.md). `AsyncFd` не встречается ни в одном из измеряемых крейтов (grep по всем деревьям) — SHA из раздела 1.
- RUSTSEC-2026-0190 (anyhow, 2026-06-25): unsound, «Unsoundness in `Error::downcast_mut()`», patched `>= 1.0.103` — [RUSTSEC-2026-0190.md](https://github.com/rustsec/advisory-db/blob/2295466c4e9df26647fd67d8fa1c705ac2fc7f81/crates/anyhow/RUSTSEC-2026-0190.md). RUSTSEC-2026-0078 (intaglio, vulnerability, «Symbol confusion after hasher panic»), patched `>= 1.13.3` — [RUSTSEC-2026-0078.md](https://github.com/rustsec/advisory-db/blob/2295466c4e9df26647fd67d8fa1c705ac2fc7f81/crates/intaglio/RUSTSEC-2026-0078.md). RUSTSEC-2026-0007 (bytes, «Integer overflow in `BytesMut::reserve`», patched `>= 1.11.1`) попадает на нижнюю границу требования `bytes = "1.11"` у fractal-nfs; у embednfs требование `bytes = "1.11.1"` — ровно исправленная версия — [RUSTSEC-2026-0007.md](https://github.com/rustsec/advisory-db/blob/2295466c4e9df26647fd67d8fa1c705ac2fc7f81/crates/bytes/RUSTSEC-2026-0007.md).
- Без lockfile (nfsserve, zerofs, fractal) проверялась только нижняя граница требований: `tokio = "1"` у nfsserve/zerofs/nessie допускает версии с RUSTSEC-2021-0072/0124, 2023-0005, 2025-0023, 2026-0386; `anyhow = "1"` — RUSTSEC-2026-0190; `intaglio = "1.6"` и `tracing-subscriber = "0.3"` (только demo-feature) — RUSTSEC-2023-0048/2026-0078 и RUSTSEC-2025-0055; у nfs-mamont нижняя граница `tokio = "1.51"` попадает под RUSTSEC-2026-0386. Для всех найденных случаев исправленная версия совместима с требованием (свежий resolve её выберет) — `deps_audit.py`, [advisory-db@2295466](https://github.com/rustsec/advisory-db/tree/2295466c4e9df26647fd67d8fa1c705ac2fc7f81).
- cargo-deny/cargo-audit: nfs-mamont — `cargo audit` в CI (`taiki-e/install-action` + `cargo audit`), `deny.toml` нет — [rust-ci.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/workflows/rust-ci.yml); NFSWolf — `deny.toml` (advisories с `unmaintained = "all"`, `unsound = "all"`, `yanked = "deny"`, лицензии, bans, sources) и workflow `security.yml` с `cargo deny check` и `cargo audit --deny warnings` (push/PR/schedule) — [deny.toml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/deny.toml), [security.yml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/.github/workflows/security.yml); nfs3 — `cargo-deny` лишь устанавливается в `copilot-setup-steps.yml` (среда Copilot-агента), в CI не запускается — [copilot-setup-steps.yml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/.github/workflows/copilot-setup-steps.yml); у остальных — ничего.

### Inferences
- Минимализм зависимостей nfs-mamont (нет anyhow/async-trait/dashmap/thiserror; async-трейты через `trait-variant`) — объективное преимущество для встраиваемой библиотеки: меньше поверхность supply-chain-риска и время сборки. Дубль `syn` (2 и 3) — от `tokio-macros` 2.7.2 vs остальных proc-macro; это общая ситуация экосистемы (есть у nfs3, nessie, nfs-v3).
- MSRV 1.75 — самый низкий в наборе и, в отличие от nessie (1.96) и nfs-v3 (1.95), оставляет совместимость со «старыми» дистрибутивными тулчейнами; при этом lockfile уже содержит `syn 3.0.4` и tokio 1.53 — поддержка 1.75 держится только пока CI-матрица 1.75.0 зелёная (см. раздел 6).
- Совпадение RUSTSEC-2026-0386 у nfs-mamont — низкий практический риск (`AsyncFd` не используется), но это видимый «красный флажок» для пользователей `cargo audit`/`cargo deny`; исправляется `cargo update -p tokio` (≥ 1.53.3). Вероятно, `cargo audit` в CI nfs-mamont не падает на нём, т.к. по умолчанию informational-предупреждения (unsound/unmaintained) не дают ненулевой код выхода без `--deny` (не проверено запуском). Dependabot у nfs-mamont настроен только на `github-actions`, поэтому Cargo-зависимости не обновляются автоматически.
- Большой workspace-lock nessie-store (482 пакета, 19 совпадений с advisories в соседних крейтах) не задевает сам nessie-nfsserve, но показывает слабую гигиену зависимостей репозитория-владельца.

### Gaps
- Статус yanked для залоченных версий не проверялся (нужны десятки запросов к индексу crates.io).
- Сопоставление не учитывает `[affected.functions]`/`os`/`arch` из advisories, т.е. завышает реальную применимость (для tokio-0386 это видно по отсутствию `AsyncFd`).
- Для nfsserve/zerofs/fractal (нет lockfile) транзитивные версии неизвестны: реальный набор зависит от момента resolve у пользователя.

## 6. CI и автоматизация

### Takeaway
По полноте CI nfs-mamont — в первой тройке вместе с NFSWolf (nfs-v3) и nfs3: fmt, clippy `-D warnings`, тесты на MSRV 1.75.0 и 1.99.0, `cargo audit`, сборка документации с `-D warnings`, `cargo udeps`, проверка актуальности `Cargo.lock` и публикация на crates.io через trusted publishing. Отставания: только Linux (у nfs3 — Windows/Linux x64+arm64/macOS), нет coverage, Miri и реальных fuzz-таргетов, dependabot только для actions, actions не закреплены по SHA (0/20; у nfsserve 4/4, у NFSWolf 48/48). zerofs_nfsserve и embednfs не имеют CI вовсе.

### Cited Findings
| Кодовая база | Workflows | Тесты в CI | clippy | fmt | MSRV | OS/arch | audit/deny | docs | coverage | fuzz | release automation | обновления зависимостей | actions по SHA |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | `rust-ci.yml`, `release.yml`, `fuzz.yml` | `cargo test --verbose --all-features` на 1.75.0 и 1.99.0 | `--all-targets --all-features -- -D warnings` (1.99.0) | `cargo fmt --all --check` | да: build+test на 1.75.0 | ubuntu-latest | `cargo audit` | `cargo doc --all-features --no-deps --document-private-items`, `RUSTDOCFLAGS=-D warnings` | нет | только workflow, целей нет, 0 запусков | `cargo publish` по GitHub release, `rust-lang/crates-io-auth-action` (OIDC), проверка тег=версия, `--dry-run` | dependabot: `github-actions` | 0/20 |
| nfsserve | `ci.yml` | `cargo test --features strict` (тестов 0) | `cargo clippy -r -- -D warnings` | `cargo +nightly fmt --check` | нет | Linux | нет | нет | нет | нет | нет | dependabot: `github-actions` | 4/4 |
| zerofs_nfsserve | нет `.github/` | — | — | — | — | — | — | — | — | — | — | нет | — |
| nfs3 | `rust.yml`, `release-plz.yml`, `copilot-setup-steps.yml` | `cargo test --locked --all-features` (debug и release) + `cargo run -p nfs3_tests --bin test_mirror_fs` | `--all-targets --all-features -- -D warnings` (+ pedantic/nursery через `[lints]`) | `cargo +$RUSTFMT_TOOLCHAIN fmt --check` | да: `cargo +<rust-version> build --locked` (без тестов) | Windows x64, Linux x64, Linux arm64, macOS arm64 | нет (cargo-deny только в среде Copilot) | нет | нет | нет | release-plz (PR + publish) | dependabot: `cargo`, `github-actions` | 0/25 |
| embednfs | нет `.github/` (есть ручные `scripts/*.sh`) | — | — | — | — | — | — | — | — | — | — | нет | — |
| nessie-nfsserve (CI монорепо) | `ci.yml`, `release.yml` | `cargo test --workspace --locked` | `--workspace --all-targets --all-features --locked -- -D warnings` | да | косвенно: `rust-toolchain.toml` = 1.96.1 при `rust-version = 1.96` | Linux; `cargo check --target x86_64-pc-windows-msvc` для nessie-nfs/nessie-nfsserve | нет | нет | нет | нет | publish на теге (токен), wheels, deb/rpm, docker | нет | 0/23 |
| fractal-nfs (CI монорепо @86200e9) | `ci.yml` | `cargo xtask precheckin` → `cargo test --workspace --exclude api_server --exclude fs_server` | нет | нет | нет | Linux x86_64 и arm64 | нет | нет | нет | нет | нет (только docker-образы) | нет | 0/13 |
| nfs-v3 (CI NFSWolf) | `ci.yml`, `codeql.yml`, `docs.yml`, `publish.yml`, `release.yml`, `security.yml` | `cargo test --all-targets` × {`--all-features`; `--no-default-features`; `--no-default-features --features fuse`} в корне **без** `--workspace` | `--all-targets --all-features -- --deny warnings` | да | да: `cargo check` на 1.95.0 | Linux; `cargo check` на macOS и Windows | `cargo deny check`, `cargo audit --deny warnings` | `cargo doc` + mkdocs `--strict` | нет | нет | trusted publishing после проверки «CI зелёный» | dependabot: `cargo`, `github-actions` | 48/48 |

Источники: [nfs-mamont rust-ci.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/workflows/rust-ci.yml), [release.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/workflows/release.yml), [dependabot.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/dependabot.yml); [nfsserve ci.yml](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/.github/workflows/ci.yml); [nfs3 rust.yml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/.github/workflows/rust.yml), [release-plz.yml](https://github.com/Vaiz/nfs3/blob/8179e9d66d655242a4aadd2153a9ef8ba8b1c083/.github/workflows/release-plz.yml); [nessie ci.yml](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/.github/workflows/ci.yml), [nessie release.yml](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/.github/workflows/release.yml); [fractal ci.yml](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e912bafc6f0d29a02733974ae9eddac9080/.github/workflows/ci.yml), [xtask cmd_precheckin.rs](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e912bafc6f0d29a02733974ae9eddac9080/xtask/src/cmd_precheckin.rs); [NFSWolf ci.yml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/.github/workflows/ci.yml), [security.yml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/.github/workflows/security.yml), [publish.yml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/.github/workflows/publish.yml); разбор YAML — `ci_scan.py`.

- Последний прогон «Rust CI» на `main` для измеренного коммита `5eae9a5` (run 1119, 2026-10-29 14:18 UTC) завершился `conclusion: success`; всего у workflow 188 завершённых прогонов на `main` — [Actions run 40890132691](https://github.com/RMamonts/nfs-mamont/actions/runs/40890132691). Детализацию по job'ам получить не удалось (GitHub API вернул 502).
- Корневой `Cargo.toml` NFSWolf — одновременно пакет и workspace (`members = [".", "crates/*"]`, без `default-members`) — [NFSWolf Cargo.toml](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/Cargo.toml); по правилам Cargo без `-p`/`--workspace` команда применяется к пакету в текущем каталоге, а для виртуального манифеста — ко всем членам — [Cargo Book: Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html). `Makefile` NFSWolf при этом использует `cargo test --workspace --all-targets --all-features` — [Makefile](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/Makefile).
- nfsserve: `Cargo.lock` в `.gitignore`, а шаг CI «Cargo.lock has uncommitted changes» проверяет `git status --porcelain Cargo.lock` — [nfsserve .gitignore](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/.gitignore), [ci.yml](https://github.com/huggingface/nfsserve/blob/327b45530f04664da678297ce2568c47bf3fbf81/.github/workflows/ci.yml).

### Inferences
- Успешный run на `5eae9a5` при job'е `test` с матрицей `["1.75.0", "1.99.0"]` (без условий `if:`) означает, что сборка и тесты на MSRV 1.75.0 проходили на этом коммите — это сильнее, чем у nfs3 (MSRV только build) и NFSWolf (MSRV только `cargo check`).
- В NFSWolf CI тесты, скорее всего, выполняются только для корневого пакета `nfswolf`, а юнит-тесты `nfs-v3` в CI не запускаются (только локально через `make test`) — вывод из семантики Cargo, не из логов CI.
- `security.yml` NFSWolf с `cargo audit --deny warnings` по расписанию, вероятно, начал падать после публикации RUSTSEC-2026-0386 (13.10.2026), т.к. lock содержит tokio 1.53.1 (не проверено).
- Для nfs-mamont дешёвые улучшения CI с наибольшей отдачей: dependabot для `cargo`, закрепление actions по SHA, Miri-job для тестов аллокатора, реальные `fuzz/`-таргеты для парсера, macOS-job (если заявлять не только Linux).

### Gaps
- Не удалось получить детализацию job'ов и длительности прогона CI nfs-mamont (HTTP 502); историю «зелёности» CI конкурентов получить нельзя (GitHub REST API для чужих репозиториев недоступен).

## 7. Документация: эвристический doc-ratio и его проверка через `missing_docs`

### Takeaway
По одинаковой для всех эвристике nfs-mamont документирован лучше всех: 82.5% `pub`-элементов и 81.3% `pub`-полей имеют `///`-доки (следующий — nfs-v3: 40.5% / 20.6%; остальные 10–31% по элементам). Проверка через rustdoc (`-W missing_docs`) показывает 184 пропуска в публичном API nfs-mamont, из них 70 — варианты enum (их эвристика не видит) и 28 — трейты; по трейтам и константам эвристика совпала с rustdoc точно. Слабые места: crate-level документация — одна строка `//!`, нет `examples/`, пример использования вынесен во внешний репозиторий.

### Cited Findings
| Кодовая база | pub items (не поля) | с `///` | ratio | pub fields | с `///` | ratio | итого | `pub(restricted)` (исключены) | doc-строк на 100 строк кода | crate-level docs | `examples/` |
|---|---|---|---|---|---|---|---|---|---|---|---|
| nfs-mamont | 623 | 514 | 0.825 | 310 | 252 | 0.813 | 0.821 | 0 | 44.2 | `//!` 1 строка | 0 |
| nfsserve | 156 | 46 | 0.295 | 79 | 1 | 0.013 | 0.200 | 3 | 11.5 | нет | 2 |
| zerofs_nfsserve | 173 | 49 | 0.283 | 111 | 4 | 0.036 | 0.187 | 3 | 9.4 | нет | 2 |
| nfs3_server+types+macros | 342 | 60 | 0.175 | 240 | 0 | 0.000 | 0.103 | 25 | 6.8 | `#![doc = include_str!("../README.md")]` во всех трёх | 3 (+1 в `nfs3_tests`) |
| embednfs+proto | 377 | 88 | 0.233 | 392 | 60 | 0.153 | 0.192 | 147 | 2.7 | `//!` 5 и 1 строка | 0 (есть бинарь `embednfsd`) |
| nessie-nfsserve | 159 | 49 | 0.308 | 82 | 4 | 0.049 | 0.220 | 3 | 11.7 | `//!` 4 строки | 0 |
| fractal-nfs | 206 | 37 | 0.180 | 90 | 5 | 0.056 | 0.142 | 2 | 6.0 | `include_str!(README.md)` | 0 |
| nfs-v3 | 257 | 104 | 0.405 | 238 | 49 | 0.206 | 0.309 | 0 | 23.3 | `//!` 34 строки | 0 |

Источники: SHA из раздела 1, `rs_metrics.py` (`_pub_kind`, `_doc_status`) — [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d), [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [nessie-store@0afb7e3](https://github.com/Gilamonster-Foundation/nessie-store/tree/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve), [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3).

Правило эвристики: учитываются неограниченные `pub` (не `pub(crate)`/`pub(super)`/`pub(in …)`) в prod-коде вне тел `macro_rules!`; виды — `fn`, `struct`, `enum`, `trait`, `type`, `const`, `static`, `mod`, `union` и поля структур; `pub use`/`pub extern crate` исключены; «документирован» = перед элементом (с пропуском атрибутов и обычных комментариев) стоит `///`/`/** */` или `#[doc = …]`; `#[doc(hidden)]` исключается из знаменателя; `pub mod x;` считается документированным и при `//!` в начале файла модуля.

Сверка эвристики с rustdoc для nfs-mamont (`RUSTDOCFLAGS="-W missing_docs" cargo doc --no-deps`, 184 предупреждения):

| Вид | эвристика: недокументированных `pub` (всего `pub`) | rustdoc `missing_docs` (достижимый публичный API) |
|---|---|---|
| trait | 28 (из 35) | 28 |
| const / associated const | 25 (из 43) | 25 (13 + 12) |
| fn / method / associated fn | 17 (из 241) | 14 (10 + 4) |
| struct | 13 (из 144) | 9 |
| struct field (вкл. 3 tuple-поля) | 58 (из 310) | 32 |
| mod | 21 (из 122) | 6 |
| enum | 4 (из 33) | 0 |
| type alias | 1 (из 5) | 0 |
| enum variant | не считается (нет `pub`) | 70 |
| **Итого** | **167** | **184** (114 без вариантов) |

Источник: [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f) (лог `results/cargo/doc_missing.json`, разбор по `message.code == "missing_docs"` с дедупликацией по файлу/строке).

- Документация в README nfs-mamont ссылается на внешний демо-репозиторий («As an example we provide our demo implementation» → RMamonts/mirror-fs) и вики по архитектуре — [README](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/README.md).
- nfs-v3 явно освобождает таблицу RFC-типов от доков: `#![expect(missing_docs, reason = "these are mechanical transcriptions of the RFC's XDR type table …")]` — [nfs-v3 wire/types.rs](https://github.com/StrongWind1/NFSWolf/blob/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3/src/wire/types.rs).

### Inferences
- Эвристика корректно ранжирует кодовые базы, но в абсолютном выражении для nfs-mamont она завышает знаменатель (элементы `pub` в приватных модулях: модулей 21 vs 6, полей 58 vs 32) и не видит варианты enum и методы трейтов; поэтому «82%» — это не «82% публичного API», а мера дисциплины `///` для всех `pub`. По rustdoc у nfs-mamont остаётся 184 пропуска, в основном в типах протокола (варианты enum статусов/кодов) и в `Vfs`-трейтах процедур (28 трейтов без доков) — именно в той части API, которую реализует пользователь библиотеки.
- У embednfs эвристика «видит» мало `pub` из-за дисциплины видимости (`unreachable_pub = warn`, 147 `pub(restricted)`), поэтому сравнивать доли с ним нужно с оговоркой.
- Для потребителей библиотеки у nfs-mamont наибольший разрыв — не в построчных доках, а в «входной» документации: однострочный `//!` в `lib.rs`, ноль `examples/`, 2 doc-test'а; у nfsserve/nfs3 есть исполняемые `examples/` (demo/mirrorfs/memfs), у nfs3/fractal README встроен в rustdoc.

### Gaps
- Не измерялась полнота документации трейт-методов и вариантов enum у конкурентов (для этого нужен rustdoc, т.е. запуск их кода — запрещено).

## 8. nfs-mamont: результаты cargo test / clippy / fmt / doc и время чистой сборки

### Takeaway
На измеренном коде (`5eae9a5`) всё «зелёное»: 336/336 юнит-тестов и 2/2 doc-теста проходят (с `--all-features` тоже), clippy `--all-targets` — 0 предупреждений, CI-вариант clippy с `-D warnings` и `cargo doc` с `-D warnings` проходят, `cargo fmt --check` чист. Чистая debug-сборка — 7.4–8.2 с (32 единицы компиляции, 4 vCPU). Единственный «красный» сигнал — 184 предупреждения `missing_docs` (не включены в CI).

### Cited Findings
| Команда (`--manifest-path /home/user/nfs-mamont/Cargo.toml`, `CARGO_TARGET_DIR` во временном каталоге) | Результат |
|---|---|
| `cargo fetch --locked` | rc=0 |
| `cargo build --locked` (чистый target, default features), прогон 1 / 2 | rc=0; 8231 мс / 7436 мс wall; 32 строк `Compiling`; cargo: «Finished `dev` profile … in 8.18s» |
| `cargo build --locked --all-features` (чистый target) | rc=0; 7366 мс |
| `cargo test --locked` | rc=0; unittests: 336 passed, 0 failed, 0 ignored, 0.61 s; Doc-tests: 2 passed, 0.07 s; 4906 мс wall вкл. сборку тестов |
| `cargo test --locked --all-features` | rc=0; 336 passed; doc-tests 2 passed |
| `cargo clippy --locked --all-targets --message-format=json` | rc=0; 0 диагностик (warning/error) |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` (как в CI) | rc=0 |
| `cargo fmt --all --check` | rc=0; 0 diff-блоков |
| `RUSTDOCFLAGS="-W missing_docs" cargo doc --locked --no-deps --message-format=json` | rc=0; 184 `missing_docs`: variant 70, struct field 32, trait 28, constant 13, associated constant 12, method 10, struct 9, module 6, associated function 4 |
| `RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps --document-private-items` (как в CI) | rc=0 |
| `git status --porcelain` после прогонов | изменений от прогонов нет (виден лишь чужой untracked `research_notes/…/rust_nfsv4_landscape.md` параллельного агента) |

Источник: [nfs-mamont@5eae9a5](https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f), скрипт `run_nfs_mamont_cargo.sh` (раздел 11), тулчейн rustc 1.97.0 / clippy 0.1.97 / rustfmt 1.9.0.

- CI использует clippy из тулчейна 1.99.0 и rustdoc 1.95.0 — [rust-ci.yml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f/.github/workflows/rust-ci.yml); локально был 1.97.0.

### Inferences
- Чистота clippy/fmt/doc и 100% прохождение тестов подтверждают, что CI-гейты nfs-mamont реально соблюдаются (а не только объявлены). 0 предупреждений clippy при дефолтном наборе линтов — хорошая база для включения более строгих групп (как у nfs3/NFSWolf) без большой стоимости.
- 7–8 с чистой сборки на 4 vCPU — следствие маленького дерева (34 внешних пакета); у конкурентов с `tokio = { features = ["full"] }`, anyhow/async-trait/dashmap время будет, вероятно, больше, но это не измерялось.

### Gaps
- MSRV 1.75.0 локально не проверялся (тулчейн не установлен; установка запрещена условиями) — опираемся на зелёный CI-run (раздел 6).
- Время сборки конкурентов не измерялось (запрет на сборку стороннего кода).

## 9. Интерпретация по каждой кодовой базе

### Takeaway
По совокупности метрик лучший «инженерный профиль» у NFSWolf/nfs-v3 (линты, supply chain, CI) и nfs3 (кроссплатформенный CI, интеграционные тесты, release-plz), лучшая робастность — у embednfs (почти ноль паникующих вызовов, большие интеграционные тесты, но без CI и, похоже, заброшен), а семейство nfsserve — зрелое по использованию, но слабое по тестам и докам. nfs-mamont выделяется тестовой плотностью, доками и минимализмом зависимостей, но несёт уникальный для набора unsafe-риск и молод по истории релизов.

### Cited Findings
- **nfsserve (huggingface)**: 3272 строк prod, 0 тестов (`doctest = false`), 9.2 паникующих/KLOC (большинство «guarded unwrap»), doc-ratio 0.20, нет lockfile/MSRV, CI: fmt + clippy + «test» без тестов, actions по SHA; 1.4 млн загрузок, первая версия 2023-06-05 — [nfsserve@327b455](https://github.com/huggingface/nfsserve/tree/327b45530f04664da678297ce2568c47bf3fbf81), [crates.io](https://crates.io/api/v1/crates/nfsserve).
- **zerofs_nfsserve**: 4173 строк prod (форк nfsserve, AGPL-3.0 на изменения), 0 тестов, нет CI, нет lockfile, 15 прямых зависимостей (больше всех), последний коммит 2026-03-02 — [barre/nfsserve@d61b084](https://github.com/barre/nfsserve/tree/d61b08456ae66108666978e29524a47d2209f68d).
- **nfs3_server + nfs3_types + nfs3_macros (Vaiz)**: 5861 строк prod, 165 тестов (88 в тест-крейте), строгие clippy-группы (pedantic/nursery = deny, unwrap_used = deny) → 33 `expect`; doc-ratio 0.10 (поля 0/240); CI на 4 платформах + MSRV build + release-plz; 15 релизов с 2025-02 — [Vaiz/nfs3@8179e9d](https://github.com/Vaiz/nfs3/tree/8179e9d66d655242a4aadd2153a9ef8ba8b1c083), [crates.io](https://crates.io/api/v1/crates/nfs3_server).
- **embednfs + embednfs-proto (NFSv4.1)**: 9613 строк prod, 259 тестов (10 интеграционных target'ов), 2 паникующих вызова на 9.6K строк (restriction-линты с `#[expect(reason)]`), нет CI и dependabot, `CLAUDE.md` в корне, 13 версий за 3 дня (2026-03-08…10) и ни одного коммита после 2026-03-10 — [embednfs@ac1a016](https://github.com/PeronGH/embednfs/tree/ac1a01641247790c5df414192f979e42c6cdaffb), [crates.io](https://crates.io/api/v1/crates/embednfs).
- **nessie-nfsserve**: hardened-форк nfsserve 0.11.0 (COMMIT, stable-aware WRITE, AUTH_UNIX), 3576 строк prod, 2 теста, 28 `unwrap`, CI монорепо (fmt/clippy/test workspace), единственный релиз 0.3.1; в workspace-lock 19 advisories вне крейта — [nessie-nfsserve README](https://github.com/Gilamonster-Foundation/nessie-store/blob/0afb7e3b3a6e510e1fbc3e3870350acac4a3d56b/crates/nessie-nfsserve/README.md).
- **fractal-nfs**: 2220 строк prod (самый компактный сервер), 12 тестов, 2 `unwrap`, tokio/compio-бэкенды через features, CI монорепо без clippy/fmt; в HEAD `main` крейта нет; 1 релиз, 44 загрузки — [fractalbits-main@86200e9](https://github.com/fractalbits-labs/fractalbits-main/tree/86200e912bafc6f0d29a02733974ae9eddac9080/crates/fs_server/fractal-nfs), [crates.io](https://crates.io/api/v1/crates/fractal-nfs).
- **nfs-v3 (NFSWolf)**: клиент, 1750 строк prod + 384 строки мёртвого кода (3 неподключённых файла, попавших в tarball), 39 живых тестов, 0 unsafe (`unsafe_code = deny`), самые строгие линты и supply-chain-контроли (deny.toml, audit, CodeQL, SHA-pinning), но CI-тесты, вероятно, не покрывают крейт — [NFSWolf@e067096](https://github.com/StrongWind1/NFSWolf/tree/e067096480626c2a7134fa45e47e0ce8bca4b9f6/crates/nfs-v3).

### Inferences
- **nfsserve**: «зрелость по адопции, а не по инженерии». Огромное число загрузок при нуле тестов означает, что качество обеспечивается эксплуатацией у пользователей (и форками, которые чинят пробелы — nessie прямо перечисляет найденные дефекты WRITE/COMMIT). Риск для потребителя: регрессии не ловятся автоматически.
- **zerofs_nfsserve**: наибольший размер в семействе при отсутствии тестов и CI в этом репозитории; качество, вероятно, обеспечивается тестами продукта ZeroFS вне форка (не проверено). AGPL-3.0 — юридический барьер для встраивания.
- **nfs3**: самая «процессная» альтернатива nfs-mamont для NFSv3-сервера: релизный конвейер, кроссплатформенность, интеграционный тест-крейт. Слабые места — документация (18% элементов, 0% полей) и замена unwrap на expect вместо обработки ошибок.
- **embednfs**: образцовая дисциплина робастности и тестов, но отсутствие CI и 7+ месяцев без коммитов — высокий риск сопровождения; к тому же это NFSv4.1, т.е. не прямой конкурент v3-библиотеке.
- **nessie-nfsserve**: полезен как индикатор дефектов upstream-nfsserve; сам по себе почти без тестов и с MSRV 1.96 — привязан к монорепо-владельцу.
- **fractal-nfs**: аккуратный маленький код, но фактически снят с основной ветки и опубликован один раз — риск заброшенности максимальный.
- **nfs-v3**: лучшая «гигиена» (линты/supply chain), но мёртвый код в публикации и тесты вне CI показывают, что строгие инструменты не гарантируют полноты; как клиент — ориентир для интероп-тестов, а не конкурент.

### Gaps
- Для zerofs_nfsserve и nessie-nfsserve не проверялось, есть ли тесты в продуктах-потребителях (ZeroFS, `nessie-nfs`), покрывающие эти крейты.

## 10. Относительная позиция nfs-mamont

### Takeaway
Среди Rust NFSv3-серверов nfs-mamont по статическим метрикам — лидер по тестовой плотности (61 тест/KLOC), документированности (82% `pub`), минимальности зависимостей (34 внешних пакета) и низкому MSRV (1.75, проверенному в CI), и входит в лучшую группу по паникующим вызовам (1.45/KLOC) и полноте CI. Его главные риски — единственный в наборе нетривиальный `unsafe` без `SAFETY`-обоснований и Miri, отсутствие интеграционных/fuzz/property-тестов при рукописном XDR-парсере, отсутствие `[lints]`, незакрытое advisory tokio в lockfile и минимальная «входная» документация; по зрелости (3 релиза с 2026-10-09, 80 загрузок) он самый молодой.

### Cited Findings
Сводная позиция (ранг nfs-mamont среди 8 кодовых баз; 1 = лучший):

| Метрика | nfs-mamont | Лучший в наборе | Худший | Ранг nfs-mamont |
|---|---|---|---|---|
| test fns / 1K prod LOC | 61.0 | nfs-mamont 61.0 | nfsserve/zerofs 0 | 1 |
| test code / prod code | 0.91 | nfs3 1.06 (с тест-крейтом) | nfsserve/zerofs 0 | 3 |
| интеграционные/E2E тесты | нет | embednfs (10 targets, клиент nfs-rs), nfs3 (`test_mirror_fs`) | нет у 5 баз (nfs-mamont, nfsserve, zerofs, nessie, nfs-v3) | — (нет) |
| Σ паникующих / KLOC (prod) | 1.45 | embednfs 0.21 | nfsserve 9.17 | 3 |
| unsafe-блоков (prod) | 8 (+1 fn, +4 impl) | 0 (5 баз) | nfs-mamont | 8 (худший) |
| `// SAFETY:` на unsafe | 0 из 8 | — | — | — |
| doc-ratio (эвристика, всё) | 0.821 | nfs-mamont | nfs3 0.103 | 1 |
| doc-строк на 100 строк кода | 44.2 | nfs-mamont | embednfs 2.7 | 1 |
| `examples/` | 0 | nfs3 3(+1), nfsserve/zerofs 2 | 0 у 5 баз | хуже среднего |
| прямые normal-зависимости | 9 | nfs-v3 1 (+3 внутр.) | zerofs 15 | 5 |
| normal+build замыкание (lock) | 34 | nfs-mamont 34 | nessie 39 | 1 (из 5 с lock) |
| MSRV | 1.75 (CI build+test) | nfs-mamont | nessie 1.96 | 1 |
| advisories в normal-замыкании (только 5 баз с lockfile) | 1 (unsound, tokio, `AsyncFd` не используется) | 1 — nfs-mamont, embednfs, nfs-v3 (ничья) | nfs3, nessie — 2 | 1–3 (ничья) |
| `[lints]` / unsafe-линт | нет / нет | NFSWolf (unsafe_code=deny, missing_docs=warn) | нет у 5 баз | хуже среднего |
| CI-гейты (fmt, clippy, test, MSRV, audit, doc) | 6 из 6 | nfs-mamont, NFSWolf | zerofs/embednfs (нет CI) | 1–2 |
| OS-матрица CI | только Linux | nfs3 (4 платформы) | — | хуже среднего |
| SHA-pinning actions | 0/20 | NFSWolf 48/48, nfsserve 4/4 | 0 у 4 баз | хуже среднего |
| релизов на crates.io / загрузок | 3 / 80 | nfs3_server 15; nfsserve 1.4 млн | fractal 1 / 44 | молодой |

Источники: таблицы разделов 2–8 (SHA из раздела 1), [crates.io API nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont), [Actions run 40890132691](https://github.com/RMamonts/nfs-mamont/actions/runs/40890132691).

### Inferences
- Профиль nfs-mamont — «молодой, но дисциплинированный» проект: инженерные гейты и тесты уже на уровне лучших конкурентов, а по докам и зависимостям — лучше всех; отставание — в зрелости (история релизов, адопция) и в «внешних» видах проверки (E2E-интероп, фаззинг, Miri, кроссплатформенность).
- Если ранжировать по риску для потенциального пользователя библиотеки, у nfs-mamont три конкретных, недорого устраняемых пункта: (1) задокументировать и проверить `unsafe` аллокатора (`// SAFETY:` на каждый блок, `#![deny(unsafe_op_in_unsafe_fn)]`, `clippy::undocumented_unsafe_blocks`, Miri-job на тестах аллокатора); (2) добавить реальные `fuzz/`-таргеты на парсер RPC/XDR (workflow уже есть) и хотя бы один интеграционный тест с настоящим клиентом (например, через `nfs3_client` или `nfs-rs`); (3) `cargo update -p tokio` до ≥ 1.53.3, dependabot для `cargo`, SHA-pinning actions, `[lints]` с `missing_docs = "warn"` (184 пропуска — в основном варианты enum и трейты `Vfs`).
- В сравнении «один на один» с ближайшими v3-альтернативами: против nfsserve-семейства nfs-mamont выигрывает почти по всем статическим метрикам качества (тесты, паники, доки, CI, lockfile, MSRV, лицензия MIT vs AGPL у zerofs), проигрывая только по адопции; против nfs3 — выигрывает по тестовой плотности, докам, зависимостям и MSRV, проигрывает по кроссплатформенному CI, интеграционным тестам, строгим линтам и релизной автоматизации (release-plz vs ручной релиз).

### Gaps
- Нет данных о реальной интероперабельности и производительности (бенчмарки/E2E не запускались ни для кого); статические метрики не отражают корректность реализации протокола.

## 11. Воспроизводимость: методика, команды и исходный код скриптов

### Takeaway
Все цифры воспроизводятся командой `bash scripts/run_all.sh` (статический анализ всех баз) плюс `bash scripts/run_nfs_mamont_cargo.sh` (cargo только для nfs-mamont) и `python3 -I scripts/make_tables.py results` (таблицы); ниже — точные команды загрузки, SHA-256 и полный текст скриптов.

### Cited Findings
Команды загрузки (каждая в свой новый пустой каталог под `…/code_metrics_comparison/downloads/`):
```bash
BASE=/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/code_metrics_comparison
D=$BASE/downloads
git clone -q --depth 1 https://github.com/huggingface/nfsserve        $D/nfsserve/repo
git clone -q --depth 1 https://github.com/barre/nfsserve              $D/zerofs_nfsserve/repo
git clone -q --depth 1 https://github.com/Vaiz/nfs3                   $D/nfs3/repo
git clone -q --depth 1 https://github.com/PeronGH/embednfs            $D/embednfs/repo
git clone -q --depth 1 https://github.com/rustsec/advisory-db         $D/advisory-db/repo
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/Gilamonster-Foundation/nessie-store $D/nessie-store/repo
git -C $D/nessie-store/repo sparse-checkout add crates/nessie-nfsserve .github
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/StrongWind1/NFSWolf $D/nfswolf/repo
git -C $D/nfswolf/repo sparse-checkout add crates .github
# fractal-nfs: коммит публикации 0.1.0 (= тег fractal-nfs-0.1.0), т.к. в HEAD main крейта нет
git -C $D/fractalbits-main-86200e9 init -q repo
git -C $D/fractalbits-main-86200e9/repo remote add origin https://github.com/fractalbits-labs/fractalbits-main
git -C $D/fractalbits-main-86200e9/repo fetch -q --depth 1 --filter=blob:none origin 86200e912bafc6f0d29a02733974ae9eddac9080
git -C $D/fractalbits-main-86200e9/repo sparse-checkout set crates/fs_server/fractal-nfs .github
git -C $D/fractalbits-main-86200e9/repo checkout -q --detach FETCH_HEAD
git -C $D/fractalbits-main-86200e9/repo sparse-checkout add xtask   # только чтение cmd_precheckin.rs
# tarball'ы crates.io (для сверки), распаковка безопасным tarfile-фильтром 'data'
for cv in nfsserve:0.11.0 zerofs_nfsserve:0.19.1 nfs3_server:0.11.1 nfs3_types:0.5.3 nfs3_macros:0.5.1 \
          embednfs:0.4.1 embednfs-proto:0.4.1 nessie-nfsserve:0.3.1 fractal-nfs:0.1.0 nfs-v3:1.2.0 nfs-mamont:0.2.0; do
  c=${cv%%:*}; v=${cv##*:}; dir="$D/crate-$c-$v"; mkdir "$dir"
  curl -sSfL -A "nfs-mamont-research" -o "$dir.crate" "https://crates.io/api/v1/crates/$c/$v/download"
  python3 -I $BASE/scripts/safe_extract.py "$dir.crate" "$dir"
done
# метрики (из $BASE/results, вне загрузок)
cd $BASE/results && bash $BASE/scripts/run_all.sh && python3 -I $BASE/scripts/make_tables.py $BASE/results > tables.md
bash $BASE/scripts/run_nfs_mamont_cargo.sh          # только для nfs-mamont
python3 -I $BASE/scripts/crosscheck_linecounts.py results/cratesio_linecounts.json $D/crate-*-*/   # сверка с crates.io
rm -rf $BASE/target                                  # target-каталог удалён после замеров
```

SHA-256 скриптов на момент замеров:
```
eba8bead0d1c7dcc82b2f481816ba5059bceabd864524f740024b8b063ce89fa  scripts/rs_metrics.py
2b375caa0024753b5a01dcf54cf4cf0b6073918d0e1484d6a8a2029b33737cf1  scripts/deps_audit.py
e02d00ed5feddb63c68924138eb18d4ae338a5d26cc6a819e2786a34434b4795  scripts/ci_scan.py
2edc1de88f931878f2136d39f32225d05bf82e599c8f21e4962a45ba7e651222  scripts/make_tables.py
729551ec387589b662f2270a7558b9fc1c4cdf8a43cc661599a0a37c79a1b040  scripts/crosscheck_linecounts.py
16acbba4bd2198288c751a4605422f0c15abb9e8e7d3f576d66445ab9ed3a27e  scripts/run_all.sh
2c76180a4f29d283bc979d5a46157a56db3b15ce6eeeec1b6887196a354612a8  scripts/run_nfs_mamont_cargo.sh
7f4ca4240440353dc722cc7c18c961d62f123aa04a0d2274b865fbad7ce20bb5  scripts/safe_extract.py
```
Сырые результаты (JSON по каждой базе: `m_*.json` — код, `d_*.json` — зависимости/RustSec, `c_*.json` — CI, `t_*.json` — tarball'ы; `cargo/*` — логи nfs-mamont) лежали в `/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/code_metrics_comparison/results/` (временный каталог сессии) — источник: собственный замер на SHA из раздела 1, [advisory-db@2295466](https://github.com/rustsec/advisory-db/tree/2295466c4e9df26647fd67d8fa1c705ac2fc7f81).

Ключевые правила методики (одинаковые для всех баз):
- Лексер: вложенные `/* */`, raw/byte/C-строки, char vs lifetime; строка = `code`, если в ней есть хоть один не-комментарийный токен (многострочные строковые литералы — code), `doc`/`comment` — только комментарии, `blank` — пробелы.
- Test vs prod: обход дерева модулей от `lib.rs`/`main.rs`/`src/bin/*` с учётом `#[path]`, inline-модулей и наследования `#[cfg(test)]`; cfg-предикат трёхзначно вычисляется при `test = false` (`cfg(test)`, `cfg(all(test, …))` → test; `cfg(any(test, feature=…))` → prod); элементы с `#[test]`, `#[tokio::test]`, `#[rstest]`, `#[test_case]` и т.п. и блоки `proptest!`/`quickcheck!` — test; `tests/` — test; крейт `nfs3_tests` целиком — test; файлы, не достижимые ни из одного target root — «dead» (вне prod и test).
- Паникующие/unsafe-счётчики — по токенам только в prod-регионах; `.unwrap()` = `.` `unwrap` `(` `)`, `.expect(` = `.` `expect` `(`, макросы — идентификатор + `!`; `unsafe {` / `unsafe fn` / `unsafe impl` различаются по следующему токену.
- RustSec: `patched`/`unaffected` разбираются собственной реализацией semver-требований (`^`, `~`, `=`, `>=`, `<`, частичные версии); затронута версия, не удовлетворяющая ни одному требованию; withdrawn пропускаются; замыкание зависимостей — по рёбрам `Cargo.lock` с фильтрацией рёбер workspace-членов по их normal/build-зависимостям из Cargo.toml.

### Inferences
- Поскольку лексер совпал с crates.io `linecounts` для 10 из 11 крейтов, а счётчики тестов nfs-mamont совпали с фактическим `cargo test` (336/336 и 2/2), погрешность методики для размеров и числа тестов практически нулевая; основная неопределённость — в интерпретационных метриках (doc-ratio, «guarded unwrap»).

### Gaps
- Скрипты не обрабатывают макро-генерируемые элементы и `include!()`; ошибки разбора скобок не обнаружены (`mismatched_brackets = 0` для всех баз), но экзотический синтаксис мог бы их дать.

#### Исходный код скриптов (дословно)

`scripts/safe_extract.py` (SHA-256 `7f4ca4240440353dc722cc7c18c961d62f123aa04a0d2274b865fbad7ce20bb5`):

````python
#!/usr/bin/env python3
"""Safely extract a .crate (tar.gz) archive into an EMPTY destination directory.

Usage: python3 -I safe_extract.py <archive.crate> <dest_dir>

Uses tarfile's 'data' filter (Python >= 3.12): rejects absolute paths, '..'
components, links pointing outside dest, device files, and strips exec bits.
Nothing from the archive is executed.
"""
import os
import sys
import tarfile


def main() -> int:
    archive, dest = sys.argv[1], sys.argv[2]
    os.makedirs(dest, exist_ok=True)
    if os.listdir(dest):
        print(f"refusing: {dest} is not empty", file=sys.stderr)
        return 2
    with tarfile.open(archive, "r:gz") as tf:
        members = tf.getmembers()
        tf.extractall(dest, filter="data")
    print(f"extracted {len(members)} members into {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
````

`scripts/rs_metrics.py` (SHA-256 `eba8bead0d1c7dcc82b2f481816ba5059bceabd864524f740024b8b063ce89fa`):

````python
#!/usr/bin/env python3
"""rs_metrics.py -- static, read-only metrics for Rust crates.

Nothing from the analysed trees is executed, imported or built: *.rs, Cargo.toml
and README files are only read as text.  Run in isolated mode from a directory
outside the analysed trees:

  python3 -I rs_metrics.py --name NAME --repo REPO_ROOT --crate REL [--crate REL ...]
         [--test-crate REL ...] --json OUT.json

--crate       crate directory (relative to REPO_ROOT) whose code is measured
--test-crate  crate directory that only exists to test the measured crates; all its
              code is counted as test code (benches/examples keep their category)

Method (identical for every codebase):
* A small Rust lexer splits files into tokens (nested block comments, raw/byte/C
  strings, char vs lifetime are handled).  A line is `code` if it holds any non-
  comment token (multi-line strings count as code), `comment` if it holds only
  comment tokens (doc comments included), `blank` if whitespace only.
* Test code = files under tests/, files reached only through `#[cfg(test)]` module
  declarations (module tree followed from lib.rs/main.rs, honouring #[path]),
  plus, inside other files, every item carrying a cfg that is false when
  `test` is false (cfg(test), cfg(all(test, ..)) ...) or a test attribute
  (#[test], #[tokio::test], #[rstest], ...), plus proptest!/quickcheck! blocks.
* Production (prod) code = everything else reachable from lib/bin roots.
* Safety counters are token patterns over prod code only.
* Doc heuristic: every `pub` (unrestricted) item/field in prod code outside
  macro_rules! bodies; documented if preceded (attributes and plain comments
  skipped) by an outer doc comment or #[doc = ..]; `pub mod x;` also counts as
  documented when the module file starts with //! inner docs.
"""
import argparse
import glob
import json
import os
import re
import sys
import tomllib
from collections import Counter

IDENT_RE = re.compile(r'[^\W\d]\w*')
NUM_RE = re.compile(r'\d\w*(?:\.\d\w*)?')
RAW_STR_RE = re.compile(r'(?:b|c)?r(#*)"')
TODO_RE = re.compile(r'\b(TODO|FIXME|XXX|HACK)\b')
SAFETY_RE = re.compile(r'\bSAFETY\s*:', re.I)
CODE_KINDS = frozenset(('ident', 'punct', 'str', 'char', 'life', 'num'))
DOC_KINDS = frozenset(('odoc', 'idoc'))
OPEN = {'(': ')', '[': ']', '{': '}'}
CLOSE = {')': '(', ']': '[', '}': '{'}
TEST_ATTRS = {
    'test', 'tokio::test', 'async_std::test', 'actix_rt::test', 'actix_web::test',
    'rstest', 'test_case', 'test_case::test_case', 'test_log::test', 'quickcheck',
    'quickcheck_macros::quickcheck', 'proptest', 'test_strategy::proptest',
    'proptest::property_test', 'sqlx::test', 'wasm_bindgen_test', 'bench',
    'compio::test', 'monoio::test', 'smol_potat::test', 'futures_test::test',
    'googletest::test', 'gtest', 'kani::proof', 'macro_rules_attribute::apply',
}
ITEM_KW_BLOCK = {'fn', 'mod', 'impl', 'struct', 'enum', 'trait', 'union',
                 'macro_rules', 'macro', 'extern'}
ITEM_KW_SEMI = {'use', 'const', 'static', 'type', 'let'}
QUALS = {'async', 'unsafe', 'default', 'safe'}
PUB_KINDS = {'fn', 'struct', 'enum', 'trait', 'type', 'const', 'static', 'mod',
             'union', 'macro'}
RISKY_IDENTS = ('transmute', 'from_raw_parts', 'from_raw_parts_mut', 'get_unchecked',
                'get_unchecked_mut', 'set_len', 'unwrap_unchecked', 'from_utf8_unchecked',
                'assume_init', 'MaybeUninit', 'ptr', 'NonNull', 'mem::forget')
NON_RUST_FENCES = {'text', 'txt', 'sh', 'bash', 'console', 'shell', 'toml', 'json',
                   'yaml', 'yml', 'c', 'cpp', 'none', 'plain', 'ini', 'diff', 'xml',
                   'html', 'md', 'markdown', 'ignore', 'python', 'powershell', 'ps1',
                   'mermaid', 'dot', 'go', 'js', 'ts', 'nix', 'dockerfile'}


# --------------------------------------------------------------------------- lexer
def scan_quoted(src, j, quote):
    n = len(src)
    while j < n:
        ch = src[j]
        if ch == '\\':
            j += 2
            continue
        if ch == quote:
            return j + 1
        j += 1
    return n


def tokenize(src):
    """Return a list of (kind, text, start_line, end_line)."""
    toks = []
    app = toks.append
    i, n, line = 0, len(src), 1
    while i < n:
        c = src[i]
        if c == '\n':
            line += 1
            i += 1
            continue
        if c.isspace():
            i += 1
            continue
        if c == '/' and i + 1 < n and src[i + 1] in '/*':
            if src[i + 1] == '/':
                j = src.find('\n', i)
                if j == -1:
                    j = n
                text = src[i:j]
                if text.startswith('///') and not text.startswith('////'):
                    kind = 'odoc'
                elif text.startswith('//!'):
                    kind = 'idoc'
                else:
                    kind = 'com'
                app((kind, text, line, line))
                i = j
                continue
            depth, j = 0, i
            while j < n:
                if src.startswith('/*', j):
                    depth += 1
                    j += 2
                elif src.startswith('*/', j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            text = src[i:j]
            nl = text.count('\n')
            if text.startswith('/**') and not text.startswith('/***') and text != '/**/':
                kind = 'odoc'
            elif text.startswith('/*!'):
                kind = 'idoc'
            else:
                kind = 'com'
            app((kind, text, line, line + nl))
            line += nl
            i = j
            continue
        if c in 'rbc':
            m = RAW_STR_RE.match(src, i)
            if m:
                close = '"' + m.group(1)
                j = src.find(close, m.end())
                j = n if j == -1 else j + len(close)
                text = src[i:j]
                nl = text.count('\n')
                app(('str', text, line, line + nl))
                line += nl
                i = j
                continue
            if c in 'bc' and i + 1 < n and src[i + 1] == '"':
                j = scan_quoted(src, i + 2, '"')
                text = src[i:j]
                nl = text.count('\n')
                app(('str', text, line, line + nl))
                line += nl
                i = j
                continue
            if c == 'b' and i + 1 < n and src[i + 1] == "'":
                j = scan_quoted(src, i + 2, "'")
                app(('char', src[i:j], line, line))
                i = j
                continue
            if c == 'r' and i + 2 < n and src[i + 1] == '#':
                m = IDENT_RE.match(src, i + 2)
                if m:
                    app(('ident', 'r#' + m.group(0), line, line))
                    i = m.end()
                    continue
        if c == '"':
            j = scan_quoted(src, i + 1, '"')
            text = src[i:j]
            nl = text.count('\n')
            app(('str', text, line, line + nl))
            line += nl
            i = j
            continue
        if c == "'":
            if i + 1 < n and src[i + 1] == '\\':
                j = scan_quoted(src, i + 1, "'")
                app(('char', src[i:j], line, line))
                i = j
                continue
            if i + 2 < n and src[i + 2] == "'" and src[i + 1] != '\n':
                app(('char', src[i:i + 3], line, line))
                i += 3
                continue
            m = IDENT_RE.match(src, i + 1)
            if m:
                app(('life', src[i:m.end()], line, line))
                i = m.end()
                continue
            app(('punct', c, line, line))
            i += 1
            continue
        if c.isdigit():
            m = NUM_RE.match(src, i)
            app(('num', m.group(0), line, line))
            i = m.end()
            continue
        m = IDENT_RE.match(src, i)
        if m:
            app(('ident', m.group(0), line, line))
            i = m.end()
            continue
        app(('punct', c, line, line))
        i += 1
    return toks


# --------------------------------------------------------------------------- cfg
def cfg_eval(p):
    """Three-valued evaluation of a cfg predicate with test=False, others unknown."""
    pos = 0

    def parse():
        nonlocal pos
        if pos >= len(p):
            return None
        tok = p[pos]
        if tok in ('all', 'any', 'not') and pos + 1 < len(p) and p[pos + 1] == '(':
            pos += 2
            vals = []
            while pos < len(p) and p[pos] != ')':
                vals.append(parse())
                if pos < len(p) and p[pos] == ',':
                    pos += 1
            pos += 1
            if tok == 'all':
                if any(v is False for v in vals):
                    return False
                if all(v is True for v in vals):
                    return True
                return None
            if tok == 'any':
                if any(v is True for v in vals):
                    return True
                if vals and all(v is False for v in vals):
                    return False
                return None
            v = vals[0] if vals else None
            return None if v is None else (not v)
        pos += 1
        if pos < len(p) and p[pos] == '=':
            pos += 2
            return None
        return False if tok == 'test' else None

    return parse()


def attr_path(a):
    out = []
    for x in a:
        if x in ('(', '=', '['):
            break
        out.append(x)
    return ''.join(out)


def is_test_cfg(a):
    return len(a) >= 3 and a[0] == 'cfg' and a[1] == '(' and cfg_eval(a[2:-1]) is False


# --------------------------------------------------------------------------- file
class FileInfo:
    def __init__(self, abspath, rel):
        with open(abspath, encoding='utf-8', errors='replace') as f:
            text = f.read()
        self.abspath, self.rel = abspath, rel
        lines = text.split('\n')
        if lines and lines[-1] == '':
            lines.pop()
        self.lines = lines
        self.toks = tokenize(text)
        self.code, self.full_of_code, self.code_of_full = [], [], {}
        for idx, t in enumerate(self.toks):
            if t[0] in CODE_KINDS:
                self.code_of_full[idx] = len(self.code)
                self.full_of_code.append(idx)
                self.code.append(t)
        self._match()
        self._attrs()
        self._test_regions()

    def t(self, k):
        return self.code[k][1] if 0 <= k < len(self.code) else ''

    def _match(self):
        n = len(self.code)
        self.match, self.rmatch, self.parent = {}, {}, [-1] * n
        self.mismatched = 0
        stack = []
        for k, tok in enumerate(self.code):
            self.parent[k] = stack[-1] if stack else -1
            if tok[0] != 'punct':
                continue
            s = tok[1]
            if s in OPEN:
                stack.append(k)
            elif s in CLOSE:
                if stack and self.code[stack[-1]][1] == CLOSE[s]:
                    o = stack.pop()
                    self.match[o], self.rmatch[k] = k, o
                    self.parent[k] = stack[-1] if stack else -1
                else:
                    self.mismatched += 1
        for o in stack:
            self.match[o] = n - 1
            self.mismatched += 1

    def _attrs(self):
        """List of attributes: dict(start, end, inner, toks)."""
        self.attrs = []
        code = self.code
        k, n = 0, len(code)
        while k < n:
            if code[k][1] == '#' and code[k][0] == 'punct':
                if self.t(k + 1) == '[' and (k + 1) in self.match:
                    e = self.match[k + 1]
                    self.attrs.append(dict(start=k, end=e, inner=False,
                                           toks=[x[1] for x in code[k + 2:e]]))
                    k += 1
                    continue
                if self.t(k + 1) == '!' and self.t(k + 2) == '[' and (k + 2) in self.match:
                    e = self.match[k + 2]
                    self.attrs.append(dict(start=k, end=e, inner=True,
                                           toks=[x[1] for x in code[k + 3:e]]))
            k += 1

    def item_end(self, s):
        code, n, t = self.code, len(self.code), self.t
        k = s
        if t(k) == 'pub':
            k += 1
            if t(k) == '(':
                k = self.match.get(k, k) + 1
        while True:
            x = t(k)
            if x in QUALS:
                k += 1
                continue
            if x == 'const' and t(k + 1) in ('fn', 'unsafe', 'async', 'extern'):
                k += 1
                continue
            if x == 'extern' and k + 1 < n and code[k + 1][0] == 'str' and t(k + 2) == 'fn':
                k += 2
                continue
            break
        kw = t(k)
        if kw in ITEM_KW_SEMI or (kw == 'extern' and t(k + 1) == 'crate'):
            mode = 'semi'
        elif kw in ITEM_KW_BLOCK:
            mode = 'block'
        else:
            mode = 'expr'
        j = s
        while j < n:
            if code[j][0] == 'punct':
                x = code[j][1]
                if x in OPEN:
                    if x == '{' and mode in ('block', 'expr'):
                        return self.match.get(j, n - 1)
                    j = self.match.get(j, n - 1) + 1
                    continue
                if x in CLOSE:
                    return j - 1
                if x == ';':
                    return j
                if x == ',' and mode == 'expr':
                    return j
            j += 1
        return n - 1

    def _test_regions(self):
        n = len(self.code)
        self.in_test = [False] * n
        self.whole_test = False
        self.test_fns = []          # (attr names) per test fn item
        self.test_attr_counts = Counter()
        self.regions = []           # (start_line, end_line)
        attrs = self.attrs
        # inner #![cfg(test)] at file level => whole file is test code
        for a in attrs:
            if a['inner'] and self.parent[a['start']] == -1 and is_test_cfg(a['toks']):
                self.whole_test = True
        i = 0
        while i < len(attrs):
            if attrs[i]['inner']:
                i += 1
                continue
            run = [attrs[i]]
            while (i + 1 < len(attrs) and not attrs[i + 1]['inner']
                   and attrs[i + 1]['start'] == run[-1]['end'] + 1):
                i += 1
                run.append(attrs[i])
            i += 1
            item_start = run[-1]['end'] + 1
            if item_start >= n:
                continue
            names = [attr_path(a['toks']) for a in run]
            test_names = [nm for nm in names if nm in TEST_ATTRS]
            for nm in test_names:
                self.test_attr_counts[nm] += 1
            cfg_test = any(is_test_cfg(a['toks']) for a in run)
            if test_names or cfg_test:
                e = self.item_end(item_start)
                if test_names and self._item_kw(item_start) == 'fn':
                    self.test_fns.append(tuple(sorted(set(test_names))))
                for k in range(run[0]['start'], e + 1):
                    self.in_test[k] = True
                self.regions.append((self.code[run[0]['start']][2], self.code[max(e, item_start)][3]))
        # proptest! { .. } / quickcheck! { .. } blocks
        self.prop_macros = Counter()
        for k in range(n - 2):
            if self.code[k][0] == 'ident' and self.code[k][1] in ('proptest', 'quickcheck') \
                    and self.t(k + 1) == '!' and self.t(k + 2) in ('{', '(') and self.t(k - 1) != '::':
                self.prop_macros[self.code[k][1] + '!'] += 1
                e = self.match.get(k + 2, n - 1)
                for j in range(k, e + 1):
                    self.in_test[j] = True
                self.regions.append((self.code[k][2], self.code[e][3]))
        if self.whole_test:
            self.in_test = [True] * n
            self.regions = [(1, max(1, len(self.lines)))]

    def _item_kw(self, s):
        k = s
        t = self.t
        if t(k) == 'pub':
            k += 1
            if t(k) == '(':
                k = self.match.get(k, k) + 1
        while t(k) in QUALS or (t(k) == 'const' and t(k + 1) in ('fn', 'unsafe', 'async', 'extern')) \
                or (t(k) == 'extern' and k + 1 < len(self.code) and self.code[k + 1][0] == 'str'):
            k += 2 if t(k) == 'extern' else 1
        return t(k)

    def outer_attrs_before(self, k):
        out = []
        j = k - 1
        t = self.t
        if t(j) == ')' and j in self.rmatch and t(self.rmatch[j] - 1) == 'pub':
            j = self.rmatch[j] - 2
        elif t(j) == 'pub':
            j -= 1
        while t(j) == ']' and j in self.rmatch:
            o = self.rmatch[j]
            if t(o - 1) == '#':
                out.append([x[1] for x in self.code[o + 1:j]])
                j = o - 2
            else:
                break
        return out

    def mod_decls(self):
        out = []
        code, t = self.code, self.t
        for k, tok in enumerate(code):
            if tok[0] != 'ident' or tok[1] != 'mod' or k + 2 >= len(code):
                continue
            if code[k + 1][0] != 'ident' or t(k + 2) not in (';', '{'):
                continue
            attrs = self.outer_attrs_before(k)
            path_attr = None
            cfg_test = False
            for a in attrs:
                if len(a) >= 3 and a[0] == 'path' and a[1] == '=':
                    path_attr = re.sub(r'^r#*"|"#*$', '', a[2]).strip('"')
                if is_test_cfg(a):
                    cfg_test = True
            inline_names = []
            p = self.parent[k]
            while p != -1:
                if t(p) == '{' and p >= 2 and t(p - 2) == 'mod':
                    inline_names.insert(0, t(p - 1).replace('r#', ''))
                p = self.parent[p]
            out.append(dict(name=code[k + 1][1].replace('r#', ''), k=k, inline=t(k + 2) == '{',
                            path_attr=path_attr, cfg_test=cfg_test, in_test=self.in_test[k],
                            inline_names=inline_names))
        return out

    def macro_rules_mask(self):
        n = len(self.code)
        mask = [False] * n
        for k in range(n - 3):
            if self.code[k][1] == 'macro_rules' and self.t(k + 1) == '!' and self.t(k + 3) in OPEN:
                e = self.match.get(k + 3, n - 1)
                for j in range(k, e + 1):
                    mask[j] = True
        return mask

    def starts_with_inner_doc(self):
        for tok in self.toks:
            if tok[0] == 'idoc':
                return True
            if tok[0] == 'com':
                continue
            break
        a = [x for x in self.attrs if x['inner'] and x['toks'][:2] == ['doc', '=']]
        return bool(a)


# --------------------------------------------------------------------------- crate
def load_toml(p):
    try:
        with open(p, 'rb') as f:
            return tomllib.load(f)
    except Exception as e:  # noqa
        return {'__error__': str(e)}


def find_workspace_root(crate_dir, repo):
    d = os.path.abspath(crate_dir)
    repo = os.path.abspath(repo)
    while True:
        p = os.path.join(d, 'Cargo.toml')
        if os.path.isfile(p) and 'workspace' in load_toml(p):
            return d
        if d == repo or d == os.path.dirname(d):
            return None
        d = os.path.dirname(d)


def target_paths(crate_dir, manifest):
    """Return dict category -> list of root files (absolute)."""
    roots = {'lib': [], 'bin': [], 'build': [], 'tests': [], 'benches': [], 'examples': []}
    pkg = manifest.get('package', {})
    lib = manifest.get('lib', {})
    libp = lib.get('path', 'src/lib.rs')
    if os.path.isfile(os.path.join(crate_dir, libp)):
        roots['lib'].append(os.path.join(crate_dir, libp))
    bins = manifest.get('bin', [])
    for b in bins:
        if 'path' in b:
            roots['bin'].append(os.path.join(crate_dir, b['path']))
    if pkg.get('autobins', True):
        for p in [os.path.join(crate_dir, 'src/main.rs')] + glob.glob(os.path.join(crate_dir, 'src/bin/*.rs')) \
                + glob.glob(os.path.join(crate_dir, 'src/bin/*/main.rs')):
            if os.path.isfile(p):
                roots['bin'].append(p)
    build = pkg.get('build', None)
    if isinstance(build, str):
        roots['build'].append(os.path.join(crate_dir, build))
    elif build is None and os.path.isfile(os.path.join(crate_dir, 'build.rs')):
        roots['build'].append(os.path.join(crate_dir, 'build.rs'))
    for key, cat, auto in (('test', 'tests', 'autotests'), ('bench', 'benches', 'autobenches'),
                           ('example', 'examples', 'autoexamples')):
        for e in manifest.get(key, []):
            if 'path' in e:
                roots[cat].append(os.path.join(crate_dir, e['path']))
            elif 'name' in e:
                p = os.path.join(crate_dir, cat, e['name'] + '.rs')
                if os.path.isfile(p):
                    roots[cat].append(p)
        if pkg.get(auto, True):
            for p in glob.glob(os.path.join(crate_dir, cat, '*.rs')) + \
                    glob.glob(os.path.join(crate_dir, cat, '*/main.rs')):
                roots[cat].append(p)
    for cat in roots:
        roots[cat] = sorted(set(os.path.normpath(p) for p in roots[cat] if os.path.isfile(p)))
    return roots


def walk_rs(crate_dir):
    out = []
    for d, dirs, files in os.walk(crate_dir):
        rel = os.path.relpath(d, crate_dir)
        keep = []
        for x in dirs:
            if x in ('.git', 'target', 'node_modules'):
                continue
            sub = os.path.join(d, x)
            if os.path.isfile(os.path.join(sub, 'Cargo.toml')) and x != 'fuzz':
                continue  # nested separate crate
            keep.append(x)
        dirs[:] = keep
        for f in files:
            if f.endswith('.rs'):
                out.append(os.path.normpath(os.path.join(d, f)))
    return sorted(out)


def line_classes(fi):
    n = len(fi.lines)
    has_code = [False] * n
    has_com = [False] * n
    has_doc = [False] * n
    for kind, _text, sl, el in fi.toks:
        for ln in range(sl - 1, min(el, n)):
            if kind in CODE_KINDS:
                has_code[ln] = True
            else:
                has_com[ln] = True
                if kind in DOC_KINDS:
                    has_doc[ln] = True
    cls = []
    for i in range(n):
        if has_code[i]:
            cls.append('code')
        elif has_com[i]:
            cls.append('doc' if has_doc[i] else 'comment')
        elif fi.lines[i].strip() == '':
            cls.append('blank')
        else:
            cls.append('code')
    return cls


def count_doc_fences(text_lines):
    """Count fenced code blocks; return (total, rust_runnable)."""
    total = rust = 0
    inside = False
    for ln in text_lines:
        s = ln.strip()
        if s.startswith('```') or s.startswith('~~~'):
            if not inside:
                inside = True
                total += 1
                info = s[3:].strip().lower()
                tags = set(x.strip() for x in re.split(r'[,\s]+', info) if x.strip())
                if not (tags & NON_RUST_FENCES) and (not tags or tags & {'rust', 'should_panic', 'no_run', 'compile_fail', 'edition2015', 'edition2018', 'edition2021', 'edition2024'} or all(t.startswith('edition') for t in tags)):
                    rust += 1
            else:
                inside = False
    return total, rust


def doc_text_lines(fi, kinds=None):
    """Concatenate doc comment content lines (prod and test alike)."""
    out = []
    prev_line = -10
    for kind, text, sl, el in fi.toks:
        if kind not in DOC_KINDS:
            if kind in CODE_KINDS and kind != 'punct':
                pass
            continue
        if text.startswith('///') or text.startswith('//!'):
            if sl != prev_line + 1:
                out.append('')  # separator between doc blocks
            out.append(text[3:])
        else:
            body = text[3:-2]
            out.append('')
            for b in body.split('\n'):
                out.append(re.sub(r'^\s*\*', '', b))
        prev_line = el
    return out


class Crate:
    def __init__(self, repo, rel, test_crate=False):
        self.repo = os.path.abspath(repo)
        self.dir = os.path.normpath(os.path.join(self.repo, rel))
        self.rel = rel
        self.test_crate = test_crate
        self.manifest = load_toml(os.path.join(self.dir, 'Cargo.toml'))
        self.name = self.manifest.get('package', {}).get('name', rel)
        self.infos = {}
        self.cat = {}            # abspath -> category
        self.is_test_file = {}   # abspath -> bool
        self.mod_target = {}     # (file, k) -> child file
        self.unresolved_mods = []
        self.orphans = []

    def info(self, p):
        if p not in self.infos:
            self.infos[p] = FileInfo(p, os.path.relpath(p, self.repo))
        return self.infos[p]

    def build(self):
        roots = target_paths(self.dir, self.manifest)
        self.roots = {k: [os.path.relpath(p, self.repo) for p in v] for k, v in roots.items()}
        queue = []
        for cat, files in roots.items():
            for p in files:
                c = cat
                if cat in ('lib', 'bin'):
                    c = 'src'
                if self.test_crate and c in ('src', 'build'):
                    c = 'tests'
                queue.append((p, c, c == 'tests', True))
        seen = {}
        while queue:
            p, c, is_test, modrs = queue.pop(0)
            if p in seen and (seen[p] is False or is_test):
                continue
            seen[p] = is_test
            fi = self.info(p)
            self.cat[p] = c if not (c == 'src' and (is_test or fi.whole_test)) else 'src_test'
            if c == 'src' and (is_test or fi.whole_test):
                self.cat[p] = 'src_test'
            self.is_test_file[p] = is_test or fi.whole_test or c == 'tests'
            base_dir = os.path.dirname(p) if modrs else os.path.join(
                os.path.dirname(p), os.path.splitext(os.path.basename(p))[0])
            for d in fi.mod_decls():
                if d['inline']:
                    continue
                child_test = is_test or fi.whole_test or d['cfg_test'] or d['in_test']
                if d['path_attr']:
                    cands = [os.path.normpath(os.path.join(os.path.dirname(p), *d['inline_names'], d['path_attr']))]
                    child_modrs = True
                else:
                    b = os.path.join(base_dir, *d['inline_names'])
                    cands = [os.path.join(b, d['name'] + '.rs'), os.path.join(b, d['name'], 'mod.rs')]
                    child_modrs = False
                found = next((x for x in cands if os.path.isfile(x)), None)
                if found is None:
                    self.unresolved_mods.append((os.path.relpath(p, self.repo), d['name']))
                    continue
                found = os.path.normpath(found)
                if os.path.basename(found) == 'mod.rs':
                    child_modrs = True
                self.mod_target[(p, d['k'])] = found
                queue.append((found, c, child_test, child_modrs))
        # files not reached through the module tree
        for p in walk_rs(self.dir):
            if p in self.cat:
                continue
            relc = os.path.relpath(p, self.dir).replace(os.sep, '/')
            first = relc.split('/')[0]
            fi = self.info(p)
            if first in ('tests', 'benches', 'examples', 'fuzz'):
                c = first
            elif first == 'src':
                parts = relc.split('/')
                heur_test = 'tests' in parts[:-1] or parts[-1] in ('tests.rs',) or fi.whole_test
                c = 'orphan_test' if heur_test else 'orphan'
                self.orphans.append(relc)
            else:
                c = 'other'
            if self.test_crate and c in ('orphan', 'other', 'orphan_test'):
                c = 'tests'
            self.cat[p] = c
            self.is_test_file[p] = c in ('tests', 'orphan_test')

    # ------------------------------------------------------------------ metrics
    def metrics(self):
        m = dict(name=self.name, rel=self.rel, test_crate=self.test_crate)
        files_by_cat = Counter()
        lines = {b: Counter() for b in ('prod', 'test', 'bench', 'example', 'fuzz', 'build', 'other', 'dead', 'all')}
        largest = []
        safety = Counter()
        safety_all = Counter()
        risky = Counter()
        allow_attrs = Counter()
        todo = Counter()
        tests = Counter()
        test_fns = 0
        ignore_attrs = should_panic = 0
        prop_macros = Counter()
        prop_idents = Counter()
        doc = Counter()
        doc_kinds = Counter()
        undoc_samples = []
        fences_total = fences_rust = 0
        safety_comments = 0
        fn_prod = pub_fn = 0
        mismatched = 0
        for p, c in sorted(self.cat.items()):
            fi = self.info(p)
            mismatched += fi.mismatched
            files_by_cat[c] += 1
            cls = line_classes(fi)
            if c in ('orphan', 'orphan_test'):
                # not reachable from any target root => never compiled; report separately
                for cl in cls:
                    lines['all'][cl] += 1
                    lines['dead'][cl] += 1
                largest.append((sum(1 for x in cls if x == 'code'), len(cls), fi.rel, c))
                continue
            test_line = [False] * len(cls)
            if self.is_test_file.get(p) or c in ('tests', 'src_test', 'orphan_test'):
                test_line = [True] * len(cls)
            elif c in ('src', 'orphan'):
                for (a, b) in fi.regions:
                    for ln in range(a - 1, min(b, len(cls))):
                        test_line[ln] = True
            for i, cl in enumerate(cls):
                lines['all'][cl] += 1
                if c in ('src', 'orphan', 'src_test', 'tests', 'orphan_test'):
                    lines['test' if test_line[i] else 'prod'][cl] += 1
                elif c == 'benches':
                    lines['bench'][cl] += 1
                elif c == 'examples':
                    lines['example'][cl] += 1
                elif c == 'fuzz':
                    lines['fuzz'][cl] += 1
                elif c == 'build':
                    lines['build'][cl] += 1
                else:
                    lines['other'][cl] += 1
            ncode = sum(1 for x in cls if x == 'code')
            largest.append((ncode, len(cls), fi.rel, c))
            # test counters (all categories)
            for nm, v in fi.test_attr_counts.items():
                tests[nm] += v
            test_fns += len(fi.test_fns)
            prop_macros.update(fi.prop_macros)
            for a in fi.attrs:
                nm = attr_path(a['toks'])
                if nm == 'ignore':
                    ignore_attrs += 1
                elif nm == 'should_panic':
                    should_panic += 1
            for tok in fi.code:
                if tok[0] == 'ident' and tok[1] in ('proptest', 'quickcheck', 'arbitrary', 'Arbitrary',
                                                     'bolero', 'loom', 'kani', 'insta', 'criterion',
                                                     'divan', 'libfuzzer_sys', 'fuzz_target'):
                    prop_idents[tok[1]] += 1
            for kind, text, _sl, _el in fi.toks:
                if kind not in CODE_KINDS:
                    for mm in TODO_RE.findall(text):
                        todo[mm] += 1
            # doctest fences in doc comments (any category)
            tot, rs = count_doc_fences(doc_text_lines(fi))
            fences_total += tot
            fences_rust += rs
            # prod-only token metrics
            if c != 'src':
                # unsafe in any code, for reference
                self._count_safety(fi, [True] * len(fi.code), safety_all)
                continue
            prod_mask = [not x for x in fi.in_test]
            self._count_safety(fi, prod_mask, safety)
            self._count_safety(fi, [True] * len(fi.code), safety_all)
            code = fi.code
            for k, tok in enumerate(code):
                if not prod_mask[k] or tok[0] != 'ident':
                    continue
                if tok[1] in RISKY_IDENTS[:9]:
                    risky[tok[1]] += 1
                if tok[1] == 'fn' and fi.t(k + 1) and code[k + 1][0] == 'ident':
                    fn_prod += 1
            for a in fi.attrs:
                if not prod_mask[a['start']]:
                    continue
                nm = attr_path(a['toks'])
                if nm in ('allow', 'expect'):
                    allow_attrs['__attributes__#' + nm] += 1  # one per attribute
                    inner = a['toks'][2:-1]
                    names = ''.join(inner).split(',')
                    for x in names:
                        if x and not x.startswith('reason'):
                            allow_attrs[('#![' if a['inner'] else '#[') + nm + '(' + x + ')]'] += 1
            for i, (kind, text, sl, _el) in enumerate(fi.toks):
                if kind == 'com' and SAFETY_RE.search(text):
                    ck = next((fi.code_of_full[x] for x in range(i + 1, len(fi.toks)) if x in fi.code_of_full), None)
                    if ck is None or prod_mask[ck]:
                        safety_comments += 1
            # documentation heuristic
            mr = fi.macro_rules_mask()
            for k, tok in enumerate(code):
                if tok[1] != 'pub' or tok[0] != 'ident' or not prod_mask[k] or mr[k]:
                    continue
                kind = self._pub_kind(fi, k)
                if kind in ('restricted',):
                    doc_kinds['pub(restricted)'] += 1
                    continue
                if kind in ('use', 'extern_crate', 'unknown'):
                    doc_kinds['pub ' + kind] += 1
                    continue
                if kind == 'fn':
                    pub_fn += 1
                status = self._doc_status(fi, k, kind)
                grp = 'field' if kind in ('field', 'tuple_field') else 'item'
                doc[grp + '_total'] += 1
                doc_kinds[kind + '_total'] += 1
                if status == 'doc':
                    doc[grp + '_doc'] += 1
                    doc_kinds[kind + '_doc'] += 1
                elif status == 'hidden':
                    doc[grp + '_hidden'] += 1
                elif len(undoc_samples) < 400:
                    undoc_samples.append('%s:%d %s' % (fi.rel, tok[2], kind))
        # crate-level docs, lints, doc settings
        lib_root = self.roots['lib'][0] if self.roots['lib'] else (self.roots['bin'][0] if self.roots['bin'] else None)
        crate_doc = dict(root=lib_root, inner_doc_lines=0, doc_include=None, readme_fences=None)
        root_lints = []
        if lib_root:
            fi = self.info(os.path.join(self.repo, lib_root))
            crate_doc['inner_doc_lines'] = sum(1 for (k, tx, a, b) in fi.toks if k == 'idoc' for _ in range(b - a + 1))
            for a in fi.attrs:
                if not a['inner'] or fi.parent[a['start']] != -1:
                    continue
                nm = attr_path(a['toks'])
                txt = ''.join(a['toks'])
                if nm == 'doc' and 'include_str' in txt:
                    inc = None
                    for x in a['toks']:
                        if x.startswith('"'):
                            inc = x.strip('"')
                    crate_doc['doc_include'] = inc
                    if inc:
                        rp = os.path.normpath(os.path.join(os.path.dirname(fi.abspath), inc))
                        if os.path.isfile(rp):
                            with open(rp, encoding='utf-8', errors='replace') as f:
                                crate_doc['readme_fences'] = count_doc_fences(f.read().split('\n'))
                if nm in ('forbid', 'deny', 'warn', 'allow', 'expect', 'cfg_attr'):
                    root_lints.append('#![' + txt + ']')
        lib_tbl = self.manifest.get('lib', {})
        m.update(
            files=dict(files_by_cat), n_rs_files=sum(files_by_cat.values()),
            lines={k: dict(v) for k, v in lines.items()},
            largest=sorted(largest, reverse=True)[:8],
            safety_prod=dict(safety), safety_all=dict(safety_all), risky_prod=dict(risky),
            allow_attrs_prod=dict(allow_attrs), todo_markers=dict(todo),
            safety_comments_prod=safety_comments,
            tests=dict(test_attrs=dict(tests), test_fns=test_fns, ignore=ignore_attrs,
                       should_panic=should_panic, prop_macros=dict(prop_macros),
                       idents=dict(prop_idents), doc_fences_total=fences_total,
                       doc_fences_rust=fences_rust, doctest_enabled=lib_tbl.get('doctest', True),
                       proc_macro=lib_tbl.get('proc-macro', False)),
            docs=dict(doc), doc_kinds=dict(doc_kinds), undoc_samples=undoc_samples,
            crate_doc=crate_doc, root_lints=root_lints, roots=self.roots,
            orphans=self.orphans, unresolved_mods=self.unresolved_mods,
            fn_prod=fn_prod, pub_fn=pub_fn, mismatched_brackets=mismatched,
            src_test_files=sorted(os.path.relpath(p, self.repo) for p, c in self.cat.items() if c == 'src_test'),
        )
        return m

    @staticmethod
    def _count_safety(fi, mask, ctr):
        code, t = fi.code, fi.t
        n = len(code)
        for k in range(n):
            if not mask[k]:
                continue
            kind, x = code[k][0], code[k][1]
            if kind != 'ident':
                continue
            if x == 'unsafe':
                nx = t(k + 1)
                if nx == '{':
                    ctr['unsafe_block'] += 1
                elif nx == 'fn' or (nx == 'extern' and t(k + 3) == 'fn') or (nx == 'extern' and t(k + 2) == 'fn'):
                    ctr['unsafe_fn'] += 1
                elif nx == 'impl':
                    ctr['unsafe_impl'] += 1
                elif nx in ('trait', 'auto'):
                    ctr['unsafe_trait'] += 1
                elif nx == 'extern':
                    ctr['unsafe_extern_block'] += 1
                elif nx == '(':
                    ctr['unsafe_attr'] += 1
                else:
                    ctr['unsafe_other'] += 1
            elif x == 'unwrap' and t(k - 1) == '.' and t(k + 1) == '(' and t(k + 2) == ')':
                ctr['unwrap'] += 1
            elif x == 'expect' and t(k - 1) == '.' and t(k + 1) == '(':
                ctr['expect'] += 1
            elif x in ('panic', 'unreachable', 'todo', 'unimplemented') and t(k + 1) == '!':
                ctr[x] += 1
            elif x in ('assert', 'assert_eq', 'assert_ne') and t(k + 1) == '!':
                ctr['assert'] += 1

    @staticmethod
    def _pub_kind(fi, k):
        t = fi.t
        j = k + 1
        if t(j) == '(':
            return 'restricted'
        while True:
            x = t(j)
            if x in QUALS:
                j += 1
                continue
            if x == 'const' and t(j + 1) in ('fn', 'unsafe', 'async', 'extern'):
                j += 1
                continue
            if x == 'extern' and j + 1 < len(fi.code) and fi.code[j + 1][0] == 'str':
                j += 2
                continue
            break
        x = t(j)
        if x == 'use':
            return 'use'
        if x == 'extern' and t(j + 1) == 'crate':
            return 'extern_crate'
        if x in PUB_KINDS:
            return x
        if j < len(fi.code) and fi.code[j][0] == 'ident' and t(j + 1) == ':' and t(j + 2) != ':':
            return 'field'
        par = fi.parent[k]
        if par != -1 and t(par) == '(':
            return 'tuple_field'
        return 'unknown'

    def _doc_status(self, fi, k, kind):
        j = fi.full_of_code[k] - 1
        toks = fi.toks
        while j >= 0:
            kd, tx = toks[j][0], toks[j][1]
            if kd == 'odoc':
                return 'doc'
            if kd == 'com':
                j -= 1
                continue
            if kd == 'idoc':
                break
            ck = fi.code_of_full.get(j)
            if tx == ']' and ck is not None and ck in fi.rmatch:
                o = fi.rmatch[ck]
                if fi.t(o - 1) == '#':
                    a = [x[1] for x in fi.code[o + 1:ck]]
                    if a[:1] == ['doc']:
                        if len(a) > 1 and a[1] == '=':
                            return 'doc'
                        if 'hidden' in a:
                            return 'hidden'
                    if a[:1] == ['cfg_attr'] and 'doc' in a and '=' in a:
                        return 'doc'
                    j = fi.full_of_code[o - 1] - 1
                    continue
            break
        if kind == 'mod':
            # pub mod x;  -> module file inner docs ; pub mod x { //! .. }
            mk = k + 1
            while fi.t(mk) != 'mod' and mk < k + 4:
                mk += 1
            if fi.t(mk + 2) == ';':
                tgt = self.mod_target.get((fi.abspath, mk))
                if tgt and self.info(tgt).starts_with_inner_doc():
                    return 'doc'
            elif fi.t(mk + 2) == '{':
                fo = fi.full_of_code[mk + 2] + 1
                while fo < len(toks) and toks[fo][0] == 'com':
                    fo += 1
                if fo < len(toks) and toks[fo][0] == 'idoc':
                    return 'doc'
        return None


def aggregate(crates):
    agg = dict(files=Counter(), lines={}, safety_prod=Counter(), safety_all=Counter(),
               risky_prod=Counter(), allow_attrs_prod=Counter(), todo_markers=Counter(),
               docs=Counter(), doc_kinds=Counter(), test_attrs=Counter(), prop_macros=Counter(),
               idents=Counter(), largest=[])
    test_fns = ignore = should_panic = fences_total = fences_rust = 0
    safety_comments = fn_prod = pub_fn = 0
    for m in crates:
        agg['files'].update(m['files'])
        for b, v in m['lines'].items():
            agg['lines'].setdefault(b, Counter()).update(v)
        for key in ('safety_prod', 'safety_all', 'risky_prod', 'allow_attrs_prod', 'todo_markers',
                    'docs', 'doc_kinds'):
            agg[key].update(m[key])
        agg['test_attrs'].update(m['tests']['test_attrs'])
        agg['prop_macros'].update(m['tests']['prop_macros'])
        agg['idents'].update(m['tests']['idents'])
        test_fns += m['tests']['test_fns']
        ignore += m['tests']['ignore']
        should_panic += m['tests']['should_panic']
        fences_total += m['tests']['doc_fences_total']
        fences_rust += m['tests']['doc_fences_rust']
        safety_comments += m['safety_comments_prod']
        fn_prod += m['fn_prod']
        pub_fn += m['pub_fn']
        agg['largest'].extend([tuple(x) for x in m['largest']])
    agg['largest'] = sorted(agg['largest'], reverse=True)[:8]
    out = {k: (dict(v) if isinstance(v, Counter) else v) for k, v in agg.items()}
    out['lines'] = {k: dict(v) for k, v in agg['lines'].items()}
    out.update(test_fns=test_fns, ignore=ignore, should_panic=should_panic,
               doc_fences_total=fences_total, doc_fences_rust=fences_rust,
               safety_comments_prod=safety_comments, fn_prod=fn_prod, pub_fn=pub_fn,
               n_rs_files=sum(agg['files'].values()))
    # derived
    L = out['lines']
    prod_code = L.get('prod', {}).get('code', 0)
    test_code = L.get('test', {}).get('code', 0)
    out['prod_code'] = prod_code
    out['test_code'] = test_code
    out['test_to_code'] = round(test_code / prod_code, 3) if prod_code else None
    k = prod_code / 1000.0 if prod_code else None
    out['per_kloc'] = {x: round(out['safety_prod'].get(x, 0) / k, 2) for x in
                       ('unsafe_block', 'unsafe_fn', 'unsafe_impl', 'unwrap', 'expect', 'panic',
                        'unreachable', 'todo', 'unimplemented', 'assert')} if k else {}
    d = out['docs']
    it = d.get('item_total', 0) - d.get('item_hidden', 0)
    ft = d.get('field_total', 0) - d.get('field_hidden', 0)
    out['doc_ratio_items'] = round(d.get('item_doc', 0) / it, 3) if it else None
    out['doc_ratio_fields'] = round(d.get('field_doc', 0) / ft, 3) if ft else None
    out['doc_ratio_all'] = round((d.get('item_doc', 0) + d.get('field_doc', 0)) / (it + ft), 3) if (it + ft) else None
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--name', required=True)
    ap.add_argument('--repo', required=True)
    ap.add_argument('--crate', action='append', default=[])
    ap.add_argument('--test-crate', action='append', default=[])
    ap.add_argument('--json', required=True)
    args = ap.parse_args()
    crates = []
    for rel in args.crate:
        c = Crate(args.repo, rel)
        c.build()
        crates.append(c.metrics())
    for rel in args.test_crate:
        c = Crate(args.repo, rel, test_crate=True)
        c.build()
        crates.append(c.metrics())
    res = dict(name=args.name, repo=os.path.abspath(args.repo), crates=crates, total=aggregate(crates))
    with open(args.json, 'w') as f:
        json.dump(res, f, indent=1, ensure_ascii=False)
    t = res['total']
    L = t['lines']
    print('%s: files=%s prod(code=%s comment=%s doc=%s blank=%s) test_code=%s ratio=%s test_fns=%s attrs=%s'
          % (args.name, t['n_rs_files'], L.get('prod', {}).get('code'), L.get('prod', {}).get('comment'),
             L.get('prod', {}).get('doc'), L.get('prod', {}).get('blank'), t['test_code'],
             t['test_to_code'], t['test_fns'], t['test_attrs']))
    print('  safety_prod=%s' % t['safety_prod'])
    print('  docs=%s items=%s fields=%s' % (t['docs'], t['doc_ratio_items'], t['doc_ratio_fields']))
    for m in crates:
        print('  crate %s: files=%s orphans=%d unresolved=%s mismatched=%d' % (
            m['name'], m['files'], len(m['orphans']), m['unresolved_mods'][:5], m['mismatched_brackets']))


if __name__ == '__main__':
    main()
````

`scripts/deps_audit.py` (SHA-256 `2b375caa0024753b5a01dcf54cf4cf0b6073918d0e1484d6a8a2029b33737cf1`):

````python
#!/usr/bin/env python3
"""deps_audit.py -- read-only dependency metrics + RustSec matching.

Reads Cargo.toml / Cargo.lock as TOML data and the RustSec advisory-db
(crates/<name>/RUSTSEC-*.md front matter).  Nothing is executed.

  python3 -I deps_audit.py --name NAME --repo REPO --crate REL [--crate REL ...]
         --advisory-db PATH --json OUT.json

Definitions
* direct deps: keys of [dependencies]/[dev-dependencies]/[build-dependencies]
  (+ target.'cfg'.* variants) of the selected crates; `workspace = true`
  is resolved against [workspace.dependencies]; path deps to the selected crates
  are "internal" and excluded from external counts.
* lock closure (normal): start from the selected crates' lock entries, keep only
  the edges that correspond to normal/build deps in Cargo.toml, then follow every
  edge of registry/git packages (their dev-deps are never in a lockfile) and the
  normal/build edges of other workspace members.  This is the "all features, all
  targets" runtime+build footprint a downstream user can pull in.
* lock closure (all): same but also following dev-dependency edges of workspace
  members (what `cargo test` in that workspace needs).
* advisory match: a locked version is affected when it matches none of the
  advisory's `patched` and `unaffected` requirements (withdrawn advisories skipped).
"""
import argparse
import glob
import json
import os
import re
import sys
import tomllib
from collections import defaultdict, Counter

# ------------------------------------------------------------------ semver
VER_RE = re.compile(r'^v?(\d+|[*xX])(?:\.(\d+|[*xX]))?(?:\.(\d+|[*xX]))?(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$')
CMP_RE = re.compile(r'^\s*(>=|<=|>|<|=|\^|~)?\s*(\S+)\s*$')


def parse_version(s):
    m = VER_RE.match(s.strip())
    if not m:
        raise ValueError('bad version ' + s)
    nums = [m.group(1), m.group(2), m.group(3)]
    pre = m.group(4)
    vals = [int(x) if x is not None and x.isdigit() else 0 for x in nums]
    pre_t = tuple(int(x) if x.isdigit() else x for x in pre.split('.')) if pre else ()
    return (vals[0], vals[1], vals[2], pre_t)


def key(v):
    a, b, c, pre = v
    if not pre:
        return (a, b, c, 1, ())
    return (a, b, c, 0, tuple((0, x, '') if isinstance(x, int) else (1, 0, x) for x in pre))


def comparators(req):
    """Return list of (op, version) primitive comparisons that must all hold."""
    out = []
    for part in req.split(','):
        part = part.strip()
        if not part or part == '*':
            continue
        m = CMP_RE.match(part)
        if not m:
            raise ValueError('bad comparator ' + part)
        op, vs = m.group(1) or '^', m.group(2)
        vm = VER_RE.match(vs)
        if not vm:
            raise ValueError('bad comparator version ' + part)
        comps = [x for x in (vm.group(1), vm.group(2), vm.group(3)) if x is not None and x not in '*xX']
        n = len(comps)
        if any(x is not None and x in ('*', 'x', 'X') for x in (vm.group(1), vm.group(2), vm.group(3))):
            op = '='
        M = int(comps[0]) if n >= 1 else 0
        mi = int(comps[1]) if n >= 2 else 0
        pa = int(comps[2]) if n >= 3 else 0
        pre = tuple(int(x) if x.isdigit() else x for x in vm.group(4).split('.')) if vm.group(4) else ()
        v = (M, mi, pa, pre)
        if n == 0:
            continue
        if op == '^':
            if n == 1:
                hi = (M + 1, 0, 0, ())
            elif n == 2:
                hi = (M + 1, 0, 0, ()) if M > 0 else (0, mi + 1, 0, ())
            else:
                hi = (M + 1, 0, 0, ()) if M > 0 else ((0, mi + 1, 0, ()) if mi > 0 else (0, 0, pa + 1, ()))
            out += [('>=', v), ('<', hi)]
        elif op == '~':
            hi = (M + 1, 0, 0, ()) if n == 1 else (M, mi + 1, 0, ())
            out += [('>=', v), ('<', hi)]
        elif op == '=':
            if n == 3:
                out.append(('==', v))
            elif n == 2:
                out += [('>=', (M, mi, 0, ())), ('<', (M, mi + 1, 0, ()))]
            else:
                out += [('>=', (M, 0, 0, ())), ('<', (M + 1, 0, 0, ()))]
        elif op == '>=':
            out.append(('>=', v))
        elif op == '<':
            out.append(('<', v))
        elif op == '>':
            if n == 3:
                out.append(('>', v))
            elif n == 2:
                out.append(('>=', (M, mi + 1, 0, ())))
            else:
                out.append(('>=', (M + 1, 0, 0, ())))
        elif op == '<=':
            if n == 3:
                out.append(('<=', v))
            elif n == 2:
                out.append(('<', (M, mi + 1, 0, ())))
            else:
                out.append(('<', (M + 1, 0, 0, ())))
    return out


def satisfies(version, req):
    v = key(parse_version(version) if isinstance(version, str) else version)
    for op, w in comparators(req):
        w = key(w)
        if op == '>=' and not v >= w:
            return False
        if op == '>' and not v > w:
            return False
        if op == '<' and not v < w:
            return False
        if op == '<=' and not v <= w:
            return False
        if op == '==' and not v == w:
            return False
    return True


def lower_bound(req):
    lo = (0, 0, 0, ())
    for op, w in comparators(req):
        if op in ('>=', '==') and key(w) > key(lo):
            lo = w
        if op == '>' and key(w) >= key(lo):
            lo = (w[0], w[1], w[2] + 1, ())
    return lo


def vstr(v):
    s = '%d.%d.%d' % v[:3]
    if v[3]:
        s += '-' + '.'.join(str(x) for x in v[3])
    return s


# ------------------------------------------------------------------ advisories
def load_advisories(db):
    adv = defaultdict(list)
    n = 0
    for p in glob.glob(os.path.join(db, 'crates', '*', '*.md')):
        with open(p, encoding='utf-8', errors='replace') as f:
            txt = f.read()
        m = re.match(r'\s*```toml\s*\n(.*?)\n```', txt, re.S)
        if not m:
            continue
        try:
            d = tomllib.loads(m.group(1))
        except Exception:
            continue
        a, v = d.get('advisory', {}), d.get('versions', {})
        tm = re.search(r'^#\s+(.+)$', txt[m.end():], re.M)
        adv[a.get('package')].append(dict(
            id=a.get('id'), date=str(a.get('date')), withdrawn=str(a['withdrawn']) if a.get('withdrawn') else None,
            informational=a.get('informational'), patched=v.get('patched', []),
            unaffected=v.get('unaffected', []), title=tm.group(1).strip() if tm else '',
            aliases=a.get('aliases', []), cvss=a.get('cvss')))
        n += 1
    return adv, n


def is_affected(version, a):
    for r in list(a['patched']) + list(a['unaffected']):
        try:
            if satisfies(version, r):
                return False
        except ValueError:
            return None
    return True


# ------------------------------------------------------------------ manifests
def load_toml(p):
    with open(p, 'rb') as f:
        return tomllib.load(f)


def find_ws_root(crate_dir, repo):
    d = os.path.abspath(crate_dir)
    repo = os.path.abspath(repo)
    while True:
        p = os.path.join(d, 'Cargo.toml')
        if os.path.isfile(p):
            t = load_toml(p)
            if 'workspace' in t:
                return d, t
        if d == repo or d == os.path.dirname(d):
            return None, None
        d = os.path.dirname(d)


def inherit(val, ws, key):
    if isinstance(val, dict) and val.get('workspace') is True:
        return (ws or {}).get('workspace', {}).get('package', {}).get(key)
    return val


def dep_tables(man):
    for kind, tkey in (('normal', 'dependencies'), ('dev', 'dev-dependencies'), ('build', 'build-dependencies')):
        yield kind, None, man.get(tkey, {})
        yield kind, None, man.get(tkey.replace('-', '_'), {}) if tkey != 'dependencies' else {}
    for cfg, t in man.get('target', {}).items():
        for kind, tkey in (('normal', 'dependencies'), ('dev', 'dev-dependencies'), ('build', 'build-dependencies')):
            yield kind, cfg, t.get(tkey, {})


def resolve_dep(k, v, ws):
    d = {'key': k}
    if isinstance(v, str):
        d['req'] = v
    else:
        v = dict(v)
        if v.get('workspace') is True:
            wv = (ws or {}).get('workspace', {}).get('dependencies', {}).get(k, {})
            if isinstance(wv, str):
                wv = {'version': wv}
            merged = dict(wv)
            for kk, vv in v.items():
                if kk == 'features':
                    merged['features'] = list(merged.get('features', [])) + list(vv)
                elif kk != 'workspace':
                    merged[kk] = vv
            v = merged
            d['via_workspace'] = True
        d['req'] = v.get('version')
        d['path'] = v.get('path')
        d['git'] = v.get('git')
        d['optional'] = v.get('optional', False)
        d['features'] = v.get('features', [])
        d['default_features'] = v.get('default-features', v.get('default_features', True))
        if v.get('package'):
            d['package'] = v['package']
    d['package'] = d.get('package', k)
    return d


def find_member_manifests(repo):
    out = {}
    for p in glob.glob(os.path.join(repo, '**', 'Cargo.toml'), recursive=True):
        if '/target/' in p or '/.git/' in p:
            continue
        try:
            t = load_toml(p)
        except Exception:
            continue
        nm = t.get('package', {}).get('name')
        if nm:
            out[nm] = (p, t)
    return out


# ------------------------------------------------------------------ lockfile
def parse_lock(p):
    t = load_toml(p)
    pk = t.get('package', [])
    entries = []
    for e in pk:
        entries.append(dict(name=e['name'], version=e['version'], source=e.get('source'),
                            deps=e.get('dependencies', [])))
    by_name = defaultdict(list)
    for i, e in enumerate(entries):
        by_name[e['name']].append(i)
    return t.get('version'), entries, by_name


def dep_ref(s, entries, by_name):
    parts = s.split(' ')
    nm = parts[0]
    cands = by_name.get(nm, [])
    if len(parts) >= 2:
        cands = [i for i in cands if entries[i]['version'] == parts[1]] or cands
    return cands[0] if cands else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--name', required=True)
    ap.add_argument('--repo', required=True)
    ap.add_argument('--crate', action='append', default=[])
    ap.add_argument('--advisory-db', required=True)
    ap.add_argument('--json', required=True)
    args = ap.parse_args()
    repo = os.path.abspath(args.repo)
    adv, n_adv = load_advisories(args.advisory_db)
    members = find_member_manifests(repo)
    res = dict(name=args.name, advisories_loaded=n_adv, crates=[])
    selected = []
    ws_root = None
    for rel in args.crate:
        cdir = os.path.join(repo, rel)
        man = load_toml(os.path.join(cdir, 'Cargo.toml'))
        wsd, ws = find_ws_root(cdir, repo)
        ws_root = ws_root or wsd
        pkg = man.get('package', {})
        info = dict(rel=rel, name=pkg.get('name'), version=inherit(pkg.get('version'), ws, 'version'),
                    edition=inherit(pkg.get('edition'), ws, 'edition'),
                    rust_version=inherit(pkg.get('rust-version'), ws, 'rust-version'),
                    license=inherit(pkg.get('license'), ws, 'license'),
                    license_file=inherit(pkg.get('license-file'), ws, 'license-file'),
                    workspace_root=os.path.relpath(wsd, repo) if wsd else None,
                    lints=None, deps=[])
        lt = man.get('lints')
        if isinstance(lt, dict) and lt.get('workspace') is True:
            info['lints'] = dict(source='workspace', table=(ws or {}).get('workspace', {}).get('lints', {}))
        elif lt:
            info['lints'] = dict(source='crate', table=lt)
        for kind, cfg, tbl in dep_tables(man):
            for k, v in tbl.items():
                d = resolve_dep(k, v, ws)
                d['kind'] = kind
                d['target'] = cfg
                info['deps'].append(d)
        selected.append(info)
    sel_names = {c['name'] for c in selected}
    member_names = set(members)
    for c in selected:
        cnt = Counter()
        ext = defaultdict(set)
        for d in c['deps']:
            internal = d['package'] in sel_names or (d.get('path') is not None) or \
                (d['package'] in member_names and d.get('req') is None)
            d['internal'] = bool(internal)
            if internal:
                cnt[d['kind'] + '_internal'] += 1
                continue
            ext[d['kind']].add(d['package'])
            if d.get('optional'):
                cnt['optional'] += 1
            if d.get('git'):
                cnt['git'] += 1
        for kind in ('normal', 'dev', 'build'):
            cnt[kind] = len(ext[kind])
        c['counts'] = dict(cnt)
        res['crates'].append(c)
    # union over selected crates
    union = defaultdict(set)
    for c in selected:
        for d in c['deps']:
            if not d['internal']:
                union[d['kind']].add(d['package'])
    res['union'] = {k: sorted(v) for k, v in union.items()}
    res['union_counts'] = {k: len(v) for k, v in union.items()}
    res['union_counts']['normal_or_build'] = len(union['normal'] | union['build'])
    res['union_counts']['dev_only'] = len(union['dev'] - union['normal'] - union['build'])
    # lockfile
    lock_path = os.path.join(ws_root or os.path.join(repo, args.crate[0]), 'Cargo.lock')
    if not os.path.isfile(lock_path):
        alt = os.path.join(repo, args.crate[0], 'Cargo.lock')
        lock_path = alt if os.path.isfile(alt) else None
    res['lockfile'] = os.path.relpath(lock_path, repo) if lock_path else None
    findings = []
    if lock_path:
        lver, entries, by_name = parse_lock(lock_path)
        srcs = Counter('registry' if (e['source'] or '').startswith('registry+') else
                       ('git' if (e['source'] or '').startswith('git+') else 'local') for e in entries)
        res['lock'] = dict(format_version=lver, total=len(entries), **dict(srcs))

        def normal_names(nm):
            if nm in sel_names:
                cc = next(c for c in selected if c['name'] == nm)
                return {d['package'] for d in cc['deps'] if d['kind'] in ('normal', 'build')}
            if nm in members:
                _, t = members[nm]
                return {resolve_dep(k, v, None)['package'] for kind, cfg, tbl in dep_tables(t)
                        if kind in ('normal', 'build') for k, v in tbl.items()}
            return None

        def closure(include_dev):
            start = [i for i, e in enumerate(entries) if e['name'] in sel_names and e['source'] is None]
            seen = set(start)
            stack = list(start)
            while stack:
                i = stack.pop()
                e = entries[i]
                allowed = None
                if e['source'] is None and not include_dev:
                    allowed = normal_names(e['name'])
                for s in e['deps']:
                    j = dep_ref(s, entries, by_name)
                    if j is None:
                        continue
                    if allowed is not None and entries[j]['name'] not in allowed:
                        continue
                    if j not in seen:
                        seen.add(j)
                        stack.append(j)
            ext_ = [i for i in seen if entries[i]['source'] is not None]
            loc_ = [i for i in seen if entries[i]['source'] is None]
            return set(ext_), loc_

        cn, ln_ = closure(False)
        ca, la_ = closure(True)
        dup = Counter(entries[i]['name'] for i in cn)
        res['lock']['closure_normal_external'] = len(cn)
        res['lock']['closure_normal_local'] = sorted(entries[i]['name'] for i in ln_)
        res['lock']['closure_all_external'] = len(ca)
        res['lock']['closure_normal_duplicates'] = sorted(k for k, v in dup.items() if v > 1)
        res['lock']['closure_normal_names'] = sorted('%s %s' % (entries[i]['name'], entries[i]['version']) for i in cn)
        for i, e in enumerate(entries):
            if not (e['source'] or '').startswith('registry+'):
                continue
            for a in adv.get(e['name'], []):
                if a['withdrawn']:
                    continue
                aff = is_affected(e['version'], a)
                if aff:
                    findings.append(dict(package=e['name'], version=e['version'], id=a['id'],
                                         kind=a['informational'] or 'vulnerability', title=a['title'],
                                         patched=a['patched'], date=a['date'],
                                         in_normal_closure=i in cn, in_all_closure=i in ca))
    res['lock_advisories'] = findings
    # without lockfile: check the lower bound of each direct requirement
    direct = []
    for c in selected:
        for d in c['deps']:
            if d['internal'] or not d.get('req'):
                continue
            try:
                lo = lower_bound(d['req'])
            except ValueError:
                continue
            for a in adv.get(d['package'], []):
                if a['withdrawn']:
                    continue
                aff = is_affected(vstr(lo), a)
                if aff:
                    fixable = False
                    for r in a['patched']:
                        try:
                            if satisfies(vstr(lower_bound(r)), d['req']):
                                fixable = True
                        except ValueError:
                            pass
                    direct.append(dict(crate=c['name'], dep=d['package'], req=d['req'], kind=d['kind'],
                                       lower_bound=vstr(lo), id=a['id'],
                                       adv_kind=a['informational'] or 'vulnerability', title=a['title'],
                                       patched=a['patched'], patched_within_req=fixable))
    res['direct_req_advisories'] = direct
    # tooling config files
    cfgs = []
    for f in ('deny.toml', '.cargo/audit.toml', 'audit.toml', '.cargo/config.toml', '.cargo/config',
              'clippy.toml', '.clippy.toml', 'rustfmt.toml', '.rustfmt.toml', 'rust-toolchain',
              'rust-toolchain.toml', '.github/dependabot.yml', '.github/dependabot.yaml', 'renovate.json',
              '.github/renovate.json', '.renovaterc', '.renovaterc.json', 'SECURITY.md', 'CHANGELOG.md',
              'release-plz.toml', '.release-plz.toml', 'codecov.yml', '.config/nextest.toml', 'justfile',
              'Makefile', 'flake.nix'):
        for base in sorted({repo} | {os.path.join(repo, r) for r in args.crate}):
            if os.path.exists(os.path.join(base, f)):
                cfgs.append(os.path.relpath(os.path.join(base, f), repo))
    res['config_files'] = cfgs
    with open(args.json, 'w') as f:
        json.dump(res, f, indent=1, ensure_ascii=False, default=str)
    print('%s: union=%s lock=%s lock_adv=%d direct_req_adv=%d cfg=%s' % (
        args.name, res['union_counts'], {k: v for k, v in res.get('lock', {}).items() if k not in ('closure_normal_names',)},
        len(findings), len(direct), cfgs))
    for c in selected:
        print('  %s %s ed=%s msrv=%s lic=%s counts=%s lints=%s' % (
            c['name'], c['version'], c['edition'], c['rust_version'], c['license'], c['counts'],
            (c['lints'] or {}).get('source')))
    for fnd in findings:
        print('  ADV %s %s %s %s in_normal=%s in_all=%s' % (fnd['package'], fnd['version'], fnd['id'], fnd['kind'],
                                                          fnd['in_normal_closure'], fnd['in_all_closure']))
    for d in direct:
        print('  REQ %s -> %s %s (lb %s) %s %s fixable=%s' % (d['crate'], d['dep'], d['req'], d['lower_bound'],
                                                              d['id'], d['adv_kind'], d['patched_within_req']))


if __name__ == '__main__':
    main()
````

`scripts/ci_scan.py` (SHA-256 `e02d00ed5feddb63c68924138eb18d4ae338a5d26cc6a819e2786a34434b4795`):

````python
#!/usr/bin/env python3
"""ci_scan.py -- parse GitHub Actions workflows (YAML as data) and classify checks.

  python3 -I ci_scan.py --name NAME --repo REPO [--msrv 1.75] --json OUT.json

Only reads .github/workflows/*.yml|yaml, dependabot/renovate configs and any repo
script a workflow `run:` step names (scripts are read as text, never executed).
"""
import argparse
import glob
import json
import os
import re
import sys

import yaml  # system PyYAML; safe_load only

FEATURES = {
    'tests': r'\bcargo\s+(\+\S+\s+)?(test|nextest)\b|nextest|\bjust\s+test|\bmake\s+test',
    'clippy': r'\bclippy\b',
    'fmt': r'\bcargo\s+(\+\S+\s+)?fmt\b|\brustfmt\b',
    'fuzz': r'cargo\s+(\+\S+\s+)?fuzz|cargo-fuzz|cifuzz|oss-fuzz',
    'coverage': r'tarpaulin|llvm-cov|grcov|codecov|coveralls',
    'release': r'cargo\s+publish|release-plz|cargo-release|release-please|cargo-dist|crates-io-auth-action|action-gh-release|gh\s+release',
    'audit/deny': r'cargo\s+audit|cargo-audit|audit-check|cargo\s+deny|cargo-deny',
    'docs': r'\bcargo\s+(\+\S+\s+)?doc\b|\bmkdocs\b|rustdoc',
    'miri': r'\bmiri\b',
    'unused-deps': r'udeps|machete|cargo-shear',
    'semver-checks': r'semver-checks',
    'codeql/sast': r'codeql|semgrep|scorecard',
    'sanitizers': r'-Zsanitizer',
    'msrv-tool': r'cargo-msrv|cargo\s+msrv|--rust-version|cargo-hack',
    'build': r'\bcargo\s+(\+\S+\s+)?(build|check)\b',
    'bench': r'\bcargo\s+(\+\S+\s+)?bench\b|criterion|codspeed',
    'lockfile-check': r'--locked|--frozen',
}


def flatten(x):
    if isinstance(x, dict):
        for k, v in x.items():
            yield from flatten(v)
    elif isinstance(x, list):
        for v in x:
            yield from flatten(v)
    elif x is not None:
        yield str(x)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--name', required=True)
    ap.add_argument('--repo', required=True)
    ap.add_argument('--msrv', default=None)
    ap.add_argument('--json', required=True)
    args = ap.parse_args()
    repo = os.path.abspath(args.repo)
    wfs = sorted(glob.glob(os.path.join(repo, '.github', 'workflows', '*.yml')) +
                 glob.glob(os.path.join(repo, '.github', 'workflows', '*.yaml')))
    out = dict(name=args.name, workflows=[], features={}, toolchains=set(), runners=set(),
               actions_total=0, actions_sha_pinned=0, scripts_called=[])
    all_text = []
    for p in wfs:
        with open(p, encoding='utf-8', errors='replace') as f:
            raw = f.read()
        try:
            d = yaml.safe_load(raw) or {}
        except Exception as e:
            out['workflows'].append(dict(file=os.path.basename(p), error=str(e)))
            continue
        trig = d.get('on', d.get(True))
        if isinstance(trig, dict):
            trig = sorted(str(k) for k in trig.keys())
        elif isinstance(trig, str):
            trig = [trig]
        jobs = []
        wf_text = []
        for jn, j in (d.get('jobs') or {}).items():
            if not isinstance(j, dict):
                continue
            steps = j.get('steps') or []
            runs, uses = [], []
            for s in steps:
                if not isinstance(s, dict):
                    continue
                if 'run' in s:
                    runs.append(str(s['run']))
                if 'uses' in s:
                    uses.append(str(s['uses']))
                    out['actions_total'] += 1
                    if re.search(r'@[0-9a-f]{40}\b', str(s['uses'])):
                        out['actions_sha_pinned'] += 1
                    w = s.get('with') or {}
                    if isinstance(w, dict) and 'toolchain' in w:
                        out['toolchains'].add(str(w['toolchain']))
                    m = re.search(r'rust-toolchain@([\w.\-]+)', str(s['uses']))
                    if m and not re.fullmatch(r'[0-9a-f]{40}', m.group(1)) and m.group(1) not in ('master', 'v1'):
                        out['toolchains'].add(m.group(1))
                wf_text += list(flatten(s))
            if 'uses' in j:  # reusable workflow job
                uses.append(str(j['uses']))
                wf_text.append(str(j['uses']))
            strat = j.get('strategy') or {}
            matrix = strat.get('matrix') if isinstance(strat, dict) else None
            ro = j.get('runs-on')
            for r in flatten(ro):
                out['runners'].add(r)
            if isinstance(matrix, dict):
                for k, v in matrix.items():
                    if k in ('os', 'runner', 'platform', 'runs-on'):
                        for r in flatten(v):
                            out['runners'].add(r)
                    if k in ('rust', 'toolchain', 'rust-version', 'msrv', 'channel'):
                        for r in flatten(v):
                            out['toolchains'].add(r)
                    if k == 'include':
                        for inc in (v or []):
                            if isinstance(inc, dict):
                                for kk in ('os', 'runner', 'target'):
                                    if kk in inc:
                                        out['runners'].add(str(inc[kk]))
                                for kk in ('rust', 'toolchain'):
                                    if kk in inc:
                                        out['toolchains'].add(str(inc[kk]))
            wf_text += [str(jn), str(j.get('name', ''))] + list(flatten(matrix)) + list(flatten(j.get('env')))
            jobs.append(dict(id=jn, name=j.get('name'), runs_on=ro, matrix=matrix,
                             needs=j.get('needs'), runs=runs, uses=uses, cond=j.get('if')))
        wf_text += list(flatten(d.get('env')))
        all_text += wf_text
        feats = sorted(k for k, rx in FEATURES.items() if re.search(rx, '\n'.join(wf_text)))
        out['workflows'].append(dict(file=os.path.basename(p), name=d.get('name'), triggers=trig,
                                     n_jobs=len(jobs), jobs=jobs, features=feats))
    # scripts referenced by run steps (read as text)
    text = '\n'.join(all_text)
    for m in re.finditer(r'(?:\./)?((?:scripts|ci|tools|\.github)/[\w./-]+\.(?:sh|ps1|py))', text):
        sp = os.path.join(repo, m.group(1))
        if os.path.isfile(sp) and m.group(1) not in [s['path'] for s in out['scripts_called']]:
            with open(sp, encoding='utf-8', errors='replace') as f:
                body = f.read()
            out['scripts_called'].append(dict(path=m.group(1), body=body[:4000]))
            text += '\n' + body
    for tool in ('justfile', 'Makefile'):
        if re.search(r'\b(just|make)\b', text) and os.path.isfile(os.path.join(repo, tool)):
            with open(os.path.join(repo, tool), encoding='utf-8', errors='replace') as f:
                body = f.read()
            out['scripts_called'].append(dict(path=tool, body=body[:6000]))
    out['features'] = {k: bool(re.search(rx, text)) for k, rx in FEATURES.items()}
    tc = sorted(out['toolchains'])
    out['msrv_in_ci'] = bool(args.msrv) and any(t.startswith(args.msrv) for t in tc)
    out['toolchains'] = tc
    out['runners'] = sorted(out['runners'])
    out['os_families'] = sorted({('windows' if 'windows' in r.lower() else 'macos' if 'macos' in r.lower()
                                  else 'linux' if ('ubuntu' in r.lower() or 'linux' in r.lower()) else r)
                                 for r in out['runners']})
    dep = None
    for f in ('.github/dependabot.yml', '.github/dependabot.yaml'):
        p = os.path.join(repo, f)
        if os.path.isfile(p):
            with open(p, encoding='utf-8') as fh:
                dd = yaml.safe_load(fh) or {}
            dep = sorted({u.get('package-ecosystem') for u in dd.get('updates', [])})
    out['dependabot'] = dep
    out['renovate'] = [f for f in ('renovate.json', '.github/renovate.json', '.renovaterc', '.renovaterc.json',
                                   'renovate.json5') if os.path.isfile(os.path.join(repo, f))]
    with open(args.json, 'w') as f:
        json.dump(out, f, indent=1, ensure_ascii=False, default=list)
    print('%s: workflows=%s' % (args.name, [(w['file'], w.get('triggers'), w.get('features')) for w in out['workflows']]))
    print('  features=%s' % sorted(k for k, v in out['features'].items() if v))
    print('  toolchains=%s msrv_in_ci=%s os=%s dependabot=%s renovate=%s pinned=%d/%d scripts=%s' % (
        tc, out['msrv_in_ci'], out['os_families'], dep, out['renovate'], out['actions_sha_pinned'],
        out['actions_total'], [s['path'] for s in out['scripts_called']]))


if __name__ == '__main__':
    main()
````

`scripts/make_tables.py` (SHA-256 `2edc1de88f931878f2136d39f32225d05bf82e599c8f21e4962a45ba7e651222`):

````python
#!/usr/bin/env python3
"""make_tables.py -- turn results/{m,d,c}_*.json into Markdown tables.

  python3 -I make_tables.py RESULTS_DIR > tables.md
"""
import json
import os
import sys

ORDER = ['nfs-mamont', 'nfsserve', 'zerofs_nfsserve', 'nfs3', 'embednfs', 'nessie-nfsserve', 'fractal-nfs', 'nfs-v3']
LABEL = {'nfs3': 'nfs3_server+types+macros', 'embednfs': 'embednfs+proto'}


def load(r, p, n):
    fp = os.path.join(r, '%s_%s.json' % (p, n))
    return json.load(open(fp)) if os.path.exists(fp) else None


def g(d, *ks, default=0):
    for k in ks:
        if not isinstance(d, dict) or k not in d:
            return default
        d = d[k]
    return d


def row(cells):
    return '| ' + ' | '.join(str(c) for c in cells) + ' |'


def main():
    r = sys.argv[1]
    M = {n: load(r, 'm', n) for n in ORDER}
    Dd = {n: load(r, 'd', n) for n in ORDER}
    C = {n: load(r, 'c', n) for n in ORDER}
    out = []
    P = out.append

    P('### Размер (size)\n')
    P(row(['Codebase', '.rs files: all / prod-src / in-src test files / tests/ / examples / benches / dead',
           'prod code', 'prod comment', 'prod doc-comment', 'prod blank', 'test code', 'all code (every .rs)',
           'dead code lines']))
    P(row(['---'] * 9))
    for n in ORDER:
        t = M[n]['total']
        f = t['files']
        L = t['lines']
        P(row([LABEL.get(n, n),
               '%d / %d / %d / %d / %d / %d / %d' % (t['n_rs_files'], f.get('src', 0), f.get('src_test', 0),
                                                    f.get('tests', 0), f.get('examples', 0), f.get('benches', 0),
                                                    f.get('orphan', 0) + f.get('orphan_test', 0)),
               g(L, 'prod', 'code'), g(L, 'prod', 'comment'), g(L, 'prod', 'doc'), g(L, 'prod', 'blank'),
               g(L, 'test', 'code'), g(L, 'all', 'code'), g(L, 'dead', 'code')]))
    P('\n### Крупнейшие файлы (top-3 по code lines; category)\n')
    P(row(['Codebase', '#1', '#2', '#3']))
    P(row(['---'] * 4))
    for n in ORDER:
        lg = M[n]['total']['largest'][:3]
        P(row([LABEL.get(n, n)] + ['`%s` %d (%s)' % (x[2].split('/', 1)[-1] if n != 'nfs-mamont' else x[2], x[0], x[3])
                                     for x in lg]))

    P('\n### Тесты (tests)\n')
    P(row(['Codebase', 'test fns', '#[test]', '#[tokio::test]', 'other test attrs', '#[ignore]',
           'files in tests/', 'in-src test-module files', 'doc-test rust blocks (doctest on?)',
           'test code / prod code', 'test fns per 1K prod LOC']))
    P(row(['---'] * 11))
    for n in ORDER:
        t = M[n]['total']
        ta = t['test_attrs']
        other = {k: v for k, v in ta.items() if k not in ('test', 'tokio::test')}
        doct = [c['tests']['doctest_enabled'] for c in M[n]['crates'] if not c.get('test_crate')]
        readme_f = [c['crate_doc'].get('readme_fences') for c in M[n]['crates']]
        rf = sum(x[1] for x in readme_f if x)
        P(row([LABEL.get(n, n), t['test_fns'], ta.get('test', 0), ta.get('tokio::test', 0), other or '-',
               t['ignore'], t['files'].get('tests', 0), t['files'].get('src_test', 0),
               '%d%s (%s)' % (t['doc_fences_rust'], (' + README %d' % rf) if rf else '',
                              '/'.join('on' if x else 'off' for x in doct)),
               t['test_to_code'], round(t['test_fns'] / (t['prod_code'] / 1000.0), 1)]))

    P('\n### Безопасность и робастность в prod-коде (абсолютные значения)\n')
    P(row(['Codebase', 'unsafe {}', 'unsafe fn', 'unsafe impl', '// SAFETY:', '.unwrap()', '.expect(', 'panic!',
           'unreachable!', 'todo!', 'unimplemented!', 'Σ panicking', 'attrs #[allow] / #[expect]', 'TODO/FIXME/XXX/HACK (all code)']))
    P(row(['---'] * 14))
    for n in ORDER:
        t = M[n]['total']
        s = t['safety_prod']
        tot = sum(s.get(k, 0) for k in ('unwrap', 'expect', 'panic', 'unreachable', 'todo', 'unimplemented'))
        P(row([LABEL.get(n, n), s.get('unsafe_block', 0), s.get('unsafe_fn', 0), s.get('unsafe_impl', 0),
               t['safety_comments_prod'], s.get('unwrap', 0), s.get('expect', 0), s.get('panic', 0),
               s.get('unreachable', 0), s.get('todo', 0), s.get('unimplemented', 0), tot,
               '%d / %d' % (t['allow_attrs_prod'].get('__attributes__#allow', 0), t['allow_attrs_prod'].get('__attributes__#expect', 0)), sum(t['todo_markers'].values())]))
    P('\n### То же на 1000 строк prod-кода (per 1K LOC)\n')
    P(row(['Codebase', 'prod code', 'unsafe blocks', 'unwrap', 'expect', 'panic!', 'unreachable!', 'todo!+unimplemented!',
           'Σ panicking / KLOC']))
    P(row(['---'] * 9))
    for n in ORDER:
        t = M[n]['total']
        s = t['safety_prod']
        k = t['prod_code'] / 1000.0
        tot = sum(s.get(x, 0) for x in ('unwrap', 'expect', 'panic', 'unreachable', 'todo', 'unimplemented'))
        P(row([LABEL.get(n, n), t['prod_code']] + ['%.2f' % (s.get(x, 0) / k) for x in
                                                   ('unsafe_block', 'unwrap', 'expect', 'panic', 'unreachable')]
              + ['%.2f' % ((s.get('todo', 0) + s.get('unimplemented', 0)) / k), '%.2f' % (tot / k)]))

    P('\n### Документация (эвристика, один скрипт для всех)\n')
    P(row(['Codebase', 'pub items (non-field)', 'documented', 'ratio', 'pub fields', 'documented', 'ratio',
           'overall', 'pub(restricted) (excluded)', 'doc-comment lines per 100 prod code lines',
           'crate-level docs (lib.rs)', 'examples/ .rs']))
    P(row(['---'] * 12))
    for n in ORDER:
        t = M[n]['total']
        d = t['docs']
        cd = []
        for c in M[n]['crates']:
            if c.get('test_crate'):
                continue
            x = c['crate_doc']
            cd.append('%s: %s' % (c['name'], ('//! %d lines' % x['inner_doc_lines']) if x['inner_doc_lines'] else
                                  ('include_str!(%s)' % x['doc_include'] if x['doc_include'] else 'none')))
        P(row([LABEL.get(n, n), d.get('item_total', 0) - d.get('item_hidden', 0), d.get('item_doc', 0),
               t['doc_ratio_items'], d.get('field_total', 0), d.get('field_doc', 0), t['doc_ratio_fields'],
               t['doc_ratio_all'], t['doc_kinds'].get('pub(restricted)', 0),
               round(100.0 * g(t['lines'], 'prod', 'doc') / t['prod_code'], 1), '; '.join(cd),
               t['files'].get('examples', 0)]))

    P('\n### Зависимости (dependencies)\n')
    P(row(['Codebase', 'direct normal / dev / build (external, union)', 'optional', 'Cargo.lock',
           'lock: packages (registry/git/local)', 'normal+build closure (external pkgs)',
           'closure incl. dev', 'dup names in closure', 'MSRV', 'edition', 'license']))
    P(row(['---'] * 11))
    for n in ORDER:
        d = Dd[n]
        uc = d['union_counts']
        opt = sum(c['counts'].get('optional', 0) for c in d['crates'])
        lk = d.get('lock')
        P(row([LABEL.get(n, n), '%d / %d / %d' % (uc.get('normal', 0), uc.get('dev', 0), uc.get('build', 0)), opt,
               d['lockfile'] or 'нет (no lockfile, direct deps only)',
               ('%d (%d/%d/%d)' % (lk['total'], lk.get('registry', 0), lk.get('git', 0), lk.get('local', 0))) if lk else '-',
               lk['closure_normal_external'] if lk else '-', lk['closure_all_external'] if lk else '-',
               ', '.join(lk['closure_normal_duplicates']) if lk else '-',
               '; '.join(sorted({str(c['rust_version']) for c in d['crates']})),
               '; '.join(sorted({str(c['edition']) for c in d['crates']})),
               '; '.join(sorted({str(c['license']) for c in d['crates']}))]))
    P('\n### Прямые зависимости (normal) по кодовым базам\n')
    for n in ORDER:
        d = Dd[n]
        deps = []
        for c in d['crates']:
            for x in c['deps']:
                if x['internal']:
                    continue
                deps.append('%s%s %s%s' % ('(dev) ' if x['kind'] == 'dev' else ('(build) ' if x['kind'] == 'build' else ''),
                                           x['package'], x.get('req') or ('git' if x.get('git') else '?'),
                                           ' [opt]' if x.get('optional') else ''))
        P('- **%s**: %s' % (LABEL.get(n, n), ', '.join(sorted(set(deps)))))

    P('\n### RustSec (advisory-db) — совпадения по Cargo.lock\n')
    P(row(['Codebase', 'advisories loaded', 'in normal closure of measured crates', 'only via dev-deps', 'elsewhere in workspace lock']))
    P(row(['---'] * 5))
    for n in ORDER:
        d = Dd[n]
        f = d['lock_advisories']
        a = ['%s %s %s (%s)' % (x['package'], x['version'], x['id'], x['kind']) for x in f if x['in_normal_closure']]
        b = ['%s %s %s (%s)' % (x['package'], x['version'], x['id'], x['kind']) for x in f if x['in_all_closure'] and not x['in_normal_closure']]
        c = ['%s %s %s (%s)' % (x['package'], x['version'], x['id'], x['kind']) for x in f if not x['in_all_closure']]
        P(row([LABEL.get(n, n), d['advisories_loaded'], '; '.join(a) or ('n/a (no lock)' if not d['lockfile'] else '0'),
               '; '.join(b) or '-', ('%d: ' % len(c) + '; '.join(c)) if c else '-']))
    P('\n### Требования прямых зависимостей, у которых нижняя граница попадает под advisory (без учёта lock)\n')
    for n in ORDER:
        d = Dd[n]
        xs = ['%s %s (lb %s) %s %s%s' % (x['dep'], x['req'], x['lower_bound'], x['id'], x['adv_kind'],
                                         '' if x['patched_within_req'] else ' NOT-fixable-within-req')
              for x in d['direct_req_advisories']]
        P('- **%s**: %s' % (LABEL.get(n, n), '; '.join(sorted(set(xs))) or 'нет'))

    P('\n### Линты и конфигурация инструментов\n')
    P(row(['Codebase', 'crate-root #![..] lint attrs', '[lints] table (source)', 'key lint levels', 'config files']))
    P(row(['---'] * 5))
    keys = [('rust', 'unsafe_code'), ('rust', 'missing_docs'), ('clippy', 'unwrap_used'), ('clippy', 'expect_used'),
            ('clippy', 'panic'), ('clippy', 'pedantic'), ('clippy', 'nursery'), ('clippy', 'all'),
            ('clippy', 'indexing_slicing'), ('clippy', 'undocumented_unsafe_blocks'), ('rust', 'unreachable_pub')]
    for n in ORDER:
        m = M[n]
        d = Dd[n]
        rl = sorted({x for c in m['crates'] for x in c['root_lints']})
        lt = [(c['name'], c['lints']) for c in d['crates'] if c['lints']]
        lv = []
        if lt:
            tbl = lt[0][1]['table']
            for grp, k in keys:
                v = tbl.get(grp, {}).get(k)
                if v is not None:
                    lv.append('%s::%s=%s' % (grp, k, v['level'] if isinstance(v, dict) else v))
        P(row([LABEL.get(n, n), '; '.join(rl) or '-',
               ', '.join('%s(%s)' % (a, b['source']) for a, b in lt) or '-', ', '.join(lv) or '-',
               ', '.join(d['config_files']) or '-']))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
````

`scripts/crosscheck_linecounts.py` (SHA-256 `729551ec387589b662f2270a7558b9fc1c4cdf8a43cc661599a0a37c79a1b040`):

````python
#!/usr/bin/env python3
"""crosscheck_linecounts.py -- compare rs_metrics' lexer with crates.io `linecounts`.

  python3 -I crosscheck_linecounts.py CRATESIO_JSON TARBALL_DIR [TARBALL_DIR ...]

CRATESIO_JSON: {"<crate>-<version>": {"code": N, "comment": N, "files": N}, ...}
For each extracted tarball, counts lines of every .rs file whose path has no
`tests`/`benches`/`examples` directory component (assumed crates.io exclusion),
with doc comments reported separately.  Reads files as text only.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # our own scripts dir only
from rs_metrics import FileInfo, line_classes  # noqa: E402

EXCL = {'tests', 'benches', 'examples'}


def main():
    ref = json.load(open(sys.argv[1]))
    print('| crate | crates.io code / comment / files | ours: code / plain comment / doc-comment / files (excl. tests,benches,examples dirs) | Δ code | Δ comment |')
    print('| --- | --- | --- | --- | --- |')
    for top in sys.argv[2:]:
        root = next(os.path.join(top, d) for d in os.listdir(top) if os.path.isdir(os.path.join(top, d)))
        key = os.path.basename(root)
        code = com = doc = files = 0
        for d, dirs, fs in os.walk(root):
            rel = os.path.relpath(d, root).split(os.sep)
            if EXCL & set(rel):
                continue
            for f in fs:
                if not f.endswith('.rs'):
                    continue
                fi = FileInfo(os.path.join(d, f), f)
                cls = line_classes(fi)
                code += cls.count('code')
                com += cls.count('comment')
                doc += cls.count('doc')
                files += 1
        r = ref.get(key, {})
        print('| %s | %s / %s / %s | %d / %d / %d / %d | %+d | %+d |' % (
            key, r.get('code'), r.get('comment'), r.get('files'), code, com, doc, files,
            code - (r.get('code') or 0), com - (r.get('comment') or 0)))


if __name__ == '__main__':
    main()
````

`scripts/run_all.sh` (SHA-256 `16acbba4bd2198288c751a4605422f0c15abb9e8e7d3f576d66445ab9ed3a27e`):

````bash
#!/usr/bin/env bash
# run_all.sh -- apply the same read-only scripts to every codebase.
# All scripts run with `python3 -I`, from a working dir outside the downloads,
# with the downloaded paths passed as arguments.
set -u
BASE=/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/code_metrics_comparison
D=$BASE/downloads; S=$BASE/scripts; R=$BASE/results
ADB=$D/advisory-db/repo
cd "$R" || exit 1
PY="python3 -I"

run() { # name repo msrv crates...  (prefix a crate with T: to mark it as a test-only crate)
  local name=$1 repo=$2 msrv=$3; shift 3
  local cargs=() dargs=()
  for c in "$@"; do
    if [[ $c == T:* ]]; then cargs+=(--test-crate "${c#T:}"); else cargs+=(--crate "$c"); dargs+=(--crate "$c"); fi
  done
  $PY "$S/rs_metrics.py" --name "$name" --repo "$repo" "${cargs[@]}" --json "$R/m_$name.json"
  $PY "$S/deps_audit.py" --name "$name" --repo "$repo" "${dargs[@]}" --advisory-db "$ADB" --json "$R/d_$name.json"
  $PY "$S/ci_scan.py" --name "$name" --repo "$repo" ${msrv:+--msrv "$msrv"} --json "$R/c_$name.json"
}

run nfs-mamont       /home/user/nfs-mamont                1.75 nfs-mamont
run nfsserve         $D/nfsserve/repo                     ""   .
run zerofs_nfsserve  $D/zerofs_nfsserve/repo              ""   .
run nfs3             $D/nfs3/repo                         1.88 crates/nfs3_server crates/nfs3_types crates/nfs3_macros T:crates/nfs3_tests
run embednfs         $D/embednfs/repo                     1.88 crates/embednfs crates/embednfs-proto
run nessie-nfsserve  $D/nessie-store/repo                 1.96 crates/nessie-nfsserve
run fractal-nfs      $D/fractalbits-main-86200e9/repo     ""   crates/fs_server/fractal-nfs
run nfs-v3           $D/nfswolf/repo                      1.95 crates/nfs-v3
# NFSWolf protocol family for reference (XDR/RPC layers nfs-v3 builds on)
$PY "$S/rs_metrics.py" --name nfswolf-family --repo $D/nfswolf/repo --crate crates/nfs-v3 --crate crates/onc-xdr \
    --crate crates/onc-xdr-derive --crate crates/onc-rpc-client --crate crates/nfs-mount --json "$R/m_nfswolf-family.json"

# Secondary: published crates.io tarballs (what users download)
for t in nfs-mamont-0.2.0 nfsserve-0.11.0 zerofs_nfsserve-0.19.1 nfs3_server-0.11.1 nfs3_types-0.5.3 nfs3_macros-0.5.1 \
         embednfs-0.4.1 embednfs-proto-0.4.1 nessie-nfsserve-0.3.1 fractal-nfs-0.1.0 nfs-v3-1.2.0; do
  dir=$(find "$D/crate-$t" -mindepth 1 -maxdepth 1 -type d | head -1)
  $PY "$S/rs_metrics.py" --name "tarball-$t" --repo "$dir" --crate . --json "$R/t_$t.json" | head -1
done
````

`scripts/run_nfs_mamont_cargo.sh` (SHA-256 `2c76180a4f29d283bc979d5a46157a56db3b15ce6eeeec1b6887196a354612a8`):

````bash
#!/usr/bin/env bash
# run_nfs_mamont_cargo.sh -- cargo checks on the user's OWN repo (nfs-mamont).
# Writes nothing into /home/user/nfs-mamont: CARGO_TARGET_DIR is redirected,
# --locked forbids Cargo.lock rewrites, cargo fmt is only run with --check.
set -u
BASE=/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/code_metrics_comparison
OUT=$BASE/results/cargo
mkdir -p "$OUT"
export CARGO_TARGET_DIR=$BASE/target
export CARGO_TERM_COLOR=never
M=/home/user/nfs-mamont/Cargo.toml
cd "$BASE/results" || exit 1
ms() { date +%s%N | cut -c1-13; }
{
  echo "date: $(date -Is)"
  echo "toolchain: $(rustc -V) | $(cargo -V) | $(cargo clippy -V) | $(cargo fmt --version)"
  echo "nproc: $(nproc)"
  echo "repo HEAD: $(git -C /home/user/nfs-mamont rev-parse HEAD)"
  cargo fetch --locked --manifest-path "$M" > "$OUT/fetch.log" 2>&1; echo "fetch rc=$?"
  for i in 1 2; do
    rm -rf "$CARGO_TARGET_DIR"
    s=$(ms); cargo build --locked --manifest-path "$M" > "$OUT/build_$i.log" 2>&1; rc=$?; e=$(ms)
    echo "clean debug build #$i (default features): rc=$rc wall_ms=$((e - s)) compiled_units=$(grep -c 'Compiling ' "$OUT/build_$i.log")"
  done
  rm -rf "$CARGO_TARGET_DIR"
  s=$(ms); cargo build --locked --all-features --manifest-path "$M" > "$OUT/build_allfeat.log" 2>&1; rc=$?; e=$(ms)
  echo "clean debug build (--all-features): rc=$rc wall_ms=$((e - s))"
  s=$(ms); cargo test --locked --manifest-path "$M" > "$OUT/test.log" 2>&1; rc=$?; e=$(ms)
  echo "cargo test (default features): rc=$rc wall_ms_incl_build=$((e - s))"
  cargo test --locked --all-features --manifest-path "$M" > "$OUT/test_allfeat.log" 2>&1; echo "cargo test --all-features: rc=$?"
  cargo clippy --locked --all-targets --manifest-path "$M" --message-format=json > "$OUT/clippy.json" 2> "$OUT/clippy.stderr"; echo "cargo clippy --all-targets: rc=$?"
  cargo clippy --locked --all-targets --all-features --manifest-path "$M" -- -D warnings > "$OUT/clippy_ci.log" 2>&1; echo "cargo clippy --all-targets --all-features -- -D warnings (CI cmd): rc=$?"
  cargo fmt --manifest-path "$M" --all --check > "$OUT/fmt.log" 2>&1; echo "cargo fmt --all --check: rc=$? diff_hunks=$(grep -c '^Diff in' "$OUT/fmt.log")"
  RUSTDOCFLAGS="-W missing_docs" cargo doc --locked --no-deps --manifest-path "$M" --message-format=json > "$OUT/doc_missing.json" 2> "$OUT/doc_missing.stderr"; echo "RUSTDOCFLAGS=-W missing_docs cargo doc --no-deps: rc=$?"
  RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps --document-private-items --manifest-path "$M" > "$OUT/doc_ci.log" 2>&1; echo "RUSTDOCFLAGS=-D warnings cargo doc --all-features --no-deps --document-private-items (CI cmd): rc=$?"
  echo "git status after runs: [$(git -C /home/user/nfs-mamont status --porcelain | tr '\n' ';')]"
  echo "done: $(date -Is)"
} > "$OUT/summary.txt" 2>&1
````
