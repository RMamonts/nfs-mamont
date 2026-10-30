# Эталонные user-space и встраиваемые NFS-серверы вне Rust: планка функциональности, качества и поддержки (состояние на 2026-10-30)

_Методика. Метрики GitHub (звёзды, форки, watchers, открытые issues и PR) сняты через WebFetch со страниц репозиториев 2026-10-30. Частота коммитов, авторы, почтовые домены и теги посчитаны по `git log` локальных клонов (`--filter=tree:0`) 2026-10-30. Функциональность проверена по исходникам (shallow-клоны) и по первичной документации. Сайты rclone.org, snia.org, learn.microsoft.com и git.linux-nfs.org из сессии недоступны, поэтому использованы зеркала на GitHub (исходники документации rclone, MicrosoftDocs) или вторичные источники; такие места помечены. Пометка **[УСТАРЕЛО]** ставится на данные старше примерно 2 лет или опровергнутые более свежими фактами, **[ПРОТИВОРЕЧИЕ]** — на расхождения между источниками._

## 1. Профили реализаций: версии протокола, блокировки, безопасность, API бэкенда

### Takeaway
Есть два уровня планки. «Полный» задают Linux nfsd и NFS-Ganesha: NFSv3 + NFSv4.0/4.1/4.2, NLM/NSM, RPCSEC_GSS (krb5/krb5i/krb5p) и RPC-with-TLS; в ядре ещё и pNFS. «Встраиваемый» задают библиотеки go-nfs, nfs4j и NFSv4-сервер Buildbarn. Они заметно уже: почти везде только AUTH_SYS (Kerberos есть только у nfs4j), RPC-with-TLS нет ни у кого, а для блокировок выбраны NFSv4-локи, а не NLM. unfs3 — демон только для NFSv3, без NLM и READDIRPLUS. nfs-mamont (NFSv3 + MOUNT, без NLM) по набору протоколов сейчас ближе всего к релизам go-nfs и к unfs3.

### Cited Findings

