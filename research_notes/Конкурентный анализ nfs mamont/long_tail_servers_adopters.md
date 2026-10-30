# Длинный хвост Rust NFS-серверов/протокольных крейтов и продукты, встраивающие NFS-сервер (контекст для nfs-mamont)

Обозначения. Все метрики сняты 2026-10-30, если не указано иное. Данные crates.io взяты из API (`https://crates.io/api/v1/crates/<name>`, `.../reverse_dependencies`, `.../<ver>/dependencies`). «Загрузки» — всего / за последние 90 дней (`recent_downloads`). Исходники крейтов скачаны с crates.io и прочитаны локально. Ссылки на файлы ведут на копии исходников на docs.rs (`https://docs.rs/crate/<crate>/<ver>/source/<path>`), это те же опубликованные файлы. Звёзды и коммиты GitHub взяты со страниц репозиториев и со страниц `network/dependents` (WebFetch, 2026-10-30). Даты последних коммитов определены по `git clone --filter=blob:none` (2026-10-30). Пометка **[STALE]** означает, что проект не обновлялся больше 6 месяцев на 2026-10-30. Обратные зависимости crates.io учитывают только текущую (default) версию зависимого крейта.

## 1. Профили оставшихся Rust NFS-серверов и протокольных крейтов (long tail)

### Takeaway
В длинном хвосте нет ни одной поддерживаемой переиспользуемой библиотеки NFS-сервера, которая конкурировала бы с nfs-mamont за встраивание. `fractal-nfs` её авторы фактически бросили через 12 дней после публикации: они ушли на nfs-ganesha. Семейство NFSWolf — это клиенты и кодеки для пентеста, а не сервер. `filemux` и `lcpfs` — внутренние реализации без подключаемого VFS. `lcpfs` вообще не имеет сетевого транспорта. Самый функциональный Rust NFS-сервер вне crates.io — `flint` (NFSv4.2 + pNFS + RPCSEC_GSS). Он вшит в Kubernetes CSI-драйвер и как крейт не публикуется. Остальные GitHub-реализации (bold-nfs, nfs-rs/nfs-rs, GitDataAI/nfs) — заброшенные, заготовки или удалённые репозитории.

### Cited Findings

#### Сводная таблица (2026-10-30)

| Крейт / проект | Что это | Версия, дата | Загрузки (всего/90 дн.) | GitHub ★, коммиты | Статус сопровождения |
|---|---|---|---|---|---|
| `fractal-nfs` | библиотека NFSv3-сервера (tokio/compio) | 0.1.0, 2026-05-21 (единственная) | 44 / 10 | fractalbits-labs/fractalbits: 306★, 1,755 комм. | исходник удалён из репо 2026-06-02, выбран nfs-ganesha |
| `nfswolf` + `nfs-v2`, `nfs-v3`, `nfs-v4`, `nfs-mount`, `onc-xdr`, `onc-xdr-derive`, `onc-rpc-client`, `onc-rpcbind` | security-тулкит + клиенты/кодеки | все 1.2.0, 2026-08-18 | 70; 131; 147; 137; 196; 296; 321; 269; 126 | StrongWind1/NFSWolf: 6★, 305 комм. | последний коммит 2026-09-12 |
| `filemux-core` (+`-cli`, `-gui`) | HTTP/TFTP/NFSv3 файловый сервер (приложение) | 0.2.0, 2026-10-26 | 82 (cli 31, gui 32) | lona-cn/filemux: 0★, 9 комм. | новый (2026-10-26…30), 1 автор |
| `links-fs` | ФС-драйвер; NFS через `nfs3_server` (адоптер, не конкурент) | 0.21.1, 2026-10-22 | 23 / 23 | link-foundation/lfs: 0★, 50 комм. | все коммиты за 2026-10-22 |
| `nfs_mirror` | CLI-зеркало каталога по NFS на `zerofs_nfsserve` | 0.1.2, 2025-10-29 | 368 / 7 | W-Mai/nfs_mirror: 2★, 7 комм. | **[STALE]** с 2025-10-29 |
| `lcpfs` | no_std CoW-ФС, модуль «NFSv4 server» без транспорта | 2026.1.102, 2026-01-03 | 104 / 12 | репозиторий 404 | **[STALE]**, репо недоступно |
| flint (GitHub, не крейт) | NFSv4.2/4.1-сервер + pNFS + Kerberos внутри CSI-драйвера | — | — | ddalton/flint: 0★, 3,301 комм. | очень активен (последний коммит 2026-10-30) |
| bold-nfs (GitHub) | NFSv4.0-сервер на tokio (WIP) | — | — | Schille/bold-nfs: 12★, 24 комм. | **[STALE]** с 2024-10-27 |
| nfs-rs/nfs-rs (GitHub) | заготовка «NFSv4 server» | — | — | 0★, 7 комм. | **[STALE]** с 2025-10-21 |
| GitDataAI/nfs (GitHub) | NFSv3/TCP-сервер (по индексу поиска) | — | — | страница 404 | недоступен |

