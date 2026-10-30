# Аудит nfs-mamont: техническое состояние и здоровье проекта (база для сравнения с другими Rust-крейтами NFS)

Срез кода: ветка `main`, коммит `5eae9a5` от 2026-10-29 (https://github.com/RMamonts/nfs-mamont/tree/5eae9a536f4ee82ff79c49366f0e9a62d3841b7f). Все ссылки вида `nfs-mamont/src/…:строки` относятся к этому коммиту (локальная копия /home/user/nfs-mamont). Метрики GitHub и crates.io сняты 2026-10-30 через REST API: GraphQL из сессии недоступен, при пагинации `--paginate` прокси блокировал ссылки на `repositories/{id}`, поэтому страницы перебирались вручную. docs.rs и Kanban-доска организации из этой сессии недоступны (сетевые ограничения). Расчёты по PR и issue выполнены собственным скриптом по выгрузкам API.

## 1. Архитектура: модули, путь запроса, модель конкурентности, фрейминг

### Takeaway
nfs-mamont построен как конвейер tokio-задач без собственного протокольного «ядра». На каждое соединение приходятся `ReadTask` и `WriteTask`. Над ними работают глобальный пул `VfsTask` (размер задаёт пользователь), одна `MountTask` и одна `NlmTask`, а связывают всё неограниченные каналы `async_channel`. Record marking и сброс фрейма при ошибках сделаны аккуратно. Чего нет: сборки многофрагментных записей, ограниченных очередей (backpressure работает только через семафор аллокатора), таймаутов и лимитов соединений, программного graceful shutdown.

### Cited Findings
- **Публичная поверхность крейта.** Приватны `allocator`, `backend`, `context`, `nlm` (с `#[allow(dead_code)]`), `parser`, `rpc`, `rpcbind`, `serializer`, `task`. Публичны `consts`, `mount`, `service`, `vfs`. Реэкспортируются `Allocator, Buffer, Impl, Slice, UnownedBuffer, BackendRegistry, ServerContext, auth, BackendId`. Источник: `nfs-mamont/src/lib.rs:3-16, 36-40`.
- **Объём.** Около 15,2 тыс. строк Rust в `nfs-mamont/src`, включая тесты; 180 нетестовых `.rs`-файлов (подсчёт `wc -l`/`find` по локальному клону). Прямых зависимостей девять: `async-channel`, `byteorder`, `crossbeam-queue`, `num-derive`, `num-traits`, `tokio` (features `rt, net, io-util, sync, time, macros, signal`), `tracing`, `trait-variant` и опциональный `libc`. В `Cargo.lock` 35 пакетов. Источник: `Cargo.toml:11-20`, `nfs-mamont/Cargo.toml:18-32`, `Cargo.lock`.
- **Модули по ролям:**
  - `parser/`: `FrameReader` (record marking и «головное окно» фрейма), `RpcParser` (заголовок RPC, аутентификация, аргументы NFSv3/MOUNT/NLM), XDR-примитивы, отдельный `parser::Error`. Источник: `nfs-mamont/src/parser/parser_struct.rs:1-18`, `parser/read_buffer.rs:1-17`, `parser/error.rs:1-28`.
  - `serializer/`: `server::serialize_struct::Serializer` с staging-буфером `WriteBuffer` и пер-процедурные сериализаторы. `serializer::client` собирается только под `#[cfg(test)]` для round-trip-тестов. Источник: `nfs-mamont/src/serializer/mod.rs:13-22`.
  - `task/`: `connection::{read, write}` и `global::{vfs, mount, nlm}`. Источник: `nfs-mamont/src/task/mod.rs:12-26`, `task/global/mod.rs:1-12`.
  - `backend/`: `BackendRegistry<V>` = `Arc<RwLock<HashMap<BackendId, Arc<V>>>>`. Источник: `nfs-mamont/src/backend/mod.rs:17-25`.
  - `context.rs`: `ServerContext<A, V, B>` объединяет `VfsManager`, общий аллокатор и реестр бэкендов. Источник: `nfs-mamont/src/context.rs:42-55`.
- **Старт сервера.** `handle_forever(listener: TcpListener, context, mount_service: Arc<M>, nlm_service: Arc<N>)` запускает `MountTask` и `NlmTask`, регистрирует NFS/MOUNT/NLM в локальном rpcbind (при неудаче пишет warning), затем крутит цикл `accept`. Источник: `nfs-mamont/src/lib.rs:43-101`.
- **Новое соединение.** Выполняются `set_nodelay(true)` и `into_split()`, создаётся неограниченный канал ответов `async_channel::unbounded::<ProcReply<B>>()`, запускаются `ReadTask` и `WriteTask`. Источник: `nfs-mamont/src/task/connection/mod.rs:26-60`.
- **`ReadTask`.**
  - В цикле вызывает `RpcParser::next_message()`. NULL для NFS, NLM и MOUNT отвечается сразу, без пула. NFS-вызов уходит в пул VFS вместе с клоном `result_sender`, MOUNT — в `MountTask`, NLM — в `NlmTask`.
  - Записи-REPLY молча отбрасываются (`MessageTypeMismatch`). На ошибку разбора с известным xid отправляется ответ-ошибка. Если xid неизвестен (включая EOF), соединение закрывается.
  - Источник: `nfs-mamont/src/task/connection/read.rs:89-217`.
- **Пул VFS.**
  - `VfsManager::new` запускает `concurrency` воркеров `VfsTask`, которые конкурируют за один неограниченный MPMC-канал. Документ прямо называет размер пула «backend queue depth». Источник: `nfs-mamont/src/task/global/vfs.rs:140-187`.
  - `dispatch` маршрутизирует по первому байту handle. Если RENAME или LINK затрагивают два разных бэкенда, возвращается `XDev`; если бэкенд не подключён — `StaleFile`. Источник: `nfs-mamont/src/task/global/vfs.rs:32-67, 246-285`.
- **`WriteTask` и ответы.**
  - Для каждого `ProcReply` вызывается `Serializer::form_reply`. Верификатор ответа всегда `AUTH_NONE` (TODO на issue #143). Ошибка сериализации или записи логируется, и цикл продолжается. Источник: `nfs-mamont/src/task/connection/write.rs:36-56`.
  - Ответы идут в порядке завершения, то есть несколько запросов одного соединения выполняются одновременно и могут получить ответы не по порядку (следует из архитектуры каналов, см. файлы выше).
- **Глобальные задачи.** `MountTask` и `NlmTask` — по одной на сервер, запросы обрабатываются строго последовательно. Источник: `nfs-mamont/src/task/global/mount.rs:57-125`, `task/global/nlm/nlm_task.rs:68-80`.
- **Record marking.**
  - Читается 4-байтовый заголовок, размер берётся из младших 31 бита. `begin_body` буферизует `min(size, capacity)` байт. Синхронный `Read` не выходит за границу фрейма, а байты следующего фрейма остаются в буфере (read-ahead). Источник: `nfs-mamont/src/parser/read_buffer.rs:1-17, 106-125, 184-197`, `parser/parser_struct.rs:168-191`.
  - Головное окно по умолчанию `DEFAULT_SIZE = 2500` байт. Все аргументы, кроме данных WRITE, обязаны в него поместиться, иначе ошибка `InvalidData`. Источник: `nfs-mamont/src/parser/parser_struct.rs:55-63`.
- **Многофрагментные записи не поддерживаются.** Запись без бита last-fragment даёт ошибку `Unsupported` ещё до чтения xid. В результате остаток фрейма отбрасывается и соединение закрывается (ветка `xid: None`). Источник: `nfs-mamont/src/parser/parser_struct.rs:181-188`, `task/connection/read.rs:210-214`. TODO ссылается на https://github.com/RMamonts/nfs-mamont/issues/124 (открыт с 2026-03-12).
- **Сброс фрейма при любой ошибке.** Любая ошибка разбора проходит через `match_errors`, который вызывает `discard_rest_of_frame`. Лишние байты в конце фрейма отклоняются (`finalize_parsing`). Версия программы проверяется раньше процедуры. Источник: `nfs-mamont/src/parser/parser_struct.rs:375-419, 428-489, 491-536`, `parser/read_buffer.rs:151-181`. Поведение введено PR #412 и #413 (коммиты `034aa70`, `86d710d`), игнорирование REPLY — PR #420 (`6398e2f`).
- **Ответ всегда одним фрагментом.** Если размер больше `MAX_FRAGMENT_SIZE` (2³¹−1), возвращается ошибка. Источник: `nfs-mamont/src/serializer/server/serialize_struct.rs:327-341`; https://github.com/RMamonts/nfs-mamont/issues/269 (label `bug`, открыт).
- **Backpressure.**
  - Он есть только у аллокатора: `allocate` ждёт освобождения буферов на `tokio::sync::Semaphore`, а вызывается он из `ReadTask` для READ и WRITE. Источник: `nfs-mamont/src/allocator/mod.rs:68-94, 151-179`, `parser/parser_struct.rs:562-665`.
  - Все межзадачные каналы неограниченные. Источник: `nfs-mamont/src/task/connection/mod.rs:9, 46`, `task/global/vfs.rs:174`, `task/global/mount.rs:39`, `task/global/nlm/nlm_task.rs:47`.
  - Issue «Bounded channels» https://github.com/RMamonts/nfs-mamont/issues/134 и «limit async_channel to vfs_pool» https://github.com/RMamonts/nfs-mamont/issues/219 открыты.
- **Жизненный цикл соединений.** Idle-таймаута, TCP keepalive и лимита соединений нет. Issue «Closing tcp connection» открыт с 2025-10-06; в обсуждении мейнтейнер предпочёл keepalive, но это не реализовано (https://github.com/RMamonts/nfs-mamont/issues/3). При ошибке `accept` сервер повторяет попытку с паузой 50 мс, кроме EBADF, EINVAL и ENOTSOCK. Источник: `nfs-mamont/src/lib.rs:76-113`, `consts/socket.rs:1-11` (PR #415).
- **Shutdown.** Библиотека сама слушает `signal::ctrl_c()` (комментарий «TODO: will be replaced with other solution in future»). По сигналу выходит из `accept` и снимает регистрацию в rpcbind, но живые соединения не дренирует. Источник: `nfs-mamont/src/lib.rs:73-100`. Issue «Convenient server interface» (нужны start/stop, «`handle_forever` … is not great at all») открыт: https://github.com/RMamonts/nfs-mamont/issues/335.
- **Wiki расходится с кодом.** Wiki-страница архитектуры показывает тот же конвейер ReadTask → AsyncChannel → VfsPool → WriteTask плюс MountTask, но разделы модулей (Allocator, Parser, Serializer, Task, ReadTask, VfsTask, WriteTask, MountTask, NlmTask) содержат только «TODO!». Источник: https://github.com/RMamonts/nfs-mamont/wiki/NFS%E2%80%90Mamont-architecture (клон wiki, последняя правка 2026-06-07).

### Inferences
- Неограниченные очереди плюс общий для всех соединений пул буферов допускают отказ в обслуживании. Клиент, который шлёт READ и не читает ответы, удерживает буферы пула в своей очереди ответов: буфер READ живёт внутри `NfsRes::Read` до сериализации (`serialize_struct.rs:98-104`). Остальные соединения тогда ждут на семафоре. Поток мелких запросов (GETATTR) от такого клиента растит память без ограничений. Это вывод из кода, не наблюдение.
- Отказ от многофрагментных записей — нарушение RFC 5531 §11. На практике это, вероятно, редко мешает: ядро Linux отправляет однофрагментные записи. Последнее не проверялось в этом исследовании.
- Латентный дефект: ошибочные ветки сериализатора не сбрасывают staging-буфер. Макрос `nfs_result!` выходит по `?` до `send_inner_buffer`, а `clean()` вызывается только на успешных путях и в проверке `count` для READ (`serialize_struct.rs:38-52, 319-349, 361-367`). Если такая ошибка случится, следующий ответ в том же соединении уйдёт с «хвостом» предыдущего. Сейчас реальные триггеры почти недостижимы (ошибки записи в `Vec` и ответы больше 2 ГиБ), так что серьёзность низкая.
- Нарушение контракта READ бэкендом (`count > data.len()`) приводит к тому, что ответа нет вообще: ошибка только логируется в `WriteTask`. Клиент в этом случае ждёт RPC-таймаута. Контракт задокументирован в `vfs/read.rs:58-60`.
- Глобальная последовательная `NlmTask` сериализует все операции блокировок сервера.

### Gaps
- Поведение при реальной нагрузке с медленными клиентами не измерялось: статический анализ, запуск запрещён условиями задания.
- Не проверено, отправляют ли многофрагментные записи клиенты macOS и Windows.

## 2. Покрытие протоколов (верхнеуровнево)

### Takeaway
Утверждение README о «full procedure set of RFC 1813» верно на уровне провода и диспетчеризации: все 22 процедуры NFSv3 и все 6 процедур MOUNT v3 разбираются, доставляются в трейты и сериализуются, заглушек в библиотеке нет. Но вся семантика лежит на бэкенде. Часть протокольных ошибок уходит как RPC-ошибки (GARBAGE_ARGS/SYSTEM_ERR), а не как `nfsstat3`. NLMv4 реализован частично (5 синхронных процедур, без GRANTED-колбэков, `_MSG`/`_RES`, SHARE, NSM), хотя и регистрируется в rpcbind. Аутентификация — только AUTH_NONE и AUTH_SYS. Транспорт — только TCP. NFSv2, NFSv4 и NFS_ACL отсутствуют. Несколько экспортов поддерживается.

### Cited Findings
- **Заявление README:** «NFSv3 via the `NFS_PROGRAM` (100003) RPC program — full procedure set of RFC 1813». Дорожная карта: `[x] MOUNT`, `[ ] NSM`, `[ ] NLMv4`, `[ ] NFSv4`. Источник: `README.md:14, 63-69`.
- **NFSv3 «насквозь».** Все 22 процедуры (`NfsProc` 0–21) есть на каждом этапе: разбор `parse_nfs_proc`, исполнение `execute` (каждая вызывает метод `Vfs`), сериализация `process_nfs3`. Источник: `nfs-mamont/src/consts/nfsv3.rs:6-32`, `parser/parser_struct.rs:305-338`, `task/global/vfs.rs:287-319`, `serializer/server/serialize_struct.rs:80-157`. Принимается только версия 3; остальные получают `PROG_MISMATCH` с low=high=3 (`parser_struct.rs:433-444`).
- **MOUNT v3.** NULL, MNT, DUMP, UMNT, UMNTALL, EXPORT разбираются, обрабатываются и сериализуются. Источник: `nfs-mamont/src/consts/mount.rs:11-21`, `parser/parser_struct.rs:344-354`, `task/global/mount.rs:66-111`, `serialize_struct.rs:183-209`.
- **Готовый `MountService`:**
  - in-memory реестр экспортов (ключ — путь) и смонтированных клиентов, добавление и удаление экспортов на лету (`add_export`/`remove_export`);
  - MNT игнорирует `cred` и не проверяет клиента по `ExportEntry.names`, при вставке `names` всегда пустой;
  - в MNT-ответе рекламируется только `AUTH_NONE` (`const AUTH: [AuthFlavor; 1] = [AuthFlavor::None]` с «TODO: should be taken from config»).
  - Источник: `nfs-mamont/src/service/mount/mod.rs:37-38, 66-74, 93-125`, `service/mount/mnt.rs:14-48`.
- **Баг реестра монтирований.** Реестр индексируется полным `SocketAddr` (IP:порт), поэтому UMNT и UMNTALL с другого эфемерного порта не находят запись MNT, и DUMP копит «мёртвые» записи. Источник: `nfs-mamont/src/service/mount/mod.rs:85-90`, `service/mount/umnt.rs:9-19`, `service/mount/umntall.rs:9-13`. Исправление в открытом PR https://github.com/RMamonts/nfs-mamont/pull/423 (с 2026-10-22). Автор PR: «MNT, UMNT and UMNTALL from one host arrive over different TCP connections … DUMP accumulated stale mounts».
- **rpcbind.**
  - Крейт содержит только **клиент** portmapper v2. `PMAPPROC_SET`/`UNSET` уходят по TCP на `127.0.0.1:111` для 100003/3, 100005/3 и 100021/4 с одним портом. Маппинг только `IPPROTO_TCP`. Таймаут 5 с, откат частичной регистрации при ошибке, снятие регистрации при завершении. Источник: `nfs-mamont/src/rpcbind/mod.rs:1-13, 35-62, 96-116, 401-479`, `lib.rs:62-71, 98`.
  - Собственного portmapper нет. Если rpcbind в системе не запущен, клиент должен указать `port=`/`mountport=` (сообщение в `lib.rs:66-70`).
- **NLMv4.**
  - Разбираются NULL, TEST, LOCK, CANCEL, UNLOCK; других номеров процедур в `NlmProc` нет. Источник: `nfs-mamont/src/consts/nlm.rs:6-15`, `parser/parser_struct.rs:360-369`.
  - Модуль `nlm` приватный и помечен `#[allow(dead_code)]`. Трейт `Nlm` не реэкспортирован, но обязателен в сигнатуре `handle_forever`. Источник: `nfs-mamont/src/lib.rs:8-9, 24, 36-40, 43-55`.
  - Встроенный `NlmService` держит блокировки в `tokio::sync::RwLock<LockRegistry>`. Источник: `nfs-mamont/src/service/nlm/mod.rs:17-39`.
  - Заблокированный LOCK ставится в очередь `pending` (`Blocked`). При UNLOCK ожидающие блокировки «выдаются» без уведомления клиента: «TODO: Add client notification logic (#267)». Источник: `nfs-mamont/src/service/nlm/procedures/lock.rs:8-39`, `service/nlm/procedures/unlock.rs:6-29`, `service/nlm/lock_types.rs:45-50`.
  - EPIC «send NLMPROC4_GRANTED callback» https://github.com/RMamonts/nfs-mamont/issues/267, EPIC «nlm: callback support» https://github.com/RMamonts/nfs-mamont/issues/326, PR колбэка GRANTED https://github.com/RMamonts/nfs-mamont/pull/377 (открыт с 2026-10-06), «Implement async NLM procedures (MSG/RES pairs)» https://github.com/RMamonts/nfs-mamont/issues/287 — все открыты.
  - Релиз v0.1.0 прямо относит NLM и `NlmService` к «What is not ready». Источник: https://github.com/RMamonts/nfs-mamont/releases/tag/v0.1.0.
  - Wiki-гайд монтирует с `nolock`. Источник: https://github.com/RMamonts/nfs-mamont/wiki/MirrorFs-User-Guide.
- **NSM (statd, 100024) отсутствует**: в коде нет ни программы, ни констант; README: `[ ] NSM protocol`. Источник: `nfs-mamont/src/consts/mod.rs:1-6`, `README.md:67`.
- **Аутентификация.**
  - Принимаются `AUTH_NONE` (с пустым телом) и `AUTH_SYS`: `machinename` не длиннее 255, не более 16 `gids`. Любой другой flavor (`AUTH_SHORT`, `AUTH_DH`, `RPCSEC_GSS` — значения перечислены в `AuthFlavor`) отклоняется с `AUTH_BADCRED`. Верификатор запроса обязан быть пустым `AUTH_NONE`, иначе `AUTH_BADVERF`. Источник: `nfs-mamont/src/parser/parser_struct.rs:236-297`, `parser/rpc.rs:22-53`, `rpc/mod.rs:35-45`.
  - Разобранный `Credential` передаётся во все методы `Vfs`, а также в MNT, UMNT, EXPORT, DUMP и NLM. Источник: `nfs-mamont/src/rpc/auth.rs:77-93`, `vfs/read.rs:61`, `mount/mnt.rs:65-70`.
  - Issue «Authentification layer»: «Current implementation does not provide any permissions checks for any operations» — открыт: https://github.com/RMamonts/nfs-mamont/issues/323.
- **Транспорт: только TCP.** `handle_forever` принимает `TcpListener`, а rpcbind-маппинги — только TCP. Источник: `nfs-mamont/src/lib.rs:43-48`, `rpcbind/mod.rs:43-44, 77-94`. README демо mirror-fs: «Only TCP is supported (no UDP)» — https://github.com/RMamonts/mirror-fs (README.md:77-78).
- **Неизвестные программы получают `PROG_UNAVAIL`.** Ядро Linux при монтировании пробует NFS_ACL (100227) и LOCALIO (400122) и получает отказ. Сервер пишет это в лог как WARN + ERROR, а закрытие соединения клиентом — как ERROR «rpc parse error: xid=<none>». Источник: https://github.com/RMamonts/nfs-mamont/issues/430 (единственный внешний issue, 2026-10-24, комментариев 0 на 2026-10-30).
- **Несколько экспортов и бэкендов.**
  - До 256 бэкендов: `BackendId = u8` в первом байте handle. Бэкенды подключаются и отключаются во время работы. Источник: `nfs-mamont/src/vfs/file.rs:10-51`, `backend/mod.rs:27-57`, `context.rs:79-96`.
  - Экспортов в `MountService` может быть сколько угодно, каждый со своим root handle. Источник: `nfs-mamont/src/service/mount/mod.rs:40-83`.
  - EPIC «Multi backend support» открыт; в плане расширить handle до 12 байт ради id и generation и скрыть backend id от клиента: https://github.com/RMamonts/nfs-mamont/issues/351.
  - Опций экспорта (ro, root_squash, anon uid/gid, sync, ACL хостов) нет: https://github.com/RMamonts/nfs-mamont/issues/101 открыт.
- **Протокольные ошибки уходят как RPC-ошибки.**
  - Handle длиной не 9 байт даёт `BadFileHandle`, на проводе это `GARBAGE_ARGS`, а не `NFS3ERR_BADHANDLE`. Источник: `nfs-mamont/src/parser/nfsv3/file.rs:12-19`, `serializer/server/serialize_struct.rs:234-246`.
  - Имя с `/`, пустое имя или имя/путь сверх лимита дают `FormatMismatch` или `MaxElemLimit`, на проводе `SYSTEM_ERR` или `GARBAGE_ARGS`. Источник: `nfs-mamont/src/vfs/file.rs:67-81`, `parser/nfsv3/file.rs:62-70`, `parser/parser_struct.rs:68-85`, `serialize_struct.rs:280-285`.
- **Канонические тексты RFC в репозитории:** 1813, 7530 (NFSv4.0), 1833, 5531, 4506, 7531. Источник: каталог `rfc/`.

### Inferences
- «Полный набор RFC 1813» точнее описать так: полный XDR-слой и диспетчеризация. Логика вроде exclusive CREATE (verifier), стабильности READDIR-cookie, WCC и COMMIT-verifier целиком на бэкенде (см. раздел 3).
- Регистрация NLM в rpcbind при неполном NLM вводит клиентов в заблуждение: без `nolock` блокирующие блокировки не получат GRANTED, а блокировки упавших клиентов никогда не освободятся (NSM нет). Вывод из кода.
- Ответ `GARBAGE_ARGS` вместо `NFS3ERR_BADHANDLE`/`STALE` на «чужой» handle (например, после замены сервера) клиент, вероятно, увидит как EIO, а не как ESTALE. Поведение клиентов не проверялось.
- MNT-ответ со списком только из `AUTH_NONE` семантически заявляет «креды игнорируются», хотя сервер передаёт креды AUTH_SYS в VFS.

### Gaps
- Полную матрицу процедур по крейтам строит другой исследователь; здесь только верхний уровень.
- Не проверялось поведение реальных клиентов macOS, Windows и FreeBSD при отказе NFS_ACL/LOCALIO и при отсутствии NSM.
- Исправление переполнения диапазонов NLM из закрытого без слияния PR #320 (https://github.com/RMamonts/nfs-mamont/pull/320) отдельно в `range_ops.rs` не верифицировалось.

## 3. API трейта `Vfs`, обработка handle, ошибки, расширяемость

### Takeaway
`Vfs<B>` — объединение 21 async-трейта на `#[trait_variant::make(Send)]` (AFIT/RPITIT, отсюда MSRV 1.75). Методы без реализаций по умолчанию получают типизированные `Args` и `&Credential`, а возвращают `Result<Success, Fail>` с WCC-данными и `vfs::Error`, который один к одному соответствует `nfsstat3`. API типобезопасный и строгий, но тяжёлый для входа: 5 обобщённых параметров у `handle_forever`, один конкретный тип бэкенда на сервер, handle фиксирован в 9 байт (1 + 8). Часть нужных типов не экспортирована: `AuthFlavor`, трейт `Nlm`. Расширяемость дают публичные `Allocator`/`Buffer` и feature `mlock`.

### Cited Findings
- **Устройство трейта.** `pub trait Vfs<B: Buffer>` = `GetAttr + SetAttr + Lookup + Access + ReadLink + Read<B> + Write<B> + Create + MkDir + Symlink + MkNode + Remove + RmDir + Rename + Link + ReadDir + ReadDirPlus + FsStat + FsInfo + PathConf + Commit`, плюс blanket-impl. У самого `Vfs` нет doc-комментария. Источник: `nfs-mamont/src/vfs/mod.rs:157-207`.
- **Стиль под-трейтов.** Каждый выглядит как `#[trait_variant::make(Send)] pub trait X { async fn x(&self, args: Args, cred: &Credential) -> Result<Success, Fail>; }`. Пример: `nfs-mamont/src/vfs/read.rs:51-62`, `vfs/write.rs:92-102`, `vfs/commit.rs:36-40`. По grep все 30 `async fn` в трейтах `vfs/` (21), `mount/` (5) и `nlm/procedures/` (4) объявлены без тела по умолчанию: 29 однострочных объявлений и многострочная сигнатура `mnt`.
- **Что приходит в метод.** Только `args` и `&Credential`, адреса клиента нет. MOUNT, напротив, получает `client_addr: SocketAddr`. Источник: `nfs-mamont/src/vfs/read.rs:61`, `mount/mnt.rs:65-70`.
- **Ошибки.** `vfs::Error` повторяет `nfsstat3` (Permission=1 … JUKEBOX=10008, `#[repr(u32)]`). Каждый `Fail` несёт post-op атрибуты или WCC, `NfsRes<B>` оборачивает все результаты. Источник: `nfs-mamont/src/vfs/mod.rs:42-137, 139-143, 209-264`.
- **Контракт READ.** Буфер под данные выделяет сервер. Бэкенд заполняет его и обязан обеспечить `count <= data.len()`, иначе «server will not be able to send a reply». Источник: `nfs-mamont/src/vfs/read.rs:51-61`, `serializer/server/serialize_struct.rs:361-367`. WRITE получает `Args<B>` с буфером данных и отдельным полем `size`. Источник: `nfs-mamont/src/vfs/write.rs:53-70`.
- **File handle.**
  - `Handle(pub [u8; NFS3_FHSIZE])`, `NFS3_FHSIZE = 9`. Первый байт — `BackendId`, остальные 8 байт (`PAYLOAD_SIZE`) интерпретирует бэкенд. Источник: `nfs-mamont/src/consts/nfsv3.rs:34-39`, `vfs/file.rs:10-51`.
  - Генерация handle, их стабильность и проверка на устаревание целиком на бэкенде; root handle экспортов пользователь вычисляет сам. Пример: `fs.handle_for_path(&export.local_path)` в https://github.com/RMamonts/mirror-fs (src/main.rs:47-66).
  - Открытые вопросы авторов: «Research: optimal NFS3_FHSIZE» https://github.com/RMamonts/nfs-mamont/issues/96 и план 12-байтного handle в #351.
- **Валидированные обёртки.** `file::Name`: не пустое, не длиннее 255, без `/`. `file::Path`: не пустой, не длиннее 1024. `Cookie`, `CookieVerifier`. Источник: `nfs-mamont/src/vfs/file.rs:53-131`, `vfs/read_dir.rs:9-83`.
- **Сборка сервера.**
  - `ServerContext::new(allocator: Arc<A>, vfs_concurrency: NonZeroUsize)`, затем `add_backend`/`remove_backend`, затем `handle_forever::<A, B, M, N, V>`. Источник: `nfs-mamont/src/context.rs:57-124`, `lib.rs:43-55`.
  - Реестр хранит `Arc<V>` одного типа: `ServerContext<A, V, B>` → `BackendRegistry<V>`. Источник: `nfs-mamont/src/context.rs:42-55`, `backend/mod.rs:22-25`.
- **Неэкспортированные типы.**
  - `AuthFlavor` лежит в приватном `rpc` и не реэкспортирован (наружу выставлен только `rpc::auth`), хотя публичное поле `mount::mnt::Success.auth_flavors: Vec<AuthFlavor>` его требует. Источник: `nfs-mamont/src/lib.rs:11, 39`, `mount/mnt.rs:11, 39-46`.
  - Трейт `Nlm` лежит в приватном `nlm` и тоже не реэкспортирован. Источник: `nfs-mamont/src/lib.rs:8-9, 36-40`.
- **Открытые улучшения API:** `BackendId` как newtype (#396), вынос `Handle` из `vfs::file` (#356), «pass export path with handle in handle_forever» (#184). Источник: https://github.com/RMamonts/nfs-mamont/issues/396, https://github.com/RMamonts/nfs-mamont/issues/356, https://github.com/RMamonts/nfs-mamont/issues/184.
- **Трейты `Allocator` и `Buffer`.**
  - `Allocator { type Buffer; fn allocate(&self, NonZeroUsize) -> impl Future<Output = Option<Buffer>> + Send; fn capacity(&self) -> NonZeroUsize }`. Документация требует, чтобы READ урезал запрос до `capacity()`.
  - `Buffer` дают `chunks`/`chunks_mut` (RPITIT), `len`, `is_empty`, `empty`.
  - Источник: `nfs-mamont/src/allocator/mod.rs:46-94`. README: «Allocator has public trait so you can put your own implementation» (`README.md:17-18`).
- **Feature `mlock`.** Пул целиком фиксируется в RAM через `libc::mlock`. Если `mlock` не удался, вызывается `panic!` — fallible-конструктора нет; при `Drop` выполняется `munlock`. Источник: `nfs-mamont/src/allocator/mod.rs:126-133, 35-44`, `nfs-mamont/Cargo.toml:18-20`.
- **Кредиты передаются, но не проверяются.** Права, root_squash и анонимные uid/gid реализует сам бэкенд: в демо `MirrorFS::new(…, root_squash)` и модуль `auth/posix.rs`. Источник: https://github.com/RMamonts/mirror-fs (src/main.rs:32-33, src/auth/posix.rs).

### Inferences
- **AFIT-трейты.** Трейты с `async fn` (AFIT) не dyn-совместимы, поэтому `dyn Vfs` невозможен. Чтобы экспортировать бэкенды разных типов на одном сервере, нужен enum-обёртка, который вручную делегирует все 21 трейт. Это противоречит wiki-диаграмме «direct call via dyn Vfs» (https://github.com/RMamonts/nfs-mamont/wiki/NFS-Server-Architectures).
- **Неэкспортированный `AuthFlavor`.** Своя реализация `Mnt` вне крейта не может записать `vec![AuthFlavor::Sys]`, только пустой список. Свою реализацию NLM снаружи сделать нельзя: `NlmService` по сути обязателен.
- **9-байтовый handle.** 8 байт полезной нагрузки хватает на номер inode, но не на inode + generation + fsid. Для passthrough-бэкендов это мешает надёжно обнаруживать устаревшие handle без внутренней таблицы. Кроме того, такие handle легко угадать.
- **Нет адреса клиента в `Vfs`.** Бэкенд не может сделать ACL экспорта по IP на уровне NFS-операций. MOUNT ограничивает только выдачу root handle, а NFS-вызовы с угаданным handle проходят без MOUNT.
- **Порог входа.** Нет методов по умолчанию (например, `NotSupported` для MKNOD/LINK/SYMLINK), поэтому даже read-only бэкенду нужно реализовать все 21 метод.

### Gaps
- Рендер API на docs.rs не проверен: docs.rs недоступен из сессии.
- Действует ли на `handle_forever` компиляторный lint `private_bounds` из-за приватного `Nlm`, не проверялось: сборка запрещена условиями.

## 4. Дизайн производительности и наличие бенчмарков

### Takeaway
Ставка — на предвыделенный пул буферов и отсутствие промежуточных user-space-копий объёмных данных. READ отдаётся `write_vectored` прямо из буфера бэкенда, хвост WRITE читается из сокета прямо в буфер пула. К этому добавляются `TCP_NODELAY` и конвейерная обработка нескольких запросов на соединение. «Zero-copy» здесь не означает kernel zero-copy: sendfile, splice, io_uring и `MSG_ZEROCOPY` не используются. В `main` нет ни одного бенчмарка. Измерения есть только в описаниях незамёрженных PR (относительные цифры) и в ручных прогонах fio.

### Cited Findings
- **Аллокатор `Impl`:**
  - при старте выделяется одна непрерывная зона `alloc_zeroed(size*count)` с выравниванием 1 байт;
  - зона режется на `count` буферов, которые хранятся в `crossbeam_queue::ArrayQueue`;
  - выдача идёт через `Semaphore::acquire_many(ceil(size/buffer_size))` с `permit.forget()`, а возврат — в `Drop` у `Slice` (буферы обратно в очередь, разрешения обратно в семафор).
  - Источник: `nfs-mamont/src/allocator/mod.rs:96-183`, `allocator/slice.rs:82-104`.
- **Буфер READ** выделяется в `ReadTask` под `args.count` до передачи в пул. Урезания до `Allocator::capacity()` нет: если запрос больше ёмкости, `allocate` возвращает `None`, и клиент получает `SYSTEM_ERR`, а не короткое чтение. Источник: `nfs-mamont/src/parser/parser_struct.rs:627-665`, `allocator/mod.rs:84-93, 154-157`, `serialize_struct.rs:280-285`.
- **WRITE.** Сначала проверяется, что `size + padding` не выходит за `frame_remaining`. Затем первые байты копируются из головного окна, остальное читается `read_exact` прямо в буферы пула. Источник: `nfs-mamont/src/parser/parser_struct.rs:562-625`, `parser/read_buffer.rs:127-149`.
- **Ответ READ** собирается как заголовок из staging-`Vec`, чанки `Buffer` и выравнивание, всё одним `write_vectored` с досылкой. Источник: `nfs-mamont/src/serializer/server/serialize_struct.rs:351-440`.
- **Прочие ответы** сериализуются в переиспользуемый `Vec` (начальная ёмкость 4096, очищается без освобождения памяти) и отправляются одним `write_all` на ответ. Источник: `nfs-mamont/src/serializer/server/serialize_struct.rs:33-36, 292-349`. Рост ёмкости обсуждается в https://github.com/RMamonts/nfs-mamont/issues/276.
- **Чтение с упреждением.** `FrameReader::fill` читает в свободный хвост буфера (2500 байт), поэтому несколько мелких конвейерных запросов могут прийти за один `read`. Источник: `nfs-mamont/src/parser/read_buffer.rs:74-104`.
- **`TCP_NODELAY`.** Источник: `nfs-mamont/src/task/connection/mod.rs:36` (PR #381, коммит `9743f69`).
- **Параллелизм.** Пул из `vfs_concurrency` воркеров вместо одной `VfsTask` (PR #376, коммит `9444e5d`). Документ обещает распределение CPU-работы по ядрам рантайма. Источник: `nfs-mamont/src/task/global/vfs.rs:140-148`.
- **Бенчмарков в `main` нет.** Каталогов `benches/`, `examples/`, `tests/`, `fuzz/` в репозитории нет (проверка `ls` на коммите `5eae9a5`). PR с бенчмарками и оптимизациями закрыты без слияния:
  - «feat: mock vfs benchmaks» — https://github.com/RMamonts/nfs-mamont/pull/265;
  - «MVP: perf improvements» (`TCP_NODELAY`, batching ответов в `WriteTask`, пофреймовый разбор; «TO BE TESTED») — https://github.com/RMamonts/nfs-mamont/pull/283;
  - «perf(parser): read pipelined calls with fewer syscalls», закрыт 2026-10-19 — https://github.com/RMamonts/nfs-mamont/pull/383. В описании: буфер 32 KiB и «End-to-end over loopback with a synthetic backend (pipelined client, 4 connections), main → this PR: WRITE 4k: +35% (server on 4 cores), +45% (server on 2 cores); WRITE 16k: +16% / +27%; NULL, GETATTR, READ, WRITE 64k / 1 MiB: within run-to-run noise». Абсолютных цифр нет, комментарий автора: «Need to be double checked».
  - Открытый черновик mock-VFS бэкенда: https://github.com/RMamonts/nfs-mamont/pull/431.
- **Ручная нагрузка fio выявила взаимоблокировку.** Прогон `fio --rw=randrw --bs=1M --numjobs=10 --iodepth=16 --ioengine=libaio` по смонтированному ядром Linux экспорту (PR #100, март 2026) показал deadlock: READ выделял память в VFS-задаче, а WRITE — в задаче чтения. Позже выделение собрано в одном месте — `ReadTask` (PR #222). Источник: обсуждение https://github.com/RMamonts/nfs-mamont/pull/100 и https://github.com/RMamonts/nfs-mamont/pull/222; текущее решение задокументировано в `nfs-mamont/src/parser/parser_struct.rs:627-633`.
- **Метрики.** Спецификация с бюджетом «no more than 2% of throughput»: https://github.com/RMamonts/nfs-mamont/issues/374 (открыт). Реализация влита только в ветку `feature/metrics` (PR #398, #409, merged_by ArtemKhel); PR #433 «detailed metrics on (de)serialization» открыт. Источник: https://api.github.com/repos/RMamonts/nfs-mamont/pulls/398, https://api.github.com/repos/RMamonts/nfs-mamont/pulls/409.

### Inferences
- **Гранулярность пула.** Пул выдаёт только целые буферы размера `buffer_size`. При конфигурации демо (1 MiB × 2048) READ на 4 KiB занимает целый мебибайт: одновременно в работе максимум 2048 READ/WRITE-нагрузок, и мелкий ввод-вывод плохо использует пул. Issue «allocator: fragmentation problem» (#161) закрыт, но схема осталась.
- **Память при старте.** Конфигурация демо — это 2 GiB, выделяемые при старте; с `mlock` все 2 GiB фиксируются в RAM. Источник конфигурации: https://github.com/RMamonts/mirror-fs (config.toml:1-3).
- **Выравнивание 1 байт.** Без собственного аллокатора буферы не годятся для `O_DIRECT`.
- **Один syscall на ответ.** Мелкие ответы не объединяются; батчинг из PR #283 не слит. Известный потенциал (+35–45% на WRITE 4k по PR #383) пока не реализован в `main`.
- **Мелкие аллокации на каждый запрос.** На каждый запрос аллоцируются `Box<NfsArguments>` и `Box<NfsRes>` (`parser/mod.rs:61-64`, `task/mod.rs:16-20`). При высоких IOPS это заметнее, чем стоимость копирования.

### Gaps
- Абсолютной пропускной способности и задержек (MB/s, IOPS, p99) в репозитории, wiki, issue и PR нет; сравнимых с конкурентами цифр не найдено.
- Полный текст отчёта по бенчмаркам из PR #265 («Read .md docs») не изучался: файлы не в `main`.

## 5. Качество кода

### Takeaway
Код аккуратный и дисциплинированный. `unsafe` локализован в аллокаторе, в сериализаторе, задачах, VFS, сервисах и rpcbind нет ни одного `unwrap` или `panic`. Около 87% публичных элементов документированы, в CI включены `clippy -D warnings` и `rustdoc -D warnings`. Свежие рефакторинги идут в сторону типобезопасности и устойчивости. Слабые места:
- у `unsafe` нет комментариев `// SAFETY:`;
- при неудаче `mlock` происходит паника;
- типы ошибок не реализуют `Display`/`std::error::Error`;
- нормальные ситуации (NOENT при LOOKUP, PROG_UNAVAIL, EOF) логируются на уровне ERROR;
- нет крейт-уровневых lint-атрибутов, конфигурация минимальна.

### Cited Findings
- **`unsafe`:** 13 строк, все в `allocator/` — `unsafe impl Send/Sync` для `AllocatorState` и `UnownedBuffer`, `alloc_zeroed`, `dealloc`, `mlock`/`munlock`, `ptr.add`, `slice::from_raw_parts(_mut)`, `pub unsafe fn from_raw_parts`. Источник: `nfs-mamont/src/allocator/mod.rs:32-33, 39-42, 120, 129, 137-139`, `allocator/buffer.rs:18, 31-32, 38, 44`.
  - Раздел `# Safety` есть только у `UnownedBuffer::from_raw_parts` (`allocator/buffer.rs:10-17`); комментариев `// SAFETY:` у блоков нет.
  - Сам `UnownedBuffer` с `pub unsafe fn` реэкспортирован наружу (`lib.rs:36`).
- **Паники в нетестовом коде: 12 мест** (собственный скрипт, без тестовых модулей):
  - `allocator/mod.rs:116, 118, 138, 146` — `expect` в `Impl::new`;
  - `allocator/mod.rs:131` — `panic!` при неудаче `mlock`;
  - `allocator/mod.rs:172` — `unreachable!` на нарушенном инварианте семафора и очереди;
  - `allocator/slice.rs:34, 43, 50` — `assert!` в `Slice::new`;
  - `parser/read_buffer.rs:54` — `assert!` ёмкости в конструкторе;
  - `parser/read_buffer.rs:113` — безопасный `try_into().unwrap()`;
  - `parser/parser_struct.rs:658` — безопасный `unwrap` после проверки `count != 0`.
  - Все пути в `nfs-mamont/src/`.
- **Обработка ошибок.**
  - Два слоя: структурный `parser::Error` и `rpc::Error`, со сменой на границе парсера `map_parser_to_rpc` (PR #426). Источник: `nfs-mamont/src/parser/error.rs:1-28`, `parser/parser_struct.rs:65-85`, `rpc/mod.rs:68-107`.
  - Оба типа только `#[derive(Debug)]`, без `Display` и `std::error::Error`.
  - Отравленный `RwLock` восстанавливается (`backend/mod.rs:82-90`).
- **Логирование.**
  - Любой `Err` от бэкенда (включая обычный NOENT при LOOKUP) пишется через `error!`. Источник: `nfs-mamont/src/task/global/vfs.rs:274-276`.
  - Неизвестная программа даёт двойной лог WARN + ERROR, EOF — ERROR. Источник: https://github.com/RMamonts/nfs-mamont/issues/430, `task/connection/read.rs:201-214`, `parser/parser_struct.rs:484-487`.
- **Документация в коде.**
  - Публичные элементы: 588 из 676 с `///` (≈87%); публичные поля: 252 из 307 (≈82%). Модульный `//!` есть у 124 из 180 файлов (эвристический подсчёт собственным скриптом; `pub mod` считались документированными).
  - Без документации, в частности: `Vfs` (`vfs/mod.rs:157`), `Mount` (`mount/mod.rs:62`), `Impl` (`allocator/mod.rs:96`), `UnownedBuffer` (`allocator/buffer.rs:4`), константы программ (`consts/*.rs`).
  - У `handle_forever` однострочная документация (`lib.rs:42`), у крейта — одна строка `//!` (`lib.rs:1`).
- **Конфигурация линтов.** `.rustfmt.toml` содержит только `edition = "2021"` и `use_small_heuristics = "Max"`. Нет `clippy.toml`, секции `[lints]` в `Cargo.toml` и атрибутов вида `#![deny(...)]`/`#![warn(missing_docs)]` (grep по `#![` пуст). В CI: `cargo clippy --all-targets --all-features -- -D warnings` и `RUSTDOCFLAGS=-D warnings cargo doc --all-features --no-deps --document-private-items`. Источник: `.rustfmt.toml:1-3`, `.github/workflows/rust-ci.yml:37-38, 112-115`.
- **`#[allow(dead_code)]`** стоит на модуле `nlm` целиком и ещё в 8 местах. Источник: `nfs-mamont/src/lib.rs:8`, `parser/nfsv3/file.rs:27, 57`, `parser/primitive.rs:20, 107`, `serializer/files.rs:73`, `serializer/mod.rs:13, 109`, `service/nlm/lock_types.rs:49`.
- **CONTRIBUTING.md:**
  - rustfmt и clippy с `-D warnings`, «Consistency», Conventional Commits (типы и scopes), именование веток;
  - чек-лист PR: MSRV 1.75, fmt, clippy, test, `cargo audit`, rustdoc для нового публичного API;
  - запрет мёржить свои PR без одобрения.
  - Источник: `CONTRIBUTING.md:158-264`. PR #427 «docs: clarify assert messages and panic docs per style guide» привёл сообщения `assert!` к виду «this is a bug in the caller» и добавил разделы `# Panics`. Источник: `nfs-mamont/src/allocator/slice.rs:24-55`, `parser/read_buffer.rs:50-58`, `context.rs:69-71`.
- **Направление последних рефакторингов** (коммиты после v0.2.0 и непосредственно перед ним):
  - `5eae9a5` «Use typed enums for NFSv3 wire discriminants (#438)» — `NfsProc`, `MountProc`, `NlmProc`, `SetTimeMode` с `FromPrimitive` (`consts/nfsv3.rs:6-32`, `vfs/set_attr.rs:24-34`);
  - `a1fb6ac` «Centralize XDR padding and NFS attribute parsing (#437)» — `consts::xdr::padding_len` с doctest (`consts/xdr.rs:9-22`);
  - `c127dac` «separate errors in parsing (#426)»;
  - `a585cba` «remove tracing-subscriber from library dependencies (#434)»;
  - перед релизом — `034aa70`/`86d710d` (сброс фрейма), `6398e2f` (игнор REPLY), `e309bbb` (повтор accept).
  - Источник: https://github.com/RMamonts/nfs-mamont/commits/main.
- **Технический долг в TODO.** Открыты: верификатор (#137, #143), фрагментация (#124/#103/#269), shutdown (`lib.rs:73`), нет логов при ошибке отправки (`task/global/mount.rs:113-117`, `nlm_task.rs:84-88`), колбэки NLM (#267). Источник: grep `TODO` по `nfs-mamont/src`.

### Inferences
- Курс проекта — укрепление: строгий разбор, типизированные дискриминанты, единая XDR-арифметика, сброс фрейма. Функционал (колбэки NLM, метрики, фаззинг, ограниченные каналы) копится в ветках и открытых PR. Это признак зрелой инженерной культуры при узкой пропускной способности команды.
- Логирование нормальных событий на уровне ERROR затрудняет эксплуатацию и мониторинг: шум в логах прятал бы настоящие ошибки.
- Паника при неудаче `mlock` (обычная ситуация при малом `RLIMIT_MEMLOCK`) для библиотеки нежелательна: лучше вернуть `Result`.

### Gaps
- Фактический вывод clippy и rustdoc, тесты и покрытие измеряет другой исследователь: сборка здесь не запускалась.
- Стиль-гайд, на который ссылается PR #427, в репозитории отдельно не найден; видимо, имеется в виду CONTRIBUTING.md или внутренняя договорённость.

## 6. Тестирование, фаззинг, CI, релизы, dependabot

### Takeaway
Модульных тестов много: 336 тестовых функций в 48 файлах, в основном парсер, аллокатор и NLM-сервис. Интеграционных и end-to-end-тестов нет, тестов с реальными клиентами ядра и конформанс-наборов (pjdfstest, cthon04, pynfs, xfstests) тоже. Фаззинг живёт в отдельной ветке, а его workflow **ни разу не запускался**. CI только на Linux, матрица 1.75.0 + 1.99.0, есть audit, udeps и docs. Публикация на crates.io автоматизирована через OIDC (trusted publishing). Dependabot настроен только для GitHub Actions.

### Cited Findings
- **Объём тестов.** 336 функций `#[test]`/`#[tokio::test]` в 48 файлах. По модулям: `parser` 116, `allocator` 93, `service` (NLM и MOUNT) 70, `serializer` 28, `rpcbind` 10, `vfs` 7, `task` 6 (маршрутизация `target`), `backend` 5, `nlm` 1; в `mount`, `context`, `lib`, `consts`, `rpc` — 0. Подсчёт grep по `nfs-mamont/src`, например `parser/tests/parser_struct.rs` (1089 строк), `allocator/tests/slice/iter_mut.rs` (509 строк).
- **Нет интеграционных тестов.** Нет тестов, поднимающих `handle_forever`, и тестов `ReadTask`/`WriteTask` поверх сокета. Реальный `TcpListener` в тестах используется только как mock-rpcbind (`rpcbind/tests/mod.rs`). Каталога `nfs-mamont/tests/` нет. Источник: grep `handle_forever`/`TcpListener::bind` по `nfs-mamont/src`.
- **Нет property-тестов и верификации в `main`.** proptest, quickcheck, loom, Miri и Kani не используются (grep пуст). Kani пробовали в PR #320 (закрыт без слияния 2026-10-05; ветки `vyar/kani*`). Источник: https://github.com/RMamonts/nfs-mamont/pull/320.
- **Фаззинг.**
  - `.github/workflows/fuzz.yml` запускается только вручную (`workflow_dispatch`) командой `cargo +nightly fuzz run "<target>" -- -ascii_only=1 -max_len=8192 -max_total_time=<time>`. Источник: `.github/workflows/fuzz.yml:1-26`.
  - Каталога `fuzz/` в `main` нет. Фазз-цели лежат в ветке `fuzz` (коммит `9296842 feat: add nlm to parser fuzz`), см. `git log --all -- fuzz` в полном клоне.
  - Перенос в `main` — черновик PR «tests: fuzzing» (открыт с 2026-10-28): https://github.com/RMamonts/nfs-mamont/pull/436. Ранние PR #109 и #275 закрыты без слияния. Workflow добавлен PR #224 (коммит `a3d9445`, 2026-04-20).
  - Actions API: у `fuzz.yml` `total_count = 0`, запусков не было на 2026-10-30. Источник: https://api.github.com/repos/RMamonts/nfs-mamont/actions/workflows/fuzz.yml/runs.
- **CI** (`rust-ci.yml`), только `ubuntu-latest`:
  - `lint`: toolchain 1.99.0, `fmt --check`, clippy `-D warnings`, `cargo check --locked`;
  - `test`: матрица `["1.75.0", "1.99.0"]`, `cargo build/test --all-features`;
  - `security`: `cargo audit`;
  - `docs`: toolchain 1.95.0, `-D warnings`;
  - `udeps`: nightly `cargo-udeps`.
  - Тулчейны закреплены, а не `stable`; README обещает «current stable». nextest закомментирован. Покрытия и macOS/Windows-раннеров нет.
  - Источник: `.github/workflows/rust-ci.yml:16-143`, `README.md:46-47`.
- **Статистика CI-запусков.** `rust-ci.yml`: всего 1124 запуска. Из последних 100 — 80 success, 19 failure (все не на `main`, то есть на ветках PR), 1 action_required. На `main` из этих 100 пришлось 19 запусков, и все они success (2026-10-30). Источник: https://api.github.com/repos/RMamonts/nfs-mamont/actions/workflows/rust-ci.yml/runs.
- **Релиз.** `release.yml` срабатывает на `release: published`, использует environment `crates-io`, проверяет совпадение тега с версией (`v$version`), делает `cargo publish --dry-run`, затем `rust-lang/crates-io-auth-action@v1` (OIDC) и `cargo publish --locked`. Запусков три: v0.1.1 сначала failure, затем success; v0.2.0 success. Источник: `.github/workflows/release.yml:1-51`, https://api.github.com/repos/RMamonts/nfs-mamont/actions/workflows/release.yml/runs.
  - Крейт создан на crates.io 2026-10-09 12:31 UTC, раньше GitHub-релиза v0.1.0 (20:58 UTC). По-видимому, 0.1.0 опубликован вручную. Источник: https://crates.io/api/v1/crates/nfs-mamont, https://api.github.com/repos/RMamonts/nfs-mamont/releases.
- **Защита веток и тегов** (rulesets API, 2026-10-30):
  - `protect-main`: 1 одобрение, обязательное ревью code owner, `require_last_push_approval`, разрешение всех тредов, только squash, линейная история, запрет удаления и force-push, без bypass. Обязательных status checks в ruleset нет.
  - `release limitation` (теги): создавать и менять может только OrganizationAdmin, требуются подписи.
  - Ruleset «Copilot review» отключён.
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont/rulesets.
- **Dependabot** только для `github-actions` (weekly). Обновлений `cargo` нет. Источник: `.github/dependabot.yml:1-6`.
- **Конформанс.** В телах всех 440 issue и PR и в 370 комментариях нет упоминаний pjdfstest, cthon, pynfs, xfstests, «conformance», macOS, Windows, FreeBSD, UDP и RPCSEC (поиск 2026-10-30 по выгрузке https://api.github.com/repos/RMamonts/nfs-mamont/issues?state=all и /issues/comments). В коде, конфигах CI и wiki нет ни этих наборов, ни упоминаний macOS, Windows, FreeBSD и UDP; RPCSEC_GSS встречается только как значения enum `AuthFlavor::RpcSecGss` и `AuthStat` (`nfs-mamont/src/rpc/mod.rs:44`, `rpc/auth.rs:48-53`).
- **Реальный клиент ядра используется только вручную:**
  - fio по смонтированному Linux-клиентом экспорту (PR #100);
  - логи монтирования ядром Linux в issue #430;
  - wiki-гайд с командами `mount -t nfs -o vers=3,tcp,…,nolock` и `nconnect=N`.
  - Источник: https://github.com/RMamonts/nfs-mamont/pull/100, https://github.com/RMamonts/nfs-mamont/issues/430, https://github.com/RMamonts/nfs-mamont/wiki/MirrorFs-User-Guide. В mirror-fs есть собственные тесты бэкенда (`src/tests/*.rs`) — https://github.com/RMamonts/mirror-fs.

### Inferences
- Главный пробел качества — отсутствие e2e-проверки «клиент ядра ↔ сервер» в CI. Регрессии вроде deadlock из PR #100 или бага UMNT (PR #423) ловятся только вручную.
- Флаг `-ascii_only=1` в `fuzz.yml`, похоже, ошибочен: в libFuzzer флаг называется `-only_ascii` (по памяти, не перепроверено). Если бы ограничение действовало, оно исключило бы байты ≥0x80, включая бит last-fragment заголовка RM, и почти все входы отсекались бы на «fragmented». Это стоит проверить.
- Закреплённые версии тулчейна (1.99.0 и 1.95.0 для docs) разойдутся со «stable» без ручного обновления, а dependabot для cargo выключен — устаревание зависимостей отслеживается только `cargo audit`.

### Gaps
- Фактическое число проходящих тестов и время прогона не измерялись (запрет на запуск cargo).
- Содержимое фазз-целей на ветке `fuzz` детально не изучалось.

## 7. Документация и онбординг

### Takeaway
Документация для пользователя библиотеки — самое слабое место:
- README краткий;
- архитектурная страница wiki почти целиком из «TODO!»;
- крейт-уровневая rustdoc-документация — одна строка, примеров в крейте нет;
- единственный «Quick start» — ссылка на отдельный демо-репозиторий mirror-fs (≈2,7 тыс. строк), README которого частично устарел.

Чтобы новичок поднял сервер, ему нужно реализовать 21 трейт без значений по умолчанию, вычислить root handle экспортов и собрать 5 обобщённых параметров. Порог входа высокий.

### Cited Findings
- **README:** список возможностей, ссылка на архитектуру в wiki, «Quick start» из одной строки («As an example we provide our demo»), фрагмент инициализации `tracing_subscriber`, требования (Rust 1.75, Linux, feature `mlock`), дорожная карта. Примера реализации `Vfs` нет. Источник: `README.md:1-74`.
- **rustdoc.**
  - Крейт-уровневая документация: `//! NFS Mamont - A Network File System (NFS) server implementation in Rust.` Нет `#![doc = include_str!("../README.md")]`. Источник: `nfs-mamont/src/lib.rs:1`.
  - Doc-примеров два: `ServerContext` (`no_run`, `context.rs:17-41`) и `padding_len` (`consts/xdr.rs:11-19`).
  - Поля `documentation` и `[package.metadata.docs.rs]` (например, чтобы включить `mlock`) отсутствуют. Источник: `nfs-mamont/Cargo.toml:1-32`; `"documentation": null` в https://crates.io/api/v1/crates/nfs-mamont.
- **Wiki** (клон https://github.com/RMamonts/nfs-mamont.wiki.git, пять страниц):
  - «NFS‐Mamont architecture»: mermaid-диаграмма конвейера и «TODO!» во всех разделах (последняя правка 2026-06-07);
  - «NFS Server Architectures»: концептуальное сравнение knfsd, FUSE и ganesha с in-process подходом;
  - «MirrorFs User Guide» (2026-06-11): конфиг с устаревшими ключами `read_buffer_size`/`write_buffer_*` и монтирование с `port=`, `mountport=`, `nolock`;
  - FAQ: одна запись про `systemd-run`;
  - Home: одна строка.
  - Источник: https://github.com/RMamonts/nfs-mamont/wiki.
- **Демо mirror-fs:**
  - отдельный репозиторий, `publish = false`, 18 коммитов, ≈2678 строк Rust;
  - зависит от `nfs-mamont = "0.2.0"` с crates.io, хотя README всё ещё пишет «fetched from GitHub at a pinned revision»;
  - вызывает `nfs_mamont::init_tracing` под `cfg(debug_assertions)`, а этой функции в `main` после #434 нет;
  - сам реализует root squash и POSIX-проверки.
  - Источник: https://github.com/RMamonts/mirror-fs (README.md:24-25, Cargo.toml:11-26, src/main.rs:12-13, 24-71, src/auth/posix.rs).
- **Минимальный запуск** по `main.rs` демо: `ServerContext::new(Arc::new(Impl::new(size, count)), vfs_pool_size)`, затем `add_backend`, затем вычисление root handle каждого экспорта, затем `MountService::with_exports(Vec<ExportEntryWrapper>)` и `NlmService::new()`, и наконец `handle_forever(listener, context, mount, nlm)`. Источник: https://github.com/RMamonts/mirror-fs (src/main.rs:24-71).
- **CONTRIBUTING.md подробный:** окружение, команды, стиль, коммиты, ветки, чек-лист PR. Есть CODE_OF_CONDUCT.md. Источник: `CONTRIBUTING.md:1-277`.
- **Community profile — 75%:** нет шаблонов issue и PR, нет SECURITY.md (черновик PR #411 с 2026-10-16). Источник: https://api.github.com/repos/RMamonts/nfs-mamont/community/profile, https://github.com/RMamonts/nfs-mamont/pull/411.
- **Задачи для новичков.** Меток `good first issue` всего 10, открыта одна (#396, она же `invalid`). Источник: выгрузка https://api.github.com/repos/RMamonts/nfs-mamont/issues?state=all (2026-10-30).

### Inferences
- На docs.rs стартовая страница крейта почти пустая: одна строка и список модулей. Пользователю придётся собирать картину по сигнатурам и демо. Это вывод по исходникам, сама страница не открывалась.
- Встроенный простой бэкенд (in-memory или passthrough) и пример `examples/` резко снизили бы порог входа. Сейчас «Hello, NFS» — это тысячи строк чужого демо.
- Расхождения wiki и демо с текущим API (ключи конфига, `init_tracing`, «pinned revision») — признак того, что документацию обновляют реже, чем код.

### Gaps
- docs.rs недоступен из сессии (DNS/403): статус сборки 0.1.0–0.2.0 и рендер не проверены. Сетевой доступ к docs.rs нужно разрешить в настройках сессии.
- Звёзды и форки mirror-fs не получены: API этого репозитория вне области сессии.

## 8. Здоровье проекта и сопровождение (контрибьюторы, ритм, ревью, issue/PR, релизы, принятие)

### Takeaway
Маленький, очень активный проект с сильной процедурой ревью и выраженной зависимостью от одного человека.
- **Активность:** 191 коммит в `main` за 13 месяцев. Автор (VadimYarovoy) сделал 68,6% всех коммитов и ≈84% за последние 90 дней. Он же единственный владелец крейта на crates.io.
- **Скорость:** медианный срок слияния PR — 19,7 ч, медиана первого ответа на PR — 6 ч. Почти все PR одобрены не автором.
- **Issue** служат внутренним трекером задач: 158 из 159 открыты командой.
- **Принятие** практически нулевое (на 2026-10-30): 80 скачиваний, 0 обратных зависимостей, 10 звёзд, 2 форка, внешних упоминаний не найдено.

### Cited Findings
- **Возраст и история.** Репозиторий создан 2025-10-05. 191 коммит в `main` с 2025-10-05 по 2026-10-29 (полный клон `git clone --filter=blob:none`). Источник: https://api.github.com/repos/RMamonts/nfs-mamont, https://github.com/RMamonts/nfs-mamont/commits/main.
- **Доли коммитов в `main`** (git shortlog, 2026-10-30): VadimYarovoy 131 (68,6%), MikhailSvirukov 33 (17,3%), ArtemNik1tin 10, dsemak 7, IgorErin 6, artemiipatov 1, dependabot 2, Copilot 1. Contributors API даёт 8 участников (6 людей и 2 бота) с теми же числами. Источник: https://api.github.com/repos/RMamonts/nfs-mamont/contributors.
  - За последние 90 дней (с 2026-08-01): VYAR 63 из 75 коммитов (84%), svmk17 8.
  - За последние 30 дней: VYAR 28 из 34 (82%).
  - По кварталам: в Q4-2025 активны 4 человека, в Q3-2026 — 3 человека плюс боты.
- **Ритм коммитов по месяцам:** 2025-10: 8, 11: 3, 12: 1; 2026-01: 1, 02: 17, 03: 49, 04: 9, 05: 17, 06: 7, 07: 4, 08: 11, 09: 30, 10: 34. Ритм всплесками, с провалами в ноябре–январе и в июне–августе. Источник: полный клон.
- **PR (всего 281):**
  - состояние: 196 смёржены, 73 закрыты без слияния (26%), 12 открыты;
  - по авторам: VadimYarovoy 184 (131 смёржен, 46 закрыто без слияния), MikhailSvirukov 55, ArtemNik1tin 14, IgorErin 7, artemiipatov 7, dsemak 5, Copilot 4, ArtemKhel 3, dependabot 2.
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont/pulls?state=all (2026-10-30).
- **Время слияния** (193 смёрженных PR людей): медиана 19,7 ч, p25 1,8 ч, p75 106 ч, p90 357 ч. 115 PR смёржены за 24 часа, 155 — за 7 дней. Источник: тот же endpoint, собственный расчёт.
- **Первый ответ не автора-человека на PR** (224 PR с ответом): медиана 6,0 ч, p75 22,5 ч, p90 92,5 ч. Источник: endpoints `/pulls/{n}/reviews`, `/pulls/comments`, `/issues/comments`, собственный расчёт.
- **Практика ревью.**
  - 183 из 193 смёрженных PR людей имеют одобрение не автора; часть исключений слита в не-`main` ветки, например #398 и #409 в `feature/metrics`.
  - События ревью: VadimYarovoy 206, MikhailSvirukov 180, IgorErin 57, dsemak 45.
  - Одобрения: MikhailSvirukov 115, VadimYarovoy 50, dsemak 36, IgorErin 19.
  - 1218 inline-комментариев, из них 523 от VadimYarovoy и 217 от Copilot. Бот `copilot-pull-request-reviewer[bot]` оставил 103 ревью.
  - Открыт PR #445 «ci: add automated Claude PR review workflow» (2026-10-30); в описании PR #383 стоит пометка «Generated with Claude Code».
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont/pulls/{n}/reviews, https://github.com/RMamonts/nfs-mamont/pull/445, https://github.com/RMamonts/nfs-mamont/pull/383.
- **Issue (без PR, всего 159):**
  - состояние: 115 закрыты (88 completed, 20 not_planned, 7 duplicate), 44 открыты;
  - авторы: VadimYarovoy 86, MikhailSvirukov 54, ArtemNik1tin 13, IgorErin 5, ArtemKhel 1;
  - комментарий не автора есть только у 21 из 159; медиана первого ответа среди них 17,9 ч;
  - время закрытия: медиана 168 ч (7 дней), p75 ≈48 дней, p90 ≈78 дней;
  - открытые: медианный возраст 39 дней, старейший 389 дней (#3), 37 из 44 без исполнителя; метки открытых: enhancement 14, question 5, EPIC 3, invalid 3, bug 1.
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont/issues?state=all, /issues/comments (2026-10-30), собственный расчёт.
- **Внешний вклад.** Единственный внешний автор — ArtemKhel: PR #398 и #409 смёржены в `feature/metrics` им самим, PR #433 открыт; issue #430 без ответа на 2026-10-30. Источник: https://api.github.com/repos/RMamonts/nfs-mamont/pulls/409, https://github.com/RMamonts/nfs-mamont/issues/430.
- **Открытые PR (12)** на 2026-10-30:
  - #445 — Claude PR review;
  - #443 — CODEOWNERS, черновик;
  - #440 — инкапсуляция `Handle`;
  - #436 — fuzzing, черновик;
  - #435 — feature-флаг клиентских сериализаторов;
  - #433 — метрики;
  - #431 — mock VFS, черновик;
  - #424 — «add u64 id»;
  - #423 — исправление ключа mount-реестра;
  - #411 — SECURITY.md, черновик;
  - #397 — типы блокировок NLM;
  - #377 — колбэк GRANTED (открыт с 2026-10-06).
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont/pulls?state=open.
- **Релизы и версионирование.**
  - v0.1.0 — 2026-10-09: «What is ready: NFSv3, Mount with some auth limitation… not ready: nlm».
  - v0.1.1 — 2026-10-12: «Use only required tokio features».
  - v0.2.0 — 2026-10-26: автосгенерированные заметки, список PR #399–#428.
  - В `main` после v0.2.0 ещё 4 коммита (#426, #434, #437, #438).
  - Файла CHANGELOG нет. Теги лёгкие (`git cat-file -t v0.2.0` → `commit`). MSRV во всех версиях 1.75.0. Предложено поднять MSRV до 1.92: https://github.com/RMamonts/nfs-mamont/issues/417.
  - Источник: https://github.com/RMamonts/nfs-mamont/releases, https://crates.io/api/v1/crates/nfs-mamont.
- **crates.io** (2026-10-30):
  - всего 80 скачиваний: 0.1.0 — 25, 0.1.1 — 31, 0.2.0 — 24;
  - пики в дни релизов: 19 (10-09), 16 (10-12), 13 (10-26), 11 (10-30);
  - 0 обратных зависимостей;
  - единственный владелец — пользователь VadimYarovoy.
  - Источник: https://crates.io/api/v1/crates/nfs-mamont, /downloads, /reverse_dependencies, /owners.
- **GitHub** (2026-10-30):
  - 10 звёзд, 2 форка, `subscribers_count` 0, `open_issues_count` 56 (44 issue + 12 PR);
  - топики `nfs-server`, `nfsv3`, `rust`; описание «NFS server that runs in userspace»; включены wiki, projects и discussions.
  - Трафик за 14 дней (2026-10-16…10-29): 688 просмотров, 77 уникальных посетителей.
  - Источник: https://api.github.com/repos/RMamonts/nfs-mamont, https://api.github.com/repos/RMamonts/nfs-mamont/traffic/views.
- **Внешние упоминания.** Веб-поиск по «"nfs-mamont" Rust NFS» (2026-10-30) не нашёл ни одного упоминания проекта, зато выдал конкурентов: nfsserve (https://github.com/nfs-rs/nfsserve), fernfs (https://docs.rs/crate/fernfs/0.1.4/source/README.md), nfs3_server (https://lib.rs/crates/nfs3_server), bold-nfs, nfswolf (https://docs.rs/nfswolf).
- **Организация и роли.** GitHub-организация RMamonts; `CODEOWNERS`: `* @VadimYarovoy @dsemak @MikhailSvirukov`. Источник: `CODEOWNERS:1`. API организации (участники, проекты) из сессии недоступен. Kanban-доска указана в README и CONTRIBUTING: https://github.com/orgs/RMamonts/projects/1/views/1.
- **Ветки.** В репозитории 60 удалённых веток, среди них `fuzz`, `feature/metrics`, `vyar/kani*` и ветки NLM-колбэков. Источник: полный клон (`git branch -r`).

### Inferences
- **Bus factor ≈ 1–2.** Почти весь код и все релизы делает один человек: он единственный владелец на crates.io, а создавать теги может только администратор организации. Второй ключевой участник (MikhailSvirukov) — главный рецензент и автор NLM-работ.
- **Сильная процедура ревью.** Обязательное ревью code owner, last-push approval, squash, быстрые ответы — по процессу это уровень выше типичного хобби-крейта.
- **Доля незамёрженных PR (26%) и много работы в ветках** говорят об экспериментальном стиле разработки: фичи (метрики, фаззинг, Kani, перф-оптимизации) часто не доходят до `main`.
- **Скачивания на уровне ботов и зеркал**: пики в дни релизов, реальных пользователей не видно. Трафик GitHub вырос с релизом 0.2.0 (до 27 уникальных посетителей в день).
- **Внешнюю отзывчивость** по одному внешнему issue оценить нельзя. Сам issue #430 за 6 дней не получил ответа.

### Gaps
- Содержимое Kanban-доски и дорожной карты на ней недоступно: API organization projects заблокирован, страница требует JS.
- Состав участников организации RMamonts и их роли неизвестны (API `orgs/*` закрыт).
- GitHub Discussions: количество обсуждений не проверялось.

## 9. Поддержка платформ и клиентов (ОС сервера; клиенты Linux, macOS, Windows)

### Takeaway
Сервер официально только для Linux, и CI это подтверждает (ubuntu-only). Из клиентов на практике проверялось только ядро Linux, вручную: NFSv3, TCP, обычно `nolock`. Свидетельств проверки клиентов macOS, Windows и FreeBSD нет. Совместимость с ними ограничивают отсутствие UDP, неполный NLM без NSM, отсутствие NFS_ACL и RPCSEC_GSS.

### Cited Findings
- **ОС сервера.** README: «Platform: Linux (`libc` is an optional dependency used only for the `mlock` feature…)». CONTRIBUTING: «development happens on Linux». Источник: `README.md:48-49`, `CONTRIBUTING.md:112-113`.
- **CI** гоняет все задачи только на `ubuntu-latest`. Источник: `.github/workflows/rust-ci.yml:18-143`.
- **Linux-специфичные errno.** `ACCEPT_ERROR_ENOTSOCK = 88` (это значение для Linux), `EBADF = 9`, `EINVAL = 22` захардкожены числами. Источник: `nfs-mamont/src/consts/socket.rs:4-11`.
- **Клиент Linux.**
  - Wiki-гайд монтирует так: `mount -t nfs -o vers=3,tcp,proto=tcp,port=<PORT>,mountport=<PORT>,nolock` и вариант с `nconnect=N`. Источник: https://github.com/RMamonts/nfs-mamont/wiki/MirrorFs-User-Guide.
  - README демо: `mount -t nfs -o tcp,vers=3 <server>:/fs01 /mnt/fs01` (с регистрацией через rpcbind). Источник: https://github.com/RMamonts/mirror-fs (README.md:66-75).
  - В issue #430 современный клиент Linux при монтировании пробует NFS_ACL (100227) и LOCALIO (400122) и получает `PROG_UNAVAIL`. По словам автора issue, шум исчезает с `-o noacl` и выключенным `localio_enabled`. Источник: https://github.com/RMamonts/nfs-mamont/issues/430.
  - fio-нагрузка шла через ядро Linux. Источник: https://github.com/RMamonts/nfs-mamont/pull/100.
- **macOS, Windows, FreeBSD.** Упоминаний нет в коде, wiki, телах issue и PR и в комментариях (поиск 2026-10-30, см. раздел 6).
- **Транспорт и безопасность.** Только TCP; только AUTH_NONE и AUTH_SYS; верификатор ответа всегда AUTH_NONE. Источник: `nfs-mamont/src/lib.rs:43-48`, `parser/parser_struct.rs:236-297`, `task/connection/write.rs:41-43`.

### Inferences
- **ENOTSOCK на BSD/macOS** равен 38. При сборке сервера на macOS проверка фатальных ошибок `accept` будет неточной (мелочь). Других явных препятствий сборке на macOS в коде не видно, но проверок нет.
- **Клиенты с UDP по умолчанию** (старые системы и часть встроенных клиентов) и клиенты, которым нужны блокировки, без явных опций работать не будут или будут работать хуже. Для Windows Client for NFS и macOS совместимость — открытый вопрос, который нужно проверить экспериментально.

### Gaps
- Экспериментальных данных о macOS, Windows и FreeBSD нет; в рамках статического аудита их и не получить.

## 10. Честные выводы: ключевые сильные стороны и список пробелов для продакшена

### Takeaway
Сильные стороны nfs-mamont — инженерная основа:
- собственный строгий XDR/RPC-слой с корректным сбросом фрейма;
- полностью типизированный async-интерфейс NFSv3;
- подключаемый пул буферов без лишних копий объёмных данных;
- пул воркеров с конвейеризацией;
- несколько бэкендов, подключаемых на лету;
- узкий набор зависимостей;
- дисциплина CI и ревью.

Для продакшена не хватает:
- безопасности (только AUTH_SYS, нет ACL и опций экспорта, угадываемые 9-байтовые handle);
- устойчивости (неограниченные очереди, нет таймаутов, нет фрагментации, нет graceful shutdown);
- блокировок (NLM частичный, без NSM);
- UDP и NFSv4;
- наблюдаемости (метрики не в `main`, шумные логи);
- e2e и конформанс-тестов;
- документации и примеров;
- пользователей.

### Cited Findings
**Сильные стороны, с привязкой к коду:**
1. **Строгий разбор RPC.**
   - Запись читается в «головное окно».
   - После любой ошибки фрейм сбрасывается, хвостовые байты отвергаются.
   - Версия программы проверяется до процедуры, REPLY игнорируется.
   - Действуют лимиты: AUTH_SYS не больше 400 байт, `machinename` до 255, 16 `gids`; имена до 255, пути до 1024; WRITE не может выйти за пределы фрейма.
   - Источник: `nfs-mamont/src/parser/parser_struct.rs:375-536, 562-625`, `parser/rpc.rs:17-53`, `consts/rpc.rs:3-11`.
2. **Типизированный NFSv3-интерфейс.** 21 async-трейт с отдельными `Success`/`Fail` и WCC-данными на каждую процедуру, `vfs::Error` повторяет `nfsstat3`, `Credential` передаётся в каждую операцию. Источник: `nfs-mamont/src/vfs/mod.rs:42-264`, `rpc/auth.rs:77-93`.
3. **Буферы без лишних копий.** Пул с семафором и подключаемый `Allocator`/`Buffer`; READ отправляется через `writev` прямо из буфера бэкенда, WRITE читается прямо в буфер; опционально `mlock`. Источник: `nfs-mamont/src/allocator/mod.rs:46-183`, `serializer/server/serialize_struct.rs:351-440`, `parser/read_buffer.rs:127-149`.
4. **Конвейер и параллелизм.** Несколько запросов на соединение одновременно; пул VFS-воркеров заданного размера; `TCP_NODELAY`; NULL отвечается сразу. Источник: `nfs-mamont/src/task/global/vfs.rs:140-187`, `task/connection/read.rs:94-111`, `task/connection/mod.rs:36`.
5. **Несколько бэкендов и экспортов.** До 256 бэкендов, подключаемых и отключаемых на лету (`BackendRegistry`); экспорты добавляются на лету; cross-backend RENAME/LINK отвечает `XDev`. Источник: `nfs-mamont/src/backend/mod.rs:1-104`, `context.rs:79-117`, `service/mount/mod.rs:112-124`, `task/global/vfs.rs:63-66`.
6. **Удобные мелочи.** Автоматическая регистрация в rpcbind с откатом; повтор `accept`. Источник: `nfs-mamont/src/rpcbind/mod.rs:401-479`, `lib.rs:76-113`.
7. **Малый набор зависимостей и безопасность цепочки поставки.** Девять прямых зависимостей (35 пакетов в lock), `cargo audit`/`udeps` в CI, trusted publishing через OIDC. Источник: `nfs-mamont/Cargo.toml:22-32`, `.github/workflows/rust-ci.yml:81-143`, `.github/workflows/release.yml:1-51`.
8. **Процесс.** Обязательное ревью code owner, быстрый цикл ревью (медиана первого ответа 6 ч, слияния — 19,7 ч), Conventional Commits, подробный CONTRIBUTING. Источник: https://api.github.com/repos/RMamonts/nfs-mamont/rulesets, раздел 8, `CONTRIBUTING.md`.

**Пробелы для продакшена, с доказательствами:**
1. **NFSv4 отсутствует.** README: `[ ] NFSv4 protocol`; в `rfc/` лежат только тексты RFC 7530 и 7531. Источник: `README.md:69`.
2. **Блокировки.**
   - NLMv4 частичный: нет GRANTED-колбэка, `_MSG`/`_RES`, SHARE и FREE_ALL, нет NSM.
   - Трейт `Nlm` закрыт для внешних реализаций.
   - Тем не менее NLM регистрируется в rpcbind.
   - Источник: `nfs-mamont/src/consts/nlm.rs:6-15`, `service/nlm/procedures/unlock.rs:23`, `lib.rs:8-9`, `rpcbind/mod.rs:110-116`; issue #267, #287, #326.
3. **Безопасность и аутентификация.**
   - Нет RPCSEC_GSS/Kerberos, серверный верификатор всегда AUTH_NONE. Источник: `parser/parser_struct.rs:265-272`, `task/connection/write.rs:41-43`; issue #137, #143. Kerberos упоминается как неопределённое будущее в https://github.com/RMamonts/nfs-mamont/issues/156.
   - Нет проверок прав на уровне сервера: https://github.com/RMamonts/nfs-mamont/issues/323.
   - Нет опций экспорта и ACL хостов: `MountService` выдаёт root handle любому клиенту, а MNT игнорирует `cred` и `names`. Источник: `service/mount/mnt.rs:14-48`; https://github.com/RMamonts/nfs-mamont/issues/101.
   - В `Vfs` нет адреса клиента. Handle 9 байт: угадываемые, без поля generation. Источник: `consts/nfsv3.rs:39`; issue #96, #351.
   - Нет проверки привилегированного порта клиента (опция «secure»): в коде такого нет.
4. **Устойчивость и ресурсы.**
   - Неограниченные каналы (#134, #219), нет таймаутов и keepalive (#3), нет лимитов соединений и запросов (#102, #103).
   - Нет сборки многофрагментных записей (#124) и фрагментации ответов (#269).
   - READ не урезается до ёмкости пула (`parser_struct.rs:653-662`).
   - Паника при неудаче `mlock` (`allocator/mod.rs:126-133`).
   - Нет graceful shutdown: ctrl_c внутри библиотеки (`lib.rs:73-100`, #335).
5. **Корректность MOUNT.** UMNT и UMNTALL не работают из-за ключа `SocketAddr`; исправление PR #423 не смёржено (`service/mount/mod.rs:85-90`).
6. **UDP не поддерживается.** Источник: `lib.rs:43-48`; README mirror-fs.
7. **Наблюдаемость.** Метрик в `main` нет: они в ветке `feature/metrics`, связанные задачи #374 и #433. Нормальные события логируются на уровне ERROR (`task/global/vfs.rs:274-276`, #430).
8. **Тестирование.** Нет e2e-тестов с клиентом ядра, нет pjdfstest, cthon04, pynfs или xfstests. Фаззинг не в `main`, а workflow ни разу не запускался (раздел 6).
9. **Совместимость.** Проверялся только клиент Linux; macOS и Windows — неизвестно; NFS_ACL нет (раздел 9).
10. **Документация и примеры.** Архитектурная wiki из «TODO!», однострочная крейт-документация, нет `examples/`, нет встроенного бэкенда, демо частично устарело (раздел 7).
11. **Принятие и bus factor.** 80 скачиваний, 0 обратных зависимостей, 10 звёзд, 2 форка (2026-10-30); ≈84% коммитов последних 90 дней от одного автора; единственный владелец на crates.io (раздел 8).
12. **Платформа сервера.** Только Linux, CI ubuntu-only, Linux-специфичные errno (раздел 9).

### Inferences
- **Главный дифференциатор** на фоне Rust-конкурентов, которых сравнивают другие исследователи: архитектура, рассчитанная на производительность (пул буферов, конвейер, пул воркеров, vectored I/O), плюс строгий и хорошо протестированный парсер. Без опубликованных абсолютных бенчмарков это пока потенциал, а не доказанное преимущество.
- **Ближайшие шаги с наибольшим эффектом:**
  - слить PR #423 (баг UMNT) и PR #436 (фаззинг) и реально запускать фаззинг;
  - перевести каналы на bounded и урезать READ до `capacity()`;
  - добавить e2e-тест с клиентом ядра в CI (например, через loopback-монтирование);
  - выпустить пример `examples/` с in-memory-бэкендом и крейт-уровневую документацию;
  - понизить уровень логов для нормальных NFS-ошибок;
  - экспортировать `AuthFlavor` и `Nlm` либо сделать NLM опциональным в `handle_forever`;
  - опубликовать абсолютные бенчмарки (fio/pjdfstest);
  - добавить второго владельца крейта на crates.io.
- **Состояние на 2026-10-30:** «альфа-библиотека с хорошей архитектурой» для доверенных Linux-сред (лаборатория, внутренние сервисы, кастомные бэкенды по TCP с `nolock`). До промышленного NFS-сервера она пока не дотягивает.

### Gaps
- Нет независимых измерений производительности и поведения под нагрузкой, а значит, нельзя количественно подтвердить главное заявленное преимущество.
- Нет данных о реальных пользователях: ни отзывов, ни обратных зависимостей, ни упоминаний.
- Kanban-доска с приоритетами команды недоступна. Дорожная карта восстановлена по README, открытым EPIC (#267, #326, #351) и issue.