#### NFS-Ganesha (C, LGPL-3.0-or-later)
- Описание репозитория: «NFS-Ganesha is an NFSv3,v4,v4.1 fileserver that runs in user mode on most UNIX/Linux systems» (GitHub, 2026-10-30) — [GitHub](https://github.com/nfs-ganesha/nfs-ganesha). Лицензия LGPL v3, в исходниках `SPDX-License-Identifier: LGPL-3.0-or-later` — [LICENSE.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/LICENSE.txt).
- Вики: NFS v3, 4.0, 4.1 и 4.2 «плюс 9P», всё одновременно. Инфраструктура pNFS есть, но работы по ней «have not been active for many years», и сообщество её сейчас не поддерживает — [Ganesha wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki).
- **[УСТАРЕЛО в вики]** 9P удалён: коммит `83c0658b` «Remove 9P support» от 2026-10-16 вошёл в тег V15.9. В сообщении тега: «Removal of 9P code - 9P is not used to my knowledge and has not been maintained» — [commit](https://github.com/nfs-ganesha/nfs-ganesha/commit/83c0658b), [V15.9](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.9).
- Подсистемы протоколов в `src/Protocols`: `NFS`, `NFSACL`, `NLM`, `RQUOTA`, `CQOS`, `XDR` — [src/Protocols](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/Protocols). Опции сборки: `USE_NFS3`, `USE_NLM`, `INTERNAL_STATD`, `USE_NFSACL3`, `USE_RQUOTA`, `USE_VSOCK`, `USE_NFS_RDMA`, `USE_TLS` (бэкенд OpenSSL или GnuTLS), `RPCBIND` (по умолчанию ON), `_NO_TCP_REGISTER` — [src/CMakeLists.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/CMakeLists.txt).
- V11.0 (2026-06-25): «Integrated NSM». V14.0 (2026-07-31): «XCOPY», «Clustered Quality of Service», «RDMA: Add rpcbind registration», «Enabling NFS over UDP for mount and rquota only by default» — [V11.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V11.0), [V14.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V14.0).
- Безопасность. `SecType` принимает `none, sys, krb5, krb5i, krb5p`. На уровне экспорта есть `XprtSec(enum, values [none, tls, mtls])` и `PrivilegedPort(bool, default false)`. Блок `TLS_CONFIG` содержит `TLS_Min_Version` (по умолчанию «TLSv1.3») и `Enable_KTLS(bool, default true)` — [config.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/config_samples/config.txt). Поддержку TLS добавил коммит `906d632e` «NFS Ganesha - TLS support» от 2026-06-18. Среди основных изменений V10.0 (2026-06-19) названы «TLS support», «Infrastructure for gRPC», «fscrypt support» — [commit](https://github.com/nfs-ganesha/nfs-ganesha/commit/906d632e), [V10.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V10.0).
- Делегирования и NFSv4.2. V8.0 (2025-10-09): «Patches to make delegation functional», включая write delegations, и «Add Ganesha resource usage metrics for Prometheus». V9.0: «Support for NFSv4.2 Delegation Timestamp Extensions (RFC 9754)». В исходниках есть `nfs4_op_allocate.c`, `nfs4_op_copy.c`, `nfs4_op_layoutget.c` и другие — [V8.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V8.0), [V9.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V9.0), [src/Protocols/NFS](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/Protocols/NFS).
- API бэкенда (FSAL). В `src/FSAL` лежат `FSAL_CEPH`, `FSAL_GLUSTER`, `FSAL_GPFS`, `FSAL_KVSFS`, `FSAL_LIZARDFS`, `FSAL_MEM`, `FSAL_PROXY_V3`, `FSAL_PROXY_V4`, `FSAL_PSEUDO`, `FSAL_RGW`, `FSAL_SAUNAFS`, `FSAL_VFS` и `Stackable_FSALs` — [src/FSAL](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/FSAL). FSAL — это C-плагин внутри демона, а не встраиваемая библиотека. RPC-слой — собственная библиотека `ntirpc` (git-submodule `nfs-ganesha/ntirpc`) — [.gitmodules](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/.gitmodules).

#### Linux kernel nfsd (базовый уровень; mainline v7.3, окно слияния 7.4, HEAD от 2026-10-30)
- В `fs/nfsd/Kconfig` есть: `NFSD_V2` «NFS server support for NFS version 2 (DEPRECATED)» (`default n`), `NFSD_V3_ACL`, `NFSD_V4`, pNFS-лэйауты `NFSD_BLOCKLAYOUT`, `NFSD_SCSILAYOUT`, `NFSD_FLEXFILELAYOUT`, `NFSD_V4_2_INTER_SSC` (межсерверный COPY), `NFSD_V4_SECURITY_LABEL`, `NFSD_LEGACY_CLIENT_TRACKING` (DEPRECATED) и `NFSD_V4_POSIX_ACLS` — [Kconfig](https://github.com/torvalds/linux/blob/master/fs/nfsd/Kconfig), [Makefile (VERSION 7, PATCHLEVEL 3)](https://github.com/torvalds/linux/blob/master/Makefile).
- Операции NFSv4.2 в `nfs4proc.c`: `ALLOCATE`, `CLONE`, `COPY`, `COPY_NOTIFY`, `DEALLOCATE`, `GETXATTR`/`SETXATTR`/`LISTXATTRS`/`REMOVEXATTR`, `OFFLOAD_CANCEL`/`OFFLOAD_STATUS`, `READ_PLUS`, `SEEK` — [nfs4proc.c](https://github.com/torvalds/linux/blob/master/fs/nfsd/nfs4proc.c).
- RPC-with-TLS (RFC 9289) есть в NFS-сервере ядра с Linux 6.4. TLS-рукопожатие выполняет userspace-демон `tlshd` (ktls-utils), режим экспорта задаётся опцией `xprtsec=` — [Phoronix](https://www.phoronix.com/news/Linux-6.4-NFSD-RPC-With-TLS), [RHEL 10 docs](https://docs.redhat.com/ja/documentation/red_hat_enterprise_linux/10/html/configuring_and_using_network_file_services/configuring-an-nfs-server-with-tls-support). Обработка `xprtsec` есть в [export.c](https://github.com/torvalds/linux/blob/master/fs/nfsd/export.c).
- Signed filehandles: опция экспорта `sign_fh`. Сервер добавляет к файловому дескриптору MAC и при несовпадении отвечает `NFS[34]ERR_STALE`. Цель — защита от угадывания дескрипторов. Описано в mainline-документации — [exporting.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/exporting.rst). По материалам о серии патчей (Benjamin Coddington, SipHash-2-4), их целили в v7.1. Точная версия слияния по git не проверена — [LWN](https://lwn.net/Articles/1057539), [архив linux-nfs](https://yhbt.net/lore/linux-nfs/3d888a3d-fa6e-41c0-9b28-6dbb878a0a77@redhat.com/).
- Сопровождение: раздел MAINTAINERS «KERNEL NFSD, SUNRPC, AND LOCKD SERVERS». Мейнтейнеры (M) — Chuck Lever и Jeff Layton, ревьюеры — NeilBrown, Olga Kornievskaia, Dai Ngo, Tom Talpey. Статус «S: Supported», ветки `nfsd-testing` и `nfsd-next`. В зону ответственности входит `fs/lockd/` (NLM) — [MAINTAINERS](https://github.com/torvalds/linux/blob/master/MAINTAINERS).

#### willscott/go-nfs (Go, Apache-2.0)
- Описание: «golang NFSv3 server». В README: «NFSv3 protocol implementation in pure Golang. Current Status: Minimally tested; Mounts, read-only and read-write support» — [GitHub](https://github.com/willscott/go-nfs), [README](https://github.com/willscott/go-nfs/blob/master/README.md). **[УСТАРЕЛО]** Для ветки master README отстаёт (см. следующий пункт).
- NFSv4.0 (RFC 7530) с byte-range locking добавлен в master через PR #163 (автор Olavo Borges, слит 2026-09-26). Ненулевую minor version сервер отклоняет. В теги это пока не попало: последний тег v0.0.4 от 2026-04-13 — [PR #163](https://github.com/willscott/go-nfs/pull/163), [nfs4.go](https://github.com/willscott/go-nfs/blob/master/nfs4.go), [tags](https://github.com/willscott/go-nfs/tags).
- Блокировки: интерфейс `ByteRangeLocker` «behind NFSv4 LOCK, LOCKT and LOCKU». Таблицу можно делить между протоколами, чтобы блокировки конфликтовали кросс-протокольно — [locker.go](https://github.com/willscott/go-nfs/blob/master/locker.go). Реализации NLM/NSM в репозитории нет (по списку файлов) — [дерево репозитория](https://github.com/willscott/go-nfs).
- Безопасность: `SECINFO` возвращает только `AuthFlavorUnix` и `AuthFlavorNull`, коды `RPCSEC_GSS` встречаются лишь как статусы ошибок. Пример в README использует `NewNullAuthHandler` — [nfs4_onsecinfo.go](https://github.com/willscott/go-nfs/blob/master/nfs4_onsecinfo.go), [errors.go](https://github.com/willscott/go-nfs/blob/master/errors.go).
- API бэкенда — интерфейс `Handler`: `Mount(ctx, net.Conn, MountRequest) (MountStatus, billy.Filesystem, []AuthFlavor)`, `Change(billy.Filesystem) billy.Change`, `FSStat`, `ToHandle`/`FromHandle`/`InvalidateHandle`, `HandleLimit()`. Расширения: `UnixChange` (`Mknod`, `Mkfifo`, `Socket`, `Link`) и `CachingHandler` (`VerifierFor`, `DataForVerifier`) — [handler.go](https://github.com/willscott/go-nfs/blob/master/handler.go). Абстракция ФС — go-billy v5.6.0 — [go.mod](https://github.com/willscott/go-nfs/blob/master/go.mod).
- Ограничения абстракции из README: дескрипторы «considered opaque identifiers here, which means they will not work as expected in cases of hard linking». uid/gid берутся только через `Sys()`/`syscall.Stat_t` — [README](https://github.com/willscott/go-nfs/blob/master/README.md).
- Параллельная обработка запросов внутри одного соединения появилась как опция только в PR #162 (2026-09-18) — [PR #162](https://github.com/willscott/go-nfs/pull/162).

#### dCache/nfs4j (Java, LGPLv2 or later)
- README: «pure java implementation of NFS server version 3, 4.0 and 4.1 including pNFS extension with nfs4.1-files and flex-files layout types». **[ПРОТИВОРЕЧИЕ]** В описании репозитория сказано «Pure Java NFSv3 and NFSv4.2 implementation» — [README](https://github.com/dCache/nfs4j/blob/master/README.md), [GitHub](https://github.com/dCache/nfs4j).
- Из NFSv4.2 реализованы `COPY`, `GETXATTR`/`SETXATTR`/`LISTXATTRS`/`REMOVEXATTR`, `LAYOUTERROR`, `LAYOUTSTATS`. Классов для `SEEK`, `ALLOCATE`, `DEALLOCATE`, `READ_PLUS` в списке операций нет. Есть операции NFSv4.1 (сессии, pNFS `LAYOUTGET`/`GETDEVICEINFO`) и `WANT_DELEGATION` — [org/dcache/nfs/v4](https://github.com/dCache/nfs4j/tree/master/core/src/main/java/org/dcache/nfs/v4).
- Блокировки: `LOCK`/`LOCKT`/`LOCKU`, плюс модуль `dlm` (`DistributedLockManager`, Hazelcast; 2026-10-28 зависимость обновлена до hazelcast 5.7.0). Пакет v3 состоит только из `MountServer` и `NfsServerV3`, NLM нет — [dlm](https://github.com/dCache/nfs4j/tree/master/dlm), [v3](https://github.com/dCache/nfs4j/tree/master/core/src/main/java/org/dcache/nfs/v3), [PR #181](https://github.com/dCache/nfs4j/pull/181).
- Безопасность: `SECINFO_NO_NAME` отдаёт `rpcsec_gss_info` для `KRB5`, `KRB5I`, `KRB5P`. Слой RPC — `oncrpc4j-core` 3.4.3 — [OperationSECINFO_NO_NAME.java](https://github.com/dCache/nfs4j/blob/master/core/src/main/java/org/dcache/nfs/v4/OperationSECINFO_NO_NAME.java), [pom.xml](https://github.com/dCache/nfs4j/blob/master/pom.xml).
- API бэкенда — интерфейс `VirtualFileSystem`, центрированный на `Inode`. Методы: `access`, `create`, `getFsStat`, `getRootInode`, `lookup`, `link`, `list(Inode, byte[] verifier, long cookie)`, `directoryVerifier`, `mkdir`, `move`, `parentOf`, `read`, `readlink`, `remove`, `symlink`, `write(..., StabilityLevel)` → `WriteResult`, `commit`, `getattr`/`setattr`, `getAcl`/`setAcl`, `hasIOLayout`, `getIdMapper`, `getCaseInsensitive`/`getCasePreserving`, а также default-методы для xattr — [VirtualFileSystem.java](https://github.com/dCache/nfs4j/blob/master/core/src/main/java/org/dcache/nfs/vfs/VirtualFileSystem.java).
- Предупреждение в README: публичный API «still _unstable_», его следует считать `beta`. Изменения API описаны в `API-changes.md`. Для сборки нужны Java 17 и Maven 4 — [README](https://github.com/dCache/nfs4j/blob/master/README.md).

#### unfs3 (C, лицензия BSD-типа)
- «UNFS3 supports all NFSv3 procedures with the exception of the READDIRPLUS procedure». Нет экспорта на netgroups и wildcard-хосты. Из опций экспорта поддержаны только `ro, rw, root_squash, all_squash, no_root_squash, no_all_squash`. Есть режим `--enable-afs` для реэкспорта AFS и кластерные расширения в стиле ClusterNFS — [README](https://github.com/unfs3/unfs3/blob/master/README.md), [LICENSE](https://github.com/unfs3/unfs3/blob/master/LICENSE).
- Ограничения из `unfsd(8)`: «The network lock manager (NLM) protocol is not supported», поэтому клиентам нужен `nolock`. При создании объекта задаётся только начальный mode — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8).
- Аутентификация — только `AUTH_UNIX`: код проверяет `oa_flavor == AUTH_UNIX`, MOUNT возвращает `AUTH_UNIX` — [mount.c](https://github.com/unfs3/unfs3/blob/master/mount.c), [user.c](https://github.com/unfs3/unfs3/blob/master/user.c).
- Это демон, а не библиотека. Бэкенд — локальная ФС с переходниками `backend_unix.h`/`backend_win32.h` — [дерево](https://github.com/unfs3/unfs3).

#### Buildbarn bb-remote-execution, NFSv4-сервер виртуальной ФС (Go, Apache-2.0)
- ADR #9 (Ed Schouten, 2022-05-23): встроенный NFSv4-сервер на localhost как альтернатива FUSE на macOS. NFSv3 отвергнут, потому что из-за отсутствия COMPOUND он «far more 'chatty'». **[УСТАРЕЛО]** Тезис ADR, что NFSv4.1/4.2 «of little use», опровергнут дальнейшей историей — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).
- Хронология по коммитам: 2024-07-09 «Add an NFSv4.1 server»; 2024-07-11 «Implement automatic negotiation of NFSv4 minor version»; 2025-08-27 «Don't enable NFSv4.1 by default» с пояснением «NFSv4.0 turns out to be more stable»; 2025-10-05 «Add support for NFSv4 named attributes» — [3a086e5](https://github.com/buildbarn/bb-remote-execution/commit/3a086e5), [54fc26e](https://github.com/buildbarn/bb-remote-execution/commit/54fc26e), [d1a53c8](https://github.com/buildbarn/bb-remote-execution/commit/d1a53c8), [4cfaff2](https://github.com/buildbarn/bb-remote-execution/commit/4cfaff2).
- Код: `nfs40_program.go` («implements most of the features of NFSv4.0»), `nfs41_program.go` («most of the features of NFSv4.1»), `minor_version_fallback_program.go` и `metrics_program.go`. `system_authenticator.go` требует флейвор `AUTH_SYS` и превращает учётные данные в метаданные через JMESPath — [pkg/filesystem/virtual/nfsv4](https://github.com/buildbarn/bb-remote-execution/tree/main/pkg/filesystem/virtual/nfsv4).
- API бэкенда: общий слой `pkg/filesystem/virtual` с интерфейсами `Directory`/`Leaf` (`VirtualReadDir`, `VirtualOpenChild`, `VirtualOpenSelf`, `VirtualGetAttributes` с `AttributesMask`) и `HandleAllocator`, который различает stateful, stateless и resolvable объекты. Слой разделён между FUSE и NFSv4 — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md). С 2025-08-20 есть и бэкенд WinFSP — [efef252](https://github.com/buildbarn/bb-remote-execution/commit/efef252).
- Блокировки — модель NFSv4.0 с open-owner/lock-owner, seqid-кэшами ответов и `ByteRangeLock`. Неиспользуемые open-owner и client records удаляются автоматически по таймауту — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).

#### Точка отсчёта: nfs-mamont (по README, 2026-10-30)
- Заявлено: NFSv3 «full procedure set of RFC 1813», MOUNT, собственный RPC/XDR, zero-copy аллокатор, трейт `Vfs`. В roadmap (не сделано): NSM, NLMv4, NFSv4. MSRV 1.75, только Linux, лицензия MIT — [README](https://github.com/RMamonts/nfs-mamont).

### Inferences
- Для встраиваемых библиотек вне Rust нормой стали AUTH_SYS и доступ через loopback или туннель. Kerberos есть только у nfs4j, RPC-with-TLS ни у одной. Значит, для nfs-mamont GSS и TLS — это отличие от конкурентов, а не обязательный минимум. Для сетевых сценариев «как Ganesha/nfsd» TLS уже стал планкой (Linux 6.4+, Ganesha V10.0+).
- Новые user-space серверы (go-nfs в 2026 году, Buildbarn) решают блокировки через NFSv4 LOCK, а не через NLM. Ganesha, наоборот, в 2026 году интегрировал NSM внутрь. Вывод: для v3-клиентов NLM/NSM остаётся единственным способом дать блокировки, но по соотношению пользы к затратам NFSv4.0 с локами может оказаться выгоднее. При этом клиент Windows NFSv4 не умеет (см. раздел 4).
- У всех трёх встраиваемых API есть явные механизмы жизненного цикла дескрипторов: `ToHandle`/`FromHandle`/`InvalidateHandle`/`HandleLimit` в go-nfs, `Inode` в nfs4j, `HandleAllocator` в Buildbarn. Есть и верификаторы каталогов: `VerifierFor` в go-nfs, `directoryVerifier` в nfs4j. Трейт `Vfs` в nfs-mamont стоит сверить с этим набором; здесь это не проверялось.

### Gaps
- Статус pNFS по отдельным FSAL в Ganesha (кроме общей оговорки вики) не выяснен.
- Есть ли RPC-with-TLS в oncrpc4j/nfs4j, не проверено.
- Когда NFSv4.0 из go-nfs попадёт в тег и в rclone, неизвестно.
- Детали API `Vfs` в nfs-mamont намеренно не изучались.

## 2. Производительность и опубликованные бенчмарки

### Takeaway
Воспроизводимых публичных бенчмарков «user-space сервер против nfsd» почти нет, а имеющиеся в основном устарели (2016–2019). Реальная планка — встроенные метрики (Prometheus, gRPC-статистика), микробенчмарки (JMH у nfs4j) и производственные отчёты вендоров (IBM), а не «headline»-цифры.

### Cited Findings
- **[УСТАРЕЛО]** Ganesha и knfsd на VFS FSAL (рассылка, 2018): у ядра около 75K IOPS при ~0,8 мс против ~18K IOPS при ~8 мс у Ganesha. Мейнтейнер ответил, что VFS FSAL добавляет переходы user/kernel, поэтому обогнать knfsd на этом пути вряд ли получится, а сила Ganesha — экспорт userspace-кластерных ФС — [mail-archive](https://www.mail-archive.com/nfs-ganesha-devel@lists.sourceforge.net/msg05031.html), [msg05044](https://www.mail-archive.com/nfs-ganesha-devel@lists.sourceforge.net/msg05044.html).
- **[УСТАРЕЛО]** Малый fio-тест 4K random (gist, 2019): Ganesha чуть впереди для NFSv3 (около 7 039 против 6 571 IOPS), ядро чуть впереди на NFSv4.1. Один клиент и один профиль — [gist](https://gist.github.com/kvaps/8c2831ca15cf161a11e62cc4276793f7).
- **[УСТАРЕЛО]** Gandi (2019, FreeBSD): из-за конкуренции за блокировки в ядерном nfsd хостер перешёл на Ganesha и сообщил, что он работает «much better» — [Gandi](https://news.gandi.net/en/2019/03/tracking-a-storage-issue-led-to-software-change/).
- IBM Storage Scale 5.2.0 перешёл с Ganesha 4.3 на 5.7. Рост IOPS виден на всех версиях протокола, сильнее всего на NFSv4.1, и объясняется переработкой MDCACHE в Ganesha 5. Источник — блог IBM от 2024-05-20, **[УСТАРЕЛО относительно V15]** — [IBM community](https://community.ibm.com/community/user/blogs/mara-miranda-bautista/2024/05/20/ibm-storage-scale-ces-nfs-520-performance-eval).
- Наблюдаемость в Ganesha: метрики Prometheus (V8.0, 2025-10-09), gRPC-статистика (V12.0–V15.0), «Memory statistics and accounting» (V15.0) — [V8.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V8.0), [V12.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V12.0), [V15.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.0).
- У nfs4j есть модуль `benchmarks` с JMH-микробенчмарками (`CacheBenchmark`, `ConcurrentLockManagerBenchmark`, `FileTrackerBenchmark`, `IpMatcherBenchmark`, `SubjectBenchmark`), запуск — `mvn verify -Pbenchmark` — [benchmarks](https://github.com/dCache/nfs4j/tree/master/benchmarks), [README](https://github.com/dCache/nfs4j/blob/master/README.md).
- dCache NFSv4.1/pNFS (в основе nfs4j; связь в найденных работах явно не названа). CHEP 2010: 4 дня ATLAS HammerCloud на ~330 MB/s **[УСТАРЕЛО]**. CHEP 2024: в NFS ≤ 4.0 весь ввод-вывод идёт через один узел, отсюда упор на pNFS. ICCS 2026: улучшенная обработка layout, read delegation и zero-copy путь данных «substantially» поднимают I/O — [CHEP2010](https://www.dcache.org/manuals/2010/CHEP2010-NFS41-kemp.pdf), [OSTI (CHEP 2024)](https://www.osti.gov/servlets/purl/3009879), [ICCS 2026](https://www.iccs-meeting.org/archive/iccs2026/papers/167860503.pdf).
- Buildbarn: в описании конфигурации сказано, что на Linux «the use of FUSE is strongly recommended, as it performs better», а NFSv4.1 рекомендован, потому что «provides better parallelism». ADR признаёт накладные расходы кодогенерации XDR (аллокации в куче) приемлемыми, а в NFSv4.0 open-owner не может слать OPEN параллельно — [virtual.proto](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/proto/configuration/filesystem/virtual/virtual.proto), [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md). Есть `metrics_program.go` — [nfsv4](https://github.com/buildbarn/bb-remote-execution/tree/main/pkg/filesystem/virtual/nfsv4).
- go-nfs и rclone serve nfs: публичных бенчмарков не нашлось. В обсуждениях rclone упоминается, что без `CacheModeFull` NFS «doesn't work at all», и что отлаживать приходилось «printf debugging» — [rclone PR #7254](https://github.com/rclone/rclone/pull/7254).

### Inferences
- Для Rust-библиотеки планка — воспроизводимый бенчмарк-харнесс (fio через ядерный NFS-клиент Linux, сравнение с knfsd на том же бэкенде) плюс метрики в рантайме. Абсолютные цифры у конкурентов почти не публикуются, поэтому nfs-mamont может выделиться, опубликовав методику.
- go-nfs до PR #162 (2026-09) обрабатывал запросы внутри соединения последовательно (вывод из описания PR). Это ориентир: у async-реализации (tokio) потенциальное преимущество как раз в конкурентности на одном соединении.

### Gaps
- Свежих (2025–2026) независимых сравнений Ganesha и knfsd не найдено. Доклад SNIA SDC 2024 «Linux NFSD Progress» недоступен (snia.org блокирован).
- Бенчмарков для go-nfs, unfs3 и NFSv4-сервера Buildbarn не найдено.

## 3. Зрелость, внедрение и сопровождение (поддержка как maintenance и сообщество)

### Takeaway
Ganesha — единственный крупный, корпоративно поддерживаемый user-space сервер: IBM, Red Hat, Google и DDN; 391 коммит и 52 автора за год; с 2025-10 «rolling release» с примерно еженедельными тегами. Встраиваемые библиотеки держатся на 1–2 людях: go-nfs (Will Scott), nfs4j (Tigran Mkrtchyan, DESY), Buildbarn (Ed Schouten). unfs3 фактически в спячке: 0 коммитов за 12 месяцев.

### Cited Findings

#### NFS-Ganesha
- GitHub на 2026-10-30: 1.8k stars, 585 forks, 92 watching, 215 открытых issues, 5 открытых PR, 14 373 коммита в `next`. Ревью идёт на GerritHub, GitHub-PR не мержатся — [GitHub](https://github.com/nfs-ganesha/nfs-ganesha).
- Активность (git log на 2026-10-30): коммитов по годам — 2023: 223, 2024: 372, 2025: 286, 2026 (до 30.10): 348. Ещё 18 коммитов датированы 2050 годом (сбитые часы). С 2025-10-30 — 391 коммит, 52 автора. Лидеры: Frank S. Filz (85), Suhas Athani (38), Prabhu Murugesan (27) — [commits/next](https://github.com/nfs-ganesha/nfs-ganesha/commits/next).
- Почтовые домены авторов за год: ibm.com 138 (+8 us.ibm.com), mindspring.com 85 (адрес мейнтейнера F. Filz), redhat.com 57, gmail.com 40, google.com 23, ddn.com 13. Google-коммиты 2026 года — исправления use-after-free и утечек refcount — [commits/next](https://github.com/nfs-ganesha/nfs-ganesha/commits/next), [V15.9](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.9).
- Релизная модель: V8.0 (2025-10-09) — «the first merge with our rolling release model». Дальше V9.0 (2025-10-24) … V10.0 (2026-06-19), V11.0 (06-25), V12.0 (07-17), V13.0 (07-24), V14.0 (07-31), V15.0 (08-07) и V15.1–V15.9 почти еженедельно, последний V15.9 от 2026-10-16. С 2024-01-01 — 85 тегов, включая dev-теги — [tags](https://github.com/nfs-ganesha/nfs-ganesha/tags), [V8.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V8.0).
- **[УСТАРЕЛО/ПРОТИВОРЕЧИЕ]** В вики написано «Version 6.5 is the current supported release», но V6.5 датирован 2025-01-08, а после него вышли V7–V15. В той же вики есть ссылка на страницу «Stable15 test results» — [wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki), [tags](https://github.com/nfs-ganesha/nfs-ganesha/tags).
- Каналы сообщества: Element/Matrix (#devel, #support, #ntirpc), рассылки devel и support, два еженедельных 30-минутных видеосозвона по вторникам (7:00 и 16:00 по тихоокеанскому времени) — [wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki).
- Внедрение. Ceph экспортирует CephFS и бакеты RGW «using the NFS-Ganesha NFS server» через cephadm или Rook, HA строится на HAProxy и Keepalived. По умолчанию включён только NFSv4, NFSv3 — по флагу `--enable-nfsv3` — [ceph doc/mgr/nfs.rst](https://github.com/ceph/ceph/blob/main/doc/mgr/nfs.rst). CES NFS в IBM Storage Scale построен на NFS-Ganesha (в 5.2.0 — Ganesha 5.7, NFSv4.2 как tech preview в 5.2.2) — [IBM support](https://www.ibm.com/support/pages/node/7174841), [IBM community](https://community.ibm.com/community/user/blogs/mara-miranda-bautista/2024/05/20/ibm-storage-scale-ces-nfs-520-performance-eval). FSAL есть также для GlusterFS, GPFS, LizardFS и SaunaFS — [src/FSAL](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/FSAL).

#### go-nfs
- GitHub на 2026-10-30: 780 stars, 113 forks, 10 watching, 6 открытых issues, 9 открытых PR, GitHub-релизов нет — [GitHub](https://github.com/willscott/go-nfs).
- Всего 212 коммитов (с 2020-04-20). По годам: 2020: 52, 2021: 35, 2022: 13, 2023: 61, 2024: 30, **2025: 1**, 2026: 20. С 2025-10-30 — 20 коммитов от 6 имён авторов («Will» и «Will Scott», вероятно, одно лицо). Теги v0.0.1 (2023-11-29), v0.0.2 (2024-01-06), v0.0.3 (2025-02-06), v0.0.4 (2026-04-13), версия всё ещё pre-1.0 — [commits](https://github.com/willscott/go-nfs/commits/master), [tags](https://github.com/willscott/go-nfs/tags).
- Внедрение: rclone импортирует `github.com/willscott/go-nfs` в `cmd/serve/nfs` — [cache.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/cache.go). Граф зависимостей GitHub: 230 зависимых репозиториев и 209 пакетов, крупнейший из видимых — GlassHaven/Haven (1 391 stars) — [dependents](https://github.com/willscott/go-nfs/network/dependents). Nick Craig-Wood (ветки `ncw/*`; связь с rclone здесь не проверялась) присылает исправления, например EOF-флаг в READ (2026-04-13) и понижение уровня лога «No handler» (2026-05-24) — [275aeb9](https://github.com/willscott/go-nfs/commit/275aeb9), [1fbc928](https://github.com/willscott/go-nfs/commit/1fbc928).
- Есть `SECURITY.md` и CI (go build/test, CodeQL) — [дерево](https://github.com/willscott/go-nfs), [workflows](https://github.com/willscott/go-nfs/tree/master/.github/workflows).

#### nfs4j
- GitHub на 2026-10-30: 271 stars, 83 forks, 29 watching, 40 открытых issues, 4 открытых PR — [GitHub](https://github.com/dCache/nfs4j).
- 1 454 коммита с 2009-02-09. По годам: 2022: 34, 2023: 19, 2024: 16, 2025: 111, 2026: 42. С 2025-10-30 — 42 коммита от 5 авторов, из них Tigran Mkrtchyan — 31. Домены: desy.de 35, iterate.ch 3 (PR #175 и #177: `time_create`, отключение делегирований по экспорту) — [commits](https://github.com/dCache/nfs4j/commits/master), [PR #177](https://github.com/dCache/nfs4j/pull/177).
- Теги: 0.25.0 (2023-08-01), 0.26.0 (2024-09-12), 0.27.0/0.27.1 (2025-06), 0.26.1/0.27.2 (2026-03-02), 0.28.0–0.28.5 (2026-07-30 … 2026-08-19). Поддерживаются несколько веток одновременно (0.10 … 0.23 и далее) — [tags](https://github.com/dCache/nfs4j/tags).
- Внедрение: «used by the dCache and other projects in production». Артефакты лежат в Maven-репозитории dcache.org. **[УСТАРЕЛО]** Пример зависимости в README указывает версию 0.19.0. Вклад принимается по модели DCO (`Signed-off-by`) — [README](https://github.com/dCache/nfs4j/blob/master/README.md).

#### unfs3
- GitHub на 2026-10-30: 162 stars, 34 forks, 9 watching, 17 открытых issues, 4 открытых PR — [GitHub](https://github.com/unfs3/unfs3).
- 487 коммитов с 2004 года. Последний коммит — 2025-07-03, за 12 месяцев до 2026-10-30 коммитов нет. По годам: 2022: 24, 2023: 2, 2024: 14, 2025: 36. Релизы: 0.9.22 (2009-01-05) → 0.9.23 (2022-10-06) → 0.10.0 (2022-11-07) → 0.11.0 (2025-03-28). Из 50 коммитов за последние 3 года 45 сделаны с адресов cendio.se (Pierre Ossman) — [commits](https://github.com/unfs3/unfs3/commits/master), [tags](https://github.com/unfs3/unfs3/tags).
- Внедрение: Yocto/OpenEmbedded `runqemu` использует user-mode NFS (unfs3) для NFS-rootfs без root-прав и без rpcbind. **[УСТАРЕЛО]** Источники — старые ветки poky — [poky runqemu-export-rootfs (atom)](https://git.enea.com/cgit/linux/poky.git/atom/scripts/runqemu-export-rootfs?h=python3), [dev-manual (dizzy)](https://git.enea.com/cgit/linux/poky.git/plain/documentation/dev-manual/dev-manual-qemu.xml?h=dizzy).

#### Buildbarn bb-remote-execution
- GitHub на 2026-10-30: 171 stars, 110 forks, 14 watching, 17 открытых issues, 25 открытых PR. В README нет упоминаний коммерческой поддержки или Slack — [GitHub](https://github.com/buildbarn/bb-remote-execution).
- 647 коммитов. По годам: 2023: 74, 2024: 73, 2025: 64, 2026: 55. С 2025-10-30 — 67 коммитов от 17 авторов, из них Ed Schouten — 34. Все 30 коммитов с адресов apple.com за этот период принадлежат Ed Schouten, в том числе коммит, прекращающий официальную поддержку NFSv4 на macOS Sonoma. Другие домены: meroton.com 8, cocotec.io 6, mercedes-benz.com 3. Теги «rolling» в формате временных меток, например `20261022T081640Z-1fbffdc` — [commits](https://github.com/buildbarn/bb-remote-execution/commits/main), [6dae65a](https://github.com/buildbarn/bb-remote-execution/commit/6dae65a), [tags](https://github.com/buildbarn/bb-remote-execution/tags).

#### Linux nfsd
- Статус «Supported», два мейнтейнера и четыре ревьюера. Патчи выдерживаются в `nfsd-testing` (zero-day боты, статический анализ, CI) до перехода в `nfsd-next`; CI гоняется и на LTS-ветках — [MAINTAINERS](https://github.com/torvalds/linux/blob/master/MAINTAINERS), [nfsd-maintainer-entry-profile.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/nfsd-maintainer-entry-profile.rst).

### Inferences
- Сводка по поддержке (на 2026-10-30):

| Проект | Коммитов/авторов за 12 мес. | Последний тег | Модель | Основная поддержка |
|---|---|---|---|---|
| NFS-Ganesha | 391 / 52 | V15.9 (2026-10-16) | rolling, ~еженедельно | IBM, Red Hat, Google, DDN |
| Linux nfsd | (часть ядра) | v7.3 mainline | цикл ядра + LTS | мейнтейнеры C. Lever и J. Layton (адреса kernel.org); ревьюеры в т.ч. с адресов oracle.com и redhat.com |
| go-nfs | 20 / ~5 | v0.0.4 (2026-04-13) | редкие 0.0.x | один мейнтейнер (Will Scott) + внешние контрибьюторы |
| nfs4j | 42 / 5 | 0.28.5 (2026-08-19) | 0.x, несколько веток | DESY/dCache |
| Buildbarn (NFSv4 внутри) | 67 / 17 | rolling timestamp | непрерывно | Ed Schouten (apple.com), meroton |
| unfs3 | 0 / 0 | 0.11.0 (2025-03-28) | редкие | Cendio |

- Почтовый домен показывает, где работает автор, но не доказывает, что компания официально поддерживает проект. «Коммерческая поддержка» Ganesha через продукты IBM (Storage Scale, Storage Ceph) и Red Hat — вывод, а не цитата.
- Ориентир по bus factor: у встраиваемых библиотек вне Rust 1–2 активных мейнтейнера (go-nfs, nfs4j, Buildbarn). История nfs-mamont здесь не анализировалась (локальный клон не отражает полную историю), поэтому прямого сравнения нет. До планки Ganesha (много компаний, каналы, созвоны, еженедельные релизы) не дотягивает ни одна из библиотек.

### Gaps
- Официальных заявлений о коммерческой поддержке Ganesha (Red Hat/IBM SLA), Buildbarn (Meroton) и nfs4j не найдено.
- Статистика скачиваний (pkg.go.dev, Maven) не собиралась.

## 4. Поддержка платформ и клиентов (серверные ОС; клиенты Linux, macOS, Windows)

### Takeaway
Клиентская матрица и есть главная скрытая сложность. Linux-клиент — основной ориентир (даже nfsd тестирует не-Linux клиентов только на Bake-a-thon). Клиент macOS по-настоящему пригоден для NFSv4 только с Sonoma 14.4+/Sequoia 15 и обходится без NLM лишь с `nolocks`/`locallocks`. Windows Client for NFS поддерживает только NFSv2/v3 и почти всегда требует `nolock` и маппинга anonymous UID/GID.

### Cited Findings
- Серверные ОС. Ganesha: «most UNIX/Linux systems» — [GitHub](https://github.com/nfs-ganesha/nfs-ganesha). unfs3 разрабатывается на Linux, исторически тестировался на NetBSD, FreeBSD, Solaris, Irix и Mac OS X, частично поддерживает Windows (`doc/README.win`); CI собирает на ubuntu, macos и windows — [README](https://github.com/unfs3/unfs3/blob/master/README.md), [build.yml](https://github.com/unfs3/unfs3/blob/master/.github/workflows/build.yml). В go-nfs есть платформенные файлы `file_unix.go`, `file_windows.go`, `file_wasm.go` — [file/](https://github.com/willscott/go-nfs/tree/master/file). rclone serve nfs «is only available on Unix platforms» — [nfs.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/nfs.go). Buildbarn использует NFSv4 на macOS, FUSE на Linux (NFSv4 там добавлен «absolutely not… in production», только для проверки переносимости) и WinFSP на Windows — [0b3dad9](https://github.com/buildbarn/bb-remote-execution/commit/0b3dad9), [efef252](https://github.com/buildbarn/bb-remote-execution/commit/efef252). nfs-mamont работает только на Linux — [README](https://github.com/RMamonts/nfs-mamont).
- Приоритет nfsd: «interoperate fully with the Linux NFS client… Non-Linux NFS clients are not part of upstream NFSD CI/CD», с другими клиентами проверяется на Bake-a-thon — [nfsd-maintainer-entry-profile.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/nfsd-maintainer-entry-profile.rst).
- Windows (MicrosoftDocs, ms.date 05/13/2025): Client for NFS поддерживает NFSv2 и NFSv3, Server for NFS — NFSv2, v3 и NFSv4.1; Kerberos (krb5/i/p) указан для Server for NFS — [nfs-overview.md](https://github.com/MicrosoftDocs/windowsserverdocs/blob/main/WindowsServerDocs/storage/nfs/nfs-overview.md). Ceph тоже отмечает, что ни одна версия Windows нативно не монтирует NFSv4.x — [ceph nfs.rst](https://github.com/ceph/ceph/blob/main/doc/mgr/nfs.rst).
- Windows, практика (вендорские и community-гайды): рекомендуется `mount -o nolock`, без него в тестах наблюдали повреждённые файлы. Доступ через anonymous UID/GID задаётся DWORD в реестре, по умолчанию -2 — [HPE](https://docs.ezmeral.hpe.com/datafabric-customer-managed/80/AdministratorGuide/MountingNFSonWindowsClient.html), [UCSD wiki](https://sdsc-ucsd.atlassian.net/wiki/spaces/SC/pages/353042457), [Netwrix](https://blog.netwrix.com/2022/11/18/mounting-nfs-client-windows).
- macOS, опции `mount_nfs`: `resvport` (по умолчанию macOS использует непривилегированный порт, а Linux/BSD-серверы ждут привилегированный), `locallocks` (блокировки локально в VFS клиента), `nolocks` — [unix.com mount_nfs(8)](https://www.unix.com/man-page/osx/8/MOUNT_NFS), [manpagez](https://www.manpagez.com/man/8/mount_nfs).
- macOS и NFSv4, хронология по Buildbarn:
  - Ventura 13.3 содержит «a large number of fixes» — [3613539](https://github.com/buildbarn/bb-remote-execution/commit/3613539);
  - Sonoma 14.4 (2024-03-07) — «the first version of macOS on which NFSv4 works reasonably well», в более ранних «quite severe bugs… that cause data loss» — [dafec6f](https://github.com/buildbarn/bb-remote-execution/commit/dafec6f);
  - в 14.5 исправлены ложные ENOENT при параллельном `open()` по хардлинкам — [b669c2e](https://github.com/buildbarn/bb-remote-execution/commit/b669c2e);
  - 2026-05-27: Sequoia 15.0 «contains a lot of bug fixes to the NFSv4 driver», официальная поддержка Sonoma и старше прекращена — [6dae65a](https://github.com/buildbarn/bb-remote-execution/commit/6dae65a);
  - NFSv4 поверх UNIX-сокетов доступен на части версий macOS, отсюда определение возможностей по версии ОС — [6625bfe](https://github.com/buildbarn/bb-remote-execution/commit/6625bfe);
  - в описании конфигурации: «supported on macOS starting with Sequoia 15.0 (24A335)» — [virtual.proto](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/proto/configuration/filesystem/virtual/virtual.proto).
- macOS, прочее. rclone рекомендует на macOS не отключать unicode-нормализацию ни для `mount`, ни для `serve`. Через NFS-клиент macOS нельзя по отдельности задать atime и mtime («the NFS client… always modifies both»), так что Finder провоцирует перезаливку файлов. Монтирования с `--read-only` при записи падают молча — [rclone_mount.md](https://github.com/rclone/rclone/blob/master/docs/content/commands/rclone_mount.md).
- macOS/FUSE-T: при ошибке «Operation not permitted» нужно разрешить доступ к «Network Volumes»; README объясняет это как «a consequence of fuse-t leveraging the nfs system under the covers» — [FUSE-T](https://github.com/macos-fuse-t/fuse-t).

### Inferences
- Если nfs-mamont предлагается для «экспорта виртуальной ФС на десктоп», реальная клиентская матрица такая: Linux (v3/v4), macOS (v3 с `nolocks`/`locallocks` или v4.0), Windows (только v3, `nolock`, anon UID/GID). Без NLM блокировки на Windows и macOS-v3 работать не будут. С NFSv4 становится доступен macOS, но не Windows.
- Ограничение «только Linux» у сервера nfs-mamont сужает рынок по сравнению с go-nfs (pure Go, кросс-платформенный), nfs4j (JVM) и unfs3 (Unix + Windows): главный сценарий конкурентов — сервер на macOS (rclone, Buildbarn, FUSE-T).

### Gaps
- Первоисточник Apple с поддерживаемыми минорными версиями NFSv4 в macOS 15/26 не найден; данные взяты из коммитов Buildbarn.
- Официальная позиция Microsoft по NLM в Client for NFS не найдена (на странице MicrosoftDocs NLM не упоминается).

## 5. Кто конкурирует с nfs-mamont за тот же сценарий (встраивание NFS-сервера в приложение для виртуальной или удалённой ФС)

### Takeaway
Прямые конкуренты — go-nfs (rclone, 230 зависимых репозиториев) и nfs4j (dCache). Решение Buildbarn — сильный референс по архитектуре, но не отдельная библиотека. FUSE-T занимает ту же нишу на macOS как продукт. Ganesha конкурирует косвенно, как демон с C-плагинами (FSAL). unfs3 и knfsd экспортируют только локальную ФС и служат базовыми линиями, а не конкурентами.

### Cited Findings
- go-nfs — библиотека: `nfs.Serve(listener, handler)` поверх `net.Listener`, бэкенд — любая `billy.Filesystem`, включая memfs — [README](https://github.com/willscott/go-nfs/blob/master/README.md). Основной потребитель — rclone serve nfs/nfsmount. Цель команды — «enable the mount command on recent macOS versions where installing FUSE is very cumbersome» — [nfs.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/nfs.go).
- nfs4j — библиотека с `VirtualFileSystem`, `NFSServerV41.Builder`, `NfsServerV3`, `MountServer` и `OncRpcSvcBuilder` — [README](https://github.com/dCache/nfs4j/blob/master/README.md).
- Buildbarn: NFSv4-сервер работает поверх общего с FUSE и WinFSP слоя `pkg/filesystem/virtual` и монтирует себя сам (на macOS через UNIX-сокет). Конфигурируется как часть `bb_worker`/`bb_clientd` — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md), [nfsv4_mount_darwin.go](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/filesystem/virtual/configuration/nfsv4_mount_darwin.go).
- FUSE-T: «kext-less implementation of FUSE for macOS that uses NFS v4 local server». Сервер конвертирует RPC в FUSE-запросы. Позже добавлены SMB и FSKit-бэкенды. На 2026-10-30: 1.6k stars, версия 1.2.6. Какая реализация NFS используется и лицензия серверной части из README не видны — [FUSE-T](https://github.com/macos-fuse-t/fuse-t). rclone описывает FUSE-T как систему, которая «"mounts" via an NFSv4 local server» — [rclone_mount.md](https://github.com/rclone/rclone/blob/master/docs/content/commands/rclone_mount.md).
- Ganesha: `FSAL_MEM`, `FSAL_PROXY_V3`/`FSAL_PROXY_V4`, `FSAL_RGW` и другие. По словам мейнтейнера, сильная сторона Ganesha — экспорт userspace-кластерных ФС — [src/FSAL](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/FSAL), [mail-archive](https://www.mail-archive.com/nfs-ganesha-devel@lists.sourceforge.net/msg05031.html).
- unfs3 — «user-space NFSv3 server» для локальных каталогов, AFS и FAT. Типичная ниша — NFS без root, например Yocto runqemu — [README](https://github.com/unfs3/unfs3/blob/master/README.md), [poky (atom)](https://git.enea.com/cgit/linux/poky.git/atom/scripts/runqemu-export-rootfs?h=python3).

### Inferences
- По позиционированию nfs-mamont ближе всего к go-nfs: библиотека, v3, подключаемая VFS. Отличия — async/tokio, zero-copy и Rust. В master go-nfs уже есть NFSv4.0 с локами, а у nfs-mamont v4 пока в планах. Тест на паритет: сценарий «rclone-подобный слой ФС на macOS» плюс устойчивые к рестарту дескрипторы.
- Для облачных и distributed-FS сценариев планку задаёт Ganesha: HA, grace, кластерные FSAL и TLS. Встраиваемой библиотеке с ним соревноваться не стоит, разумнее позиционироваться как «Rust-аналог go-nfs/nfs4j».

### Gaps
- Какой NFS-стек использует FUSE-T (свой или форк go-nfs), не подтверждено.
- Полный список крупных пользователей go-nfs и nfs4j вне rclone и dCache не установлен.

## 6. Практика тестирования на соответствие (pynfs, cthon04, pjdfstest, xfstests/fstests, nfstest, Bake-a-thon)

### Takeaway
Де-факто стандарт для NFSv4-серверов — pynfs (4.0 и 4.1). Для v3 и v4 через реальный клиент — cthon04 (basic/general/special/lock). Для ядра — fstests, pynfs, nfstest, gitr и ltp через kdevops; меж-вендорская совместимость проверяется на Bake-a-thon. Внешние наборы реально запускают только Ganesha (вручную/presubmit), nfs4j (pynfs в CI) и nfsd (kdevops). go-nfs, unfs3, Buildbarn и nfs-mamont внешних наборов в репозиториях не упоминают.

### Cited Findings
- Ganesha, `CONTRIBUTING_HOWTO.txt`. Перед отправкой патча обязательно тестирование: cthon04 «should be run for both NFSv3 and NFSv4.x»; pynfs «must be run for both 4.0 and 4.1» (4.1 не покрывает базовую функциональность 4.0), плюс Ganesha-специфичные тесты. Мейнтейнер публикует свою команду `./testserver.py ... --maketree ... --rundeps all ganesha` и перечень «non-success» результатов на FSAL_VFS начиная с V6.0: GSS-тесты пропущены, `WRT18` (change attribute) — FAILURE, `SATT9` — UNSUPPORTED — [CONTRIBUTING_HOWTO.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/CONTRIBUTING_HOWTO.txt). Есть скрипт `run_test_mode.sh` «to run Ganesha for presubmit testing (e.g. pynfs, cthon)» — [run_test_mode.sh](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/test/run_test_mode.sh).
- Ganesha регулярно чинит то, что находит pynfs: CSESS20/21 (2026-03-13), RD5a (2025-10-09), SATT12x и RDLK1 (2026-06). Через Coverity исправлено много дефектов, проверка стиля — `checkpatch.pl` в CI — [commits](https://github.com/nfs-ganesha/nfs-ganesha/commits/next), [wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki).
- nfs4j гоняет pynfs в CI (GitHub Actions, `podman-compose -f full-test.yml`, образ `dcache/pynfs:0.5`). Прогон v4.0 исключает 113 именованных тестов плюс категории char/socket/block/fifo. Прогон v4.1 идёт с `--minorversion=2`, исключает 21 тест, отчёт публикуется как «pynfs protocol compatibility test» — [full-test.yml](https://github.com/dCache/nfs4j/blob/master/full-test.yml), [build.yml](https://github.com/dCache/nfs4j/blob/master/.github/workflows/build.yml).
- Linux nfsd: kdevops содержит «several NFS-specific workflows, as well as the community standard fstests suite… based on… ltp and fio» — [nfsd-maintainer-entry-profile.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/nfsd-maintainer-entry-profile.rst). README kdevops перечисляет workflow `pynfs`, `gitr`, `ltp`, `nfstest` с baseline-режимами (37 stars на 2026-10-30) — [kdevops](https://github.com/linux-kdevops/kdevops). В документации kdevops fstests выбирается с ФС «nfs» и отдельными узлами под каждую версию NFS; для pNFS нужны 4.1 или 4.2 — [kdevops docs/nfs.md](https://github.com/mcgrof/kdevops/blob/main/docs/nfs.md). В mainline есть `tools/testing/selftests/nfsd` — [selftests/nfsd](https://github.com/torvalds/linux/tree/master/tools/testing/selftests/nfsd).
- **[УСТАРЕЛО]** Jeff Layton (2023-08) публиковал результаты xfstests group auto против NFS-сервера с двумя клиентами (по умолчанию и NFSv3) на трёх ядрах — [linux-nfs](https://spinics.net/lists/linux-nfs/msg98488.html).
- pynfs: входная точка `testserver.py <server>:<export> --maketree`, далее `--rundeps`. Полный прогон занимает около 30 минут. Сервер должен принимать соединения с непривилегированных портов (на Linux — опция `insecure`). Падение теста не обязательно означает ошибку сервера: сверяйтесь с RFC — [FreeBSD D798](https://reviews.freebsd.org/D798), [Fedora QA](https://fedoraproject.org/wiki/QA:Testcase_nfs_pynfs), [зеркало README (vast-data/pynfs)](https://awesome.ecosyste.ms/projects/github.com%2Fvast-data%2Fpynfs).
- cthon04 состоит из групп basic, general, special и lock (`runtests -b/-g/-s/-l/-a`). Сначала нужно полностью пройти basic. GitHub-зеркала — у Philippe Deniel (один из ранних разработчиков Ganesha по истории коммитов) и dkruchinin — [phdeniel/cthon04](https://github.com/phdeniel/cthon04), [cthon-nfs-tests](https://www.github.com/dkruchinin/cthon-nfs-tests). Официальный git.linux-nfs.org из сессии недоступен (403).
- NFStest (Jorge Mora, NetApp, GPLv2; Debian 3.2) нацелен в основном на клиент, но годится и для сервера. Покрывает TCP/RPC, NFSv4.0 и v4.1/pNFS. Инструменты: `nfstest_posix`, `nfstest_lock`, `nfstest_cache` (консистентность кэша атрибутов и данных), `nfstest_interop` (запись одной версией NFS, чтение другой), `nfstest_pnfs` — [Debian PKG-INFO](https://sources.debian.org/src/nfstest/3.2-3/PKG-INFO), [nfstest_posix](https://www.mankier.com/1/nfstest_posix), [nfstest_cache](https://manpages.ubuntu.com/manpages/resolute/man1/nfstest_cache.1.html), [nfstest_interop](https://manpages.ubuntu.com/manpages/noble/man1/nfstest_interop.1.html).
- **[УСТАРЕЛО]** pjdfstest на NFS (баги Red Hat по Ganesha): в 2014 году прогон зависал на тесте link; в 2016 году на NFSv4 падения, хотя на gluster NFS(v3) тот же набор проходил, — «differences in what passes between NFS v3 and NFS v4» — [RHBZ 1114542](https://bugzilla.redhat.com/show_bug.cgi?id=1114542), [RHBZ 1357750](https://bugzilla.redhat.com/show_bug.cgi?id=1357750), [pjd-fstest](https://github.com/tuxera/pjd-fstest).
- Bake-a-thon — интероп-события для NFSv4.0/4.1/pNFS. Весна 2025: Oracle, Ann Arbor, 12–16 мая, очно плюс VPN. 2022 и 2023 прошли виртуально. Исторически их проводили CITI (UMich) и Red Hat (Westford) — [bakeathon-announce](https://groups.google.com/g/bakeathon-announce/c/dUWm1orn-vQ), [lore (Spring 2023)](https://lore.kernel.org/all/901dd325-ba62-e327-571f-e3897130f835@redhat.com/T/), [UMich](https://eecs.engin.umich.edu/event/nsfv4-bake-a-thon/).
- Без внешних наборов:
  - go-nfs: CI = `go build` + `go test` + CodeQL — [go.yml](https://github.com/willscott/go-nfs/blob/master/.github/workflows/go.yml);
  - unfs3: только сборка на трёх ОС — [build.yml](https://github.com/unfs3/unfs3/blob/master/.github/workflows/build.yml);
  - Buildbarn: юнит-тесты `nfs40_program_test.go`/`nfs41_program_test.go`, NFSv4 на Linux включён ради проверки переносимости — [nfsv4](https://github.com/buildbarn/bb-remote-execution/tree/main/pkg/filesystem/virtual/nfsv4), [0b3dad9](https://github.com/buildbarn/bb-remote-execution/commit/0b3dad9).
- nfs-mamont (поиск по репозиторию 2026-10-30): упоминаний cthon/pynfs/nfstest/pjdfstest/fstests нет. Workflows: «Lint and Format», «Build and Test», «Security Audit», «Build Documentation», «Check Unused Dependencies», `fuzz.yml`, `release.yml` — [nfs-mamont](https://github.com/RMamonts/nfs-mamont).

### Inferences
- Убедительный набор для Rust-библиотеки уровня NFSv3 (сейчас):
  - cthon04 basic/general/special через Linux-клиент в CI (`-o vers=3,nolock` до появления NLM, затем группа lock);
  - pjdfstest и fstests `generic` через ядерный клиент (FSTYP=nfs);
  - `nfstest_cache` и `nfstest_posix` для семантики;
  - fuzzing XDR/RPC (у nfs-mamont уже есть);
  - smoke-тесты монтирования с macOS-клиентом (и Windows, если он заявлен).
- С приходом NFSv4 — pynfs 4.0 и 4.1 в CI по образцу nfs4j, с явным списком исключений.
- Публиковать список «known failures», как Ganesha и nfs4j, — признак зрелости. Скрытые исключения подрывают доверие.

### Gaps
- Анонсов Bake-a-thon на 2026 год не найдено.
- Работает ли `nfstest_posix`/`nfstest_lock` против NFSv3, по найденным страницам не подтверждено.
- Официальные вики-страницы linux-nfs.org (pynfs, Connectathon) недоступны из сессии.

## 7. Задокументированные подводные камни user-space NFS-серверов

### Takeaway
Почти все документированные проблемы сводятся к трём вещам: (1) стабильность и время жизни файловых дескрипторов (при рестарте, rename, хардлинках, без FORGET); (2) состояние, которое сервер обязан помнить «вечно» (cookieverf, writeverf, seqid и open-owner в v4); (3) клиентские причуды (macOS, Windows, отсутствие NLM, rpcbind и привилегированные порты, silly rename, кэш атрибутов).

### Cited Findings
- **Стабильность файловых дескрипторов.**
  - Ядро требует, чтобы ссылка на файл была «stable across renames, truncates, and server-reboot» — [exporting.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/exporting.rst).
  - Buildbarn: длина дескриптора 0–128 байт в v4 и 0–64 в v3. Сервер «has absolutely no information on when a client is going to stop interacting with a file handle» — в NFS нет аналога FUSE FORGET. Для бесконечных каталогов динамическая выдача дескрипторов ведёт к утечке памяти, поэтому состояние (digest) кодируется прямо в дескриптор (resolvable objects) — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).
  - rclone: кэш дескрипторов `memory` теряется при рестарте, и клиенты получают «stale handle errors». `disk` переживает рестарт. `symlink` работает только на Linux и требует `CAP_DAC_READ_SEARCH`. `--nfs-cache-handle-limit` по умолчанию 1000000 — [nfs.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/nfs.go). Раньше `HandleLimit = -1` ломал запись из Finder — [rclone PR #7254](https://github.com/rclone/rclone/pull/7254).
  - go-nfs: дескрипторы непрозрачны, хардлинки «will not work as expected» — [README](https://github.com/willscott/go-nfs/blob/master/README.md). Issue #127 «Symbolic Links Cause Stale NFS File Handles» закрыт 2024-02-16 — [#127](https://github.com/willscott/go-nfs/issues/127). Коммит «Invalidate a file's handle when NFSv4 RENAME or REMOVE retires its path» (2026-09-25) — [66688be](https://github.com/willscott/go-nfs/commit/66688be).
  - unfs3: после rename между каталогами дескриптор становится stale. Опция `-b` (brute force поиск) даёт «huge performance impact». Семантика `nohide` может давать одинаковые inode в одной видимой ФС — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8). В 0.10.0: «fix stale handle when moving files or directories» — [NEWS](https://github.com/unfs3/unfs3/blob/master/NEWS).
  - Предсказуемые дескрипторы позволяют обходить права каталогов (угадывание дескриптора), против этого в ядре сделан `sign_fh` — [exporting.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/exporting.rst).
- **Стабильность cookie в READDIR.**
  - RFC 1813 §3.3.16: в первом запросе `cookieverf` равен 0, далее он «must match that returned by the READDIR in which the cookie was acquired» — [RFC 1813](https://www.rfc-editor.org/rfc/rfc1813#section-3.3.16).
  - go-nfs сравнивает верификатор и отвечает `NFS3ERR_BAD_COOKIE`, cookie — это индекс + 2. Снимки листингов кэшируются по верификатору (`VerifierFor`/`DataForVerifier`, отсортировано по имени) — [nfs_onreaddir.go](https://github.com/willscott/go-nfs/blob/master/nfs_onreaddir.go), [handler.go](https://github.com/willscott/go-nfs/blob/master/handler.go).
  - nfs4j передаёт `verifier`/`cookie` прямо в `VirtualFileSystem.list` и `directoryVerifier` — [VirtualFileSystem.java](https://github.com/dCache/nfs4j/blob/master/core/src/main/java/org/dcache/nfs/vfs/VirtualFileSystem.java).
  - Buildbarn: в FUSE READDIR stateful, в NFS — stateless, поэтому пагинацию пришлось опустить в реализацию каталога (`VirtualReadDir` с начальным offset) — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).
  - unfs3 `-3`: усечение fileid и cookie до 32 бит для старых 32-битных клиентов (EOVERFLOW) — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8).
- **Write verifier, COMMIT и exclusive create.**
  - RFC 1813: verf позволяет клиенту «detect server reboots» и переотправить UNSTABLE-данные, не прошедшие COMMIT — [RFC 1813 §3.3.7](https://www.rfc-editor.org/rfc/rfc1813#section-3.3.7).
  - nfs4j моделирует это через `StabilityLevel`, `WriteResult` и `commit()` — [VirtualFileSystem.java](https://github.com/dCache/nfs4j/blob/master/core/src/main/java/org/dcache/nfs/vfs/VirtualFileSystem.java).
  - unfs3 хранит верификатор CREATE EXCLUSIVE в mtime/atime, и локальные процессы могут его испортить — [unfsd.8 BUGS](https://github.com/unfs3/unfs3/blob/master/unfsd.8). В 0.11.0 исправлен неверный mode при exclusive create — [NEWS](https://github.com/unfs3/unfs3/blob/master/NEWS).
- **Кэширование и консистентность атрибутов.**
  - Buildbarn задаёт длительность кэша атрибутов при монтировании, потому что «NFSv4 provides no facilities for conveying this on a per GETATTR response basis» — [nfsv4_mount_darwin.go](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/filesystem/virtual/configuration/nfsv4_mount_darwin.go). CB_NOTIFY для инвалидирования каталогов «rarely implemented by clients» — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).
  - Ganesha: pynfs WRT18 «consecutive SETATTR(mode)'s don't all change change attribute» — [CONTRIBUTING_HOWTO.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/CONTRIBUTING_HOWTO.txt). В V8.0 добавлено «Support disabling dentry caching for export» — [V8.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V8.0).
  - macOS-клиент через NFS меняет atime и mtime только вместе — [rclone_mount.md](https://github.com/rclone/rclone/blob/master/docs/content/commands/rclone_mount.md).
- **rpcbind и привилегированные порты.**
  - go-nfs: portmap — «the only part that needs a privileged listening port». Обход — опции монтирования `port=n,mountport=n` — [README](https://github.com/willscott/go-nfs/blob/master/README.md).
  - unfs3: `-p` отключает регистрацию в portmap/rpcbind, `-u` включает непривилегированные порты — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8).
  - Ganesha: опции `RPCBIND` и `_NO_TCP_REGISTER`, `PrivilegedPort` по умолчанию false — [CMakeLists.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/CMakeLists.txt), [config.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/config_samples/config.txt).
  - nfs4j: `withAutoPublish()` — [README](https://github.com/dCache/nfs4j/blob/master/README.md).
  - Yocto: работа без rpcbind при заранее известных портах; была ошибка вычисления портов, из-за которой конфликтовали несколько экземпляров QEMU — [poky (atom)](https://git.enea.com/cgit/linux/poky.git/atom/scripts/runqemu-export-rootfs?h=python3).
  - OpenBSD `mount_nfs` не понимает `mountport=`, нужен `-T` — [nfsmount.go](https://github.com/rclone/rclone/blob/master/cmd/nfsmount/nfsmount.go).
  - macOS по умолчанию ходит с непривилегированного порта, отсюда `resvport` — [mount_nfs(8)](https://www.unix.com/man-page/osx/8/MOUNT_NFS). pynfs требует от сервера `insecure` — [зеркало README pynfs](https://awesome.ecosyste.ms/projects/github.com%2Fvast-data%2Fpynfs).
- **Отсутствие NLM.**
  - unfs3 предлагает клиентам `nolock` — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8).
  - Для Windows рекомендуют `nolock` — [HPE](https://docs.ezmeral.hpe.com/datafabric-customer-managed/80/AdministratorGuide/MountingNFSonWindowsClient.html).
  - На macOS есть `nolocks`/`locallocks`; при нескольких клиентах `nolocks` рискует повреждением данных — [mount_nfs(8)](https://www.unix.com/man-page/osx/8/MOUNT_NFS), [inventivehq](https://inventivehq.com/knowledge-base/macos/how-to-mount-nfs-share-macos).
  - Ganesha, напротив, держит NLM и в 2026 году интегрировал NSM — [V11.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V11.0).
- **Silly rename.** Клиенты переименовывают удаляемые открытые файлы в `.nfs*`. Это даёт ENOTEMPTY при удалении каталогов. Для macOS NFSv4.0 Buildbarn рекомендует regex `^\.nfs\.[0-9a-f]{8}\.[0-9a-f]{4}$`. В NFSv4.1 silly rename не нужен благодаря `OPEN4_RESULT_PRESERVE_UNLINKED` — [virtual.proto](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/proto/configuration/filesystem/virtual/virtual.proto).
- **Сложность состояния NFSv4.0.**
  - Глава 9 RFC 7530 — «about 40 pages». В протоколе нет механизма удаления неиспользуемых open-owner и client records, поэтому нужна авто-очистка — [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md).
  - Детекция ретрансмиссий только по seqid и типу операции опасна при багах клиента, поэтому Buildbarn сравнивает и аргументы — [b1a8462](https://github.com/buildbarn/bb-remote-execution/commit/b1a8462).
  - Рекомендованные значения lease: `enforced_lease_time` 120s, анонсируемое значение 60s — [virtual.proto](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/proto/configuration/filesystem/virtual/virtual.proto).
- **Ошибки на уровне RPC и границы ресурсов.**
  - В go-nfs из-за сдвига на единицу в значениях `accept_stat` (не было `PROG_MISMATCH`) macOS-клиенты сообщали «RPC struct is bad». Issue #154 закрыт 2026-07-20 — [#154](https://github.com/willscott/go-nfs/issues/154).
  - go-nfs ограничивает переменные поля NFSv4 через `nfs4ArgLimit = 64 KiB`, чтобы испорченная длина не вызвала большую аллокацию — [nfs4.go](https://github.com/willscott/go-nfs/blob/master/nfs4.go).
  - Ganesha V15.9: «XDR: reject empty NLM caller_name and lock owner» и исправлен use-after-free сессии — [V15.9](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.9).
- **Отсутствие аутентификации.** rclone serve nfs «does not implement any authentication». По умолчанию он слушает loopback на случайном порту — [nfs.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/nfs.go).
- **Прочие клиентские проблемы.** go-nfs #54 «Could not edit Microsoft Office files from mountpoint» открыт с 2022-12-05 — [#54](https://github.com/willscott/go-nfs/issues/54). На macOS/FUSE-T «Operation not permitted» лечится разрешением Network Volumes — [FUSE-T](https://github.com/macos-fuse-t/fuse-t).

### Inferences
- Для nfs-mamont самые рискованные места: (а) персистентная и стабильная схема дескрипторов в трейте `Vfs` (rclone пришёл к disk-кэшу, Buildbarn — к кодированию состояния в дескриптор); (б) новый writeverf на каждый запуск процесса; (в) верификаторы каталогов, переживающие пагинацию; (г) документированные рецепты монтирования для macOS и Windows с `nolock`/`nolocks` и `port=/mountport=`.

### Gaps
- Свежего первоисточника по проблемам READDIR-cookie именно в macOS-клиенте не найдено.
- Список AppleDouble/`._*`-эффектов на NFS-монтированиях macOS не подтверждён источником.

## 8. Чек-лист production-readiness (для Rust-крейтов и nfs-mamont)

### Takeaway
Из практики Ganesha, nfsd, nfs4j, go-nfs, Buildbarn и unfs3 выводится чек-лист из 8 блоков. Встраиваемая NFSv3-библиотека считается «готовой к продакшену», если закрыты пункты «минимум». Пункты «полная планка» отделяют её от Ganesha и nfsd. По README на 2026-10-30 nfs-mamont закрывает протокольную базу v3, CI, fuzzing и аудит, но не закрывает NLM/NSM, внешние наборы соответствия и задокументированные клиентские рецепты.

### Cited Findings
- Планка протокола: Ganesha (v3/v4.x, NLM, NSM, NFSACL, RQUOTA) — [src/Protocols](https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/Protocols); nfsd — [Kconfig](https://github.com/torvalds/linux/blob/master/fs/nfsd/Kconfig); встраиваемые библиотеки — [go-nfs PR #163](https://github.com/willscott/go-nfs/pull/163), [nfs4j README](https://github.com/dCache/nfs4j/blob/master/README.md).
- Планка безопасности: SecType `sys/krb5/krb5i/krb5p`, `XprtSec tls/mtls`, squash-опции — [config.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/config_samples/config.txt); RPC-with-TLS в ядре с 6.4 — [Phoronix](https://www.phoronix.com/news/Linux-6.4-NFSD-RPC-With-TLS); `sign_fh` — [exporting.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/exporting.rst); squash-опции в unfs3 — [README](https://github.com/unfs3/unfs3/blob/master/README.md).
- Планка тестирования: cthon04 + pynfs перед каждым патчем у Ganesha — [CONTRIBUTING_HOWTO.txt](https://github.com/nfs-ganesha/nfs-ganesha/blob/next/src/CONTRIBUTING_HOWTO.txt); pynfs в CI у nfs4j — [build.yml](https://github.com/dCache/nfs4j/blob/master/.github/workflows/build.yml); kdevops (fstests/pynfs/nfstest/gitr/ltp) и soak в `nfsd-testing` — [nfsd-maintainer-entry-profile.rst](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/nfsd-maintainer-entry-profile.rst).
- Планка наблюдаемости: Prometheus (Ganesha V8.0), gRPC-статистика (V12–V15), учёт памяти (V15.0) — [V15.0](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.0); `metrics_program.go` в Buildbarn — [nfsv4](https://github.com/buildbarn/bb-remote-execution/tree/main/pkg/filesystem/virtual/nfsv4); отладочное логирование Ganesha описано в вики — [wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki); в Ganesha есть LTTng-трассировки, в том числе tracepoints для refcount state-объектов (2026-04-10) — [9308bfff](https://github.com/nfs-ganesha/nfs-ganesha/commit/9308bfff), [cd20f69a](https://github.com/nfs-ganesha/nfs-ganesha/commit/cd20f69a).
- Планка эксплуатации:
  - перечитывание конфигурации по `SIGHUP` и дерегистрация в portmapper по `SIGTERM` (unfs3) — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8);
  - авто-размонтирование при завершении — [98bce71](https://github.com/buildbarn/bb-remote-execution/commit/98bce71);
  - grace и HA через haproxy/keepalived (Ceph + Ganesha) — [ceph nfs.rst](https://github.com/ceph/ceph/blob/main/doc/mgr/nfs.rst).
- Планка документации и API-политики:
  - явное «API unstable/beta» и `API-changes.md` — [nfs4j README](https://github.com/dCache/nfs4j/blob/master/README.md);
  - разделы RESTRICTIONS/BUGS в man — [unfsd.8](https://github.com/unfs3/unfs3/blob/master/unfsd.8);
  - рецепты монтирования для Linux/macOS — [go-nfs README](https://github.com/willscott/go-nfs/blob/master/README.md), [rclone nfs.go](https://github.com/rclone/rclone/blob/master/cmd/serve/nfs/nfs.go);
  - `SECURITY.md` — [go-nfs](https://github.com/willscott/go-nfs).
- Планка поддержки:
  - каналы (Matrix, рассылки, еженедельные созвоны) — [Ganesha wiki](https://github.com/nfs-ganesha/nfs-ganesha/wiki);
  - профиль мейнтейнера с ролями и правилами — [nfsd profile](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/nfs/nfsd-maintainer-entry-profile.rst);
  - DCO — [nfs4j README](https://github.com/dCache/nfs4j/blob/master/README.md).
- Статус nfs-mamont по README: NFSv3 (полный набор RFC 1813, заявлено), MOUNT, NLM/NSM/NFSv4 в roadmap, MSRV 1.75 (CI на 1.75 и stable), только Linux, MIT, `tracing` без глобального subscriber. Workflows: lint, build/test, security audit, docs, unused deps, fuzz, release — [nfs-mamont](https://github.com/RMamonts/nfs-mamont).

### Inferences
Чек-лист. «Мин.» — минимум для встраиваемой библиотеки уровня go-nfs/nfs4j, «Полн.» — планка Ganesha/nfsd. В колонке nfs-mamont статус по README и CI на 2026-10-30; «?» означает «не заявлено, не проверялось».

| # | Блок / пункт | Уровень | Откуда планка | nfs-mamont |
|---|---|---|---|---|
| P1 | NFSv3 (все процедуры RFC 1813, включая READDIRPLUS) + MOUNT v3 | Мин. | unfs3 (без READDIRPLUS — минус), go-nfs, nfs4j | заявлено |
| P2 | NLMv4 + NSM или явная документация `nolock`/`nolocks`/`locallocks` | Мин. (док.), Полн. (NLM) | Ganesha NLM + integrated NSM; unfs3 man | roadmap; док.: ? |
| P3 | NFSv4.0 (минимум для macOS-v4), затем 4.1 (сессии, без silly rename) | Полн. для библиотеки 2026 | go-nfs master, Buildbarn, nfs4j | roadmap |
| P4 | NFSv4.2 (COPY, SEEK, ALLOCATE, xattrs) и pNFS | Полн. | nfsd, частично nfs4j и Ganesha | нет |
| P5 | Регистрация в rpcbind опциональна, фиксированные порты, MOUNT и NFS на одном порту | Мин. | unfs3 `-p/-n/-m`, go-nfs README, Yocto | ? |
| S1 | Стабильные и персистентные дескрипторы (переживают рестарт и rename), hooks инвалидации, лимит | Мин. | exporting.rst, rclone disk-кэш, Buildbarn HandleAllocator | ? (зависит от `Vfs`) |
| S2 | cookieverf/cookie переживают пагинацию, `BAD_COOKIE` при рассинхроне, опция 32-битных cookie/fileid | Мин. | RFC 1813 §3.3.16, go-nfs, nfs4j, unfs3 `-3` | ? |
| S3 | writeverf новый на каждый старт процесса, корректные UNSTABLE/COMMIT, exclusive-create verifier | Мин. | RFC 1813 §3.3.7, nfs4j `StabilityLevel`, unfs3 BUGS | ? |
| S4 | Корректные wcc_data и change-атрибут, ctime/mtime, рекомендации по `actimeo` | Мин. | pynfs WRT18, nfstest_cache, Buildbarn attrcache | ? |
| S5 | Хардлинки, symlink, special files, uid/gid, squash | Мин. | go-nfs `UnixChange`, unfs3 squash | ? |
| SEC1 | AUTH_SYS + root_squash/all_squash + ACL экспорта по IP/сети | Мин. | unfs3, Ganesha | ? |
| SEC2 | Безопасные значения по умолчанию: bind на loopback, без аутентификации — только с явным предупреждением | Мин. | rclone serve nfs | ? |
| SEC3 | RPCSEC_GSS krb5/krb5i/krb5p | Полн. | Ganesha, nfsd, nfs4j | нет (не заявлено) |
| SEC4 | RPC-with-TLS (RFC 9289), kTLS | Полн. | nfsd ≥ 6.4, Ganesha V10.0 | нет (не заявлено) |
| SEC5 | Fuzzing XDR/RPC, лимиты длин и аллокаций, защита от угадывания дескрипторов | Мин. (fuzz, лимиты), Полн. (sign_fh) | go-nfs `nfs4ArgLimit`, Ganesha V15.9, `sign_fh` | fuzz есть; остальное ? |
| T1 | cthon04 basic/general/special (+lock с NLM) через Linux-клиент в CI | Мин. | Ganesha CONTRIBUTING | нет |
| T2 | pjdfstest и fstests `generic` (FSTYP=nfs) через ядерный клиент | Мин. → Полн. | kdevops, RHBZ по Ganesha | нет |
| T3 | pynfs 4.0 и 4.1 с публичным списком исключений (после v4) | Полн. | nfs4j CI, Ganesha | н/п |
| T4 | Интероп-матрица: Linux v3/v4, macOS (14.4+/15), Windows v3; Bake-a-thon для «полной» планки | Мин. (smoke) / Полн. | nfsd profile, Buildbarn, MS docs | ? |
| T5 | Статический анализ, аудит зависимостей, MSRV в CI | Мин. | Ganesha Coverity/checkpatch | есть (audit, MSRV, lint) |
| O1 | Метрики по операциям (latency, ошибки), Prometheus-friendly | Мин. → Полн. | Ganesha V8.0, Buildbarn `metrics_program.go` | ? (`tracing` есть) |
| O2 | Структурированные логи/трейсинг с уровнем запроса | Мин. | Ganesha debug logging, lttng-трейсы | `tracing` есть |
| OPS1 | Graceful shutdown, размонтирование, дерегистрация rpcbind | Мин. | unfs3 SIGTERM, Buildbarn auto-unmount | ? |
| OPS2 | Лимиты ресурсов (handles, state, память), учёт памяти | Мин. → Полн. | rclone handle-limit, Ganesha V15.0 | частично (allocator) |
| OPS3 | Grace period, восстановление состояния, HA | Полн. | Ceph + Ganesha | нет |
| D1 | Рецепты монтирования по ОС (Linux/macOS/Windows), включая `port=/mountport=`, `nolock`, `resvport` | Мин. | go-nfs, rclone, unfs3 | нет в README |
| D2 | Разделы «Restrictions/Known bugs», совместимость клиентов | Мин. | unfsd.8 | ? |
| D3 | Политика стабильности API (semver, changelog API) | Мин. | nfs4j API-changes.md | v0.2.0, ? |
| SUP1 | Каналы поддержки, SECURITY.md, больше одного мейнтейнера, регулярные релизы | Мин. | go-nfs SECURITY.md, Ganesha wiki | частично (CONTRIBUTING, Kanban) |
| SUP2 | Поддержка платформ сервера (macOS для сценария «как rclone»), кросс-компиляция | Полн. для desktop-сценария | go-nfs, unfs3, Buildbarn | только Linux |

### Gaps
- Статусы «?» требуют проверки кода nfs-mamont: дескрипторы, верификаторы, AUTH-флейворы, метрики. Это намеренно вне рамок данной заметки.
- Количественных порогов (например, «≥ X% cthon04 pass») в найденных источниках нет. Ganesha и nfs4j публикуют списки исключений, но не процентные цели.

## 9. Технологии-заменители (кратко)

### Takeaway
NFS выбирают в основном там, где нет хорошего FUSE (macOS) или где нужен сетевой доступ без клиентского ПО. Внутри одной Linux-машины выигрывает FUSE, для ВМ — virtiofs, для Windows — WinFSP или SMB.

### Cited Findings
- Buildbarn рекомендует FUSE на Linux: он «performs better» и аккуратнее переживает остановку сервера. На macOS FUSE удалён в пользу NFSv4, так как macFUSE «has a very high probability of locking up your system» и закрыт (не open source). Для Windows добавлен бэкенд WinFSP — [virtual.proto](https://github.com/buildbarn/bb-remote-execution/blob/main/pkg/proto/configuration/filesystem/virtual/virtual.proto), [9791c09](https://github.com/buildbarn/bb-remote-execution/commit/9791c09), [ADR 0009](https://github.com/buildbarn/bb-adrs/blob/main/0009-nfsv4.md), [efef252](https://github.com/buildbarn/bb-remote-execution/commit/efef252).
- rclone поддерживает на macOS три пути — встроенный NFS-сервер, macFUSE (kext) и FUSE-T (NFSv4 local server). На Windows используется WinFsp. Кроме того, rclone умеет отдавать данные по WebDAV, SFTP, FTP, S3, HTTP и DLNA (`rclone serve ...`) — [rclone_mount.md](https://github.com/rclone/rclone/blob/master/docs/content/commands/rclone_mount.md), [docs/content/commands](https://github.com/rclone/rclone/tree/master/docs/content/commands).
- FUSE-T, начав с NFSv4, добавил SMB- и FSKit-бэкенды как «NFS alternatives» — [FUSE-T](https://github.com/macos-fuse-t/fuse-t).
- ВМ: активная разработка virtio-9p прекратилась в 2012 году. virtio-fs (протокол FUSE поверх virtio) задуман как замена — [KVM Forum 2019](https://kvm-forum.qemu.org/2019/virtio-fs_%20A%20Shared%20File%20System%20for%20Virtual%20Machines.pdf). **[УСТАРЕЛО]** В тесте 2020 года 9p (VirtFS) показал примерно половину производительности NFS — [etbe blog](https://etbe.coker.com.au/2020/09/23/qemu-kvm-9p/).
- Ganesha удалил 9P в V15.9, так как он «not used» — [V15.9](https://github.com/nfs-ganesha/nfs-ganesha/releases/tag/V15.9).

### Inferences
- Короткий абзац для отчёта. Проекты уходят от NFS к FUSE (на Rust — fuser) на Linux, где FUSE быстрее и даёт точные инвалидации (FORGET и NOTIFY_INVAL, которых нет в NFS). К virtiofs уходят для ВМ, к SMB/WinFSP — для Windows. WebDAV, SFTP и S3 выбирают там, где не нужна POSIX-семантика, но нужна работа без root и через HTTP-инфраструктуру. NBD — когда хватает блочного устройства. NFS остаётся лучшим выбором для «смонтировать без драйверов» на macOS (kext-less) и в разнородных сетях; ровно эту нишу занимают go-nfs/rclone, FUSE-T и Buildbarn.

### Gaps
- Отдельных первоисточников о выборе NBD или WebDAV вместо NFS для встраиваемых сценариев не найдено. Упоминание NBD — без источника (вывод).