Источники строк: [crates.io API: fractal-nfs](https://crates.io/api/v1/crates/fractal-nfs), [nfswolf](https://crates.io/api/v1/crates/nfswolf), [nfs-v3](https://crates.io/api/v1/crates/nfs-v3), [filemux-core](https://crates.io/api/v1/crates/filemux-core), [links-fs](https://crates.io/api/v1/crates/links-fs), [nfs_mirror](https://crates.io/api/v1/crates/nfs_mirror), [lcpfs](https://crates.io/api/v1/crates/lcpfs). GitHub: [fractalbits](https://github.com/fractalbits-labs/fractalbits-main), [NFSWolf](https://github.com/StrongWind1/NFSWolf), [filemux](https://github.com/lona-cn/filemux), [lfs](https://github.com/link-foundation/lfs), [nfs_mirror](https://github.com/w-mai/nfs_mirror), [lcpfs (404)](https://github.com/artst3in/lcpfs), [flint](https://github.com/ddalton/flint), [bold-nfs](https://github.com/schille/bold-nfs), [nfs-rs org](https://github.com/nfs-rs), [GitDataAI/nfs (404)](https://github.com/GitDataAI/nfs). Даты коммитов взяты из локальных blobless-клонов (2026-10-30).

#### fractal-nfs (fractalbits-labs)
- Метаданные: v0.1.0 — единственная версия, опубликована 2026-05-21. 44 загрузки (10 за 90 дней). Apache-2.0. Издатель на crates.io — `fractal-bits`. Обратных зависимостей 0 — [crates.io](https://crates.io/api/v1/crates/fractal-nfs), [revdeps](https://crates.io/api/v1/crates/fractal-nfs/reverse_dependencies).
- Опубликован из `crates/fs_server/fractal-nfs`, коммит `86200e9` монорепозитория fractalbits — [.cargo_vcs_info.json](https://docs.rs/crate/fractal-nfs/0.1.0/source/.cargo_vcs_info.json). Сейчас репозиторий называется fractalbits-labs/fractalbits: «Ultra-high performance distributed storage system in the AI era», 306★, 15 forks, 1,755 коммитов — [GitHub](https://github.com/fractalbits-labs/fractalbits-main).
- Хронология по git (ключевые коммиты автора Thomas):
  - 2026-03-03 — «feat(fs_server): add native NFSv3 server support» ([242f3c1](https://github.com/fractalbits-labs/fractalbits/commit/242f3c141e7477c59066a345d9290d3da9c4d910)).
  - 2026-05-21 — «refactor(fractal-nfs): harden API and runtime for crates.io publish» ([8e12623](https://github.com/fractalbits-labs/fractalbits/commit/8e1262307f5dc6694ef27570bd0142c13f8a2071)).
  - 2026-06-02 — «extract fractal-vfs, drop fractal-nfs / NFS mode … Drops the in-tree fractal-nfs server, since we are going to use nfs-ganesha for NFS protocols.» ([b2c83f1](https://github.com/fractalbits-labs/fractalbits/commit/b2c83f13bf26e6dfe6c1c9eafc3d8a30be6fe2fd)).
  - В HEAD (`caa883b`, 2026-09-18) нет ни одного пути с «nfs» (`git ls-tree`).
- Архитектура (README):
  - Один трейт `Nfs3Filesystem`. Все методы имеют дефолты (`NotSupp`/`Noent`/`Rofs`/`Inval`), так что read-only ФС делается «в одну строку».
  - Рантайм выбирается фичей: `tokio-runtime` (по умолчанию) или `compio-runtime`.
  - По одному listener-потоку на CPU с `SO_REUSEPORT`. Каждый поток крутит свой однопоточный рантайм (`tokio current_thread + LocalSet` или compio), поэтому futures не обязаны быть `Send`.
  - MOUNT v3 работает на том же TCP-порту, portmapper не нужен. Монтирование: `mount -o port=2049,mountport=2049,vers=3,tcp,nolock` — [README](https://docs.rs/crate/fractal-nfs/0.1.0/source/README.md).
- Покрытие протокола:
  - Диспетчер обрабатывает 19 процедур NFSv3 (NULL, GETATTR, SETATTR, LOOKUP, ACCESS, READLINK, READ, WRITE, CREATE, MKDIR, REMOVE, RMDIR, RENAME, READDIR, READDIRPLUS, FSSTAT, FSINFO, PATHCONF, COMMIT). **SYMLINK, MKNOD и LINK отсутствуют** — [src/dispatch.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/dispatch.rs).
  - MOUNT: только NULL/MNT/UMNT/EXPORT. В ответе MNT объявлен только `AUTH_NONE` — [src/mount.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/mount.rs).
  - Из `AUTH_SYS` извлекаются uid/gid — [src/rpc.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/rpc.rs).
- Транспорт и лимиты:
  - Только TCP. Bind только IPv4 (`socket2::Domain::IPV4`), backlog 128.
  - Предел фрагмента и сообщения `DEFAULT_MAX_RPC_FRAGMENT_BYTES` = 16 MiB; ответ READ урезается до этого лимита — [src/config.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/config.rs).
- Соединение обслуживается строго последовательно: прочитать запись → `dispatch_rpc(...).await` → `write_all(reply)` → следующая запись. Конвейеризации запросов внутри одного TCP-соединения нет — [src/tokio_server.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/tokio_server.rs).
- Объём — около 2,9 тыс. строк Rust вместе с тестами (подсчёт `wc -l` по опубликованному крейту). Платформы: «the server is cross-platform but only Linux/macOS are exercised in CI» — [README](https://docs.rs/crate/fractal-nfs/0.1.0/source/README.md).

#### Семейство NFSWolf (StrongWind1)
- Девять крейтов, у всех 3 версии: 1.0.0 (2026-08-08), 1.1.0 (2026-08-16), 1.2.0 (2026-08-18). Apache-2.0, MSRV 1.95, edition 2024. Загрузки на 2026-10-30: `nfswolf` 70, `nfs-v2` 131, `nfs-v3` 147, `nfs-v4` 137, `nfs-mount` 196, `onc-xdr` 296, `onc-xdr-derive` 321, `onc-rpc-client` 269, `onc-rpcbind` 126 — [crates.io: nfs-v3](https://crates.io/api/v1/crates/nfs-v3), [nfswolf](https://crates.io/api/v1/crates/nfswolf), [onc-xdr](https://crates.io/api/v1/crates/onc-xdr).
- Репозиторий: 6★, 0 forks, 305 коммитов, 3 автора в git. Первый коммит 2026-07-27, последний 2026-09-12 — [GitHub](https://github.com/StrongWind1/NFSWolf), blobless-клон.
- Назначение: «Pure-Rust NFS (v2/v3/v4) security toolkit: recon, analysis, export-escape, FUSE mount, and an interactive shell for authorized red-team and pentest work». Подкоманды: `scan`, `analyze`, `escape`, `shell`, `mount` (FUSE), `brute-handle`, `uid-spray` — [README](https://docs.rs/crate/nfswolf/1.2.0/source/README.md).
- **Сервера в семействе нет, это клиенты и кодеки.**
  - `nfs-v3`: «A complete NFSv3 client split into two layers» — `wire` (XDR-типы из RFC) и `api` (доменные типы), плюс `Nfs3Client<T>` с одним методом на каждую из 22 процедур — [nfs-v3 README](https://docs.rs/crate/nfs-v3/1.2.0/source/README.md).
  - NOTICE прямо говорит: «The NFSv3 server implementation was not absorbed; NFSWolf depends on the published nfs3_server crate as a dev-dependency for integration tests» — [NOTICE](https://docs.rs/crate/nfs-v3/1.2.0/source/NOTICE). В зависимостях `nfswolf` указано `nfs3_server ^0.11 [dev]` — [deps API](https://crates.io/api/v1/crates/nfswolf/1.2.0/dependencies).
- Происхождение: wire-типы, RPC-клиент и portmap перенесены из Vaiz/nfs3 (`nfs3_types`, `nfs3_client`, Unlicense). Отличия от upstream: «The smol runtime backend was dropped; only tokio is supported»; AUTH_SYS-креденшл можно менять на живом соединении; при `PROG_MISMATCH` сохраняется диапазон версий — [onc-rpc-client NOTICE](https://docs.rs/crate/onc-rpc-client/1.2.0/source/NOTICE).
- `onc-xdr`:
  - Трейты `Pack`/`Unpack` и derive `XdrCodec` (из `onc-xdr-derive`), типы `Opaque`, `List`, `BoundedList`.
  - Декодеры «length-hardened»: `read_bytes()` и `vec_with_capacity()` резервируют не больше `PREALLOC_CAP` = 1 MiB вне зависимости от длины, заявленной на проводе (CWE-789/CWE-770) — [onc-xdr README](https://docs.rs/crate/onc-xdr/1.2.0/source/README.md).
- `onc-rpc-client`:
  - Трейт `RpcTransport` и `DirectTransport`. Поддержаны `AUTH_NONE`/`AUTH_SYS`/`AUTH_DH` (фича `auth-dh`) и replay `AUTH_SHORT`.
  - Ответ ограничен 8 MiB на фрагмент. Есть одиночные UDP-вызовы.
  - Заявлены «runtime-agnostic async I/O traits», но в зависимостях только `tokio` (features `full`) — [README](https://docs.rs/crate/onc-rpc-client/1.2.0/source/README.md), [Cargo.toml](https://docs.rs/crate/onc-rpc-client/1.2.0/source/Cargo.toml).
- `nfs-v2` реализует все 18 процедур RFC 1094 (клиент `Nfs2Client`, `MountV1Client`). `nfs-v4` — клиент NFSv4.0 (37 операций, `CompoundBuilder`, `Nfs4Session`) плюс recon-расширения v4.1/v4.2 — [nfs-v2 README](https://docs.rs/crate/nfs-v2/1.2.0/source/README.md), [nfs-v4 README](https://docs.rs/crate/nfs-v4/1.2.0/source/README.md).
- Обратные зависимости только внутри семейства (`onc-xdr` ← 7 крейтов семейства). Снаружи никто не зависит — [revdeps onc-xdr](https://crates.io/api/v1/crates/onc-xdr/reverse_dependencies), [revdeps nfswolf](https://crates.io/api/v1/crates/nfswolf/reverse_dependencies).
- Платформы бинаря `nfswolf`: Linux (static musl без `mount`; glibc с FUSE), macOS (для `mount` нужен macFUSE), Windows (без `mount`) — [README](https://docs.rs/crate/nfswolf/1.2.0/source/README.md).

#### filemux-core / filemux-cli / filemux-gui (lona-cn)
- 0.2.0 опубликован 2026-10-26 (у core 4 версии за один день). Загрузки: core 82, cli 31, gui 32. MIT OR Apache-2.0. Заявлен Rust 1.88, CI на 1.99.0 — [crates.io](https://crates.io/api/v1/crates/filemux-core), [README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md). Репозиторий: 0★, 9 коммитов (2026-10-26…2026-10-30), 1 автор — [GitHub](https://github.com/lona-cn/filemux).
- Назначение: «A cross-platform file server … for boot-file distribution and trusted-LAN sharing», CLI и egui-GUI для Windows/Linux/macOS, «without requiring a host kernel NFS server» — [README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md).
- NFS реализован собственный, без NFS-крейтов в зависимостях ([deps API](https://crates.io/api/v1/crates/filemux-core/0.2.0/dependencies)). `src/nfs/{mod,nfs,rpc,xdr}.rs` — около 1,4 тыс. строк. Свойства:
  - «Bounded userspace NFSv3 over UDP and TCP, with MOUNTv3 and portmapper v2».
  - Только чтение, анонимный экспорт.
  - Хэндлы persistent: формат-тег + persistent export identity + persistent object id (SQLite через `rusqlite`), «never a startup nonce».
  - READ ограничен 1 MiB по TCP и < 60 KiB по UDP; листинги — 10,000 записей.
  - Поддержан quirk U-Boot (лишнее нулевое слово в READ).
  - Нет NLM/statd, NFSv2/v4, RPCSEC_GSS.
  - Источники: [src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs), [src/nfs/rpc.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/rpc.rs), [README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md).
- Как библиотека NFS не переиспользуется. `pub mod nfs` есть, но `nfs::start(fs: Arc<SharedFs>, config: NfsConfig, events: EventSender)` принимает конкретный `SharedFs` (каталог на диске), а не трейт — [src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs), [src/lib.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/lib.rs).
- Платформы и клиенты:
  - Сервер на Windows требует NTFS и отдаёт виртуальные права `0555`. На Linux чтение файлов опирается на `/proc/self/fd`.
  - Клиенты: Linux `mount -t nfs -o ro,vers=3,…,nolock`, macOS `mount_nfs` с `nolocks`.
  - «Trusted networks only… NFS AUTH_SYS does not verify remote UID/GID claims» — [README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md).

#### links-fs (link-foundation/lfs) — адоптер, а не конкурент
- 0.21.1, 2026-10-22, 23 загрузки. Unlicense, MSRV 1.89, издатель `konard` — [crates.io](https://crates.io/api/v1/crates/links-fs). Репозиторий: 0★, 50 коммитов (все за 2026-10-22), 3 автора — [GitHub](https://github.com/link-foundation/lfs).
- Это драйвер ФС с драйверами FUSE (`fuser` 0.18), Dokan 2 и NFSv3 через `nfs3_server` (фича `nfs`). Встроенного NFS-стека нет — [README](https://docs.rs/crate/links-fs/0.21.1/source/README.md), [src/driver/nfs.rs](https://docs.rs/crate/links-fs/0.21.1/source/src/driver/nfs.rs).
- Обоснование выбора см. в п. 2. Ограничения, на которые указывает сам проект: «NFS version 3 has no hard links in the server library, ignores the mode of `mkdir` (directories start as `0755`) and trusts the uid a client sends» — [README](https://docs.rs/crate/links-fs/0.21.1/source/README.md).

#### nfs_mirror (W-Mai) [STALE]
- 0.1.2 вышел 2025-10-29 (3 версии за 2 дня), с тех пор обновлений нет. 368 загрузок (7 за 90 дней). Лицензия «non-standard» (`license-file`) — [crates.io](https://crates.io/api/v1/crates/nfs_mirror). LICENSE — BSD-3-Clause с копирайтами «2023, XetData» и «2025, Benign X» — [LICENSE](https://docs.rs/crate/nfs_mirror/0.1.2/source/LICENSE). На GitHub сайдбар показывает BSD-3-Clause, а README говорит MIT: это противоречие отметил WebFetch — [GitHub](https://github.com/w-mai/nfs_mirror). Репозиторий: 2★, 7 коммитов (все 2025-10-29).
- Построен на `zerofs_nfsserve ^0.15.0` (`use zerofs_nfsserve::{fs_util,nfs,tcp,vfs}`), Unix-only (`std::os::unix::ffi::OsStrExt`) — [src/main.rs](https://docs.rs/crate/nfs_mirror/0.1.2/source/src/main.rs), [deps API](https://crates.io/api/v1/crates/nfs_mirror/0.1.2/dependencies). Возможности: зеркалирование одного или нескольких каталогов, read-only, IP ACL, TOML-конфиг, режим демона — [README](https://docs.rs/crate/nfs_mirror/0.1.2/source/README.md).

#### lcpfs (artst3in / «LunaOS Contributors») [STALE]
- 2026.1.102 вышел 2026-01-03. 104 загрузки, 2 из 4 версий yanked. Apache-2.0. Указанный репозиторий `github.com/artst3in/lcpfs` отдаёт 404 — [crates.io](https://crates.io/api/v1/crates/lcpfs), [GitHub 404](https://github.com/artst3in/lcpfs).
- Заявления README: «ZFS-equivalent» CoW-ФС, около 92 тыс. строк, 1,841 тест, `no_std`. В списке функций есть «Native NFS Server» (`nfs/`, 98 тестов) — [README](https://docs.rs/crate/lcpfs/2026.1.102/source/README.md).
- Модуль `nfs/` описан как «NFSv4/v4.1 server implementation» (COMPOUND, сессии, экспорты), `net/nfs.rs` — как «NFSv4.2 Server» — [src/nfs/mod.rs](https://docs.rs/crate/lcpfs/2026.1.102/source/src/nfs/mod.rs), [src/net/nfs.rs](https://docs.rs/crate/lcpfs/2026.1.102/source/src/net/nfs.rs). Однако поиск по `src/nfs` и `src/net/nfs.rs` не находит ни `TcpListener`, ни `UdpSocket`, ни `std::net`, ни `tokio::net`, ни `xid`, ни RPC-record-marking. Есть только in-process `process_compound()` — [src/nfs/server.rs](https://docs.rs/crate/lcpfs/2026.1.102/source/src/nfs/server.rs).

#### Реализации только на GitHub (без публикации на crates.io)
- **flint (ddalton/flint)** — «Kubernetes CSI driver: replicated NVMe storage on SPDK (NVMe-oF/TCP), NFSv4.2 shared volumes, and S3-backed volumes». 0★, 3,301 коммит, MIT — [GitHub](https://github.com/ddalton/flint). По git: 2025-05-30 → 2026-10-30, 2 автора (HEAD `eb7c28f`).
  - Сервер написан на Rust: `spdk-csi-driver/src/nfs/` (58 файлов), бинари `flint-nfs-server`, `nfs_mds_main.rs`, `nfs_ds_main.rs` (pNFS MDS/DS) и `flint-nfs-proxy` («many lite hubs behind one NFSv4.1 port») — [Cargo.toml](https://github.com/ddalton/flint/blob/eb7c28fda25d5b78044ade16cafcd43911f889a8/spdk-csi-driver/Cargo.toml), [src/nfs/mod.rs](https://github.com/ddalton/flint/blob/eb7c28fda25d5b78044ade16cafcd43911f889a8/spdk-csi-driver/src/nfs/mod.rs).
  - Из шапки `mod.rs`: «Flint NFSv4.2 Server … Transport layer: TCP handling (Tokio-based async I/O) … RPC layer: Sun RPC message encoding/decoding … Lock-free: State management uses DashMap». Модули `rpcsec_gss`, `gss_ldap`, `sec_policy`, `export_access`, `pipeline` («Per-connection RPC pipelining (RFC 8881 §2.10.6)»), `v4/deleg_recall`, `v4/back_channel` — там же.
  - Зависимости: `tokio`, `io-uring = "0.7"`, `krb5 = "1.0.0"` — там же.
  - README: pNFS «Phase A (CB_LAYOUTRECALL backchannel) and Phase B (state persistence) shipped» — [GitHub](https://github.com/ddalton/flint).
- **`krb5` (crates.io, тот же автор)** — 1.0.0, 2026-10-16. 21 версия с 2026-10-13, 456 загрузок. «Pure-Rust Kerberos 5 … Built for servers authenticating Kerberos clients (e.g. NFS with `sec=krb5`/`krb5i`/`krb5p` via RPCSEC_GSS)». «The acceptor was extracted from the NFS server in flint» — [crates.io](https://crates.io/api/v1/crates/krb5), [README](https://docs.rs/crate/krb5/1.0.0/source/README.md).
- **bold-nfs (Schille/bold-nfs)** — «Bold is a v4-series NFS server written in async Rust based on Tokio». Статус в README: v4.0 «WIP», v4.1 и v4.2 «Not started». 12★, 3 forks, 24 коммита, MIT — [GitHub](https://github.com/schille/bold-nfs). Коммиты с 2024-07-23 по 2024-10-27 (**[STALE]**, 2 года). В дереве есть операции `nfs40/op_*` (access, close, commit, create, getattr, lookup, open, putfh, read, readdir, remove, renew, setclientid, setattr, write…) и демо `bold-mem` (in-memory ФС из YAML) — по `git ls-tree` HEAD `8dd591e`.
- **nfs-rs/nfs-rs** — README заявляет «Full implementation of NFSv4 specification», «Runs on Linux, macOS, and Windows» — [GitHub org](https://github.com/nfs-rs). На деле около 900 строк (`server.rs` 193, `proto/nfs4.rs` 161 …) и 7 коммитов (2025-09-24…2025-10-21); на неподдержанные операции сервер отвечает `NFS4ERR_NOTSUPP` (10004) — blobless-клон, HEAD `6eda688`. В той же организации лежат форки `nfsserve` и `bold-nfs`.
- **GitDataAI/nfs** — по индексу поиска, «lightweight, secure NFS server implementation in Rust, supporting NFSv3 protocol with TCP transport», с Docker. Страница на 2026-10-30 отдаёт 404 — [404](https://github.com/GitDataAI/nfs).
- Форки nfsserve на GitHub (`nfs-rs/nfsserve`, `darti/nfsserve`, `helyim/nfsserve`) вне скоупа: форки профилирует другой исследователь — [поиск](https://github.com/darti/nfsserve), [nfs-rs/nfsserve](https://github.com/nfs-rs/nfsserve).

#### Прочие протокольные крейты длинного хвоста (клиентские/кодеки, кратко)
- `prolink-proto` 0.5.0 (2026-10-14, 141 загрузка, GPL-3.0-only) — кодеки Pioneer Pro DJ Link, в том числе ONC RPC / NFSv2 (UDP 2049). «Pure encoding and decoding — no sockets». По 38 захватам трафика деки вызывали только LOOKUP/READ/GETATTR. CDJ-2000NXS «keeps only the leading twelve» байт 32-байтного хэндла — [crates.io](https://crates.io/api/v1/crates/prolink-proto), [src/rpc/nfs2.rs](https://docs.rs/crate/prolink-proto/0.5.0/source/src/rpc/nfs2.rs).
- `pinfish` (2023-04-01, 4,980 загрузок, MIT) — «Library for encoding and decoding NFS packets», клиенты NFSv3/NFSv4 (`nfs3/client.rs`, `nfs4/client.rs`), **[STALE]** — [crates.io](https://crates.io/api/v1/crates/pinfish).
- `nfs4` / `nfs4_client` (bobbobbio, 2023-08-11) — типы и клиент NFSv4.1 на `serde-xdr`/`sun_rpc`, **[STALE]** — [crates.io](https://crates.io/api/v1/crates/nfs4).
- Ложные срабатывания: `eudaemonfs` (подстрока «nfs» есть только в имени, NFS-кода нет) и `minigraf` (графовая БД; в CI тестирует блокировки на NFS-монтировании ядерного `nfs-kernel-server`) — [eudaemonfs src](https://docs.rs/crate/eudaemonfs/0.1.0/source/src/lib.rs), [minigraf nfs-lock.yml](https://docs.rs/crate/minigraf/2.0.11/source/.github/workflows/nfs-lock.yml).

### Inferences
- В длинном хвосте нет живой переиспользуемой NFSv3-библиотеки с подключаемым VFS, кроме самой nfs-mamont и линейки nfsserve (её профилируют другие). `fractal-nfs` формально опубликован, но мёртв: исходник удалён, а авторы объявили переход на nfs-ganesha. Решение команды, которая писала высокопроизводительный io_uring-сторедж, — показательный сигнал. Для «серьёзного» серверного NFS (вероятно, нужны NFSv4.x, pNFS, зрелость) они предпочли C-сервер своему Rust-стеку.
- `nfs-v3` (NFSWolf) годится как фундамент сервера только частично. Derive-типы `*3args`/`*3res` кодируют и декодируют в обе стороны, но нет диспетчера, record-marking-сервера, MOUNT-сервера и проверки auth, а авторы сознательно не перенесли серверную часть из nfs3. Для nfs-mamont с собственным парсером ценность семейства в другом: это готовый «атакующий» клиент для тестов безопасности (handle-oracle, UID spraying, export escape) и пример hardening-а декодера.
- Среди найденных в длинном хвосте (без учёта крейтов, которые профилируют другие исследователи) `flint` по функциональности — самый продвинутый Rust NFS-сервер на 2026-10-30 (NFSv4.2, pNFS, RPCSEC_GSS, делегации, конвейеризация). Но он не библиотека, у него 0★ и 2 автора. В конкуренцию за встраивание он может войти, только если его вынесут в крейт, как это уже сделали с `krb5`.
- Общий паттерн хвоста: проекты пишут свой NFS ради узкой задачи (PXE/U-Boot у filemux, CDJ у prolink, CSI у flint) и не делают его универсальным API.

### Gaps
- Пропускную способность `fractal-nfs` (последовательная обработка на соединение) и `filemux` не замеряли: собирать и запускать чужой код в рамках задачи запрещено.
- Почему fractalbits выбрал nfs-ganesha, сверх фразы в коммите, не выяснено. Публичных постов не нашлось.
- Контент GitDataAI/nfs недоступен (404), его состояние неизвестно. Репозиторий lcpfs тоже 404, поэтому кто его сопровождает, не установлено.
- Сколько людей реально пользуется flint (0★), неизвестно. Насколько готов его pNFS и Kerberos, видно только по коммит-сообщениям и README; работу кода не проверяли.

## 2. Кто что выбрал: downstream-продукты → NFS-крейт («доля рынка») и причины выбора

### Takeaway
Почти все найденные продукты, встраивающие Rust NFS-сервер, используют линейку `nfsserve` напрямую или её потомков: `nfs3_server`, `zerofs_nfsserve`, `nessie-nfsserve`, вендоренную копию в sandfs. У `nfsserve` на crates.io 14 прямых зависимых. На GitHub — 51 репозиторий и 15 пакетов, включая trycua/cua (31k★), ontola/atomic-server (1.6k★) и hf-mount (820★). `nfs3_server` занимает №2: 6 зависимых на crates.io, 13 репозиториев на GitHub, в том числе ArcBox (4.3k★), conda/rattler (463★) и Lockbook (444★). У embednfs (NFSv4.1) один продакшен-зависимый (собственный демон). У nfs-mamont и fractal-nfs — ноль.

### Cited Findings

#### Прямые зависимые на crates.io (reverse_dependencies, 2026-10-30)
- `nfsserve` — 14 зависимых (`heddle-mount`, `aegis-orchestrator-core`, `foundation_nativeapis`, `braid_rs`, `internxt-cli`, `mq-mount`, `ossuary-mount`, `ghfs`, `braid_http_rs`, `pando-vcs`, `netget-tool`, `braid-core`, `tapfs`, `leyline-fs`) — [API](https://crates.io/api/v1/crates/nfsserve/reverse_dependencies).
- `nfs3_server` — 6 (`lb-fs`, `cargo-nfs3-server`, `arcbox-agent`, `rattler_vfs`, `nfswolf` [dev], `links-fs`). `nfs3_types` — 4 (`nfs3_server`, `nfs3_client`, `rattler_vfs`, `netget-tool`) — [API nfs3_server](https://crates.io/api/v1/crates/nfs3_server/reverse_dependencies), [API nfs3_types](https://crates.io/api/v1/crates/nfs3_types/reverse_dependencies).
- `zerofs_nfsserve` — 2 (`zerofs`, `nfs_mirror`); `embednfs` — 2 (`embednfsd`, `nfs-crust` [dev]); `nessie-nfsserve` — 1 (`nessie-nfs`); `filemux-core` — 2 (собственные `-cli`/`-gui`) — [API zerofs_nfsserve](https://crates.io/api/v1/crates/zerofs_nfsserve/reverse_dependencies), [API embednfs](https://crates.io/api/v1/crates/embednfs/reverse_dependencies), [API nessie-nfsserve](https://crates.io/api/v1/crates/nessie-nfsserve/reverse_dependencies), [API filemux-core](https://crates.io/api/v1/crates/filemux-core/reverse_dependencies).
- **0 зависимых:** `nfs-mamont`, `fractal-nfs`, `fernfs`, `nfsembed`, `nfswolf` — [API nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont/reverse_dependencies), [API fernfs](https://crates.io/api/v1/crates/fernfs/reverse_dependencies), [API nfsembed](https://crates.io/api/v1/crates/nfsembed/reverse_dependencies).
- Контекст по загрузкам библиотек (профили у других исследователей): `nfsserve` 0.11.0 — 1,415,987 (90 дней: 1,132,218); `zerofs_nfsserve` — 53,245; `nfs3_server` 0.11.1 — 43,752 (33,415); `embednfs` — 2,434; `nessie-nfsserve` — 760; `fernfs` — 645; `nfsembed` — 91; `nfs-mamont` 0.2.0 — 80 — [crates.io search](https://crates.io/api/v1/crates?q=nfs&per_page=100).

#### Зависимые на GitHub (страницы network/dependents, 2026-10-30)
- `huggingface/nfsserve`: «51 Repositories», «15 Packages» (GitHub пишет, что числа приблизительные). На двух страницах видно 38 репозиториев. Числа у строк не подписаны, но для arcbox/rattler/lockbook они совпали со звёздами на страницах репозиториев, значит это ★/forks.
  - Крупнейшие: trycua/cua 31,357★; ontola/atomic-server 1,631★; huggingface/hf-mount 820★; supermemoryai/smfs 491★; xetdata/xet-core 93★; OpenCoven/coven 75★; xetdata/pyxet 62★; bnsd55/openscion 30★; agucova/oxcrypt 16★; benbrittain/jj-yak 11★; onexay/msl 9★; superradcompany/monofs 9★.
  - Мелкие и нулевые: luckydye/vektor 5, ley-line-open 4, superradcompany/virtualfs 4, xetdata/docker-volume-xetfs 3, rrauch/janus-fs 2, bazhenov/pmem 2, gulrotkake/nfssh 1, kabirnarang39/pando 1, aegis-orchestrator 1, ewe_platform 1; с нулём — rill-vcs/rill, TMA-Cloud/TMA, danplischke/origofs, mussolene/rdrkit, nnunley/p9rs, wizz-cmd/icloud-nfs-exporter, AmateurECE/instant-netboot, hilaolu/nfs, AsPulse/mofu, darti/opendal-mount, johnmerchant/neochumby, CaffeeLake/cua и несколько форков hf-mount.
  - Источники: [стр. 1](https://github.com/huggingface/nfsserve/network/dependents), [стр. 2](https://github.com/huggingface/nfsserve/network/dependents?dependents_after=NDA0Mzg0NTQ2NjU).
- `Vaiz/nfs3`, пакет `nfs3_server`: «13 Repositories», «6 Packages». Видны metavert-io/hearthfs 0, link-foundation/lfs 0, conda/rattler 463, arcboxlabs/arcbox 4,256, fork-archive-hub/crab 0, StrongWind1/NFSWolf 6, lockbook/lockbook 444, Vaiz/nfs3 25 — [GitHub](https://github.com/Vaiz/nfs3/network/dependents?package_id=UGFja2FnZS01NzY5Mzg1OTE2).

#### Таблица: продукт → NFS-крейт → версия → дата → популярность продукта

Колонки «Дата» и «Загрузки» относятся к релизу продукта на crates.io. Для GitHub-only продуктов указан последний коммит. ★ — на 2026-10-30.

| Продукт (назначение) | NFS-крейт (как подключён) | Требование | Дата | Загрузки продукта (всего/90 д.) | ★ | Источник |
|---|---|---|---|---|---|---|
| trycua/cua → crate `cua-volume` (диск агентов, S3) | `nfsserve` (фича `nfs`, macOS) | "0.11" | 2026-10-30 (коммит) | не на crates.io (`publish = false`) | 31,357 | [Cargo.toml](https://github.com/trycua/cua/blob/a3176d95ec0fa41bcf7f44ff7c1432265aee66f1/libs/cua/crates/cua-volume/Cargo.toml) |
| ArcBox → `arcbox-agent` (ВМ для AI-агентов, macOS) | `nfs3_server` (rootfs машины → хост) | ^0.11.0 | 0.10.5, 2026-10-22 | 545 / 298 | 4.3k | [deps](https://crates.io/api/v1/crates/arcbox-agent/0.10.5/dependencies), [GitHub](https://github.com/arcboxlabs/arcbox) |
| ontola/atomic-server, desktop «Virtual Drive» | `nfsserve` (unix-only) | "0.10" | 2026-10-30 (коммит) | — | 1,631 | [desktop/Cargo.toml](https://github.com/ontola/atomic-server/blob/54dc025958dd370c1730dc240854d9614ecf1b60/desktop/Cargo.toml) |
| huggingface/hf-mount | `nfsserve` (HF) | — | — | — | 820 | [dependents](https://github.com/huggingface/nfsserve/network/dependents) (профиль у др. исследователя) |
| supermemoryai/smfs (память агента как ФС) | `nfsserve` (macOS; `fuser` на Linux) | "0.11" | 2026-06-12 (коммит) | — | 491 | [Cargo.toml](https://github.com/supermemoryai/smfs/blob/f4a0f5034431638d66f8c350408f34bb5b8cbce5/Cargo.toml) |
| conda/rattler → `rattler_vfs` (conda-окружения как ФС) | `nfs3_server` + `nfs3_types` (opt.) | ^0.10 / ^0.5 | 0.1.4, 2026-10-30 | 166 / 166 | 463 | [deps](https://crates.io/api/v1/crates/rattler_vfs/0.1.4/dependencies) |
| lockbook → `lb-fs` (виртуальная ФС заметок) | `nfs3_server` | ^0.8 (текущая 0.11.1) | 26.10.30, 2026-10-30 | 3,595 / 280 | 444 | [deps](https://crates.io/api/v1/crates/lb-fs/26.10.30/dependencies), [GitHub](https://github.com/lockbook/lockbook) |
| xetdata/xet-core (`git xet mount`) **[STALE]** | `nfsserve` | "0.10" | 2024-10-16 (коммит) | — | 93 | [Cargo.toml](https://github.com/xetdata/xet-core/blob/e45aeb2f8c110438f6017be79c0b4c854bdbd2d0/rust/gitxetcore/Cargo.toml) |
| OpenCoven/coven → `coven-afs` (ФС агента, AgentFS SPEC v0.4) | `nfsserve` (фича `mount`) | "0.11" | 2026-10-30 (коммит) | — | 75 | [Cargo.toml](https://github.com/OpenCoven/coven/blob/aa7581e37099471cd7bc0b8c1ef21110ed5d07ca/crates/coven-afs/Cargo.toml) |
| bnsd55/openscion → `scn` (VCS, read-only NFS mount) | `nfsserve` | "0.11" | 2026-10-30 (коммит) | — | 30 | [Cargo.toml](https://github.com/bnsd55/openscion/blob/479a661d354c7d6664f52fc0c2023c4962162f98/Cargo.toml) |
| HeddleCo/heddle → `heddle-mount` (AI-native VCS) | `nfsserve` (opt. фича `nfs`, fallback) | ^0.11 | 0.31.3, 2026-10-15 | 1,335 / 955 | 17 | [deps](https://crates.io/api/v1/crates/heddle-mount/0.31.3/dependencies), [GitHub](https://github.com/HeddleCo/heddle) |
| agucova/oxcrypt → `oxcrypt-nfs` (Cryptomator-хранилища) | `nfsserve` | "0.10" | 2026-08-23 (коммит) | — | 16 | [Cargo.toml](https://github.com/agucova/oxcrypt/blob/916864f66184dae6f97b0e97e02b879144fcfed6/crates/oxcrypt-nfs/Cargo.toml) |
| benbrittain/jj-yak (виртуальная копия jj) **[STALE]** | `nfsserve` | "0.10" | 2024-12-18 (коммит) | — | 11 | [Cargo.toml](https://github.com/benbrittain/jj-yak/blob/16a36d79010233c561c96805c47e214d497b2adf/Cargo.toml) |
| lydakis/sandfs (CoW-воркспейсы для агентов) | **вендоренная копия** `nfsserve` 0.11.0 в `src/nfsserve` | — | 0.9.0, 2026-10-28 | 887 / 887 | 9 | [src/nfsserve/mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs), [GitHub](https://github.com/lydakis/sandfs) |
| superradcompany/monofs **[STALE]** | `nfsserve` | "0.10" | 2025-04-11 (коммит) | — | 9 | [Cargo.toml](https://github.com/superradcompany/monofs/blob/210aef7a5bc5ff90e2a95f131dc78e854b6cbf17/Cargo.toml) |
| Bebbssos/internxt-cli-rust → `internxt-cli` (`serve nfs`, experimental) | `nfsserve` (opt.) | ^0.11 | 0.3.1, 2026-08-24 | 108 / 63 | 8 | [deps](https://crates.io/api/v1/crates/internxt-cli/0.3.1/dependencies), [GitHub](https://github.com/Bebbssos/internxt-cli-rust) |
| harehare/mq-mount (Markdown как ФС) | `nfsserve` (unix; WinFSP на Windows) | ^0.11.0 | 0.1.7, 2026-10-30 | 105 / 105 | 7 | [deps](https://crates.io/api/v1/crates/mq-mount/0.1.7/dependencies), [GitHub](https://github.com/harehare/mq-mount) |
| Vaiz/nfs3 → `cargo-nfs3-server` (эталонный бинарь авторов) | `nfs3_server` | ^0.11.1 | 0.2.3, 2026-10-21 | 1,648 / 49 | 25 | [deps](https://crates.io/api/v1/crates/cargo-nfs3-server/0.2.3/dependencies) |
| StrongWind1/NFSWolf → `nfswolf` (тест-цель) | `nfs3_server` [dev] | ^0.11 | 1.2.0, 2026-08-18 | 70 / 70 | 6 | [deps](https://crates.io/api/v1/crates/nfswolf/1.2.0/dependencies) |
| rgodha24/ghfs (GitHub-репо как ФС для агентов) | `nfsserve` (только macOS) | ^0.10 | 0.0.3, 2026-03-02 | 66 / 10 | 4 | [deps](https://crates.io/api/v1/crates/ghfs/0.0.3/dependencies), [GitHub](https://github.com/rgodha24/ghfs) |
| netget-net/netget → `netget-tool` (LLM-заглушки протоколов) | `nfsserve` (+`nfs3_client`/`nfs3_types` для клиента) | ^0.10 | 0.3.5, 2026-10-16 | 38 / 38 | 4 | [deps](https://crates.io/api/v1/crates/netget-tool/0.3.5/dependencies), [GitHub](https://github.com/netget-net/netget) |
| agentic-research/ley-line-open → `leyline-fs` | `nfsserve` (opt., default на macOS) | ^0.10 | 0.14.0, 2026-07-31 | 17 / 6 | 4 | [deps](https://crates.io/api/v1/crates/leyline-fs/0.14.0/dependencies), [GitHub](https://github.com/agentic-research/ley-line-open) |
| W-Mai/nfs_mirror **[STALE]** | `zerofs_nfsserve` | ^0.15.0 | 0.1.2, 2025-10-29 | 368 / 7 | 2 | [deps](https://crates.io/api/v1/crates/nfs_mirror/0.1.2/dependencies) |
| kabirnarang39/pando → `pando-vcs` | `nfsserve` (unix) | ^0.11 | 3.5.1, 2026-10-19 | 45 / 45 | 1 | [deps](https://crates.io/api/v1/crates/pando-vcs/3.5.1/dependencies), [GitHub](https://github.com/kabirnarang39/pando) |
| 100monkeys-ai/aegis-orchestrator → `aegis-orchestrator-core` | `nfsserve` (NFS Server Gateway, ADR-036) | ^0.10 | 1.2.0-beta, 2026-10-23 | 840 / 230 | 1 | [deps](https://crates.io/api/v1/crates/aegis-orchestrator-core/1.2.0-beta/dependencies), [GitHub](https://github.com/100monkeys-ai/aegis-orchestrator) |
| ewe-studios/ewe_platform → `foundation_nativeapis` | `nfsserve` (opt. `vfs-nfs`) | ^0.11 | 0.0.3, 2026-10-20 | 531 / 439 | 1 | [deps](https://crates.io/api/v1/crates/foundation_nativeapis/0.0.3/dependencies) |
| sniner/ossuary → `ossuary-mount` (архив, read-only) | `nfsserve` (только macOS) | ^0.11.0 | 0.14.0, 2026-10-28 | 87 / 87 | 0 | [deps](https://crates.io/api/v1/crates/ossuary-mount/0.14.0/dependencies), [GitHub](https://github.com/sniner/ossuary) |
| Gilamonster-Foundation/nessie-store | `nessie-nfs` → `nessie-nfsserve` (форк nfsserve) | ^0.3.1 | 0.3.1, 2026-06-26 | 74 (nessie-nfs 111) | 0 | [deps](https://crates.io/api/v1/crates/nessie-store/0.3.1/dependencies), [GitHub](https://github.com/Gilamonster-Foundation/nessie-store) |
| link-foundation/lfs → `links-fs` | `nfs3_server` (opt. `nfs`) | ^0.11.1 | 0.21.1, 2026-10-22 | 23 / 23 | 0 | [deps](https://crates.io/api/v1/crates/links-fs/0.21.1/dependencies) |
| lona-cn/filemux | собственный NFSv3 (UDP+TCP, read-only) | — | 0.2.0, 2026-10-26 | 82 / 82 | 0 | п. 1 |
| braid-org/braid-rs → `braid_rs`/`braid_http_rs`/`braid-core` (BraidFS) | `nfsserve` | ^0.10 | 2026-01-27 / 2026-02-04 | 119 / 50 / 33 | репо 404 | [deps](https://crates.io/api/v1/crates/braid-core/0.1.4/dependencies) |
| tapfs/tap → `tapfs` (REST API как ФС для агентов) | `nfsserve` (opt., macOS) | ^0.11 | 0.1.0, 2026-04-25 | 26 / 2 | репо 404 | [deps](https://crates.io/api/v1/crates/tapfs/0.1.0/dependencies) |
| ZeroFS → `zerofs` | `zerofs_nfsserve` | ^0.17.1 | 0.19.1, 2025-12-02 (на crates.io) | 3,854 / 9 | — | [deps](https://crates.io/api/v1/crates/zerofs/0.19.1/dependencies) (профиль у др. исследователя) |
| `embednfsd` | `embednfs` | ^0.4.1 | 0.4.1, 2026-03-10 | 240 / 36 | — | [revdeps](https://crates.io/api/v1/crates/embednfs/reverse_dependencies) |
| `nfs-crust` (NFSv4.1-клиент) | `embednfs` [dev, тестовый сервер] | ^0.4.1 | 0.1.0, 2026-07-12 | 26 | — | там же |
| `lcpfs` | своё (без транспорта) | — | 2026-01-03 | 104 / 12 | 404 | п. 1 |
| `minigraf` (не адоптер) | нет; в CI ядерный nfsd для тестов блокировок | — | 2.0.11, 2026-10-26 | 2,861 / 2,110 | — | [nfs-lock.yml](https://docs.rs/crate/minigraf/2.0.11/source/.github/workflows/nfs-lock.yml) |

Ещё зависимые на GitHub с 0–9★ (не профилировались): onexay/msl, luckydye/vektor, superradcompany/virtualfs, rrauch/janus-fs, bazhenov/pmem, gulrotkake/nfssh, wizz-cmd/icloud-nfs-exporter, AmateurECE/instant-netboot, metavert-io/hearthfs (`nfs3_server`) и др. — [dependents nfsserve](https://github.com/huggingface/nfsserve/network/dependents), [dependents nfs3_server](https://github.com/Vaiz/nfs3/network/dependents?package_id=UGFja2FnZS01NzY5Mzg1OTE2).

#### Заявленные причины выбора конкретной библиотеки
- **links-fs → `nfs3_server`.** Явное сравнение в кейс-стади: «macOS mount without a kernel extension | macFUSE, FUSE-T, FSKit, NFS (`nfsserve` 0.11, `nfs3_server` 0.11) | `nfs3_server` 0.11 (BSD-3-Clause) on `tokio` | macOS mounts NFS version 3 with the built-in `mount_nfs`, without root or an extension; `nfs3_server` is a maintained fork of `nfsserve`» — [docs/case-studies/issue-1/README.md](https://docs.rs/crate/links-fs/0.21.1/source/docs/case-studies/issue-1/README.md).
- **sandfs → копия nfsserve.**
  - «An NFSv3 server: nfsserve 0.11.0 … copied in so it can be changed here first». Причина копирования: `transaction_tracker` «took most of the server's time under load: a cold Rust build on the mount took 65 s with it and 14 s without» — [src/nfsserve/mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
  - Коммит «Copy the NFS server into sandfs to fix its slowness, and hide macOS's ._name files (#119)», 2026-10-21 — [commit 4dc07b5](https://github.com/lydakis/sandfs/commit/4dc07b5d61323e0047e1b4aaaf38aa181d24079b).
- **nessie-store → свой форк `nessie-nfsserve`.** README: «Vendored, hardened fork of the `nfsserve` userspace NFSv3 server» ради «durable writes + NFSPROC3_COMMIT + AUTH_UNIX ownership fixes upstream can't express». Встроенный сервер — это «Embedded NFSv3 data-plane server (PassthroughFs)», без NFS ядра хоста — [GitHub](https://github.com/Gilamonster-Foundation/nessie-store).
- **ArcBox → `nfs3_server` в процессе вместо ядерного nfsd** (для дистрибутивных машин). Цитата: «the machine boots a stock distro image whose kernel nfsd would claim port 2049 and `/etc/exports` from a user's own NFS server, and the shim's EROFS — the only place `rpc.mountd` ships — is gone by the time the agent serves. A userspace server on an ephemeral port owns nothing the distro might want» — [src/machine_export/mod.rs](https://docs.rs/crate/arcbox-agent/0.10.5/source/src/machine_export/mod.rs). Для docker-данных собственной ВМ ArcBox берёт **ядерный nfsd (NFSv4)** через vsock-релей: «Configures the in-kernel Linux nfsd to export `/var/lib/docker` read-only» — [src/nfs.rs](https://docs.rs/crate/arcbox-agent/0.10.5/source/src/nfs.rs), [GitHub](https://github.com/arcboxlabs/arcbox). Почему `nfs3_server`, а не `nfsserve`, не сказано.
- **smfs → nfsserve.** «fuser is Linux-only: on macOS it requires macFUSE which we explicitly avoid. nfsserve is unix-wide: pure Rust, used as the NFSv3 server on macOS» — [Cargo.toml](https://github.com/supermemoryai/smfs/blob/f4a0f5034431638d66f8c350408f34bb5b8cbce5/Cargo.toml).
- **cua → nfsserve.** «The localhost NFSv3 server macOS mounts natively (no kernel or system extension), through nfsserve (BSD-3-Clause)» — [cua-volume/Cargo.toml](https://github.com/trycua/cua/blob/a3176d95ec0fa41bcf7f44ff7c1432265aee66f1/libs/cua/crates/cua-volume/Cargo.toml).
- **pando → NFS по прецеденту EdenFS.** «On macOS this needs no kernel extension and nothing to install, which is why EdenFS moved to NFS there as Apple deprecated kernel extensions» — [src/mount/nfs.rs](https://docs.rs/crate/pando-vcs/3.5.1/source/src/mount/nfs.rs).
- **heddle → NFS как универсальный fallback.** «The CLI's mount lifecycle tries the host's native adapter first and falls back to NfsShell … Linux without the FUSE kernel module … macOS without a code-signed `.fsmodule` System Extension (the common case) … Windows without the "Projected File System"» — [src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs). При этом про ProjFS сказано, что «user-visible performance is much better with ProjFS» — [README](https://docs.rs/crate/heddle-mount/0.31.3/source/README.md).
- **aegis → nfsserve как «transport layer»** для «NFS Server Gateway (ADR-036)». Контейнеры агентов монтируют `/{tenant_id}/{volume_id}` и при этом «require zero elevated privileges (no CAP_SYS_ADMIN)» — [server.rs](https://docs.rs/crate/aegis-orchestrator-core/1.2.0-beta/source/src/infrastructure/nfs/server.rs). (Исходник смотрели локально; путь соответствует опубликованному крейту.)
- **leyline-fs.** «userspace NFSv3 via `nfsserve` (default on macOS). The kernel's native NFS client handles page cache and readahead» — [README](https://docs.rs/crate/leyline-fs/0.14.0/source/README.md).
- **netget → nfsserve**, но через собственный «guard». «`nfsserve` owns RPC, XDR and the MOUNT protocol», однако «NetGet owns the listening socket, not `nfsserve`» из-за DoS (см. п. 3) — [src/server/nfs/mod.rs](https://docs.rs/crate/netget-tool/0.3.5/source/src/server/nfs/mod.rs).
- **fractalbits → отказ от собственного `fractal-nfs` в пользу nfs-ganesha** — [commit b2c83f1](https://github.com/fractalbits-labs/fractalbits/commit/b2c83f13bf26e6dfe6c1c9eafc3d8a30be6fe2fd).
- Причину для `nfsserve` назвал её автор: FUSE «annoying» на Mac и Windows из-за драйверов. Используется в pyxet/xet-core — [nfsserve README](https://docs.rs/crate/nfsserve/latest/source/README.md) (по результатам поиска; профиль nfsserve у др. исследователя).

### Inferences
- Считаем различные продукты на crates.io. braid (3 крейта) идёт за один продукт. Не учитываются dev-зависимости (`nfswolf`, `nfs-crust`) и `cargo-nfs3-server` — эталонный бинарь самого Vaiz/nfs3. Получается около 22 продуктов. Из них около 20 стоят на линейке nfsserve: `nfsserve` 12, `nfs3_server` 4 (lb-fs, arcbox-agent, rattler_vfs, links-fs), `zerofs_nfsserve` 2, `nessie-nfsserve` 1, вендоренная копия (sandfs) 1. На `embednfs` один продукт, `embednfsd`, и это демон того же автора. Своя реализация — одна (filemux). Это оценка, а не точная метрика: reverse-deps на crates.io видят только default-версии.
- С учётом звёзд «тяжёлые» продукты распределились так: nfsserve — cua (31k★), atomic-server (1.6k), hf-mount (820), smfs (491); nfs3_server — ArcBox (4.3k), rattler (463), lockbook (444). nfs3_server выигрывает там, где важна «поддерживаемость форка» (links-fs говорит об этом явно). nfsserve выигрывает по инерции и узнаваемости бренда HF.
- Большие продукты регулярно чинят библиотеку у себя, а не апстримят: копия в sandfs, форк nessie, форк zerofs, relay-guard в netget. Это признак слабой отзывчивости апстрима и прямой спрос на «hardened» NFSv3-библиотеку с хуками.
- У nfs-mamont ноль зависимых. Главная структурная причина видна из таблицы: по меньшей мере 15 из примерно 30 продуктов прямо называют macOS основной платформой или причиной NFS-бэкенда. Это sandfs, cua, smfs, ossuary, leyline, mq-mount, tapfs, ghfs, pando, heddle, rattler_vfs, links-fs, oxcrypt, nfs_mirror, xet-core. В этих сценариях сервер работает **на самом Mac** (localhost), а nfs-mamont декларирует Linux-only. Есть и сегмент, где сервер живёт в Linux-ВМ или контейнере, а клиент — Mac или Linux-хост: ArcBox-машины, aegis, nessie, ZeroFS, flint. Linux-only библиотека туда формально подходит, но конкурирует с ядерным nfsd и nfs-ganesha.
- Почти все адоптеры — проекты 2026 года из ниши AI-агентов, VCS и sandbox. Кроме cua и ArcBox, у них малые аудитории (0–30★). Рынок новый, привязка к библиотеке слабая (зависимость часто opt-in фича), поэтому переключение на другую NFS-библиотеку для них дёшево.

### Gaps
- На страницах dependents видны только 38 из «51» репозиториев nfsserve и 8 из «13» у nfs3_server; остальные скрыты (приватные или неиндексированные).
- Для GitHub-only адоптеров (cua, atomic-server, smfs и др.) загрузок нет: они не на crates.io. Звёзды не отражают долю использования NFS-фичи внутри продукта.
- Почему ArcBox, rattler и lockbook выбрали `nfs3_server`, а не `nfsserve`, они не объясняют. Issues и блоги этих проектов не изучались.
- Звёзды/статус braid-org/braid-rs и tapfs/tap не получены (на 2026-10-30 оба отдают 404).

## 3. Паттерны: зачем встраивают user-space NFS-сервер, какие функции нужны, какие жалобы повторяются

### Takeaway
Главный драйвер — смонтировать виртуальную ФС на **macOS без kext/macFUSE**: localhost NFSv3-сервер плюс встроенный `mount_nfs`. За ним идут sandbox-ы и воркспейсы для AI-агентов, виртуальные чекауты VCS, обмен файлами ВМ ↔ хост и тома контейнеров без привилегий. Повторяющиеся требования: совместимость с macOS-клиентом, symlink/hard link, корректные READDIR-cookie, «AppleDouble»-файлы `._*` вместо xattr, долговечность при COMMIT и обязательная защита localhost-порта. Повторяющиеся жалобы на nfsserve-линейку: производительность, отсутствие LINK, неограниченные аллокации (DoS), отсутствие хуков, не собирается под Windows. На подходе долгосрочный конкурент всего паттерна — Apple FSKit (macOS 15.4+/26).

### Cited Findings

#### Мотивы встраивания
- **macOS без расширения ядра.**
  - sandfs: «macOS with an NFS server on localhost that macOS's own client mounts, with no kernel extension» — [README](https://docs.rs/crate/sandfs/0.9.0/source/README.md).
  - ossuary: «Neither platform needs root, and neither installs a kernel extension or module» — [README](https://docs.rs/crate/ossuary-mount/0.14.0/source/README.md).
  - mq-mount: «macOS: no system packages required; mounting uses the OS's built-in NFS client» — [README](https://docs.rs/crate/mq-mount/0.1.7/source/README.md).
  - Аналогично cua, smfs, ghfs, tapfs, links-fs — см. п. 2. У rattler_vfs в таблице платформ: «| Linux | FUSE | FUSE, NFS |», «| macOS | NFS | NFS, FUSE (requires macFUSE) |», «| Windows | ProjFS | `ProjFS` |» — [rattler_vfs src/lib.rs](https://docs.rs/crate/rattler_vfs/0.1.4/source/src/lib.rs).
  - nfs_mirror даёт рецепт клиента macOS: `sudo mount -t nfs -o resvport,nolocks,vers=3,tcp,port=11451,mountport=11451 127.0.0.1:/ /mnt/nfs` — [README](https://docs.rs/crate/nfs_mirror/0.1.2/source/README.md).
- **Прецедент EdenFS (Meta).** «On macOS EdenFS uses NFS … Apple deprecated third party kernel extensions … As of Ventura, third party kernel extensions require turning off SIP … So we force migrated everyone to NFS on Ventura» — [sapling/eden/fs/docs/macOS.md](https://github.com/facebook/sapling/blob/main/eden/fs/docs/macOS.md).
- **FUSE-T** (1.6k★) решает ту же задачу сервером NFS v4 в userspace: «FUSE-T is a kext-less implementation of FUSE for macOS that uses NFS v4 local server instead of a kernel extension». Позже добавлены «SMB and FSKit backends … as NFS alternatives» — [GitHub](https://github.com/macos-fuse-t/fuse-t).
- **AI-агенты и sandbox-ы** (по описаниям продуктов):
  - sandfs — «A copy-on-write sandbox filesystem for agents» ([GitHub](https://github.com/lydakis/sandfs));
  - cua-volume — «one versioned, S3-compatible drive per account shared by every Space and agent» ([Cargo.toml](https://github.com/trycua/cua/blob/a3176d95ec0fa41bcf7f44ff7c1432265aee66f1/libs/cua/crates/cua-volume/Cargo.toml));
  - coven-afs — «AgentFS SPEC v0.4-compatible» ([Cargo.toml](https://github.com/OpenCoven/coven/blob/aa7581e37099471cd7bc0b8c1ef21110ed5d07ca/crates/coven-afs/Cargo.toml));
  - aegis — «runs AI agents in isolated, policy-enforced runtimes» ([GitHub](https://github.com/100monkeys-ai/aegis-orchestrator));
  - nessie — «Distributed Content Addressable Storage for Agentic Harnesses» ([GitHub](https://github.com/Gilamonster-Foundation/nessie-store));
  - mq-mount — «Token-efficient reading for LLM agents» ([README](https://docs.rs/crate/mq-mount/0.1.7/source/README.md));
  - ArcBox — «Run AI agents on real and isolated machines» ([GitHub](https://github.com/arcboxlabs/arcbox)).
- **VCS и виртуальные чекауты:**
  - heddle — «An AI-native version control system» ([crates.io](https://crates.io/api/v1/crates/heddle-mount));
  - pando — «source control for monorepos where hundreds of AI agents commit at once» ([GitHub](https://github.com/kabirnarang39/pando));
  - jj-yak — «It implements an NFS server and manages the local mounting of repos» ([README](https://github.com/benbrittain/jj-yak/blob/16a36d79010233c561c96805c47e214d497b2adf/README.md));
  - openscion — «mount (read-only NFS)» ([README](https://github.com/bnsd55/openscion/blob/479a661d354c7d6664f52fc0c2023c4962162f98/README.md)).
- **ВМ ↔ хост.** ArcBox показывает rootfs машины в `~/ArcBox/machines/<name>` «the way OrbStack shows a machine» — [machine_export/mod.rs](https://docs.rs/crate/arcbox-agent/0.10.5/source/src/machine_export/mod.rs).
- **Контейнеры без привилегий.** У aegis Docker-тома монтируются как `addr=…,nfsvers=3,proto=tcp,soft,timeo=10,nolock` — [nfs/server.rs](https://docs.rs/crate/aegis-orchestrator-core/1.2.0-beta/source/src/infrastructure/nfs/server.rs). Но для rootless Podman выбраны host-local FUSE-монтирования: «in rootless container runtimes (Podman) where NFS volume drivers are unsupported» — [fuse/mod.rs](https://docs.rs/crate/aegis-orchestrator-core/1.2.0-beta/source/src/infrastructure/fuse/mod.rs).
- **Загрузочные и LAN-файлы.** filemux: «boot-file distribution and trusted-LAN sharing», quirk U-Boot — [README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md).
- **Тест-цели и заглушки:**
  - NFSWolf использует `nfs3_server` для интеграционных тестов ([NOTICE](https://docs.rs/crate/nfs-v3/1.2.0/source/NOTICE));
  - nfs-crust — `embednfs` ([revdeps](https://crates.io/api/v1/crates/embednfs/reverse_dependencies));
  - netget отдаёт ФС «entirely by the LLM» ([src/server/nfs/mod.rs](https://docs.rs/crate/netget-tool/0.3.5/source/src/server/nfs/mod.rs)).

#### Что нужно чаще всего
- **Опции монтирования macOS и без portmapper.**
  - heddle: `vers=3,tcp,port={port},mountport={port},nolocks,soft,intr,actimeo=0,resvport=off` — «keeps the kernel from waiting on a remote lock manager (we don't run rpc.lockd), forbids reserved-port binding» — [src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs).
  - filemux и fractal-nfs: `port=…,mountport=…` без rpcbind — [filemux README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md), [fractal-nfs README](https://docs.rs/crate/fractal-nfs/0.1.0/source/README.md).
- **Symlink, hard link, cookie.**
  - sandfs добавил LINK («which upstream left unimplemented») и собственные cookie для записей READDIR («instead of their file ids, which two names of one file share») — [src/nfsserve/mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
  - links-fs: «no hard links in the server library» — [README](https://docs.rs/crate/links-fs/0.21.1/source/README.md).
- **xattr на macOS.**
  - sandfs: «macOS's NFS client keeps extended attributes in `._name` files … git, cargo and the like never see them» — [src/nfs.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfs.rs).
  - EdenFS: «NFSv3 has no support for extended attributes» — [macOS.md](https://github.com/facebook/sapling/blob/main/eden/fs/docs/macOS.md).
  - FUSE-T выбирает «NFSv4 for better support of extended attributes and file locks» — [GitHub](https://github.com/macos-fuse-t/fuse-t).
- **Долговечность и COMMIT.**
  - sandfs: «`fsync` never reaches an NFS server (writes are answered as already stable)», поэтому нужен отдельный порт «save requests» — [src/nfs.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfs.rs).
  - links-fs: применение на `COMMIT` и stable-write — [src/driver/nfs.rs](https://docs.rs/crate/links-fs/0.21.1/source/src/driver/nfs.rs).
  - nessie: «durable writes + NFSPROC3_COMMIT» — [GitHub](https://github.com/Gilamonster-Foundation/nessie-store).
- **Безопасность localhost.**
  - sandfs: «Any local user can connect to a port on localhost, so the server only answers its owner», плюс секретный путь экспорта и секрет в хэндлах — [src/nfs.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfs.rs).
  - coven: «Loopback NFS has no authentication… Bind only 127.0.0.1»; хэндлы аутентифицируются «Truncated HMAC-SHA256 over the file id» — [coven-afs/src/nfs.rs](https://github.com/OpenCoven/coven/blob/aa7581e37099471cd7bc0b8c1ef21110ed5d07ca/crates/coven-afs/src/nfs.rs).
  - heddle планирует перейти на «UNIX-domain socket (nfsserve supports it)» — [src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs).
  - ArcBox пускает только адреса хоста — [machine_export/mod.rs](https://docs.rs/crate/arcbox-agent/0.10.5/source/src/machine_export/mod.rs).
- **Стабильные fileid и хэндлы между рестартами.**
  - atomic-server: «NFS needs small `u64` file ids that are stable across restarts» — [desktop/src/vfs.rs](https://github.com/ontola/atomic-server/blob/54dc025958dd370c1730dc240854d9614ecf1b60/desktop/src/vfs.rs).
  - filemux: persistent handles — [src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs).
- **Маппинг владельцев.**
  - ArcBox `attr::IdMap` — [machine_export/mod.rs](https://docs.rs/crate/arcbox-agent/0.10.5/source/src/machine_export/mod.rs).
  - nessie «AUTH_UNIX ownership fixes» — [GitHub](https://github.com/Gilamonster-Foundation/nessie-store).
- **Блокировки** почти всегда выключены (`nolock`/`nolocks`). minigraf фиксирует, что при NFSv3 `-o nolock` «the Linux NFS client serves locks out of its own local, in-kernel lock table instead of over NLM, so `try_lock` always reports success» — [nfs-lock.yml](https://docs.rs/crate/minigraf/2.0.11/source/.github/workflows/nfs-lock.yml).

#### Повторяющиеся жалобы
1. **Производительность nfsserve.** «65 s with it and 14 s without» (transaction tracker) — [sandfs src/nfsserve/mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs). Протокол-уровень на macOS: «compiling ripgrep from scratch took about 18 s in a mount against 13 s in a plain folder» — [sandfs README](https://docs.rs/crate/sandfs/0.9.0/source/README.md); «work on many small files several times slower than on the disk» — [GitHub](https://github.com/lydakis/sandfs).
2. **Неограниченные аллокации, DoS.** netget: «The crate sizes buffers from a wire-supplied length with no cap, which made a forty-byte unauthenticated MOUNT a whole-process denial of service, and `NFSTcpListener` exposes no seam to fix that inside it» — [src/server/nfs/mod.rs](https://docs.rs/crate/netget-tool/0.3.5/source/src/server/nfs/mod.rs). sandfs: «Messages over 2 MiB are refused instead of read whole, closing the connection (which upstream left open, reading on)» — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
3. **Нет хука перед диспетчером.** sandfs добавил `CallFilter`, «which sandfs used to do in a relay in front of it, costing every call a second trip over the loopback» — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs). netget тоже ставит relay-guard (п. 2).
4. **Не хватает процедур и семантики:**
   - нет LINK; cookie = fileid (sandfs);
   - «ignores the mode of `mkdir`» (links-fs, на `nfs3_server`);
   - у heddle часть операций отдаёт `NFS3ERR_ROFS`;
   - источники: [sandfs mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs), [links-fs README](https://docs.rs/crate/links-fs/0.21.1/source/README.md), [heddle src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs).
5. **Windows.**
   - atomic-server: «`nfsserve` reaches for `std::os::unix::fs::{MetadataExt, PermissionsExt}` in fs_util.rs, so it does not compile for windows at all; windows ships without the virtual drive» — [desktop/Cargo.toml](https://github.com/ontola/atomic-server/blob/54dc025958dd370c1730dc240854d9614ecf1b60/desktop/Cargo.toml).
   - Про клиент: «Windows' own built-in NFS client is Pro/Enterprise/Server-only and doesn't support the custom ports this tool needs» — [mq-mount README](https://docs.rs/crate/mq-mount/0.1.7/source/README.md).
   - heddle: `mount.exe` требует «Services for NFS» и букву диска — [src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs).
6. **`unsafe`/`static mut` в nfsserve.** «The generation number is a `OnceLock` instead of a `static mut`, as sandfs forbids unsafe code» — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
7. **Ограничения самого NFS для виртуальных ФС** (EdenFS):
   - «NFS never tells us when a file has been forgotten»;
   - «does not know the client pid»;
   - «IO in NFS mounts will hang» при падении сервера;
   - «macOS NFS client does not support close-to-open guarantees»;
   - readdir без типов записей;
   - источник: [macOS.md](https://github.com/facebook/sapling/blob/main/eden/fs/docs/macOS.md).
   - Плюс диалог «Server connections interrupted» под нагрузкой — [Apple Developer Forums, thread 766793](https://developer.apple.com/forums/thread/766793).
8. **Права на монтирование — противоречивые данные.**
   - heddle: «`mount_nfs` needs sudo … the `mount` syscall itself is still root-only» — [src/nfs.rs](https://docs.rs/crate/heddle-mount/0.31.3/source/src/nfs.rs).
   - links-fs/sandfs/ossuary заявляют монтирование на macOS без root — [links-fs кейс-стади](https://docs.rs/crate/links-fs/0.21.1/source/docs/case-studies/issue-1/README.md), [ossuary README](https://docs.rs/crate/ossuary-mount/0.14.0/source/README.md).
   - На Linux root нужен — [mq-mount README](https://docs.rs/crate/mq-mount/0.1.7/source/README.md).

#### Сдвиг рынка: FSKit / ProjFS
- heddle на macOS 26.0+ использует FSKit (FSKit V2 URL resources), а на старых macOS откатывается на NFS — [README](https://docs.rs/crate/heddle-mount/0.31.3/source/README.md).
- oxcrypt: FSKit (macOS 15.4+) «is promising for kernel-extension-free mounts … but the API is still underdocumented» — [README](https://github.com/agucova/oxcrypt/blob/916864f66184dae6f97b0e97e02b879144fcfed6/README.md).
- Инженер EdenFS на форуме Apple изучал FSKit как замену NFS (тред создан в октябре 2024). Apple ответила, что `reclaimItem` в FSKit «very similar to FUSE's forget», но «FSKit does not support process attribution» — [thread 766793](https://developer.apple.com/forums/thread/766793).

### Inferences
- Ценность встраиваемой NFS-библиотеки сегодня определяется в первую очередь тем, **работает ли она с macOS-клиентом при сервере на macOS**. Linux-only сервер выпадает из основного сегмента (агенты, VCS, desktop-приложения на Mac). Linux-сценарии (ArcBox, aegis, nessie, flint) — меньшинство, и там же конкурируют ядерный nfsd и nfs-ganesha.
- Большинство требований адоптеров — инженерные, а не протокольные: безопасность localhost, ограничение аллокаций, хуки, LINK/cookie, корректный COMMIT, отсутствие `unsafe`, сборка под Windows. «Полный RFC 1813» — необходимое, но не достаточное условие выбора.
- Окно возможностей для NFS-на-macOS ограничено: FSKit (macOS 15.4+/26) уже используется как основной путь в heddle, есть в FUSE-T, его изучали в EdenFS. На горизонте 1–3 лет NFS на Mac, вероятно, станет fallback-ом, а не основным путём (прогноз, а не факт).
- NFSv4.x (embednfs, FUSE-T) привлекателен для macOS из-за xattr и блокировок. Это аргумент в пользу дорожной карты nfs-mamont с NFSv4, но рынок платит за удобство встраивания, а не за версию протокола.

### Gaps
- Публичных обсуждений этого паттерна на HN или Reddit с жалобами на nfsserve найти не удалось: поиск выдавал только страницы проектов. Все жалобы выше взяты из кода и README адоптеров.
- Количественных бенчмарков «localhost NFS vs FSKit vs macFUSE» в источниках нет, кроме цифр sandfs.
- Противоречие о правах root для `mount_nfs` на macOS не разрешено: нет первичной документации Apple в выборке.

## 4. Идеи из long-tail крейтов и адоптеров, которые стоит перенять nfs-mamont

### Takeaway
Самые ценные идеи — инженерные «hardening + embedding» практики, которые адоптеры вынуждены дописывать поверх nfsserve:
- лимиты на размер записи и аллокации;
- хук-фильтр перед диспетчером;
- аутентифицированные хэндлы и loopback-гейт;
- независимые READDIR-cookie, LINK, persistent-хэндлы;
- macOS-специфика: `._*`, side-channel для fsync.

Архитектурные идеи (runtime-agnostic через compio, thread-per-core с `SO_REUSEPORT`, derive-XDR, NFSv2) вторичны. fractal-nfs, где они воплощены, не нашёл пользователей и был заброшен.

### Cited Findings
- **Лимиты и hardening декодера.**
  - `onc-xdr` ограничивает упреждающую аллокацию `PREALLOC_CAP` = 1 MiB и даёт `BoundedList` — [README](https://docs.rs/crate/onc-xdr/1.2.0/source/README.md).
  - `fractal-nfs` закрывает соединение, если запись больше `max_rpc_fragment_bytes` (16 MiB), и урезает READ — [src/tokio_server.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/tokio_server.rs), [src/config.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/config.rs).
  - sandfs отказывает сообщениям больше 2 MiB — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
  - filemux валидирует `max_connections` 1..=1024 и `max_request_bytes` 1 KiB..4 MiB и отбрасывает лишние TCP-соединения «before allocating record buffers» — [src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs).
- **Хук перед диспетчером.** `context::CallFilter` в sandfs: «Each call can be checked, answered or have its reply changed before it reaches the server» — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
- **Ответ одной записью.** «a reply goes out in one write instead of two (its record mark, then the rest)». То же делает `frame_reply` в fractal-nfs — [sandfs mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs), [fractal-nfs src/tokio_server.rs](https://docs.rs/crate/fractal-nfs/0.1.0/source/src/tokio_server.rs).
- **Дешёвый DRC/трекер транзакций.** sandfs чистит старые записи раз в секунду, а не на каждом вызове — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
- **READDIRPLUS без атрибутов** («A listing's entry may come without attributes … which READDIRPLUS allows») и **cookie, независимые от fileid** — [mod.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfsserve/mod.rs).
- **Аутентификация хэндлов и экспорта.**
  - HMAC-SHA256 по fileid с секретом на экспорт и жёсткий отказ от не-loopback bind (coven) — [coven-afs/src/nfs.rs](https://github.com/OpenCoven/coven/blob/aa7581e37099471cd7bc0b8c1ef21110ed5d07ca/crates/coven-afs/src/nfs.rs).
  - Секретный путь экспорта и гейт по uid владельца (sandfs) — [src/nfs.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfs.rs).
  - Маршрутизация экспорта `/{tenant_id}/{volume_id}` с авторизацией и хэндлы ≤ 64 байт (48 + overhead) в aegis — [server.rs](https://docs.rs/crate/aegis-orchestrator-core/1.2.0-beta/source/src/infrastructure/nfs/server.rs).
- **Persistent-хэндлы.** filemux: «Handles contain a format tag, persistent export identity and persistent object id, never a startup nonce» (SQLite) — [src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs).
- **macOS-специфика** (из sandfs):
  - фильтровать `._name` из листингов, но открывать по имени;
  - отдельный localhost-канал «save requests», потому что fsync до NFS-сервера не доходит;
  - источник: [src/nfs.rs](https://docs.rs/crate/sandfs/0.9.0/source/src/nfs.rs), [commit 4dc07b5](https://github.com/lydakis/sandfs/commit/4dc07b5d61323e0047e1b4aaaf38aa181d24079b).
- **Runtime-agnostic и thread-per-core.**
  - fractal-nfs: фичи `tokio-runtime`/`compio-runtime`, per-CPU потоки с `SO_REUSEPORT`, не-`Send` futures, трейт с дефолтами (read-only «в одну строку»), MOUNT на том же порту — [README](https://docs.rs/crate/fractal-nfs/0.1.0/source/README.md).
  - Сам проект от NFS отказался — [commit b2c83f1](https://github.com/fractalbits-labs/fractalbits/commit/b2c83f13bf26e6dfe6c1c9eafc3d8a30be6fe2fd).
- **Derive-XDR.** `#[derive(XdrCodec)]` из `onc-xdr-derive` генерирует `Pack`/`Unpack` — [onc-xdr README](https://docs.rs/crate/onc-xdr/1.2.0/source/README.md).
- **NFSv2.**
  - Кодеки есть только у клиентов: `nfs-v2` (NFSWolf, все 18 процедур) и `prolink-proto` (деки CDJ: только LOOKUP/READ/GETATTR, UDP).
  - filemux явно не поддерживает NFSv2.
  - Источники: [nfs-v2 README](https://docs.rs/crate/nfs-v2/1.2.0/source/README.md), [prolink src/rpc/nfs2.rs](https://docs.rs/crate/prolink-proto/0.5.0/source/src/rpc/nfs2.rs), [filemux README](https://docs.rs/crate/filemux-core/0.2.0/source/README.md).
- **UDP + portmapper v2 + MOUNT в одном диспетчере** (для PXE/U-Boot и старых клиентов) — [filemux src/nfs/mod.rs](https://docs.rs/crate/filemux-core/0.2.0/source/src/nfs/mod.rs). Для сравнения: в nfs-mamont есть модуль `rpcbind` — это **клиент** регистрации (`PMAPPROC_SET`/`UNSET`), а не сервер portmapper — [/home/user/nfs-mamont/nfs-mamont/src/rpcbind/mod.rs](/home/user/nfs-mamont/nfs-mamont/src/rpcbind/mod.rs).
- **Kerberos/RPCSEC_GSS** — готовый pure-Rust акцептор `krb5` 1.0.0, вынесенный из NFS-сервера flint. Там же конвейеризация RPC на соединение — [krb5 README](https://docs.rs/crate/krb5/1.0.0/source/README.md), [flint src/nfs/mod.rs](https://github.com/ddalton/flint/blob/eb7c28fda25d5b78044ade16cafcd43911f889a8/spdk-csi-driver/src/nfs/mod.rs).
- **Тестовая инфраструктура.**
  - links-fs в CI ходит к серверу in-process NFS-клиентом (`nfs3_client`) на всех ОС, а на macOS CI ещё и монтирует реальным `mount_nfs` (`LFS_REQUIRE_NFS=1`) — [кейс-стади](https://docs.rs/crate/links-fs/0.21.1/source/docs/case-studies/issue-1/README.md).
  - NFSWolf берёт `nfs3_server` целью интеграционных тестов — [NOTICE](https://docs.rs/crate/nfs-v3/1.2.0/source/NOTICE).
  - Подкоманды NFSWolf `analyze`, `escape`, `brute-handle`, `uid-spray` проверяют handle-oracle (STALE vs BADHANDLE), no_root_squash и export escape — [README](https://docs.rs/crate/nfswolf/1.2.0/source/README.md), [nfs-v3 README](https://docs.rs/crate/nfs-v3/1.2.0/source/README.md).

### Inferences
- **Приоритет 1 (дёшево, отвечает на жалобы рынка):**
  - публичные лимиты (`max_record_bytes`, `max_connections`) с закрытием соединения;
  - хук `CallFilter`/middleware до диспетчера (auth-gate, перехват MOUNT);
  - готовый «loopback guard»: bind только 127.0.0.1 или UNIX-socket, HMAC-хэндлы, секретный путь экспорта;
  - независимые cookie;
  - LINK;
  - ответ одной записью;
  - O(1)-amortized DRC.

  Всё это адоптеры nfsserve дописывают сами (sandfs, netget, coven). Если nfs-mamont даст это из коробки, у неё будет внятное позиционирование «hardened nfsserve-альтернатива».
- **Приоритет 2 (стратегически):** поддержка сервера на macOS (и хотя бы сборка под Windows). Без этого nfs-mamont недоступна основному, «macOS-localhost» сегменту адоптеров: ≥15 из ~30 продуктов, см. п. 2–3. MSRV 1.75 у nfs-mamont ниже, чем у адоптеров, так что здесь барьера нет. Поле `rust_version` на crates.io: arcbox-agent 1.96, internxt-cli 1.95, aegis-orchestrator-core 1.91, sandfs 1.89, links-fs 1.89, ossuary-mount 1.85 — [arcbox-agent](https://crates.io/api/v1/crates/arcbox-agent), [internxt-cli](https://crates.io/api/v1/crates/internxt-cli), [sandfs](https://crates.io/api/v1/crates/sandfs), [ossuary-mount](https://crates.io/api/v1/crates/ossuary-mount).
- **Приоритет 3:** persistent-хэндлы и стабильные fileid как часть контракта `Vfs`, документированные macOS-рецепты (опции `mount_nfs`, `._*`, side-channel для fsync), интеграционный CI с реальным `mount_nfs` на macOS и прогон NFSWolf (`analyze`/`escape`/`brute-handle`) как security-регрессия.
- **Низкий приоритет:**
  - compio/io_uring-бэкенд: fractal-nfs его сделал, но это не помогло ни adoption-у, ни выживанию;
  - derive-XDR: у nfs-mamont ручной zero-copy парсер, а derive даёт меньше boilerplate ценой контроля над аллокациями;
  - NFSv2: спрос нишевый (DJ-оборудование, legacy), серверной реализации на Rust нет ни у кого.
- **Kerberos (через `krb5`)** и UDP/portmapper-сервер нужны для enterprise/LAN-сценариев (flint, filemux). В основной нише localhost-адоптеров они не востребованы.

### Gaps
- Не проверено, какие из перечисленных hardening-мер уже есть в nfs-mamont: аудит кода nfs-mamont вне этой задачи. В локальной копии видны модули `rpcbind` (клиент) и `nlm` — [/home/user/nfs-mamont/nfs-mamont/src](/home/user/nfs-mamont/nfs-mamont/src).
- Влияние thread-per-core против work-stealing tokio на NFS-нагрузку количественно не оценено: открытых бенчмарков у fractal-nfs нет.
- Совместимость `krb5` с RPCSEC_GSS-стеком вне flint и его зрелость (21 версия за 4 дня, 2026-10-13…16) не проверялись.
