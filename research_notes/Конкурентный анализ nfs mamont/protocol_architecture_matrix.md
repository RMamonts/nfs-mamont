# Матрицы покрытия протоколов и архитектуры: nfs-mamont vs nfsserve, zerofs_nfsserve, nfs3_server/nfs3_types, embednfs, fractal-nfs (по исходному коду)

## 1. Матрица процедур NFSv3 (RFC 1813)

### Takeaway
nfs-mamont — единственный крейт, где все 22 процедуры RFC 1813 доходят до бэкенда с типизированными аргументами, включая verifier для EXCLUSIVE и guard для SETATTR. Но это только «проводка»: вся семантика лежит на реализации `Vfs`, а встроенного бэкенда в крейте нет. Из конкурентов все 22 процедуры диспетчеризует только zerofs_nfsserve. У nfs3_server их 20, у nfsserve и fractal-nfs — по 19. Недостающие процедуры у всех отвечают RPC `PROC_UNAVAIL`, а не `NFS3ERR_NOTSUPP`. embednfs вообще не реализует NFSv3: это NFSv4.1-only.

### Cited Findings

#### Прочитанные версии (статическое чтение, без сборки)
| Крейт | Версия / коммит | Источник | Примечание |
|---|---|---|---|
| nfs-mamont | локальный HEAD `5eae9a536f4e…` (2026-10-29, «Use typed enums for NFSv3 wire discriminants (#438)»), на 4 коммита новее релиза 0.2.0 = `d0f2317a5a84…` | `/home/user/nfs-mamont/nfs-mamont/src`; tarball 0.2.0 сверен через `diff -rq`. Все ключевые находки ниже (фрагменты, READ count, `AUTH`, unbounded-каналы, `by_client`, GARBAGE/SYSTEM_ERR) есть и в 0.2.0 | [crates.io nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont) |
| nfsserve (huggingface) | 0.11.0 (2026-04-01), `.cargo_vcs_info` sha `af4f1176dd70…`. Git HEAD `327b4553…` (2026-10-14) отличается от 0.11.0 только doc-комментарием в `src/vfs.rs` | tarball crates.io + `git clone --depth 1` | [crates.io nfsserve](https://crates.io/api/v1/crates/nfsserve) |
| zerofs_nfsserve | 0.19.1 (2026-03-02), sha `d61b08456ae6…` = HEAD `github.com/barre/nfsserve` | tarball + clone | [crates.io zerofs_nfsserve](https://crates.io/api/v1/crates/zerofs_nfsserve) |
| nfs3_server / nfs3_types | 0.11.1 / 0.5.3 (2026-10-21), sha `8179e9d66d65…` = HEAD `github.com/Vaiz/nfs3` | tarball + clone | [crates.io nfs3_server](https://crates.io/api/v1/crates/nfs3_server) |
| embednfs | 0.4.1 (2026-03-10), sha `168a11f80cfa…` (git HEAD `ac1a0164…` не сравнивался). `embednfs-proto` не читался | tarball | [crates.io embednfs](https://crates.io/api/v1/crates/embednfs) |
| fractal-nfs | 0.1.0 (2026-05-21), sha `86200e912baf…`, путь `crates/fs_server/fractal-nfs` в `fractalbits-labs/fractalbits-main` | только tarball (репозиторий не клонировался) | [crates.io fractal-nfs](https://crates.io/api/v1/crates/fractal-nfs) |

Скачивания по crates.io API на 2026-10-30:
- nfsserve: 1 415 987;
- nfs3_types: 61 536;
- zerofs_nfsserve: 53 245;
- nfs3_server: 43 752;
- embednfs: 2 434;
- nfs-mamont: 80;
- fractal-nfs: 44.

Источник — [crates.io API](https://crates.io/api/v1/crates/nfsserve), запросы по каждому крейту.

Ссылки ниже — permalink'и на перечисленные коммиты. Номера строк проверены по прочитанным файлам: `.rs` в tarball совпадают с git.

#### Легенда
- **✓** — реализовано: декодирование, логика и корректный ответ.
- **✓Б** — библиотека делает только XDR и диспетчеризацию, семантика целиком на бэкенде.
- **◐** — частично (что именно не так, указано в ячейке).
- **✗PU** — нет реализации, отвечает RPC `PROC_UNAVAIL`, а не `NFS3ERR_NOTSUPP`.
- **n/a** — неприменимо.

#### Матрица
| Процедура | nfs-mamont (HEAD) | nfsserve 0.11.0 | zerofs_nfsserve 0.19.1 | nfs3_server 0.11.1 | fractal-nfs 0.1.0 |
|---|---|---|---|---|---|
| NULL | ✓ (fast-path в read-task, минует пул VFS) | ✓ | ✓ | ✓ | ✓ |
| GETATTR | ✓Б | ✓ | ✓ | ✓ | ✓Б (default `NotSupp`) |
| SETATTR | ✓Б (guard передаётся бэкенду) | ✓ (guard проверяет сервер, неатомарно) | ✓ (то же) | ✓ (`NOT_SYNC`) | ✓Б (`guard_ctime` бэкенду) |
| LOOKUP | ✓Б | ✓ | ✓ | ✓ | ✓Б |
| ACCESS | ✓Б (`Credential` бэкенду) | ◐ (возвращает запрошенную маску, без проверок прав) | ◐ (то же) | ◐ (то же) | ✓Б (uid/gid бэкенду, но см. §3: не-AUTH_SYS → 0/0) |
| READLINK | ✓Б | ✓ | ✓ | ✓ | ✓Б |
| READ | ✓Б (буфер из аллокатора размером `count`, без clamp) | ✓ (`Vec<u8>` от бэкенда) | ✓ | ✓ | ✓Б (`count` ограничен 16 MiB) |
| WRITE | ✓Б (`committed` и verifier от бэкенда) | ◐ (всегда `FILE_SYNC`) | ◐ (всегда `FILE_SYNC`, verifier = 0) | ✓ (`stable_how` от бэкенда) | ✓Б (проверка `count == len(data)`) |
| CREATE: UNCHECKED / GUARDED / EXCLUSIVE | ✓Б / ✓Б / ✓Б (verifier бэкенду) | ✓ / ◐ (lookup+create, гонка) / ◐ (verifier игнорируется) | ✓ / ◐ / ◐ (то же) | ✓ / ◐ (lookup+create) / ✓ (verifier бэкенду) | ✓Б (весь `CreateHow3` бэкенду) |
| MKDIR | ✓Б | ◐ (`sattr3` не передаётся) | ✓ (attrs передаются) | ◐ (attrs не передаются) | ✓Б |
| SYMLINK | ✓Б | ✓ | ✓ | ✓ | ✗PU |
| MKNOD | ✓Б | ✗PU | ✓ (CHR/BLK/SOCK/FIFO, иначе `BADTYPE`) | ✗PU | ✗PU |
| REMOVE | ✓Б | ✓ | ✓ | ✓ | ✓Б |
| RMDIR | ✓Б | ◐ (тот же обработчик, что REMOVE) | ◐ (то же) | ◐ (то же) | ✓Б (отдельный `rmdir`) |
| RENAME | ✓Б (между бэкендами → `XDEV`) | ✓ | ✓ | ✓ | ✓Б |
| LINK | ✓Б (между бэкендами → `XDEV`) | ✗PU | ✓ | ✗PU | ✗PU |
| READDIR | ✓Б (сервер не урезает ответ по `count`) | ✓ (байтовый бюджет; underflow `dircount-128`) | ✓ (то же) | ✓ (итератор, проверка cookieverf, `TOOSMALL`) | ✓Б |
| READDIRPLUS | ✓Б | ✓ (underflow `maxcount-128`) | ✓ (то же) | ✓ (`TOOSMALL` при `maxcount<128`) | ✓Б |
| FSSTAT | ✓Б | ◐ (захардкоженные значения) | ✓ (default-метод с фиктивными значениями, переопределяем) | ✓ (значения не проверялись) | ✓Б |
| FSINFO | ✓Б | ✓ (default-метод: rtmax/wtmax 1 MiB) | ✓ (то же) | ✓ (default 1 MiB) | ✓Б |
| PATHCONF | ✓Б | ◐ (захардкожено: `linkmax: 0`, `name_max: 32768`) | ✓ (значения не проверялись) | ✓ (значения не проверялись) | ✓Б |
| COMMIT | ✓Б (verifier от бэкенда) | ✗PU | ✓ (default: no-op, возвращает `get_write_verf()`) | ✓ (verifier = generation) | ✓Б |
| **Итого с обработчиком** | 22/22 | 19/22 | 22/22 | 20/22 | 19/22 |

embednfs: n/a. Сервер принимает только программу NFS версии 4 и `minorversion == 1`, см. §2.

#### Доказательства по ячейкам

**nfs-mamont**
- Парсер разбирает все 22 процедуры — [parser_struct.rs:305-338](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L305-L338).
- Воркер VFS вызывает соответствующий метод бэкенда с `&header.cred` — [task/global/vfs.rs:288-319](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L288-L319).
- Трейт `Vfs` = 21 обязательный под-трейт — [vfs/mod.rs:157-207](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/mod.rs#L157-L207).
- NULL отвечается прямо из read-task — [task/connection/read.rs:94-111](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/read.rs#L94-L111).
- RENAME/LINK между разными бэкендами получают `XDev` — [task/global/vfs.rs:63-66](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L63-L66) и [task/global/vfs.rs:256-271](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L256-L271).
- Все три режима CREATE разбираются, verifier передаётся — [parser/nfsv3/create.rs:15-23](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/nfsv3/create.rs#L15-L23), [vfs/create.rs:11-33](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/create.rs#L11-L33).
- READDIR-сериализатор пишет все записи, полученные от бэкенда, без контроля `count` — [serializer/server/nfs/read_dir.rs:19-33](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/nfs/read_dir.rs#L19-L33).
- Ни одной реализации `Vfs` в крейте нет: `impl … for` находится только как blanket-impl в `vfs/mod.rs` (поиск `impl.*GetAttr for|impl.*Vfs.*for` по `src/`) — [vfs/mod.rs:182-207](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/mod.rs#L182-L207).

**nfsserve 0.11.0**
- Диспетчер — [nfs_handlers.rs:132-160](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L132-L160).
- `RMDIR → nfsproc3_remove` — [nfs_handlers.rs:147](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L147).
- MKNOD/LINK/COMMIT → `proc_unavail_reply_message` — [nfs_handlers.rs:152-159](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L152-L159).
- ACCESS: «TODO better checks here», маска режется только для ReadOnly — [nfs_handlers.rs:519-523](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L519-L523).
- WRITE всегда `FILE_SYNC`, `verf: serverid()` — [nfs_handlers.rs:1196-1198](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1196-L1198).
- GUARDED реализован как lookup, затем create — [nfs_handlers.rs:1325-1346](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1325-L1346).
- EXCLUSIVE: verifier даже не десериализуется, `create_exclusive(dirid, name)` — [nfs_handlers.rs:1347-1359](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1347-L1359), [vfs.rs:137](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L137).
- `mkdir(dirid, dirname)` без атрибутов — [vfs.rs:142](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L142).
- FSSTAT захардкожен (`tbytes: 1024^4`…) — [nfs_handlers.rs:691-694](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L691-L694).
- PATHCONF: `linkmax: 0, name_max: 32768` — [nfs_handlers.rs:607-608](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L607-L608).
- READDIR/READDIRPLUS: `args.maxcount as usize - 128` и `args.dircount as usize - 128` — [nfs_handlers.rs:869](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L869), [nfs_handlers.rs:994](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L994).

**zerofs_nfsserve 0.19.1**
- Диспетчер со всеми 22 процедурами — [nfs_handlers.rs:140-164](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L140-L164).
- `RMDIR → nfsproc3_remove` — [nfs_handlers.rs:156](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L156).
- MKNOD — [nfs_handlers.rs:2403-2490](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L2403-L2490); LINK — [nfs_handlers.rs:2525-2609](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L2525-L2609).
- COMMIT по умолчанию — [vfs.rs:293-310](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L293-L310).
- EXCLUSIVE по-прежнему без verifier — [nfs_handlers.rs:1545-1560](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1545-L1560).
- WRITE: `FILE_SYNC` и `serverid()` — [nfs_handlers.rs:1255-1256](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1255-L1256).
- ACCESS — тот же TODO — [nfs_handlers.rs:543-546](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L543-L546).
- underflow — [nfs_handlers.rs:901](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L901), [nfs_handlers.rs:1039](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1039).
- `mkdir(..., attrs)` — [vfs.rs:192-198](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L192-L198).

**nfs3_server 0.11.1**
- Проверка версии, затем диспетчер — [nfs_handlers.rs:30-67](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L30-L67).
- `NFSPROC3_REMOVE | NFSPROC3_RMDIR` — [nfs_handlers.rs:58](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L58).
- `NFSPROC3_MKNOD | NFSPROC3_LINK → PROC_UNAVAIL` — [nfs_handlers.rs:64-66](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L64-L66).
- `mkdir` без атрибутов — [vfs/mod.rs:265-269](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L265-L269).
- EXCLUSIVE с verifier — [nfs_handlers.rs:657-661](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L657-L661).
- GUARDED реализован как lookup, затем create — [nfs_handlers.rs:644-654](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L644-L654).
- guard в SETATTR → `NFS3ERR_NOT_SYNC` — [nfs_handlers.rs:717-721](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L717-L721).
- ACCESS — [nfs_handlers.rs:205-207](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L205-L207).

**fractal-nfs 0.1.0**
- Диспетчер — [dispatch.rs:64-386](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L64-L386).
- `_ => write_reply_proc_unavail` для SYMLINK(10), MKNOD(11) и LINK(15) — [dispatch.rs:382-386](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L382-L386).
- У всех методов трейта есть default-реализации (`NotSupp`/`Rofs`/`Noent`/`Inval`) — [lib.rs:51-242](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/lib.rs#L51-L242).
- READ count ограничен `max_reply_bytes` — [dispatch.rs:154-157](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L154-L157).
- WRITE проверяет `data.len() == count` — [nfs3_wire.rs:142-147](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/nfs3_wire.rs#L142-L147).

**RFC** — [RFC 1813](https://www.rfc-editor.org/rfc/rfc1813.txt), локальный `rfc/nfs/v3/rfc1813.txt`:
- при неподдержке EXCLUSIVE сервер «should fail the CREATE request with the error, NFS3ERR_NOTSUPP» — стр. 3159-3162;
- для MKNOD при неподдержке типов «the error, NFS3ERR_NOTSUPP, should be returned» — стр. 3650-3652.

### Inferences
- Корректнее всего сравнивать «полноту» так: у nfs-mamont полнота **интерфейса** максимальная, а у zerofs_nfsserve максимальна полнота **готовой серверной логики**. nfs-mamont перекладывает на бэкенд всё: wcc, guard, verifier, проверку прав, урезание READDIR по `count`. Поэтому качество протокольной семантики у него неизвестно, пока нет эталонного бэкенда.
- `PROC_UNAVAIL` вместо `NFS3ERR_NOTSUPP` у nfsserve, nfs3_server и fractal-nfs: клиент получает ошибку уровня RPC вместо NFS-статуса. По RFC 1813 правильнее `NFS3ERR_NOTSUPP` (см. выше).
- Общий обработчик REMOVE/RMDIR у семейства nfsserve и у nfs3_server не даёт бэкенду отличить `rmdir` от `unlink`. RMDIR на файле или REMOVE на каталоге может пройти, если бэкенд сам не проверит тип.

### Gaps
- Сборка и тесты не запускались (по условию), поэтому поведение с реальным Linux-клиентом не проверено.
- Не сверялись точные значения PATHCONF/FSSTAT у zerofs_nfsserve и nfs3_server.
- У nfs-mamont нет эталонного бэкенда, поэтому семантика 21 процедуры статически не проверяема.

## 2. Прочие протоколы: MOUNT v3, portmap/rpcbind, NLM/NSM; операции NFSv4.1 у embednfs

### Takeaway
- **MOUNT.** Полный MOUNT v3 (все 6 процедур, проверка версии, сменяемый трейт `Mount`) есть только у nfs-mamont. У всех конкурентов нет DUMP, а у fractal-nfs ещё и UMNTALL.
- **rpcbind.** Настоящую регистрацию в системном rpcbind делает только nfs-mamont (PMAP v2 SET/UNSET с откатом). nfsserve, zerofs_nfsserve и nfs3_server вместо этого держат на том же порту «фальшивый» portmapper (GETPORT всегда возвращает свой порт). У fractal-nfs нет ничего.
- **NLM.** NLM v4 (подмножество NULL/TEST/LOCK/CANCEL/UNLOCK) есть только у nfs-mamont, причём уже в 0.2.0, хотя проект относит NLM к roadmap. NSM нет ни у кого.
- **embednfs** — полноценный NFSv4.1 (сессии, OPEN/LOCK-состояние, ~42 операции); pNFS и делегации там заглушены.

### Cited Findings

#### MOUNT v3
| | nfs-mamont | nfsserve / zerofs_nfsserve | nfs3_server | fractal-nfs | embednfs |
|---|---|---|---|---|---|
| NULL | ✓ | ✓ | ✓ | ✓ | n/a (в NFSv4 нет MOUNT) |
| MNT | ✓ (реестр экспортов; неизвестный путь → `MNT3ERR_ACCES`) | ✓ (один экспорт, префиксное сравнение; иначе `MNT3ERR_NOENT`) | ✓ | ◐ (принимает **любой** путь, отдаёт root FH) | n/a |
| DUMP | ✓ | ✗PU | ✗PU | ✗PU | n/a |
| UMNT | ✓ (ключ — `SocketAddr`, см. баги) | ◐ (состояния нет, только сигнал `mount_signal`) | ✓ | ◐ (no-op) | n/a |
| UMNTALL | ✓ | ◐ (no-op и сигнал) | ✓ | ✗PU | n/a |
| EXPORT | ✓ (список из реестра) | ✓ (один экспорт, без групп) | ✓ | ✓ (только `"/"`) | n/a |
| Проверка версии MOUNT | ✓ `PROG_MISMATCH(3,3)` | ✗ (версия не проверяется) | ✓ | ✗ | n/a |
| `auth_flavors` в ответе MNT | только `AUTH_NONE` | `AUTH_NULL`, `AUTH_UNIX` | `AUTH_NULL`, `AUTH_UNIX` | только `AUTH_NONE` | n/a |

**nfs-mamont**
- Разбор MOUNT-процедур — [parser_struct.rs:344-354](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L344-L354).
- Проверка версии — [parser_struct.rs:450-465](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L450-L465).
- Глобальная последовательная MOUNT-задача — [task/global/mount.rs:57-125](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/mount.rs#L57-L125).
- Трейт `Mount` = `Mnt + Umnt + Umntall + Export + Dump` — [mount/mod.rs:62-64](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/mount/mod.rs#L62-L64).
- `const AUTH: [AuthFlavor; 1] = [AuthFlavor::None]` с комментарием «TODO: should be taken from config» — [service/mount/mod.rs:37-38](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mod.rs#L37-L38). Ответ MNT — [service/mount/mnt.rs:15-47](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mnt.rs#L15-L47).
- Реестр монтирований `HashMap<SocketAddr, HashSet<MountEntry>>` — [service/mount/mod.rs:86-90](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mod.rs#L86-L90).
- Экспорты добавляются и снимаются в рантайме — [service/mount/mod.rs:115-124](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mod.rs#L115-L124).

**nfsserve** — диспетчер MOUNT без проверки `call.vers` и без DUMP — [mount_handlers.rs:39-59](https://github.com/huggingface/nfsserve/blob/af4f117/src/mount_handlers.rs#L39-L59). Flavors — [mount_handlers.rs:104-107](https://github.com/huggingface/nfsserve/blob/af4f117/src/mount_handlers.rs#L104-L107). UMNT/UMNTALL — [mount_handlers.rs:176-207](https://github.com/huggingface/nfsserve/blob/af4f117/src/mount_handlers.rs#L176-L207). В zerofs_nfsserve отличие только в передаче `AuthContext` и форматировании (сравнение `diff` двух tarball).

**nfs3_server** — версия проверяется — [mount_handlers.rs:28-30](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/mount_handlers.rs#L28-L30). `MOUNTPROC3_DUMP → PROC_UNAVAIL` — [mount_handlers.rs:43-50](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/mount_handlers.rs#L43-L50). Flavors — [mount_handlers.rs:93](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/mount_handlers.rs#L93).

**fractal-nfs** — есть только NULL/MNT/UMNT/EXPORT — [mount.rs:18-30](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/mount.rs#L18-L30). «we accept any path» — [mount.rs:42-44](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/mount.rs#L42-L44). `AUTH_NONE` — [mount.rs:50-52](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/mount.rs#L50-L52). Версия MOUNT не проверяется — [dispatch.rs:33-36](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L33-L36).

#### portmap / rpcbind
- **nfs-mamont**
  - Клиент PMAP v2 (`PMAPPROC_SET=1`, `PMAPPROC_UNSET=2`, TCP на `127.0.0.1:111`, тайм-аут 5 с) — [rpcbind/mod.rs:36-62](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L36-L62).
  - Регистрирует NFS v3, MOUNT v3 и NLM v4 на одном TCP-порту — [rpcbind/mod.rs:110-116](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L110-L116).
  - Откатывает частичную регистрацию — [rpcbind/mod.rs:422-446](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L422-L446). Снимает регистрацию при выходе — [lib.rs:98](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L98).
  - Если rpcbind недоступен, пишет только предупреждение «clients must pass port=/mountport= options» — [lib.rs:62-71](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L62-L71).
  - Размер ответа rpcbind ограничен 4096 байтами — [rpcbind/mod.rs:310-334](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L310-L334).
  - Встроенного responder'а нет.
- **nfsserve / zerofs_nfsserve** — встроенный responder: NULL и GETPORT, «We fake a portmapper here. And always direct back to the same host port». SET/UNSET/DUMP/CALLIT → `PROC_UNAVAIL` — [portmap_handlers.rs:40-90](https://github.com/huggingface/nfsserve/blob/af4f117/src/portmap_handlers.rs#L40-L90). В системном rpcbind ничего не регистрируется.
- **nfs3_server** — такой же фальшивый GETPORT плюс проверка версии — [portmap_handlers.rs:19-58](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/portmap_handlers.rs#L19-L58).
- **fractal-nfs** — portmap отсутствует: любая программа, кроме MOUNT и NFS, получает `PROG_UNAVAIL` — [dispatch.rs:33-49](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L33-L49).
- **embednfs** — тоже нет: принимается только программа NFS — [server/transport.rs:131-134](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L131-L134).

#### NLM / NSM
- **nfs-mamont**
  - `NLM_PROGRAM = 100021`, `NLM_VERSION = 4`, процедуры `Null=0, Test=1, Lock=2, Cancel=3, Unlock=4` — [consts/nlm.rs:3-15](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/consts/nlm.rs#L3-L15).
  - Отдельная глобальная `NlmTask` — [task/global/nlm/nlm_task.rs:46-80](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/nlm/nlm_task.rs#L46-L80).
  - In-memory `LockRegistry` — [service/nlm/mod.rs:22-38](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/nlm/mod.rs#L22-L38).
  - Модуль `nlm` приватный и помечен `#[allow(dead_code)]` — [lib.rs:8-9](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L8-L9). При этом `handle_forever` требует `N: Nlm` — [lib.rs:43-55](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L43-L55).
  - Нет GRANTED-callback, асинхронных `_MSG`/`_RES`, SHARE/UNSHARE, NM_LOCK и FREE_ALL. Кода NSM (100024) тоже нет: grep по `nsm|statd|100024|sm_mon` пуст.
- **nfsserve, zerofs_nfsserve, nfs3_server, fractal-nfs** — NLM и NSM отсутствуют (кода нет; неизвестная программа → `PROG_UNAVAIL`, например [nfsserve rpcwire.rs:54-65](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L54-L65)).
- **embednfs** — блокировки сделаны в протоколе: `LOCK`/`LOCKT`/`LOCKU` — [server/compound.rs:362-381](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L362-L381).

#### embednfs: операции NFSv4.1 (вместо матрицы v3)
- `minorversion != 1 → NFS4ERR_MINOR_VERS_MISMATCH` — [server/compound.rs:35-37](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L35-L37). RPC-процедуры: 0 = NULL, 1 = COMPOUND, остальные → `PROC_UNAVAIL` — [server/transport.rs:146-219](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L146-L219).
- **Реализованы** (диспетчер [server/compound.rs:184-398](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L184-L398)):
  - файловые и навигационные: ACCESS, CLOSE, COMMIT, CREATE, GETATTR, GETFH, LINK, LOOKUP, LOOKUPP, OPEN, OPENATTR, OPEN_DOWNGRADE, PUTFH, PUTPUBFH, PUTROOTFH, READ, READDIR, READLINK, REMOVE, RENAME, RESTOREFH, SAVEFH, SETATTR, WRITE, VERIFY, NVERIFY;
  - безопасность: SECINFO (возвращает flavors 1 и 0), SECINFO_NO_NAME;
  - сессии и клиенты: EXCHANGE_ID, CREATE_SESSION, DESTROY_SESSION, SEQUENCE, RECLAIM_COMPLETE, DESTROY_CLIENTID, BIND_CONN_TO_SESSION;
  - стейты и блокировки: FREE_STATEID, TEST_STATEID, LOCK, LOCKT, LOCKU.
- **Заглушки и NOTSUPP:**
  - `DELEGRETURN` и `DELEGPURGE` просто отвечают OK — [compound.rs:360](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L360), [compound.rs:386](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L386).
  - LAYOUTGET/LAYOUTRETURN/LAYOUTCOMMIT, GET_DIR_DELEGATION, WANT_DELEGATION, BACKCHANNEL_CTL, GETDEVICEINFO/GETDEVICELIST, SET_SSV → `NFS4ERR_NOTSUPP`; ILLEGAL → `OP_ILLEGAL` — [compound.rs:399-408](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L399-L408).
  - Операции «MUST NOT implement» → `NOTSUPP` — [compound.rs:361](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L361).
- **Сессия:** 64 слота, `MAX_REQUEST_SIZE = 1_049_620`, `MAX_CACHED_RESPONSE = 6144` — [session/mod.rs:26-28](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/session/mod.rs#L26-L28).

### Inferences
- **rpcbind.** Только nfs-mamont позволяет штатный `mount -t nfs host:/export` без `port=`/`mountport=` на хосте с системным rpcbind. Встроенный «фальшивый» portmapper у nfsserve и nfs3_server работает, лишь когда клиент обращается к нему на нестандартном порту (обычно передаётся `mountport=`/`port=`). На порт 111 его нельзя посадить одновременно с системным rpcbind. Вывод сделан из того, что responder слушает тот же порт, что и NFS ([tcp.rs bind](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L146-L163)).
- **Регистрация nfs-mamont** ограничена PMAP v2/TCP/IPv4: нет netid `tcp6`/`udp` и RPCBIND v3/v4. Клиенты, запрашивающие порт через UDP-маппинг или по IPv6, порт не получат. Это вывод из `IPPROTO_TCP` и `Ipv4Addr::LOCALHOST` в [rpcbind/mod.rs:36-62](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L36-L62).
- **Тезис «NLM на roadmap»** у nfs-mamont неточен: базовый NLM v4 уже поставляется и регистрируется в rpcbind. Он, однако, не подменяем пользователем (приватный трейт) и без NSM не переживает перезапуск клиента или сервера.

### Gaps
- Не проверялось, как Linux mount.nfs реагирует на список flavors `[AUTH_NONE]` из MNT (nfs-mamont, fractal-nfs) при `sec=sys` по умолчанию. Нужен runtime-тест.
- Сериализатор EXPORT в nfs-mamont (передаются ли `names` как groups) подробно не читался.
- `embednfs-proto` не читался, поэтому полнота XDR для NFSv4.1 в нём не оценена.

## 3. RPC-слой: record marking, лимиты, UDP, auth, коды ошибок, DRC

### Takeaway
- **Коды отказов.** nfs-mamont генерирует их точнее всех (`RPC_MISMATCH`, `PROG_UNAVAIL`, `PROG_MISMATCH` с диапазоном, `PROC_UNAVAIL`, `AUTH_BADCRED`/`AUTH_BADVERF`), и его разбор строго ограничен по памяти.
- **Фрагменты.** nfs-mamont не поддерживает многофрагментные записи, закрывает соединение и не имеет DRC. Ошибки декодирования аргументов у него частично уходят в `SYSTEM_ERR` вместо `GARBAGE_ARGS`.
- **Ограничения размеров.** nfsserve, zerofs_nfsserve и nfs3_server собирают фрагменты, но без какого-либо лимита: `Vec::resize` под заявленную длину.
- **Лимиты и коды у остальных.** У embednfs (2 MiB) и fractal-nfs (16 MiB) лимиты есть, но они путают коды отказов RPC.
- **UDP** нет ни у кого.

### Cited Findings

#### Матрица RPC-слоя
| | nfs-mamont | nfsserve 0.11.0 | zerofs_nfsserve 0.19.1 | nfs3_server 0.11.1 / nfs3_types 0.5.3 | embednfs 0.4.1 | fractal-nfs 0.1.0 |
|---|---|---|---|---|---|---|
| Многофрагментная запись | ✗ (ошибка `Unsupported`, соединение закрывается) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Лимит на запись или фрагмент | Явного лимита нет (`0x7FFF_FFFF`), но в памяти держится только окно 2500 Б плюс payload WRITE в буфере пула | нет | нет | нет | 2 MiB на сумму фрагментов | 16 MiB (настраивается) |
| Предаллокация по длине с провода | нет (`vec_max_size` с лимитами) | да (`resize` в XDR `Vec<u8>`/`Vec<u32>`) | да | да (`vec![0; len]`, `with_capacity(len)`) | не проверено (`embednfs-proto`) | нет (чтение из среза с проверкой границ) |
| UDP | нет | нет | нет | нет | нет | нет |
| Разбираемые flavors | `AUTH_NONE` (пустое тело), `AUTH_SYS` (`machinename` ≤ 255, `gids` ≤ 16, тело ≤ 400) | `AUTH_UNIX`; enum знает 0–3 | то же | `AUTH_UNIX`; enum знает 0–3 | `AUTH_SYS` валидируется, `AUTH_NONE`, прочие → `AuthContext::Unknown` | `AUTH_SYS` (uid/gid), прочие → uid=0/gid=0 |
| Принуждение (enforcement) | прочие flavors → `AUTH_BADCRED`; verifier ≠ `AUTH_NONE`/пустой → `AUTH_BADVERF`; `Credential` идёт в каждый метод `Vfs`/`Mount`/`Nlm` | нет; auth в VFS не передаётся | `AuthContext` идёт в VFS | хук `authorize()` → `AUTH_TOOWEAK` | `AUTH_BADCRED`/`AUTH_BADVERF` для битого `AUTH_SYS` | нет |
| RPCSEC_GSS | отказ `AUTH_BADCRED` | ошибка декодирования, соединение закрыто | то же | то же | принимается как `Unknown` | считается uid 0 |
| `RPC_MISMATCH` | ✓ (`MSG_DENIED`, low=high=2) | ◐ (`mismatch_info::default()`, т. е. 0/0) | ◐ (то же) | ✓ (2/2) | ✗ (отправляет `PROG_MISMATCH`) | ✗ (отправляет `PROG_UNAVAIL`) |
| `PROG_UNAVAIL` | ✓ | ✓ | ✓ | ✓ | ✗ (`PROG_MISMATCH`) | ✓ |
| `PROG_MISMATCH` (low/high) | ✓ NFS/MOUNT/NLM | ◐ NFS и portmap, но не MOUNT | ◐ | ✓ NFS/MOUNT/portmap | ✓ (версия ≠ 4) | ✗ (`PROG_UNAVAIL`) |
| `PROC_UNAVAIL` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `GARBAGE_ARGS` | ◐ (только enum, лимит, UTF-8, длина FH; обрыв или мусор → `SYSTEM_ERR`) | ◐ (только несовпадение длины WRITE; прочее → соединение закрывается) | ◐ (то же) | ✓ (ошибка unpack или лишние байты) | ◐ (ошибка COMPOUND → `NFS4ERR_BADXDR`; ошибка заголовка → соединение закрыто) | ✓ |
| `AUTH_ERROR` | ✓ | ✗ | ✗ | ✓ (`TOOWEAK`) | ✓ | ✗ |
| DRC | нет | «трекер ретрансмиссий»: дубликат **отбрасывается** без ответа | то же | то же + лимит 256 активных на клиента → `SYSTEM_ERR` | слотовый replay-кэш NFSv4.1 с fingerprint | нет |
| Получен REPLY или мусорный заголовок | REPLY молча игнорируется; без xid соединение закрывается | соединение закрывается | то же | то же | ошибка заголовка → соединение закрывается | молча пропускается, ответа нет |

#### Доказательства

**nfs-mamont**
- Фрагменты:
  - «Fragmented messages not supported» — [parser_struct.rs:181-188](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L181-L188); в 0.2.0 это стр. 175.
  - Ошибка возникает до чтения xid, а при `xid: None` read-task завершает соединение — [task/connection/read.rs:210-214](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/read.rs#L210-L214).
  - Ответы всегда одним фрагментом — [serialize_struct.rs:327-341](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L327-L341).
- Окно и лимиты:
  - `DEFAULT_SIZE = 2500` — head window, в который должны помещаться все аргументы, кроме данных WRITE — [parser_struct.rs:55-63](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L55-L63), [read_buffer.rs:1-17](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/read_buffer.rs#L1-L17).
  - `MAX_FRAGMENT_SIZE = 0x7FFF_FFFF`, `MAX_AUTH_SIZE = 400` — [consts/rpc.rs:5-15](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/consts/rpc.rs#L5-L15).
  - WRITE-данные проверяются на границу фрагмента — [parser_struct.rs:586-589](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L586-L589).
- Auth:
  - Разбор `AUTH_SYS` с лимитами — [parser/rpc.rs:18-53](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/rpc.rs#L18-L53).
  - Решения по flavor и verifier — [parser_struct.rs:251-282](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L251-L282).
  - `Credential::{None, Sys}` — [rpc/auth.rs:77-93](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpc/auth.rs#L77-L93).
  - Verifier ответа всегда `AUTH_NONE` (TODO #143) — [task/connection/write.rs:41-43](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/write.rs#L41-L43).
- Коды ответов:
  - Отображение ошибок в коды — [serialize_struct.rs:233-285](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L233-L285): `GarbageArgs` для `ImpossibleTypeCast|BadFileHandle|EnumDiscMismatch|MaxElemLimit|IncorrectString` (235-246), `RpcMismatch` (247-253), `AuthError` (254-259), `ProgUnavail` (260-265), `ProcUnavail` (266-271), `ProgMismatch` (272-279), `IO → SystemErr` (280-285).
  - Обрыв данных: `u32()` даёт `Error::InvalidData` — [primitive.rs:28-30](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/primitive.rs#L28-L30); `InvalidData`/`FormatMismatch` превращаются в `rpc::Error::IO` — [parser_struct.rs:68-85](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L68-L85).
  - Проверки версии и процедуры — [parser_struct.rs:428-489](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L428-L489).
  - REPLY игнорируется — [task/connection/read.rs:193-199](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/read.rs#L193-L199).
- DRC: grep `retransm|duplicate|replay|drc` в `src/` дал только посторонние комментарии.

**nfsserve**
- Склейка фрагментов: `append_to.resize(append_to.len() + length, 0)` без лимита — [rpcwire.rs:94-106](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L94-L106).
- XDR `Vec<u8>`: `self.resize(length as usize, 0)` — [xdr.rs:120-125](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L120-L125); `Vec<u32>` — [xdr.rs:152-155](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L152-L155).
- `auth_flavor` знает только `AUTH_NULL..AUTH_DES` — [rpc.rs:94-100](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpc.rs#L94-L100); неизвестное значение enum даёт `InvalidData` — [xdr.rs:24-33](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L24-L33).
- Обработка запроса — [rpcwire.rs:20-74](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L20-L74):
  - `AUTH_UNIX` разбирается в контекст (29-33);
  - `rpcvers != 2 → rpc_vers_mismatch` (34-38);
  - ретрансмиссия → `Ok(false)`, ответа нет (40-45);
  - REPLY → `Err`, соединение закрывается (70-73).
- `rpc_vers_mismatch` с `mismatch_info::default()` — [rpc.rs:496-502](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpc.rs#L496-L502).
- `TransactionTracker`: ключ `(xid, client_addr)`, на каждом запросе `housekeeping` проходит по всей мапе под `Mutex`, удержание 60 с — [transaction_tracker.rs:23-52](https://github.com/huggingface/nfsserve/blob/af4f117/src/transaction_tracker.rs#L23-L52), [tcp.rs:161](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L161).
- WRITE `GARBAGE_ARGS` — [nfs_handlers.rs:1160-1163](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1160-L1163). NFS `PROG_MISMATCH` — [nfs_handlers.rs:125-129](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L125-L129).

**zerofs_nfsserve** — та же склейка — [rpcwire.rs:116-135](https://github.com/barre/nfsserve/blob/d61b084/src/rpcwire.rs#L116-L135) и тот же трекер — [transaction_tracker.rs:23-36](https://github.com/barre/nfsserve/blob/d61b084/src/transaction_tracker.rs#L23-L36). XDR отличается только переименованием макросов (`diff` tarball'ов).

**nfs3_server / nfs3_types**
- Склейка: `self.0.resize(prev_length + fragment_length, 0)` — [rpcwire/messages.rs:41-51](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire/messages.rs#L41-L51).
- `Opaque::unpack`: `vec![0u8; len]` — [opaque.rs:88-105](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/opaque.rs#L88-L105); `Vec<u32>`: `with_capacity(len)` — [primitives.rs:26-42](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/primitives.rs#L26-L42).
- `auth_flavor` = `AUTH_NULL`/`AUTH_UNIX`/`AUTH_SHORT`/`AUTH_DES` — [rpc.rs:125-133](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/rpc.rs#L125-L133).
- Обработка запроса — [rpcwire.rs:30-117](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire.rs#L30-L117):
  - `RPC_MISMATCH` (42-45);
  - `authorize` → `AUTH_TOOWEAK` (47-58);
  - `GARBAGE_ARGS` при ошибке unpack или лишних байтах (102-113).
- Ретрансмиссия → `NoReply`; `TooManyRequests` → `SYSTEM_ERR` — [rpcwire.rs:119-141](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire.rs#L119-L141). Параметры трекера (60 с, 256, 2048) — [tcp.rs:232-241](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L232-L241).
- REPLY → `bail!` — [messages.rs:83-88](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire/messages.rs#L83-L88).

**embednfs**
- Лимиты `MAX_FRAGMENT_SIZE = 2 MiB` — [server/mod.rs:20-23](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L20-L23), [transport.rs:56-73](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L56-L73). Ответ режется на фрагменты — [transport.rs:88-104](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L88-L104).
- Неверные `rpcvers`, программа и версия все дают `encode_rpc_reply_prog_mismatch` — [transport.rs:126-139](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L126-L139).
- Auth: `validate_rpc_auth` — [server/mod.rs:157-171](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L157-L171); `request_context` с `AuthContext::Unknown` — [server/mod.rs:173-190](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L173-L190).
- Replay-кэш SEQUENCE: `prepare_sequence`/`finish_sequence`, fingerprint строится из cred и payload — [transport.rs:154-204](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L154-L204), [server/mod.rs:466-472](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L466-L472).
- Ошибка декодирования COMPOUND → `BadXdr` — [transport.rs:206-215](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L206-L215).

**fractal-nfs**
- Лимиты: `DEFAULT_MAX_RPC_FRAGMENT_BYTES = 16 MiB` — [config.rs:8](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/config.rs#L8); проверки фрагмента и собранного сообщения — [tokio_server.rs:217-241](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L217-L241); `read_opaque` с `ensure()` — [xdr.rs:54-66](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/xdr.rs#L54-L66).
- Auth: не-`AUTH_SYS` → `(0, 0)` — [rpc.rs:73-87](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L73-L87); verifier пропускается — [rpc.rs:89-91](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L89-L91). Хелперов для `RPC_MISMATCH`/`AUTH_ERROR` нет — [rpc.rs:109-147](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L109-L147).
- Коды: `rpc_version` ≠ 2 → `prog_unavail` — [dispatch.rs:26-30](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L26-L30); версия NFS ≠ 3 → `prog_unavail` — [dispatch.rs:38-41](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L38-L41).
- REPLY, как и мусорный заголовок, «skipping» без ответа — [rpc.rs:64-66](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L64-L66), [tokio_server.rs:252-258](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L252-L258).

**UDP** — grep `UdpSocket` по всем шести исходникам пуст: прослушивание TCP-only, см. [nfs-mamont lib.rs:43-48](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L43-L48).

**RFC** — [RFC 5531](https://www.rfc-editor.org/rfc/rfc5531.txt), локальный `rfc/rpc/rfc5531.txt`:
- «A record is composed of one or more record fragments… 0 to (2**31) - 1 bytes» — стр. 855-861;
- `RPC_MISMATCH = 0 /* RPC version number != 2 */` — стр. 612;
- `PROG_MISMATCH = 2` — стр. 603;
- `GARBAGE_ARGS = 4 /* procedure can't decode params */` — стр. 605;
- `opaque body<400>` соблюдает только nfs-mamont (`MAX_AUTH_SIZE`).

### Inferences
- **DoS (сильный вывод).** В nfsserve, zerofs_nfsserve и nfs3_server достаточно 4-байтного заголовка фрагмента с длиной `0x7FFFFFFF`: `Vec::resize` выделит и обнулит около 2 GiB на соединение ещё **до** прихода данных. Несколько соединений приводят к OOM.
- В nfsserve XDR-`Vec<u8>` с заявленной длиной до 4 GiB внутри маленького сообщения тоже вызывает `resize` и реальную аллокацию.
- В nfs3_types `Vec<u32>::with_capacity(len)` для `auth_unix.gids` ([rpc.rs:176-182](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/rpc.rs#L176-L182)) может запросить до 16 GiB. При отказе аллокатора Rust делает abort процесса. Будет ли отказ, зависит от политики overcommit (не проверялось).
- У nfs-mamont, embednfs и fractal-nfs такого класса нет.
- **«DRC» семейства nfsserve/nfs3_server хуже отсутствия DRC.** Дубликат с тем же `(xid, ip:port)` в течение 60 с **не получает ответа вовсе**. Если Linux-клиент переподключится с того же порта и пошлёт ретрансмиссию, запрос «повиснет» до следующего тайм-аута. Вывод сделан из кода трекера; поведение клиента не тестировалось.
- У nfs-mamont без DRC неидемпотентные операции (REMOVE, RENAME, CREATE EXCLUSIVE, LINK) при ретрансмиссии после реконнекта выполнятся повторно. Клиент получит ложные `NOENT`/`EXIST`.
- Корректный exactly-once есть только у embednfs благодаря слотам NFSv4.1.
- **`SYSTEM_ERR` вместо `GARBAGE_ARGS` у nfs-mamont.** Он означает «сервер сломался», а не «клиент прислал мусор». Для фаззинга и интероперабельности это заметное отклонение от RFC 5531.
- **Фрагменты.** Linux-клиент, по распространённой практике, шлёт каждый RPC одним фрагментом, поэтому отсутствие сборки у nfs-mamont на практике проявится редко. Это не проверялось, а RFC 5531 требует поддержки нескольких фрагментов.

### Gaps
- XDR-декодер `embednfs-proto` не читался: неизвестно, есть ли там предаллокация по длине с провода.
- Реальное поведение аллокатора при запросах 2–16 GiB зависит от `vm.overcommit_memory` и не проверялось.
- Не проверено, переиспользует ли текущий Linux-клиент исходный порт при реконнекте, от чего зависит «зависание» с трекерами nfsserve/nfs3_server.

## 4. Семантика: стабильность записи и COMMIT, EXCLUSIVE, wcc_data, READDIR, file handle, ACCESS, guard, ссылки, большие READ/WRITE

### Takeaway
- **nfs-mamont** почти всю NFSv3-семантику делегирует бэкенду. Сюда входят `committed`/verifier, EXCLUSIVE verifier, guard, wcc, cookieverf, ACCESS по `Credential`. Это даёт максимум гибкости и ноль гарантий из коробки.
- **Ограничения handle.** File handle жёстко 9 байт: 1 байт backend id и 8 байт payload. На неверную длину отвечается `GARBAGE_ARGS`.
- **Clamp READ.** Значение `count` у READ не ограничивается rtmax.
- **nfsserve и zerofs_nfsserve** — самые «опасные» для данных: EXCLUSIVE verifier игнорируется, WRITE всегда `FILE_SYNC`. У zerofs write verifier ещё и константный 0.
- **nfs3_server** — самая аккуратная серверная семантика: verifier в EXCLUSIVE, guard, cookieverf в READDIR, `TOOSMALL`. Но при фиксированной generation write verifier перестаёт меняться между перезапусками.
- **embednfs** объявляет `FH4_PERSISTENT` при volatile handle.

### Cited Findings

#### Матрица семантики
| | nfs-mamont | nfsserve | zerofs_nfsserve | nfs3_server | embednfs (v4.1) | fractal-nfs |
|---|---|---|---|---|---|---|
| Стабильность WRITE | `StableHow` запроса бэкенду; `committed` от бэкенда | всегда `FILE_SYNC` | всегда `FILE_SYNC` | бэкенд возвращает `stable_how` | `WriteStability` от бэкенда | бэкенд пишет ответ сам |
| Write verifier | от бэкенда (в `write::Success` и `commit::Success`) | generation = время старта процесса в мс | `serverid()` = 0 (WRITE), `get_write_verf()` = нули (COMMIT) | generation (по умолчанию время старта; `bind_with_generation` задаёт константу) | `state.write_verifier`, генерируется при старте | бэкенд |
| COMMIT | бэкенд | ✗PU | default no-op | бэкенд, verf = generation | `CommitSupport` (опционально) | бэкенд |
| EXCLUSIVE CREATE | `Verifier` бэкенду | verifier не читается | не читается | `createverf3` бэкенду | в паттерне `Exclusive4_1 { attrs, .. }` verifier не извлекается | `CreateHow3` бэкенду |
| wcc_data | заполняет бэкенд (`WccData`); при сбое маршрутизации пустой wcc | сервер делает `getattr` до и после (неатомарно) | то же | то же | n/a (`change_info4`) | бэкенд |
| READDIR cookie, cookieverf, пейджинг | всё бэкенду, сервер `count` не контролирует | cookie = fileid, cookieverf = mtime каталога, проверка закомментирована; байтовый бюджет | cookie от бэкенда, проверка закомментирована | итератор; READDIR проверяет cookieverf (mtime), READDIRPLUS нет; `TOOSMALL` | `readdir(cookie, max_entries)` | бэкенд |
| Формат FH и стабильность | ровно 9 байт: `[backend_id u8][payload 8 B]`; стабильность зависит от бэкенда | 16 байт `[gen LE][fileid LE]`; после рестарта `STALE` | 16 байт `[0][fileid]`; стабилен | `[gen 8 B LE][Handle бэкенда, переменной длины]`; стабилен с `bind_with_generation` | 8-байтный `ObjectId` из in-memory счётчика; не стабилен | 16 байт `[ino BE][fsid BE]`; стабилен, без generation |
| ACCESS и uid/gid | `Credential` в каждый метод; ACCESS решает бэкенд | без проверок; auth в VFS не передаётся | без проверок; `AuthContext` в VFS | без проверок; хук `authorize` | `fs.access(ctx, …)` | `access(…, uid, gid)` бэкенду |
| SETATTR guard | бэкенду (`Option<Guard>`) | сервер сравнивает ctime | то же | сервер → `NOT_SYNC` | n/a | бэкенду (`guard_ctime`) |
| symlink / hardlink | ✓ / ✓ (`XDEV` между бэкендами) | ✓ / ✗ | ✓ / ✓ | ✓ / ✗ | опциональные трейты `Symlinks` / `HardLinks` | ✗ / ✗ |
| rtmax/wtmax, большие READ/WRITE | FSINFO от бэкенда; READ буфер = `count` (без clamp), при `count` > ёмкости пула `SYSTEM_ERR`; WRITE ограничен кадром и пулом | rtmax/wtmax 1 MiB (default, `rtpref = 1024*124`); clamp нет | то же | 1 MiB; clamp нет | `maxrequestsize` ≤ ~1 MiB | clamp READ до 16 MiB |

#### Доказательства

**nfs-mamont**
- WRITE: `StableHow` в аргументах, `committed` и `verifier` в результате — [vfs/write.rs:22-43](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/write.rs#L22-L43).
- COMMIT: `verifier` в `commit::Success` — [vfs/commit.rs:7-15](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/commit.rs#L7-L15).
- `WccData { before, after }` — [vfs/mod.rs:139-143](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/mod.rs#L139-L143); пустой wcc при отсутствии бэкенда или `XDEV` — [task/global/vfs.rs:73-138](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L73-L138).
- Cookie и `CookieVerifier` передаются бэкенду — [vfs/read_dir.rs:116-144](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/read_dir.rs#L116-L144).
- Guard: «implementation must preserve the object attributes and must return… `NotSync`» — [vfs/set_attr.rs:47-96](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/set_attr.rs#L47-L96).
- ACCESS — [vfs/access.rs:56-68](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/access.rs#L56-L68).
- File handle:
  - `NFS3_FHSIZE = 9` («The first byte carries the BackendId») — [consts/nfsv3.rs:34-39](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/consts/nfsv3.rs#L34-L39); то же в 0.2.0 (стр. 32); значение введено коммитом `53a1b24` (#352).
  - Структура `Handle` и `PAYLOAD_SIZE = 8` — [vfs/file.rs:10-51](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/file.rs#L10-L51).
  - Длина ≠ 9 → `Error::BadFileHandle` — [parser/nfsv3/file.rs:13-19](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/nfsv3/file.rs#L13-L19) → `GARBAGE_ARGS` — [serialize_struct.rs:235-246](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L235-L246).
- Имена:
  - `Name::new` отвергает пустые имена, имена со `/` и длиннее 255 — [vfs/file.rs:61-81](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/file.rs#L61-L81).
  - Длина > 255 → `MaxElemLimit` → `GARBAGE_ARGS` — [primitive.rs:88-97](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/primitive.rs#L88-L97).
  - Пустое имя или имя со `/` → `FormatMismatch` → IO → `SYSTEM_ERR` — [parser/nfsv3/file.rs:63-65](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/nfsv3/file.rs#L63-L65).
- READ: аллокация `args.count` без clamp — [parser_struct.rs:645-665](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L645-L665); `allocate` возвращает `None`, если `size > capacity` — [allocator/mod.rs:154-157](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L154-L157). Документация трейта сама говорит про short read при `count` > rtmax — [vfs/read.rs:44-48](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/read.rs#L44-L48).
- WRITE: `size` (count) и длина opaque не сверяются — [parser_struct.rs:562-625](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L562-L625).

**nfsserve**
- generation через `static mut`/`Once` — [vfs.rs:49-59](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L49-L59).
- FH `[gen][id]`: `Less → STALE`, `Greater → BADHANDLE` — [vfs.rs:222-242](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L222-L242).
- `serverid` — [vfs.rs:257-260](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L257-L260).
- FSINFO по умолчанию — [vfs.rs:196-219](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L196-L219).
- cookieverf = mtime — [nfs_handlers.rs:799-804](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L799-L804); проверка закомментирована — [nfs_handlers.rs:861-867](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L861-L867); `cookie: entry.fileid` — [nfs_handlers.rs:896](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L896).
- Guard — [nfs_handlers.rs:1499-1505](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1499-L1505).
- READ: `read(id, offset, count)` без clamp — [nfs_handlers.rs:340-383](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L340-L383).

**zerofs_nfsserve**
- `static GENERATION_NUMBER: u64 = 0` — [vfs.rs:75](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L75).
- `get_write_verf()` возвращает нули — [vfs.rs:306-310](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L306-L310); `serverid` — [vfs.rs:393-395](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L393-L395).
- FH — [vfs.rs:359-378](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L359-L378).
- `AuthContext` — [vfs.rs:7-31](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L7-L31).
- cookie в `DirEntry` — [vfs.rs:44-50](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L44-L50).

**nfs3_server**
- Модульная документация: «We do not use cookie verifier» — [vfs/mod.rs:1-22](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L1-L22).
- `write(…, stable) -> (fattr3, stable_how)` — [vfs/mod.rs:228-234](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L228-L234); `create_exclusive(…, createverf)` — [vfs/mod.rs:255-260](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L255-L260).
- FH-конвертер — [vfs/handle.rs:97-174](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/handle.rs#L97-L174), где `verf()` = generation (171-173). `bind_with_generation` — [tcp.rs:149-155](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L149-L155).
- READDIR с cookieverf → `BAD_COOKIE` — [nfs_handlers.rs:422-441](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L422-L441); READDIRPLUS проверку не делает — [nfs_handlers.rs:334-337](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L334-L337); `TOOSMALL` — [nfs_handlers.rs:343-349](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L343-L349), [nfs_handlers.rs:504-513](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L504-L513).
- cookieverf из mtime — [nfs_ext.rs:14-22](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_ext.rs#L14-L22).
- WRITE и COMMIT verf — [nfs_handlers.rs:552-561](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L552-L561), [nfs_handlers.rs:604](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L604).

**embednfs**
- FH = ровно 8 байт — [server/mod.rs:147-149](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L147-L149). Маппинг `HashMap<F::Handle, ObjectId>` в обе стороны со счётчиком `next_object_id` — [server/mod.rs:82-89](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L82-L89), [server/objects.rs:9-46](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/objects.rs#L9-L46). Удаление из мапы не найдено.
- `FATTR4_FH_EXPIRE_TYPE`: «FH4_PERSISTENT = 0x00» — [attrs.rs:113-117](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/attrs.rs#L113-L117).
- `write_verifier` — [session/mod.rs:80-96](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/session/mod.rs#L80-L96).
- `Exclusive4_1 { attrs: fa, .. }` — [server/ops/file.rs:265](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/ops/file.rs#L265), [server/ops/file.rs:330](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/ops/file.rs#L330).

**fractal-nfs**
- `NfsFh3::new(ino, fsid)` — [nfs3_types.rs:83-95](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/nfs3_types.rs#L83-L95); root = `NfsFh3::new(1, fsid)` — [tokio_server.rs:41](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L41).
- Неизвестные значения `stable_how` трактуются как `Unstable` — [nfs3_wire.rs:136-140](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/nfs3_wire.rs#L136-L140).

**RFC**
- [RFC 1813](https://www.rfc-editor.org/rfc/rfc1813.txt):
  - `NFS3_FHSIZE 64` — стр. 791; `FHSIZE3 = 64 /* Maximum bytes in a V3 file handle */` — стр. 5924;
  - `NFS3ERR_BADHANDLE = 10001` — стр. 894; `NFS3ERR_NAMETOOLONG = 63` — стр. 889;
  - short read при `count` > rtmax — стр. 2590;
  - EXCLUSIVE использует verifier — стр. 3067-3072.
- [RFC 7530](https://www.rfc-editor.org/rfc/rfc7530.txt): определение `FH4_PERSISTENT` — стр. 1832-1835 локального файла.

### Inferences
- **Узкий payload FH у nfs-mamont.** 8 байт payload вмещают только 64-битный inode. Для generation-счётчика (защита от повторного использования inode), идентификатора экспорта или подписи HMAC места нет. nfs3_server, наоборот, допускает handle бэкенда произвольной длины. Ограничение 9 байт законно по RFC (это максимум 64), но архитектурно ограничивает бэкенды.
- **Ответ на чужой FH.** На handle «чужой» длины nfs-mamont отвечает RPC-уровнем `GARBAGE_ARGS`, хотя RFC ожидает `NFS3ERR_BADHANDLE`. Клиент, получивший handle от другого сервера (например, после failover или смены реализации), увидит `EIO`, а не `ESTALE`.
- **READ без clamp.** Нет согласования между FSINFO `rtmax` (его выдаёт бэкенд) и ёмкостью пула. Если бэкенд объявит rtmax больше `Allocator::capacity()`, каждый крупный READ получит `SYSTEM_ERR` вместо short read.
- **nfs3_server с `bind_with_generation`.** FH становятся стабильными, но write verifier тоже перестаёт меняться между перезапусками. Если бэкенд возвращает `UNSTABLE` и теряет данные при падении, клиент не обнаружит перезагрузку и не перешлёт данные.
- **zerofs_nfsserve.** Нулевой verifier безопасен только потому, что WRITE всегда `FILE_SYNC`. Если ZeroFS действительно буферизует запись, но отвечает `FILE_SYNC`, это нарушение контракта стабильности. Это вывод: бэкенд ZeroFS не читался.
- **embednfs.** Объявленный `FH4_PERSISTENT` при in-memory `ObjectId` означает, что после рестарта старый FH может указать на **другой** объект: ID раздаются заново с 1 в порядке lookup'ов. Это нарушение семантики `FH4_PERSISTENT` и риск порчи данных.
- **fractal-nfs.** Без generation в FH повторно использованный inode отдаст клиенту новый файл по старому handle.

### Gaps
- Не проверено, как `embednfs` генерирует `verifier_value` (время или случайность) и обрабатывается ли EXCLUSIVE4_1 verifier где-то ещё.
- Не проверено, как ZeroFS-бэкенд реализует `write` и `get_write_verf`.
- Поведение Linux-клиента на `GARBAGE_ARGS` в ответ на FH-ошибку — вывод, не тест.

## 5. Архитектура: конкурентность, backpressure, буферы, async-трейты, рантайм, API бэкенда, расширяемость, наблюдаемость, shutdown

### Takeaway
- **nfs-mamont** имеет самую «инженерную» архитектуру: конвейер read-task → глобальный пул из N VFS-воркеров → write-task, пул заранее выделенных буферов (опционально mlock), WRITE читается прямо из сокета в буфер пула, READ отправляется через `write_vectored`, native AFIT через `trait_variant`, MSRV 1.75, несколько бэкендов в рантайме.
- **Слабые места nfs-mamont:** unbounded-каналы (backpressure только через пул буферов), жёсткая привязка к tokio, shutdown только по Ctrl-C.
- **Семейство nfsserve и nfs3_server** — по задаче на запрос без лимита и многократное копирование.
- **embednfs и fractal-nfs** обрабатывают запросы строго последовательно в пределах соединения. При этом fractal-nfs — единственный runtime-agnostic вариант (tokio или compio, thread-per-core, `SO_REUSEPORT`).
- **Наблюдаемость.** Метрик и span'ов нет ни у кого, только события `tracing`.

### Cited Findings

#### Матрица архитектуры
| | nfs-mamont | nfsserve | zerofs_nfsserve | nfs3_server | embednfs | fractal-nfs |
|---|---|---|---|---|---|---|
| Конкурентность внутри соединения | read-task и write-task; NFS-вызовы в глобальный пул из `vfs_concurrency` воркеров (MPMC); MOUNT и NLM — по одной глобальной задаче | `tokio::spawn` на каждый запрос | то же | то же | строго последовательно (read → handle → write) | строго последовательно |
| Backpressure | только семафор пула для READ/WRITE; канал команд VFS и канал ответов unbounded | duplex 256 KB перед парсером; spawn и канал ответов unbounded | то же | то же + отказ `SYSTEM_ERR` при >256 активных на клиента | естественная (последовательность) | естественная |
| Буферы и zero-copy | пул `ArrayQueue` + `Semaphore` из одного `alloc_zeroed`, опционально `mlock`; WRITE из сокета в чанки пула; READ через `write_vectored` из пула | стековый буфер 128 KB → duplex → `Vec`; ответ `Vec`; READ `Vec<u8>` от бэкенда | то же | то же (`Box<[u8]>` 128 KiB) | `BytesMut` под запись и `BufWriter`; копия при склейке | `tmp` → `recv_buf` → `message` (копии); бэкенд пишет XDR прямо в `XdrWriter` |
| Стиль async-трейтов | native AFIT, `#[trait_variant::make(Send)]` на 21 под-трейте | `async-trait` (boxed, `dyn`) | `async-trait` | native RPITIT `impl Future + Send` (edition 2024, MSRV 1.88) | `async-trait` | native RPITIT **без** `Send` (локальные рантаймы) |
| Рантайм | только tokio (`tokio::spawn`, `tokio::net`, `signal`) | tokio | tokio + tokio-util | tokio; `process_socket` generic по `AsyncRead + AsyncWrite` | tokio (`features = ["full"]`) | tokio **или** compio (features), `current_thread` на ядро + `SO_REUSEPORT` |
| Форма API бэкенда | handle-based (9-байтный `Handle`), типизированные `Args`/`Success`/`Fail` на процедуру; 21 обязательный метод + `Mount` (5) + `Nlm` (4); ошибка `vfs::Error` (зеркало `nfsstat3`) | id-based (`fileid3` = u64); 15 обязательных методов; `nfsstat3` | id-based + `&AuthContext`; 17 обязательных | generic `type Handle: FileHandle`; 6 (RO) + 9 (RW) = 15 обязательных; `nfsstat3` | generic `type Handle: Clone+Eq+Hash`; 13 обязательных + опциональные трейты; `FsError` | handle = `&NfsFh3`; 0 обязательных (всё default); бэкенд сам кодирует XDR; `Nfsstat3` |
| Точки расширения | трейт `Allocator`; трейт `Mount`; `BackendRegistry` (до 256 бэкендов в рантайме); `Credential` во всех вызовах; добавление и снятие экспортов в рантайме | `mount_signal`, имя экспорта | то же + `AuthContext` | хук `authorize`, `ReadOnlyAdapter`, generation, feature `memfs` | `IdMapper`, `Symlinks`/`HardLinks`/`Xattrs`/`CommitSupport`, `MemFs` | `NfsServerConfig` (потоки, `fsid`, лимит фрагмента), выбор рантайма |
| Наблюдаемость | события `tracing` (debug/warn/error), метрик нет | `tracing` | `tracing` | `tracing` | `tracing` | `tracing` |
| Graceful shutdown | Ctrl-C внутри `handle_forever`: прекращение accept и `unregister` в rpcbind; без дренажа; сигнал не настраивается | нет | `CancellationToken`: прекращение accept и выход из циклов соединений | нет (`Notify` останавливает только cleaner) | нет | внутренний токен; `run()` «currently loop forever» |
| Ошибки `accept` | нефатальные — backoff 50 мс; фатальные (EBADF/EINVAL/ENOTSOCK) — выход | любая ошибка завершает сервер (`?`); `peer_addr().unwrap()` | то же | `?` и `expect("failed to get peer address")` | `?` на `accept` и на `set_nodelay` | логируется, цикл продолжается |

#### Доказательства

**nfs-mamont**
- Конвейер и каналы:
  - Конвейер описан в модульной документации; канал ответов unbounded — [task/connection/mod.rs:1-59](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/mod.rs#L1-L59) (`unbounded` на стр. 46).
  - Пул воркеров на одном unbounded-канале — [task/global/vfs.rs:170-181](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L170-L181); цикл воркера — [task/global/vfs.rs:239-243](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L239-L243).
  - Read-task отправляет в пул через `send().await` на unbounded-канал — [task/connection/read.rs:132-146](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/read.rs#L132-L146).
  - MOUNT — одна задача, тоже на unbounded-канале — [task/global/mount.rs:38-44](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/mount.rs#L38-L44).
  - `vfs_concurrency` задаётся в `ServerContext::new` — [context.rs:63-77](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/context.rs#L63-L77).
- Аллокатор:
  - Трейты `Buffer`/`Allocator` — [allocator/mod.rs:49-94](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L49-L94).
  - `Impl::new`: одна аллокация, опциональный `mlock` — [allocator/mod.rs:109-148](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L109-L148).
  - `acquire_many` на семафоре — [allocator/mod.rs:154-179](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L154-L179).
- Zero-copy:
  - WRITE по чанкам — [parser_struct.rs:591-608](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L591-L608) и [read_buffer.rs:132-149](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/read_buffer.rs#L132-L149).
  - READ через `write_vectored` — [serialize_struct.rs:356-440](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L356-L440).
- API бэкенда:
  - `#[trait_variant::make(Send)]`, например [vfs/create.rs:70-74](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/vfs/create.rs#L70-L74).
  - Зависимости: tokio, trait-variant, async-channel, crossbeam-queue, без XDR-крейтов; MSRV `1.75.0` — [Cargo.toml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/Cargo.toml).
  - Реестр бэкендов: `BackendId` в первом байте FH — [backend/mod.rs:1-45](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/backend/mod.rs#L1-L45).
- Shutdown и accept:
  - Ctrl-C с TODO «will be replaced with other solution in future» — [lib.rs:73-96](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L73-L96).
  - Классификация ошибок `accept` — [lib.rs:104-113](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L104-L113).
- Ошибки записи игнорируются и цикл продолжается — [task/connection/write.rs:49-53](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/write.rs#L49-L53).

**nfsserve**
- Duplex 256000 + `mpsc::unbounded_channel` — [rpcwire.rs:135-148](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L135-L148); spawn на сообщение — [rpcwire.rs:151-176](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L151-L176).
- Цикл сокета с буфером `[0; 128000]` — [tcp.rs:33-85](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L33-L85).
- `accept().await?` и `peer_addr().unwrap()` — [tcp.rs:196-214](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L196-L214).
- `#[async_trait] pub trait NFSFileSystem` — [vfs.rs:94-193](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L94-L193).

**zerofs_nfsserve** — `handle_with_shutdown(CancellationToken)` — [tcp.rs:179-208](https://github.com/barre/nfsserve/blob/d61b084/src/tcp.rs#L179-L208); соединения тоже слушают токен — [tcp.rs:36-96](https://github.com/barre/nfsserve/blob/d61b084/src/tcp.rs#L36-L96). Трейт — [vfs.rs:111-396](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L111-L396).

**nfs3_server**
- `process_socket<IO: AsyncRead + AsyncWrite>` — [tcp.rs:47-102](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L47-L102); `handle_forever` с cleaner'ом — [tcp.rs:286-317](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L286-L317); `Drop → notify_waiters` — [tcp.rs:31-35](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L31-L35).
- Трейты `NfsReadFileSystem`/`NfsFileSystem` — [vfs/mod.rs:51-323](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L51-L323).
- Итераторы READDIR — [vfs/iterator.rs:47-56](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/iterator.rs#L47-L56).
- spawn на сообщение — [rpcwire.rs:199-234](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire.rs#L199-L234).
- edition 2024, `rust-version = "1.88"`, зависимости anyhow/nfs3_types/tokio/tracing (`filetime` опционально) — [crates.io nfs3_server](https://crates.io/api/v1/crates/nfs3_server).

**embednfs**
- Цикл соединения без spawn — [server/transport.rs:35-106](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L35-L106).
- `serve`: `accept().await?` и `set_nodelay(true)?` — [server/mod.rs:112-129](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L112-L129).
- `#[async_trait] pub trait FileSystem` и опциональные трейты — [fs.rs:537-723](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/fs.rs#L537-L723); `IdMapper` — [server/mod.rs:34-53](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L34-L53).

**fractal-nfs**
- Модульная документация: «per-CPU tokio current-thread runtimes and SO_REUSEPORT… the Nfs3Filesystem trait doesn't bound its returned futures with Send» — [tokio_server.rs:1-5](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L1-L5).
- Привязка к портам — [config.rs:64-78](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/config.rs#L64-L78).
- Ошибки `accept` только логируются — [tokio_server.rs:168-185](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L168-L185).
- Последовательное соединение — [tokio_server.rs:190-269](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L190-L269).
- compio-вариант — [compio_server.rs:134-281](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/compio_server.rs#L134-L281); `write_all` копирует каждый чанк через `to_vec()` (269-281).
- «they currently loop forever» — [tokio_server.rs:120](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L120).
- Требование хотя бы одного runtime-feature — [lib.rs:3-7](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/lib.rs#L3-L7).

**Наблюдаемость** — по всем шести исходникам: 0 файлов с `metrics::`/`prometheus`/`opentelemetry`, 0 вхождений `#[instrument]`, `info_span!`, `debug_span!` и `span!(`. Grep по распакованным tarball'ам; пример одного из них: [nfs-mamont task/global/vfs.rs:274-276](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L274-L276), где ошибки выводятся через `error!`.

### Inferences
- **Backpressure nfs-mamont.**
  - Для метаданных (GETATTR, LOOKUP, …) обратного давления нет. Read-task парсит запросы со скоростью сети и складывает их в unbounded-очередь VFS. Медленный бэкенд или флуд клиента дают неограниченный рост памяти.
  - Медленный читатель (клиент не читает ответы) копит ответы в unbounded-канале write-task. Ответы READ при этом держат буферы пула до отправки.
  - Отсюда вероятный кросс-клиентский DoS: один клиент может исчерпать общий пул, и READ/WRITE всех остальных соединений встанут на `acquire_many`. Квот на соединение нет. Вывод основан на [allocator/mod.rs:154-179](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L154-L179) и [task/connection/mod.rs:46](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/mod.rs#L46). Нагрузочно не проверялся.
- **Head-of-line blocking.** Аллокация READ/WRITE-буфера происходит в read-task. При пустом пуле соединение перестаёт читать вообще, в том числе дешёвые запросы метаданных.
- **Прямое кодирование XDR в fractal-nfs.** Бэкенд сам пишет XDR в `XdrWriter`. Это убирает промежуточные структуры, но переносит риск некорректного wire-формата на каждого реализатора бэкенда. У nfs-mamont типизированные `Success`/`Fail` дают строгость.
- **Хрупкий accept у конкурентов.** `accept()?` и `unwrap`/`expect` на `peer_addr` у nfsserve, zerofs_nfsserve и nfs3_server, а также `set_nodelay()?` у embednfs могут завершить весь сервер из-за одного «умершего» соединения (ENOTCONN) или EMFILE. nfs-mamont и fractal-nfs здесь устойчивее.

### Gaps
- Производительность не измерялась: сравнение пропускной способности «пул и пайплайн» (nfs-mamont) против «task на запрос» (nfsserve) против thread-per-core (fractal-nfs) требует бенчмарков другого исследователя.
- Не проверено, сохраняется ли порядок ответов: nfs-mamont и nfsserve отвечают вне порядка. Для ONC RPC это допустимо, но отдельно не тестировалось.

## 6. Топ-3 сильных и слабых стороны каждого крейта относительно nfs-mamont; баги и нарушения спецификаций

### Takeaway
- **Против nfs-mamont у конкурентов три типичных преимущества:**
  - сборка многофрагментных записей (у всех);
  - готовая серверная логика и эталонные бэкенды (memfs у nfs3_server, MemFs у embednfs);
  - проверенность в продакшене (nfsserve: 1,4 млн скачиваний; zerofs_nfsserve используется ZeroFS).
- **Типичные слабости конкурентов:** DoS через предаллокацию у семейства nfsserve и nfs3_server, отсутствие MOUNT DUMP/LINK/MKNOD, неверные коды RPC у embednfs и fractal-nfs, фактическое отсутствие auth (fractal-nfs).
- **Главные дефекты nfs-mamont:**
  - нет многофрагментных записей;
  - 9-байтный FH с `GARBAGE_ARGS` на чужую длину;
  - `SYSTEM_ERR` вместо `GARBAGE_ARGS`/`NAMETOOLONG`;
  - READ без clamp;
  - unbounded-очереди;
  - MNT объявляет только `AUTH_NONE`;
  - реестр монтирований по `ip:port`.

### Cited Findings

#### nfsserve 0.11.0 относительно nfs-mamont
**Сильные стороны**
1. Многофрагментная сборка ([rpcwire.rs:94-106](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L94-L106)) и встроенный GETPORT-responder ([portmap_handlers.rs:73-90](https://github.com/huggingface/nfsserve/blob/af4f117/src/portmap_handlers.rs#L73-L90)): сервер запускается без системного rpcbind.
2. Простой id-based API: 15 методов, default `fsinfo`/`id_to_fh`/`path_to_id` ([vfs.rs:94-261](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L94-L261)). Сервер сам считает wcc, guard и байтовый бюджет READDIR ([nfs_handlers.rs:889-931](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L889-L931)), тогда как nfs-mamont всё отдаёт бэкенду.
3. Зрелость и распространённость: 1 415 987 скачиваний против 80 у nfs-mamont ([crates.io](https://crates.io/api/v1/crates/nfsserve)).

**Слабые стороны**
1. Нет защиты от DoS:
   - склейка фрагментов без лимита ([rpcwire.rs:99-103](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L99-L103));
   - `resize` в XDR по длине с провода ([xdr.rs:123](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L123));
   - spawn на запрос и unbounded-канал ([rpcwire.rs:135-176](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L135-L176)).

   У nfs-mamont окно 2500 Б и пул.
2. Меньше протокола: MKNOD/LINK/COMMIT и DUMP отвечают `PROC_UNAVAIL`, EXCLUSIVE verifier игнорируется, auth не передаётся в VFS, нет NLM и rpcbind ([nfs_handlers.rs:152-159](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L152-L159)).
3. FH «протухают» при каждом рестарте ([vfs.rs:49-59](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L49-L59)). Zero-copy нет (`Vec<u8>` в `read`). Используется `async-trait`. Сервер падает на первой ошибке accept ([tcp.rs:198](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L198)).

#### zerofs_nfsserve 0.19.1 относительно nfs-mamont
**Сильные стороны**
1. Обработчики всех 22 процедур, включая MKNOD для четырёх типов, LINK и COMMIT, живут в самой библиотеке ([nfs_handlers.rs:140-164](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L140-L164)).
2. `AuthContext` (uid/gid/gids) передаётся во все вызовы VFS ([vfs.rs:7-31](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L7-L31)). FH стабилен между рестартами благодаря gen = 0 ([vfs.rs:75](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L75)).
3. Управляемый shutdown через `CancellationToken` ([tcp.rs:179-208](https://github.com/barre/nfsserve/blob/d61b084/src/tcp.rs#L179-L208)) плюс многофрагментная сборка. У nfs-mamont только Ctrl-C.

**Слабые стороны**
1. Тот же DoS-профиль, что у nfsserve ([rpcwire.rs:116-135](https://github.com/barre/nfsserve/blob/d61b084/src/rpcwire.rs#L116-L135)).
2. Целостность данных:
   - EXCLUSIVE без verifier ([nfs_handlers.rs:1545-1560](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1545-L1560));
   - verifier равен 0, перезагрузку не обнаружить ([vfs.rs:306-310](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L306-L310));
   - WRITE всегда `FILE_SYNC`.
3. Слабая RPC-гигиена (мусорный запрос закрывает соединение), «DRC», отбрасывающий ретрансмиссии, нет DUMP, NLM и регистрации в rpcbind.

#### nfs3_server 0.11.1 + nfs3_types 0.5.3 относительно nfs-mamont
**Сильные стороны**
1. Многофрагментная сборка, `GARBAGE_ARGS` на любую ошибку unpack и на лишние байты, хук `authorize` → `AUTH_TOOWEAK`, `PROG_MISMATCH` для всех программ ([rpcwire.rs:42-113](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire.rs#L42-L113)). У nfs-mamont ошибки декодирования частично становятся `SYSTEM_ERR`.
2. Гибкий FH: generic `Handle` переменной длины и настраиваемая generation ([vfs/handle.rs:15-174](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/handle.rs#L15-L174)) против жёстких 8 байт payload у nfs-mamont. Потоковый readdir через итераторы с ограничением по байтам и `TOOSMALL` ([nfs_handlers.rs:343-363](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L343-L363)).
3. Экосистема: отдельный переиспользуемый крейт типов `nfs3_types`, `ReadOnlyAdapter`, feature `memfs` для тестов, native RPITIT ([vfs/mod.rs:51-323](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/vfs/mod.rs#L51-L323)).

**Слабые стороны**
1. DoS: склейка фрагментов без лимита ([messages.rs:47](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire/messages.rs#L47)) и предаллокация в `nfs3_types` ([opaque.rs:93](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/opaque.rs#L93), [primitives.rs:33](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/primitives.rs#L33)); spawn на запрос.
2. Протокол:
   - MKNOD/LINK и DUMP отвечают `PROC_UNAVAIL`;
   - RMDIR обрабатывается как REMOVE;
   - ACCESS без проверок;
   - нет NLM, portmap фальшивый ([nfs_handlers.rs:58-66](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L58-L66)).
3. Копирование на всём пути, отбрасывание ретрансмиссий, `expect` на `peer_addr` в цикле accept ([tcp.rs:296-302](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L296-L302)), MSRV 1.88 против 1.75.

#### embednfs 0.4.1 относительно nfs-mamont
**Сильные стороны**
1. NFSv4.1 с сессиями и exactly-once (слотовый replay-кэш с fingerprint, [transport.rs:154-204](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L154-L204)). Один порт, MOUNT, rpcbind и NLM не нужны. Блокировки и состояние OPEN — в протоколе ([compound.rs:184-398](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/compound.rs#L184-L398)).
2. Ограничение записи 2 MiB, фрагментированные ответы, согласованный `maxrequestsize` ([transport.rs:56-104](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L56-L104), [session/clients.rs:165-166](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/session/clients.rs#L165-L166)).
3. Богатый API: опциональные трейты для symlink, hardlink, xattr и commit, `IdMapper`, встроенный `MemFs` ([fs.rs:537-723](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/fs.rs#L537-L723)).

**Слабые стороны**
1. Неверные отказы RPC: `PROG_MISMATCH` вместо `RPC_MISMATCH` и `PROG_UNAVAIL` ([transport.rs:126-134](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L126-L134)). Неизвестные flavors принимаются ([server/mod.rs:173-190](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L173-L190)).
2. Volatile FH при объявленном `FH4_PERSISTENT` и бесконечно растущая мапа handle↔ID ([attrs.rs:113-117](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/attrs.rs#L113-L117), [objects.rs:9-46](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/objects.rs#L9-L46)).
3. Последовательная обработка в соединении, нет пула буферов и zero-copy, `async-trait`. Ошибка `accept` или `set_nodelay` останавливает сервер ([server/mod.rs:119-120](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L119-L120)).

#### fractal-nfs 0.1.0 относительно nfs-mamont
**Сильные стороны**
1. Runtime-agnostic: tokio или compio (io_uring), thread-per-core + `SO_REUSEPORT`, `Send` не требуется ([tokio_server.rs:1-5](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L1-L5), [config.rs:64-78](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/config.rs#L64-L78)). nfs-mamont привязан к tokio.
2. Строгие лимиты входа:
   - 16 MiB на фрагмент и на сообщение (настраивается);
   - clamp READ;
   - проверка `count == len` у WRITE;
   - многофрагментная сборка;
   - устойчивый accept-цикл.

   Источники: [tokio_server.rs:217-241](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L217-L241), [dispatch.rs:154-157](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L154-L157).
3. Минимальный бэкенд: ноль обязательных методов, ответ пишется прямо в `XdrWriter` без промежуточных структур ([lib.rs:51-242](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/lib.rs#L51-L242)).

**Слабые стороны**
1. Безопасность: любой не-`AUTH_SYS` вызов (включая `AUTH_NONE`) получает uid/gid 0, ответов `AUTH_ERROR` нет, MNT принимает любой путь ([rpc.rs:84-87](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L84-L87), [mount.rs:42-44](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/mount.rs#L42-L44)).
2. Протокол:
   - нет SYMLINK, MKNOD, LINK, а также DUMP/UMNTALL;
   - `PROG_UNAVAIL` отправляется вместо `RPC_MISMATCH`/`PROG_MISMATCH`;
   - portmap и rpcbind отсутствуют совсем ([dispatch.rs:26-41](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L26-L41)).
3. Последовательная обработка в соединении, нет DRC, нет API для shutdown, FH без generation. Бэкенд сам отвечает за корректность XDR.

#### Баги и нарушения спецификаций (file:line)

**nfs-mamont (HEAD; эти же места есть в 0.2.0)**
1. Многофрагментные записи отвергаются, соединение закрывается после дочитывания до 2 GiB «хвоста» — [parser_struct.rs:181-188](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L181-L188), [read_buffer.rs:153-174](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/read_buffer.rs#L153-L174), [read.rs:210-214](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/read.rs#L210-L214). Нарушает RFC 5531, стр. 855-861.
2. FH длины ≠ 9 → `GARBAGE_ARGS`, а не `NFS3ERR_BADHANDLE` — [parser/nfsv3/file.rs:13-19](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/nfsv3/file.rs#L13-L19), [serialize_struct.rs:235-246](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L235-L246). RFC 1813, стр. 894.
3. Оборванные или нечитаемые аргументы и лишние байты → `SYSTEM_ERR` вместо `GARBAGE_ARGS` — [primitive.rs:28-30](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/primitive.rs#L28-L30), [parser_struct.rs:75-82](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L75-L82), [parser_struct.rs:497-505](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L497-L505), [serialize_struct.rs:280-285](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L280-L285). RFC 5531, стр. 605.
4. Имя длиннее 255 → `GARBAGE_ARGS` вместо `NFS3ERR_NAMETOOLONG`; пустое имя или имя со `/` → `SYSTEM_ERR` — [primitive.rs:88-97](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/primitive.rs#L88-L97), [parser/nfsv3/file.rs:63-65](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/nfsv3/file.rs#L63-L65). RFC 1813, стр. 889.
5. `count` у READ не ограничивается rtmax: при `count` > ёмкости пула ответ `SYSTEM_ERR` вместо short read — [parser_struct.rs:655-661](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L655-L661), [allocator/mod.rs:155-157](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/allocator/mod.rs#L155-L157). RFC 1813, стр. 2590.
6. WRITE не сверяет `count` с длиной данных — [parser_struct.rs:571-624](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/parser_struct.rs#L571-L624). fractal-nfs и nfsserve это делают.
7. MNT объявляет только `AUTH_NONE`, хотя `AUTH_SYS` принимается и передаётся бэкенду — [service/mount/mod.rs:37-38](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mod.rs#L37-L38).
8. Реестр монтирований по `SocketAddr` (ip:port): UMNT с другого исходного порта запись не снимает, DUMP копит устаревшие записи — [service/mount/mod.rs:86-90](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mod.rs#L86-L90), [service/mount/umnt.rs:10-19](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/umnt.rs#L10-L19).
9. MNT не применяет `ExportEntry.names` (ACL хостов) и игнорирует `cred` — [service/mount/mnt.rs:15-47](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/service/mount/mnt.rs#L15-L47), [mount/mod.rs:43-50](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/mount/mod.rs#L43-L50).
10. Unbounded-каналы VFS, MOUNT и ответов не дают backpressure для метаданных — [task/global/vfs.rs:170-181](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/global/vfs.rs#L170-L181), [task/connection/mod.rs:46](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/mod.rs#L46).
11. Ошибки записи или сериализации глотаются, ответ на этот xid теряется, клиент ждёт ретрансмиссии — [task/connection/write.rs:45-53](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/task/connection/write.rs#L45-L53), [serialize_struct.rs:361-367](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L361-L367).
12. Трейт `Nlm` лежит в приватном модуле, но стоит в bounds публичного `handle_forever`. Подменить NLM-сервис нельзя, NLM — подмножество без NSM — [lib.rs:8-9](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L8-L9), [lib.rs:43-55](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L43-L55), [consts/nlm.rs:9-15](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/consts/nlm.rs#L9-L15).
13. Shutdown жёстко завязан на Ctrl-C без дренажа; регистрация в rpcbind только PMAP v2, TCP, IPv4 — [lib.rs:73-74](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/lib.rs#L73-L74), [rpcbind/mod.rs:36-62](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/rpcbind/mod.rs#L36-L62).

**nfsserve 0.11.0**
1. Склейка без лимита — [rpcwire.rs:99-103](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpcwire.rs#L99-L103).
2. XDR `resize` по длине с провода — [xdr.rs:123](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L123), [xdr.rs:155](https://github.com/huggingface/nfsserve/blob/af4f117/src/xdr.rs#L155).
3. Underflow `maxcount - 128` и `dircount - 128`: паника в debug, в release — ответ больше лимита клиента — [nfs_handlers.rs:869](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L869), [nfs_handlers.rs:994](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L994).
4. EXCLUSIVE без verifier — [nfs_handlers.rs:1347-1359](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L1347-L1359). RFC 1813, стр. 3067-3072.
5. MKNOD/LINK/COMMIT → `PROC_UNAVAIL` — [nfs_handlers.rs:152-159](https://github.com/huggingface/nfsserve/blob/af4f117/src/nfs_handlers.rs#L152-L159).
6. Нет MOUNT DUMP, версия MOUNT не проверяется — [mount_handlers.rs:39-59](https://github.com/huggingface/nfsserve/blob/af4f117/src/mount_handlers.rs#L39-L59).
7. `RPC_MISMATCH` с low=high=0 — [rpc.rs:496-502](https://github.com/huggingface/nfsserve/blob/af4f117/src/rpc.rs#L496-L502).
8. Трекер отбрасывает ретрансмиссии без ответа и делает O(n) уборку на каждом запросе — [transaction_tracker.rs:23-52](https://github.com/huggingface/nfsserve/blob/af4f117/src/transaction_tracker.rs#L23-L52).
9. `accept()?` и `peer_addr().unwrap()` — [tcp.rs:198-201](https://github.com/huggingface/nfsserve/blob/af4f117/src/tcp.rs#L198-L201).
10. `static mut` для generation — [vfs.rs:49-59](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L49-L59).
11. Странный `rtpref = 1024 * 124` — [vfs.rs:205](https://github.com/huggingface/nfsserve/blob/af4f117/src/vfs.rs#L205).

**zerofs_nfsserve 0.19.1**
- Наследует nfsserve-баги 1–3, 4, 6, 8, 9. Соответствующие места: [rpcwire.rs:127](https://github.com/barre/nfsserve/blob/d61b084/src/rpcwire.rs#L127), [nfs_handlers.rs:901](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L901), [nfs_handlers.rs:1545-1560](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1545-L1560), [tcp.rs:187-190](https://github.com/barre/nfsserve/blob/d61b084/src/tcp.rs#L187-L190).
- Новое: WRITE берёт verifier из `serverid()`, а COMMIT — из `get_write_verf()`. При переопределении `get_write_verf` они расходятся — [nfs_handlers.rs:1255-1256](https://github.com/barre/nfsserve/blob/d61b084/src/nfs_handlers.rs#L1255-L1256), [vfs.rs:293-310](https://github.com/barre/nfsserve/blob/d61b084/src/vfs.rs#L293-L310).

**nfs3_server 0.11.1 / nfs3_types 0.5.3**
1. Склейка без лимита — [messages.rs:47](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire/messages.rs#L47).
2. Предаллокация в `nfs3_types` — [opaque.rs:93](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/opaque.rs#L93), [primitives.rs:33](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/xdr_codec/primitives.rs#L33).
3. Ретрансмиссии отбрасываются — [rpcwire.rs:129-134](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/rpcwire.rs#L129-L134).
4. READDIR возвращает `BAD_COOKIE` при любом изменении mtime каталога, а READDIRPLUS — нет (несогласованно) — [nfs_handlers.rs:422-441](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L422-L441), [nfs_handlers.rs:334-337](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L334-L337).
5. `RPCSEC_GSS` не декодируется, соединение закрывается — [rpc.rs:125-133](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_types/src/rpc.rs#L125-L133).
6. `expect` на `peer_addr` внутри accept-цикла — [tcp.rs:299-302](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/tcp.rs#L299-L302).
7. MKNOD/LINK/DUMP → `PROC_UNAVAIL` — [nfs_handlers.rs:64-66](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/nfs_handlers.rs#L64-L66), [mount_handlers.rs:48-50](https://github.com/Vaiz/nfs3/blob/8179e9d/crates/nfs3_server/src/mount_handlers.rs#L48-L50).

**embednfs 0.4.1**
1. `rpcvers` ≠ 2 и неизвестная программа → `PROG_MISMATCH` — [transport.rs:126-134](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/transport.rs#L126-L134). RFC 5531, стр. 612.
2. `FH4_PERSISTENT` объявлен при volatile handle — [attrs.rs:113-117](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/attrs.rs#L113-L117). RFC 7530, стр. 1832-1835.
3. Мапа handle↔ID не вычищается — [objects.rs:9-46](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/objects.rs#L9-L46).
4. Неизвестные flavors не отвергаются — [server/mod.rs:173-190](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L173-L190).
5. `set_nodelay()?` и `accept()?` останавливают `serve` — [server/mod.rs:119-120](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/mod.rs#L119-L120).
6. EXCLUSIVE4_1 verifier не извлекается — [ops/file.rs:265](https://github.com/PeronGH/embednfs/blob/168a11f/crates/embednfs/src/server/ops/file.rs#L265). Это вывод из паттерна.

**fractal-nfs 0.1.0**
1. `PROG_UNAVAIL` при `rpcvers` ≠ 2 и при неверной версии NFS — [dispatch.rs:26-41](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L26-L41). RFC 5531, стр. 603 и 612.
2. Не-`AUTH_SYS` → uid=0, gid=0 — [rpc.rs:84-87](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/rpc.rs#L84-L87).
3. MNT принимает любой путь, версия MOUNT не проверяется — [mount.rs:42-44](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/mount.rs#L42-L44), [dispatch.rs:33-36](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L33-L36).
4. Недопустимый `stable_how` превращается в `UNSTABLE` вместо `GARBAGE_ARGS` — [nfs3_wire.rs:136-140](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/nfs3_wire.rs#L136-L140).
5. На битый заголовок ответа нет — [tokio_server.rs:252-258](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/tokio_server.rs#L252-L258).
6. SYMLINK/MKNOD/LINK → `PROC_UNAVAIL` — [dispatch.rs:382-386](https://github.com/fractalbits-labs/fractalbits-main/blob/86200e9/crates/fs_server/fractal-nfs/src/dispatch.rs#L382-L386).

#### Проверка заявлений nfs-mamont (README и Cargo)
- **«hand-written ONC RPC/XDR layer»** — подтверждено. Есть собственные `parser/` и `serializer/`, XDR-крейтов в зависимостях нет ([Cargo.toml](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/Cargo.toml)).
- **«MOUNT»** — подтверждено, все 6 процедур (§2).
- **«полный набор процедур RFC 1813»** — подтверждено на уровне парсера и трейта (§1), но без встроенного бэкенда и с делегированием семантики.
- **«zero-copy allocator»** — подтверждено с оговорками. Данные WRITE попадают в буфер пула за одну копию из ядра; первые байты, уже попавшие в окно 2500 Б, копируются через `memcpy`. READ уходит через `writev` прямо из пула. Источники: [read_buffer.rs:132-149](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/parser/read_buffer.rs#L132-L149), [serialize_struct.rs:389-436](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/serializer/server/serialize_struct.rs#L389-L436).
- **«NLM/NSM на roadmap»** — неточно: NLM v4 (подмножество) уже есть (§2), NSM действительно нет.
- **«NFSv4 на roadmap»** — подтверждено: `NFS_VERSION = 3` — единственная версия ([consts/nfsv3.rs:3-4](https://github.com/RMamonts/nfs-mamont/blob/5eae9a5/nfs-mamont/src/consts/nfsv3.rs#L3-L4)).

### Inferences
- **Приоритеты исправлений для nfs-mamont, если цель — паритет с конкурентами по устойчивости и совместимости:**
  1. Сборка многофрагментных записей с явным лимитом. Так делают embednfs и fractal-nfs, не повторяя ошибку семейства nfsserve.
  2. Корректные коды: `GARBAGE_ARGS` для любых ошибок декодирования, `NFS3ERR_BADHANDLE` и `NFS3ERR_NAMETOOLONG`.
  3. Clamp `count` у READ до min(rtmax, ёмкость пула).
  4. Ограниченные (bounded) каналы или квоты на соединение.
  5. `AUTH_SYS` в списке flavors для MNT.
  6. Ключ реестра монтирований по IP.
  7. Эталонный бэкенд (memfs или passthrough) для проверки семантики.
- **Явные архитектурные преимущества nfs-mamont**, которых нет ни у одного конкурента:
  - глобальный лимит конкурентности бэкенда;
  - пул буферов с mlock и zero-copy;
  - несколько бэкендов в одном сервере;
  - реальная регистрация в rpcbind;
  - полный MOUNT;
  - NLM.

  Их стоит подчёркивать в отчёте.

### Gaps
- Все выводы о поведении под нагрузкой и о DoS статические: стенд не поднимался.
- Не читались: `embednfs-proto`; ZeroFS-бэкенд к zerofs_nfsserve; git-история fractalbits-main (кроме tarball 0.1.0).
- Сравнение с git HEAD embednfs (`ac1a0164…`) не делалось.
