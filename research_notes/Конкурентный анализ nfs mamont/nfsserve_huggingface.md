# nfsserve (huggingface/nfsserve, ранее xetdata/nfsserve): профиль конкурента nfs-mamont

Срез на **2026-10-30**. HEAD ветки `main` — `327b455` от 2026-10-14. Ссылки на код ведут на этот коммит. Локальный клон: `/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/nfsserve_huggingface/repo/nfsserve`. Сырые JSON-ответы crates.io API лежат в `/tmp/claude-0/-home-user-nfs-mamont/d21d69b2-45fa-5397-92a4-351867428d0d/scratchpad/nfsserve_huggingface/api/`. Из сессии не открылись huggingface.co, docs.rs и geekwire.com (не резолвятся, сетевые ограничения). Факты по ним взяты из поисковых сниппетов и помечены. «Вывод» означает умозаключение автора заметок, а не факт из источника.

## 1. История и владение: XetHub → Hugging Face, мейнтейнеры, лицензия, вложения HF

### Takeaway
nfsserve написан в XetHub (основной автор — Yucheng Low/ylow), импортирован в отдельный репозиторий 2023-06-04 и опубликован 2023-06-05. После покупки XetHub компанией Hugging Face (объявлено 2024-08-08) репозиторий переехал в организацию `huggingface`, а в 0.11.0 (2026-04-01) метаданные переписаны на HF. HF поддерживает крейт ровно настолько, насколько это нужно продукту hf-mount: выпустила релиз-«переупаковку», навела CI-гигиену и подключила ботов. Функциональной разработки в upstream в 2025–2026 годах почти нет.

### Cited Findings
- Первый коммит — 2023-06-04, автор Ajit Banerjee, сообщение «First import of nfsserve rust code into nfsserve from xethub» (`a58719b`) — [commit a58719b](https://github.com/huggingface/nfsserve/commit/a58719b).
- Крейт создан на crates.io 2023-06-05: версия 0.10.1, опубликовал `port8080` (Ajit Banerjee) — [crates.io API](https://crates.io/api/v1/crates/nfsserve), [owners API](https://crates.io/api/v1/crates/nfsserve/owners).
- Мотивация из README: «I wanted to implement a user-mode file-system mount that is truly cross-platform… NFS». Довод против FUSE: «FUSE is annoying to users on Mac and Windows (drivers necessary)». Суть подхода: «a FUSE-like user-mode filesystem API that basically works by creating a localhost NFSv3 server you can mount» — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- Раньше крейт использовался в pyxet/xet-core (команда «xet mount»). В 0.11.0 README переписан: «remove xetdata/pyxet and xethub references», теперь там «This is used in hf-mount» — [commit af4f117](https://github.com/huggingface/nfsserve/commit/af4f117), [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md). **Устарело:** TODO в README по-прежнему ссылается на `xet-core/.../gitxetcore/src/xetmnt/mod.rs` — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- XetHub основали в 2021 году бывшие инженеры Apple Yucheng Low, Ajit Banerjee и Rajat Arya. HF объявила о покупке 2024-08-08. Условия не раскрыты; в HF перешли 14 сотрудников; это «largest acquisition» HF. Цель — хранилище для HF (Git для ТБ-репозиториев, chunking, дедупликация, «instant repository mounting and file streaming») — [SiliconANGLE](https://siliconangle.com/2024/08/08/hugging-face-acquires-xethub-enhance-ai-storage-infrastructure/), [GeekWire](https://www.geekwire.com/2024/hugging-face-acquires-seattle-data-storage-startup-xethub/), [VentureBeat](https://venturebeat.com/ai/hugging-face-acquires-xethub-from-ex-apple-researchers-for-large-ai-model-hosting). Здесь только поисковые сниппеты: страницы не открывались, а [блог HF](https://huggingface.co/blog/xethub-joins-hf) недоступен из сессии.
- Первичное подтверждение покупки из GitHub: ylow 2024-08-09 в PR #29 написал «Apologies as you might have heard we just got acquired by HuggingFace» — [PR #29](https://github.com/huggingface/nfsserve/pull/29).
- В `Cargo.toml` 0.11.0 указаны `authors = ["Rajat Arya <rajat@huggingface.co>", "Assaf Vayner <assaf@huggingface.co>"]` (было `"Yucheng Low <ylow@xethub.com>"`). homepage/repository теперь `github.com/huggingface/nfsserve` (было `xetdata/nfsserve`) — [Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b455/Cargo.toml), diff `6ec8752..af4f117` в клоне.
- Владельцы крейта на crates.io на 2026-10-30: `port8080` (Ajit Banerjee), `assafvayner` (Assaf Vayner), `rajatarya` (Rajat Arya) и команда `github:huggingface:xet` — [owners API](https://crates.io/api/v1/crates/nfsserve/owners).
- Лицензия BSD-3-Clause. В LICENSE: «Copyright (c) 2023, XetData / Copyright (c) 2025, Hugging Face» — [LICENSE](https://github.com/huggingface/nfsserve/blob/327b455/LICENSE).
- Авторы коммитов (43 коммита в `main` на 2026-10-30, `git shortlog -sne`):
  - Yucheng Low — 21 (≈49%, в том числе все merge-коммиты 2023–2024);
  - Roland Rauch — 3, Ajit Banerjee — 3, Assaf Vayner — 2, dignifiedquire — 2;
  - по одному: Denis Bazhenov, DrRac27, Han Young, Nathan Hawkins, Pauline Bailly-Masson, Protryon, Reiner Singer (@sap.com), Sai Asish Y, lashomb;
  - 3 бота: dependabot, hf-dependantbot-rollout, hf-security-analysis.

  Итого 15 людей — локальный клон, `git shortlog`.
- Вклад HF в 2026 году:
  - Assaf Vayner 2026-04-01 сделал коммиты «update deps and lint (#1) (#40)» и «Update package metadata for Hugging Face ownership (#41)» с bump до 0.11.0 — [commit b98ff73](https://github.com/huggingface/nfsserve/commit/b98ff73), [commit af4f117](https://github.com/huggingface/nfsserve/commit/af4f117).
  - На следующий день он же перевёл hf-mount на новую версию: «chore: bump nfsserve dependency from 0.10 to 0.11 (#93)», 2026-04-02 — [hf-mount a935efb](https://github.com/huggingface/hf-mount/commit/a935efb).
  - Остальное — CI-гигиена: Dependabot (#45, 2026-06-18), hardening workflow ботом hf-security-analysis (#51, 2026-09-18), закрепление Actions по SHA (#53, 2026-10-14) — `git log` клона, [pulls?closed](https://github.com/huggingface/nfsserve/pulls?q=is%3Apr+is%3Aclosed).
- XciD (Adrien) — Member HF и основной автор hf-mount: первый коммит hf-mount сделан 2026-02-28 («chore: init»). 2026-04-27 он открыл draft PR #44 «feat: add portmap_listener for Windows NFS clients»; на 2026-10-30 PR не влит — [PR #44](https://github.com/huggingface/nfsserve/pull/44), история hf-mount (`git log --reverse`).
- Ветка hf-mount `feat/windows-nfs-support` (последний коммит 2026-05-04) содержит комментарий «# Pin nfsserve to the HF fork until the portmap_listener PR is merged & released» и строку `nfsserve = { git = "https://github.com/huggingface/nfsserve.git", branch = "feat/portmap-listener" }` — [hf-mount feat/windows-nfs-support Cargo.toml](https://github.com/huggingface/hf-mount/blob/feat/windows-nfs-support/Cargo.toml).
- 2026-10-19 ylow открыл PR #55 (LINK) из личного форка `ylow:main`; GitHub показывает у него бейдж «Contributor» — [PR #55](https://github.com/huggingface/nfsserve/pull/55).
- Workflow CI называется «xet-core CI», то есть скопирован из xet-core — [ci.yml](https://github.com/huggingface/nfsserve/blob/327b455/.github/workflows/ci.yml).

### Inferences
- Коммерческая поддержка есть, но косвенная. У HF есть права на публикацию и бюджет на CI-гигиену, а видимого ответственного за ревью и развитие протокола нет. Продукт-«якорь» — hf-mount: релиз 0.11.0 вышел за сутки до перевода hf-mount на 0.11, и его сделал тот же человек.
- Даже собственная доработка HF (Windows-portmapper, PR #44) полгода висит в draft. Значит, upstream для HF — не приоритет: продукту проще держать патч на ветке.
- Бейдж «Contributor» у ylow не доказывает, что он ушёл из HF: членство в организации бывает скрытым.

### Gaps
- Официальный текст HF о судьбе nfsserve после покупки не прочитан (huggingface.co заблокирован). Публичных данных о команде или бюджете HF на nfsserve не найдено.
- Текущая роль ylow в HF неизвестна.

## 2. Версии и каденс релизов (до 0.11.0)

### Takeaway
За 3,4 года вышло всего три версии: 0.10.1 (2023-06-05), 0.10.2 (2024-04-03) и 0.11.0 (2026-04-01). Между 0.10.2 и 0.11.0 прошло почти ровно два года. Влитые в 2024 году фичи ждали релиза около 18 месяцев. Тегов, GitHub Releases и CHANGELOG нет, MSRV не объявлен. 0.11.0 по публичному API практически совместим с 0.10.2.

### Cited Findings

Версии на 2026-10-30 — [crates.io API](https://crates.io/api/v1/crates/nfsserve):

| Версия | Дата | Опубликовал | Загрузок | Размер | rust_version |
|---|---|---|---|---|---|
| 0.10.1 | 2023-06-05 | port8080 | 16 125 | 43 253 B | — |
| 0.10.2 | 2024-04-03 | port8080 | 1 226 696 | 39 802 B | — |
| 0.11.0 | 2026-04-01 | assafvayner | 173 166 | 38 336 B | — |

Ни одна версия не отозвана (yank).

- Тегов нет (`git tag -l` пуст). Блок Releases на GitHub пуст. Файла CHANGELOG нет — локальный клон, [страница репо](https://github.com/huggingface/nfsserve).
- Интервалы: 0.10.1 → 0.10.2 — около 10 месяцев, 0.10.2 → 0.11.0 — 728 дней. После 0.11.0 релизов не было 7 месяцев (на 2026-10-30) — [crates.io API](https://crates.io/api/v1/crates/nfsserve).
- Что вошло в 0.10.2: Display для `nfsstring` (#9), вынос примеров за feature `demo` (#10), «default readdir implementation based on readdirplus» (#11), снижение уровня логов (#12), публичный модуль `xdr` (#14), `fsinfo` перенесён в VFS-трейт (#16) — `git log` клона, [closed PRs](https://github.com/huggingface/nfsserve/pulls?q=is%3Apr+is%3Aclosed).
- 0.10.2 опубликован 2024-04-03 с ветки `ajit/fix-cargo-publish` (коммит `6ec8752`). PR #22 с этой веткой влит в `main` только 2024-06-21 — `git log` клона. Отдельно пользователь просил о публикации в issue #19 «Publish v0.10.2 to crate.io», закрыт 2024-04-28 — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue).
- Что вошло в 0.11.0:
  - #28 — исправление «non-unique fileids», но меняет только `examples/demo.rs`;
  - #31 — детектор ретрансмиссий `TransactionTracker`, влит 2024-09-23;
  - #32 — настраиваемое имя экспорта `with_export_name`, влит 2024-09-23;
  - #40 — обновление зависимостей и линт: удалены `bytestream`, `futures`, `tracing-attributes`, `smallvec`; `num-derive` 0.3 → 0.4; добавлен CI;
  - #41 — метаданные HF.

  Источники: `git show --stat 66b19cf`, `git diff 6ec8752 af4f117 -- Cargo.toml` в клоне.
- Совместимость API. В diff публичных строк `6ec8752..af4f117` единственное видимое добавление — `pub fn with_export_name`. Остальные изменения (`RPCContext`, `SocketMessageHandler::new`, `TransactionTracker`) лежат в приватных модулях: `lib.rs` объявляет `mod context; mod rpcwire; mod transaction_tracker;` — [lib.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/lib.rs). В hf-mount переход на 0.11 изменил одну строку `Cargo.toml` и `Cargo.lock` — [hf-mount a935efb](https://github.com/huggingface/hf-mount/commit/a935efb).
- Коммиты в `main` по годам: 2023 — 18, 2024 — 18, **2025 — 0**, 2026 — 7. После 2024-09-23 следующий коммит в `main` появился только 2026-04-01 — `git log` клона.
- Готовое, но невыпущенное: ветка `feat/portmap-listener`, 3 коммита от 2026-04-27, +327 строк (`src/portmap_listener.rs`) — `git log origin/main..origin/feat/portmap-listener` в клоне.

### Inferences
- Релизы выходят по событиям (нужды XetHub в 2023–2024, нужды hf-mount в 2026), а не по графику. Пользователи 0.10.x (требование `^0.10`) годами не получали влитые исправления: ретрансмиссии, имя экспорта, нужное Windows.
- Формально 0.10 → 0.11 — semver-major (для 0.x меняется минор), поэтому зависимости с `"0.10"` на 0.11 сами не обновляются. Отсюда длинный хвост загрузок 0.10.2 (см. раздел 3).

### Gaps
- Нет официального changelog, поэтому состав релизов восстановлен по `git log`. Не проверено, был ли 0.10.2 на crates.io побайтно равен коду `6ec8752`.

## 3. Принятие: кто реально генерирует ~1,42 млн загрузок

### Takeaway
Загрузки не объясняются обратными зависимостями на crates.io: таких крейтов 14, у всех вместе около 3,4 тыс. загрузок, это ~0,24% от загрузок nfsserve. Почти весь объём — CI-сборки неопубликованных приложений с GitHub. 80% всех загрузок за всю историю приходятся на последние 90 дней. Вероятные главные источники: ontola/atomic-server (0.10.2) в всплеске августа — начала октября 2026 и trycua/cua (0.11.0) с 1 октября 2026. Флагманский «осмысленный» пользователь — hf-mount (HF).

### Cited Findings
- Всего загрузок **1 415 987**, за 90 дней — **1 132 218** (на 2026-10-30) — [crates.io API](https://crates.io/api/v1/crates/nfsserve). Значит, с 2023-06 по ~2026-08-01 набралось лишь около 283,8 тыс.
- Окно 2026-08-02…2026-10-30 по версиям: 0.10.2 — 971 065 (85,8%), 0.11.0 — 160 863 (14,2%), 0.10.1 — 544 — [downloads API](https://crates.io/api/v1/crates/nfsserve/downloads).
- По месяцам (там же):
  - 0.10.2: август — 255 899, сентябрь — 531 015, октябрь (по 30-е) — 184 151;
  - 0.11.0: август — 15 000, сентябрь — 18 669, октябрь — 127 194.
- Пик — 2026-09-17: 57 210 загрузок за день, из них 56 957 у 0.10.2. 2026-10-06 у 0.10.2 было 29 635, 2026-10-07 — 8 330, дальше ~0,7–4 тыс. в день. 0.11.0 вырос с <1,5 тыс. в день до 2,3–5 тыс. 1–3 октября и до 10 114 2026-10-20 — [downloads API](https://crates.io/api/v1/crates/nfsserve/downloads).
- Средние за 90 дней: в будни 14 162 в день, в выходные 8 478 в день — расчёт по тому же JSON.
- `reverse_dependencies`: `meta.total = 14` — [reverse_dependencies API](https://crates.io/api/v1/crates/nfsserve/reverse_dependencies?per_page=100). Все крейты мелкие (всего / за 90 дней на 2026-10-30):
  - heddle-mount — 1 335 / 955;
  - aegis-orchestrator-core — 840 / 230;
  - foundation_nativeapis — 531 / 439;
  - braid_rs — 119 / 19;
  - internxt-cli — 108 / 63;
  - mq-mount — 105 / 105;
  - ossuary-mount — 87 / 87;
  - ghfs — 66 / 10;
  - braid_http_rs — 50 / 9;
  - pando-vcs — 45 / 45;
  - netget-tool — 38 / 38;
  - braid-core — 33 / 5;
  - tapfs — 26 / 2;
  - leyline-fs — 17 / 6.

  Сумма ≈3 400. Семь требуют `^0.10`, семь — `^0.11` — [пример: heddle-mount](https://crates.io/api/v1/crates/heddle-mount).
- GitHub «Used by» (числа приблизительные) на 2026-10-30: 51 repository и 15 packages. На первой странице видны:
  - trycua/cua — 31 357★ / 2 309 форков;
  - ontola/atomic-server — 1 631★;
  - huggingface/hf-mount — 820★;
  - supermemoryai/smfs — 491★;
  - OpenCoven/coven — 75★;
  - bnsd55/openscion — 30★;
  - agucova/oxcrypt — 16★;
  - benbrittain/jj-yak — 11★;
  - superradcompany/monofs — 9★.

  Источник — [network/dependents](https://github.com/huggingface/nfsserve/network/dependents).
- **trycua/cua.** `libs/cua/crates/cua-volume/Cargo.toml`: `nfsserve = { version = "0.11", optional = true }`, feature `nfs = ["dep:nfsserve"]`, комментарий «…through nfsserve (BSD-3-Clause)». nfsserve 0.11.0 стоит в пяти `Cargo.lock`: `apps/cua-spaces/src-tauri`, `libs/cua`, `libs/cua-spacesd`, `libs/cua-spacesd/tests/spaces-e2e`, `examples/openkoalabot-example-tauri`. Крейт `cua-volume` добавлен **2026-10-01** коммитом `91668174` «feat: merge updated sdk from cua-staging (#4397)» — [cua-volume Cargo.toml](https://github.com/trycua/cua/blob/514b2a8/libs/cua/crates/cua-volume/Cargo.toml), [commit 91668174](https://github.com/trycua/cua/commit/91668174).
- Активность cua: коммитов по неделям W38–W44 — 25, 130, 294, 140, 321, 290, 119. Не меньше 20 workflow-файлов вызывают `cargo build/test/check/clippy` — blobless-клон cua, `git log`, `.github/workflows`.
- **ontola/atomic-server.** `desktop/Cargo.toml`: `nfsserve = "0.10"`, в `Cargo.lock` версия 0.10.2. Зависимость появилась **2026-07-15** в коммите `e85a7c8a` «feat(desktop): read-only NFS virtual drive — mount your Atomic drives» — [desktop/Cargo.toml](https://github.com/ontola/atomic-server/blob/54dc025/desktop/Cargo.toml), [commit e85a7c8a](https://github.com/ontola/atomic-server/commit/e85a7c8a).
- Активность atomic-server: коммитов по неделям W31–W44 — 87, 91, 112, 171, **38**, 84, 101, 166, 160, 54, 39, 49, 14, 10. В октябре коммиты вида «Batch 2026-10-05 (big): 30 PRs combined (#2085)». Номера PR выросли с #1325 (2026-09-01) до #2402 (2026-10-30); часть коммитов от «Cursor Agent» — клон atomic-server (`git log`).
- **huggingface/hf-mount.** `nfsserve = { version = "0.11", optional = true }`, feature `nfs`, в `Cargo.lock` 0.11.0 из реестра. Первый коммит — 2026-02-28. 2026-03-23 Julien Chaumond сделал «docs: refresh README for launch (#48)» — [hf-mount Cargo.toml](https://github.com/huggingface/hf-mount/blob/cb84c3e/Cargo.toml), `git log` клона.
- hf-mount: «NFS (recommended) — works everywhere, no root, no kernel extension». Через Homebrew на macOS ставится только NFS-бэкенд. 820★ / 74 форка на 2026-10-30 — [hf-mount](https://github.com/huggingface/hf-mount). Пакет `hf-mount-nfs` есть на conda-forge — [anaconda.org](https://anaconda.org/conda-forge/hf-mount-nfs) (сниппет поиска).
- **supermemoryai/smfs** использует 0.11.0. Комментарий в манифесте: «nfsserve compiles on both macOS and Linux (pure Rust). fuser is Linux-only because on macOS it would require macFUSE, which we deliberately avoid by using NFSv3-over-localhost instead» — [smfs Cargo.toml](https://github.com/supermemoryai/smfs/blob/f4a0f50/Cargo.toml).
- **OpenCoven/coven** (`crates/coven-afs`) использует 0.11.0 — [coven-afs Cargo.toml](https://github.com/OpenCoven/coven/blob/aa7581e/crates/coven-afs/Cargo.toml).
- **superradcompany/monofs** (последний коммит 2025-04-11) и **benbrittain/jj-yak** (2024-12-18) застряли на 0.10.2 — [monofs Cargo.toml](https://github.com/superradcompany/monofs/blob/210aef7/Cargo.toml), [jj-yak Cargo.toml](https://github.com/benbrittain/jj-yak/blob/16a36d7/Cargo.toml).

### Inferences
- Объём 0.10.2 за август — начало октября 2026, вероятнее всего, дают CI- и агентные сборки atomic-server. Три совпадения:
  1. зависимость появилась 2026-07-15, прямо перед взлётом загрузок;
  2. недельный провал активности (W35, 38 коммитов) совпадает с падением загрузок 26–30 августа (~2–3,5 тыс. в день против 9–18 тыс.);
  3. в сентябре идут «батчи» по 18–30 PR, а снижение активности в W40–W41 близко к обвалу 0.10.2 после 2026-10-07.

  Это корреляция, а не доказательство: crates.io не раскрывает, кто скачивает.
- Рост 0.11.0 с 2026-10-01 по времени точно совпадает с появлением `cua-volume` в trycua/cua — это лучший кандидат. Конечные пользователи hf-mount ставят бинарники (Homebrew, Releases, conda) и crates.io почти не нагружают.
- Метрика «1,42 млн загрузок» — в основном шум CI нескольких крупных репозиториев. Реальная база — около 50 репозиториев на GitHub и ~15 пакетов. При этом у nfsserve есть реальная ниша: «смонтировать что угодно на macOS без macFUSE» (hf-mount, smfs, cua, coven). В этой нише он стандарт де-факто.

### Gaps
- Точной атрибуции загрузок нет и не будет: crates.io не публикует данные о потребителях.
- Не просмотрены страницы 2+ GitHub dependents, так что крупные зависимые за пределами первой страницы не исключены.
- Утверждение заказчика «самая скачиваемая Rust-библиотека NFSv3-сервера» здесь не перепроверялось: другие крейты не сравнивались.

## 4. Функциональность и дизайн API (`NFSFileSystem`, процедуры, MOUNT, portmapper, auth, транспорт, file handle, клиенты)

### Takeaway
API простой: async-трейт `NFSFileSystem` на `#[async_trait]`, адресация по 64-битному `fileid3`, 15 обязательных методов. Это и сделало крейт популярным. Но реализация неполная:
- LINK, MKNOD и COMMIT отвечают RPC-ошибкой `PROC_UNAVAIL`, а не `NFS3ERR_NOTSUPP`;
- FSSTAT и PATHCONF зашиты константами;
- `AUTH_UNIX` разбирается, но не применяется и не передаётся в VFS;
- WRITE всегда объявляет `FILE_SYNC`;
- cookie в readdir — это `fileid`, что даёт известные баги пагинации;
- только TCP, нет NLM, нет graceful shutdown;
- file handle после рестарта сервера становится STALE.

### Cited Findings
- Трейт `NFSFileSystem: Sync` с `#[async_trait]`.
  - Обязательные методы: `capabilities`, `root_dir`, `lookup`, `getattr`, `setattr`, `read`, `write`, `create`, `create_exclusive`, `mkdir`, `remove`, `rename`, `readdir`, `symlink`, `readlink`.
  - Методы с реализацией по умолчанию: `readdir_simple`, `fsinfo`, `id_to_fh`, `fh_to_id`, `path_to_id` (обходит дерево через `lookup`), `serverid`.
  - Адресация по `fileid3` (u64), не по пути. Сигнатуры: `read(...) -> Result<(Vec<u8>, bool), nfsstat3>`, `write(&self, id, offset, data: &[u8]) -> Result<fattr3, nfsstat3>`. `mkdir` не получает `sattr3`, `create_exclusive` не получает verifier. Хуков для link, mknod, commit, fsstat, pathconf и access нет.
  - Источник — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs).
- Документированные требования трейта: «getattr needs to be fast», «The 0 fileid is reserved», readdir должен уметь начинать с любого `start_after` — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs).
- Процедуры NFSv3, которые обрабатываются: NULL, GETATTR, SETATTR, LOOKUP, ACCESS, READLINK, READ, WRITE, CREATE, MKDIR, SYMLINK, REMOVE, RMDIR, RENAME, READDIR, READDIRPLUS, FSSTAT, FSINFO, PATHCONF. RMDIR обрабатывается той же `nfsproc3_remove` (строка 147). Остальное (`MKNOD`, `LINK`, `COMMIT`) падает в ветку `warn!("Unimplemented message…")` и `proc_unavail_reply_message` — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs) (`handle_nfs`, ~строки 120–160).
- PR #55 подтверждает последствие: «The server answered LINK requests with "procedure unavailable," so `ln a b` failed on every mount» — [PR #55](https://github.com/huggingface/nfsserve/pull/55).
- WRITE всегда отвечает `committed: stable_how::FILE_SYNC` и `verf: context.vfs.serverid()` (≈строка 1197), а COMMIT не реализован — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs). В режиме NFS hf-mount пишет через «staging files and async flush» — [hf-mount README](https://github.com/huggingface/hf-mount).
- FSSTAT захардкожен (строки 691–697): `tbytes/fbytes/abytes = 1 TiB`, `tfiles/ffiles/afiles = 1Gi`, `invarsec = u32::MAX`. PATHCONF тоже (строки 607–612): `linkmax 0`, `name_max 32768`, `case_insensitive false`, `case_preserving true` — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs). PR #48: «every nfsserve-backed file system reports a 1 TiB volume with 1 TiB free»; на macOS `df` показывал `1.0Ti` для тома 3,6 TiB — [PR #48](https://github.com/huggingface/nfsserve/pull/48).
- FSINFO по умолчанию: `rtmax 1 MiB`, `rtpref 1024*124`, `wtmax/wtpref 1 MiB`, `maxfilesize 128 GiB`, `properties = FSF_SYMLINK | FSF_HOMOGENEOUS | FSF_CANSETTIME` (без `FSF_LINK`) — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs).
- ACCESS: для ReadWrite-ФС клиенту возвращаются все запрошенные биты, для ReadOnly они урезаются до `READ|LOOKUP`; в коде комментарий «// TODO better checks here» (строки 493–529) — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs).
- Auth. Учётные данные `AUTH_UNIX` разбираются в `context.auth` — [rpcwire.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/rpcwire.rs). Методы VFS контекст не получают — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs). Обсуждения:
  - Issue #15 «Handle auth» закрыт как «not planned» 2024-04-29. ylow: проект «mainly for localhost operation, acting like a FUSE mount» — [issue #15](https://github.com/huggingface/nfsserve/issues/15).
  - Issue #38 «How to do permission control?» открыт с 2025-12-04. Единственный ответ дал не мейнтейнер (rockwotj, 2026-06-04): «NFSv3 assumes a trusted network - you have to control access at the network level» — [issue #38](https://github.com/huggingface/nfsserve/issues/38).
  - PR #29 с расширенным интерфейсом (uid/gid в вызовах) открыт с 2024-06-26 — [PR #29](https://github.com/huggingface/nfsserve/pull/29).
- MOUNT v3: реализованы NULL, MNT, UMNT, UMNTALL, EXPORT; `DUMP` отвечает `PROC_UNAVAIL` — [mount_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/mount_handlers.rs). README: «our Mount protocol implementation is entirely stateless» — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- Подписка на mount/umount: `set_mount_listener(mpsc::Sender<bool>)`. Имя экспорта задаётся через `with_export_name` (с 0.11.0) — [tcp.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/tcp.rs).
- Portmapper встроен в тот же TCP-порт. Отвечает только на `PMAPPROC_NULL` и `PMAPPROC_GETPORT`, причём всегда возвращает собственный порт («We fake a portmapper here»). SET, UNSET, DUMP и CALLIT отвечают `PROC_UNAVAIL` — [portmap_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/portmap_handlers.rs).
- Программы `NFS_ACL` (100227), `NFS_ID_MAP` (100270) и `NFS_METADATA` (200024) получают `PROG_UNAVAIL` — [rpcwire.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/rpcwire.rs).
- Транспорт — только TCP (`tokio::net::TcpListener`), UDP нет. Вызов `NFSTcpListener::bind("ip:port", fs)`; режим `"auto"` перебирает адреса `127.88.x.y`. NFS, MOUNT и portmap живут на одном порту. `handle_forever()` — «Loops forever and never returns» — [tcp.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/tcp.rs). PR #46 о транспорте поверх любого `AsyncRead+AsyncWrite` (TLS) закрыт без merge 2026-08-05 — [PR #46](https://github.com/huggingface/nfsserve/pull/46).
- Graceful shutdown отсутствует. В hf-mount написано: «The NFS accept loop in `nfsserve::handle_forever()` would keep running after `mount_nfs` returned, leaking the bound TCP port until process exit». Исправлено явным `.abort()` в коммите `8bce82f` ветки `feat/windows-nfs-support` hf-mount — [hf-mount commit 8bce82f](https://github.com/huggingface/hf-mount/commit/8bce82f).
- File handle — 16 байт: 8 байт generation (время старта сервера в мс, `static mut` + `Once`, один `unsafe`) плюс 8 байт fileid (LE). После рестарта сервера handle даёт `NFS3ERR_STALE`. Это задумано: README пишет «If the server has restarted, all previous handles will therefore be "expired"». Схему можно переопределить через `id_to_fh`/`fh_to_id` — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs), [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- READDIR/READDIRPLUS:
  - `cookie = fileid`;
  - `cookieverf` вычисляется из mtime каталога, но не проверяется: блок с `NFS3ERR_BAD_COOKIE` закомментирован, рядом пояснение «The best solution is simply to really completely avoid sending BAD_COOKIE»;
  - лимит записей — `dircount / 16`;
  - каждая запись сериализуется во временный `Vec` и учитывается по байтам.

  Источник — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs) (строки ~795–960).
- Баги пагинации:
  - PR #49: fileid не уникален в каталоге (hard links, `.`/`..` в корне), из-за чего листинг «rewinds the listing or skips entries»; `cp -r` и `rsync` могут молча терять файлы. Фикс с отдельным cookie ломает API — [PR #49](https://github.com/huggingface/nfsserve/pull/49).
  - PR #35: «When directory has more than ~26 files `ls` will show duplicate files» — [PR #35](https://github.com/huggingface/nfsserve/pull/35).
- Retransmission detection (0.11.0): `TransactionTracker` с ключом `(xid, client_addr)` и хранением 60 с. Дубликаты просто отбрасываются без ответа; кэша ответов (DRC) нет — [transaction_tracker.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/transaction_tracker.rs), [rpcwire.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/rpcwire.rs).
- NLM/NSM нет. Монтировать нужно с `nolock` (Linux) или `nolocks` (macOS) — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md). hf-mount на macOS использует `locallocks` («macOS mount_nfs treats `nolocks` as…», «keeps lock handling inside the client kernel (no NLM…»); его строка опций: `locallocks,vers=3,tcp,rsize=1048576,actimeo=…,port=…,mountport=…`. На Linux — `nolock,vers=3,tcp,…` — [hf-mount src/nfs.rs](https://github.com/huggingface/hf-mount/blob/cb84c3e/src/nfs.rs), [commit eee917b «fix(nfs): use locallocks on macOS so flock/fcntl work (#106)»](https://github.com/huggingface/hf-mount/commit/eee917b).
- Совместимость клиентов по README:
  - Linux: `mount.nfs -o user,noacl,nolock,vers=3,tcp,…,port=11111,mountport=11111`;
  - macOS: `mount_nfs -o nolocks,vers=3,tcp,…`;
  - Windows: «"kinda" works (only on Windows 11 Pro with the NFS server), but prints a lot of garbage due to various unimplemented APIs. Windows 11 somehow tries to poll with very old NFS protocols constantly».

  Источник — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- Windows подробнее:
  - Клиенту нужен portmapper на порту 111. По PR #44, `mount.exe` «always queries portmapper at port 111»; PR в draft, но «Verified end-to-end on Windows Server 2022» — [PR #44](https://github.com/huggingface/nfsserve/pull/44). Ранее это поднималось в issue #18 «Windows NFS server port need to be 111», закрыт 2024-03-28 — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue).
  - Переименование папок на Windows ломалось из-за экспорта `/`. rrauch: «caused by the NFS export path being set to `/` and the Windows client not liking it very much». Исправлено в PR #32 (2024-09), но в релиз попало только в 0.11.0 (2026-04) — [issue #24](https://github.com/huggingface/nfsserve/issues/24).
- **Устарело в README:** «we implement `READDIR_PLUS` but not `READDIR`», хотя READDIR реализован с PR #11 (2024-02). Ещё README отсылает к `demofs.rs` и `bin/main.rs`, которых нет; реальные файлы — `examples/demo.rs` и `examples/mirrorfs.rs` — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md), `git log` клона.
- ОС сервера. `fs_util` компилируется только при `#[cfg(not(target_os = "windows"))]` — [lib.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/lib.rs). atomic-server утверждает обратное: «`nfsserve` reaches for `std::os::unix::fs::{MetadataExt, PermissionsExt}` in fs_util.rs, so it does not compile for windows at all; windows ships without the virtual drive» — [atomic-server desktop/Cargo.toml](https://github.com/ontola/atomic-server/blob/54dc025/desktop/Cargo.toml). Это противоречит коду и тому, что HF запускает сервер под Windows в ветке hf-mount ([PR #44](https://github.com/huggingface/nfsserve/pull/44)).

### Inferences
- `PROC_UNAVAIL` вместо `NFS3ERR_NOTSUPP` — ошибка на уровне RPC, а не процедуры. Клиенты видят EIO или «procedure unavailable» вместо аккуратного EOPNOTSUPP (подтверждено для `ln` в PR #55).
- `FILE_SYNC` без COMMIT перекладывает гарантии долговечности целиком на `write()` бэкенда. Если бэкенд буферизует (как staging в hf-mount), клиент считает данные стабильными раньше, чем они реально сохранены.
- Cookie = fileid и отсутствие проверки `cookieverf` — архитектурное решение «под macOS-клиент». Оно даёт тихие пропуски и дубликаты в больших каталогах, а исправить его можно только с поломкой API (#49).
- Модель безопасности — «localhost, доверенный пользователь»: auth не применяется, VFS не знает uid/gid. Для многопользовательского или сетевого сервера крейт без доработки не годится.

### Gaps
- На реальных клиентах (Linux/macOS/Windows) ничего не проверялось; сборка под Windows не запускалась (код из сети не исполняли).
- Содержимое PR #50, #58 и #30 подробно не читалось: известны только заголовки, авторы и даты.

## 5. Производительность

### Takeaway
Конвейер на одном соединении асинхронный и допускает pipelining: каждый RPC запускается отдельной tokio-задачей, ответы приходят в любом порядке. Но в нём много копий и нет backpressure (unbounded-канал), а размер фрагмента не ограничен. В 0.11.0 появился квадратичный по нагрузке узкий участок `TransactionTracker`: на macOS-сборке он съедал ~70% времени. Фикс (#57/#55) не влит. Других опубликованных бенчмарков нет.

### Cited Findings
- Чтение из сокета:
  1. `process_socket` читает в стековый буфер `[0; 128000]` (tcp.rs:48);
  2. копирует в `tokio::io::duplex(256000)` (rpcwire.rs:136);
  3. `read_fragment` собирает фрагмент в `Vec` через `append_to.resize(append_to.len() + length, 0)` (rpcwire.rs:102);
  4. каждая полная запись запускается через `tokio::spawn`;
  5. ответ сериализуется в новый `Vec<u8>` и уходит по `mpsc::unbounded_channel()` (rpcwire.rs:137);
  6. запись в сокет — два `write_all` (заголовок и тело), с комментарием «// TODO: split into many fragments».

  Источники — [tcp.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/tcp.rs), [rpcwire.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/rpcwire.rs).
- READ: VFS возвращает собственный `Vec<u8>`, затем он копируется в буфер ответа. WRITE: данные десериализуются в `args.data`, и VFS получает `&[u8]` — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs), [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs).
- PR #57 (lydakis, 2026-10-21): `is_retransmission` на каждом RPC вызывает `housekeeping()`, то есть `HashMap::retain` под мьютексом.
  - Холодная сборка ripgrep на NFS-маунте macOS: около 200 000 RPC; сервер потратил «67.7 s of CPU during a 60.8 s build».
  - Микробенчмарк, 200 000 вызовов: 46,1 с (230 мкс на вызов) до фикса и 0,049 с (0,24 мкс) после.
  - Сборка ripgrep 14.1.1: 64,2 с → 18,0 с, CPU сервера ~68 с → 14,6 с.

  Источник — [PR #57](https://github.com/huggingface/nfsserve/pull/57). Тот же фикс входит в PR #55 от ylow — [PR #55](https://github.com/huggingface/nfsserve/pull/55).
- Трекер появился в PR #31 (влит 2024-09-23) и впервые вышел в 0.11.0 — `git show --stat 741a217` в клоне. В 0.10.2 его нет.
- Issue #4 «Evaluate Profile-Guided Optimization (PGO) and LLVM BOLT» закрыт 2023-10-02 — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue). В README в TODO значится «Many many perf optimizations» — [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- hf-mount оптимизирует производительность у себя, в адаптере: «perf(inode): O(1) lookup_child via name->ino HashMap (#166)», «fix: prevent NFS handle eviction during reads and parallelize per-inode reads (#80)» — `git log` hf-mount.

### Inferences
- На пути полезной нагрузки примерно 3–4 `memcpy`: стековый буфер → duplex → `Vec` фрагмента → `Vec` ответа плюс `Vec` из VFS. К этому добавляется boxing future на каждый вызов из-за `async_trait` и `dyn NFSFileSystem`. На localhost этого достаточно, для пропускной способности уровня 10–100 GbE — нет.
- Нет лимита на размер фрагмента: заголовок объявляет до 2³¹−1 байт, и `resize` выделяет столько же. Нет лимита на число задач в полёте. Отсюда риски по памяти (DoS) от некорректного или злонамеренного клиента.
- `rtpref = 1024 * 124` выглядит как опечатка (ожидаемо 128 KiB). `estimated_max_results = dircount / 16` — грубая эвристика.

### Gaps
- Независимых бенчмарков nfsserve (MB/s, IOPS, сравнение с ядерным nfsd или FUSE) не найдено. Сам я ничего не измерял.

## 6. Issues и PR: что просят, что висит, что важно из закрытого

### Takeaway
На 2026-10-30: 2 открытых и 17 закрытых issues, 11 открытых и 26 закрытых PR. Открытые PR закрывают ровно главные дыры: LINK, корректная пагинация readdir, реальный FSSTAT, Windows-portmapper, производительность трекера, auth-контекст. Многие висят от 6 до 28 месяцев без ревью. Сообщество дублирует работу: два независимых LINK-PR и два PR о cookie. Это явный признак проблем с сопровождением.

### Cited Findings
- Счётчики на 2026-10-30: Issues — 2 open / 17 closed; PR — 11 open / 26 closed. На первой странице закрытых PR видно 19 merged и 6 закрытых без merge — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue), [pulls](https://github.com/huggingface/nfsserve/pulls), [closed PRs](https://github.com/huggingface/nfsserve/pulls?q=is%3Apr+is%3Aclosed).

Открытые PR (возраст на 2026-10-30) — [pulls](https://github.com/huggingface/nfsserve/pulls):

| PR | Название | Автор | Открыт | Возраст | Статус |
|---|---|---|---|---|---|
| #29 | introduce extended filesystem interface (uid/gid в VFS) | reinersinger, SAP | 2024-06-26 | ~28 мес. | последний ответ мейнтейнера 2024-08-09 ([PR #29](https://github.com/huggingface/nfsserve/pull/29)) |
| #30 | Add fake port option | Protryon | 2024-07-16 | ~27 мес. | — |
| #35 | Fix duplicate files for large directories | i404788 | 2025-06-09 | ~17 мес. | «No reviews»; 2025-07-23 ylow влил его в свой форк, не в upstream ([PR #35](https://github.com/huggingface/nfsserve/pull/35)) |
| #44 | portmap_listener для Windows | XciD, Member HF | 2026-04-27 | 6 мес. | draft, без ревью |
| #48 | реальный FSSTAT | centrual | 2026-08-18 | 2,5 мес. | draft по воле автора |
| #49 | явный cookie (breaking) | centrual | 2026-08-18 | 2,5 мес. | draft по воле автора |
| #50 | Fixed readdir lack of pagination cookie | max619 | 2026-09-15 | 1,5 мес. | — |
| #55 | NFSPROC3_LINK + атрибуты MKDIR + перф трекера | ylow | 2026-10-19 | 11 дней | «No reviews»; первые юнит-тесты хендлеров |
| #56 | Dependabot | бот | 2026-10-20 | 10 дней | — |
| #57 | перф `TransactionTracker` | lydakis | 2026-10-21 | 9 дней | «No reviews» |
| #58 | Add NFSv3 LINK (hard link) support | SeanMollet | 2026-10-25 | 5 дней | дублирует #55 |

- Закрытые без merge PR:
  - #46 — транспорт-дженерик и TLS (rockwotj). Единственное ревью от XciD: «RPCContext is still private no ?». Закрыт автором 2026-08-05 — [PR #46](https://github.com/huggingface/nfsserve/pull/46);
  - #37 — update deps, 2025-10, заменён #40;
  - #33 — Reorganize dependencies, 2025-02;
  - #27 — nfsstring без heap alloc, 2024-05.

  Источник — [closed PRs](https://github.com/huggingface/nfsserve/pulls?q=is%3Apr+is%3Aclosed).
- Важные закрытые issues:
  - #3 «Demo doesn't work on Ubuntu 22.04». Новые ядра Linux считают `nlink 0` невалидным inode; исправлено PR #6 2023-10-15 — `git log` (`9a73d54`), [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue);
  - #15 auth — not planned (см. раздел 4);
  - #17 «Use AFIT instead of async_trait» закрыт 2024-10-09, а в 0.11.0 по-прежнему `async_trait` — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs);
  - #18 — Windows и порт 111;
  - #24 — rename на Windows;
  - #25 — Case Insensitivity;
  - #26 — «Chromium network access from NFS mount»;
  - #1 — «Do You Plan to Implement NFSv4 Also?», закрыт 2023-10-02;
  - #23 — вопрос про Android;
  - #43 — вопрос о framing RFC, закрыт 2026-04-24.

  Источник — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue).
- Открытые issues: #38 (permission control, с 2025-12-04, мейнтейнеры не отвечали) и #36 «Real file system was added» (с 2025-09-13) — [issues](https://github.com/huggingface/nfsserve/issues?q=is%3Aissue).

### Inferences
- После 2024-09-23 ни один внешний функциональный PR не влит; единственный внешний merge — исправление docstring (#42, 2026-04-27). Баги с потерей или дублированием записей каталога (#35, #49/#50) больше года не получают ревью. Это прямой риск целостности данных для пользователей вроде `cp -r`/`rsync` — серьёзный сигнал о плохом сопровождении.
- Самое ценное в очереди — #55 от ylow: LINK, тесты и перф. Его судьба покажет, вернётся ли активное сопровождение. На 2026-10-30 ревью нет.
- Форки, которые закрывают те же дыры, косвенно подтверждают застой upstream: HF держит ветку `feat/portmap-listener` для hf-mount, ylow мёржит #35 в свой форк, есть отдельный крейт `nfs3_server` («originated as a fork of xetdata/nfsserve», по [lib.rs](https://lib.rs/crates/nfs3_server), сниппет поиска). Подробнее о форках — у другого исследователя.

### Gaps
- Не видна вторая страница закрытых PR (1 PR) и содержание PR #30, #50 и #58.
- GitHub API недоступен (403), поэтому точное время первого ответа по событиям не получено.

## 7. Сигналы качества кода (тесты, CI, фаззинг, docs, примеры)

### Takeaway
Крейт маленький (~4,8 тыс. строк в `src`). В `main` **ноль** тестов. CI запускается только на Linux и проверяет fmt, clippy и компиляцию. Фаззинга нет, MSRV нет. Документировано около 29% публичных элементов. README частично устарел. Сильная сторона — два работающих примера (demo, mirrorfs).

### Cited Findings
- Размер `src`: ≈4 769 строк, из них `nfs_handlers.rs` — 2 115. Примеры: `examples/demo.rs` — 359 строк, `examples/mirrorfs.rs` — 625 — `wc -l` по клону.
- Тестов нет: `#[test]` и `#[tokio::test]` встречаются 0 раз в `src/` и `examples/` — `grep` по клону. Тесты добавляют только невлитые PR #44, #46, #55 и #57 — страницы этих PR.
- CI `ci.yml` («xet-core CI»), только `ubuntu-latest`:
  - `cargo +nightly fmt --check`;
  - `cargo clippy -r -- -D warnings`;
  - `cargo test --features "strict"` (тестов нет);
  - проверка `Cargo.lock`.

  Комментарий «Install Rust 1.90» стоит рядом с action, помеченным `# 1.100`. Нет macOS- и Windows-раннеров, нет интеграции с реальным NFS-клиентом, нет фаззинга — [ci.yml](https://github.com/huggingface/nfsserve/blob/327b455/.github/workflows/ci.yml).
- MSRV не объявлен: `rust_version = None` для всех версий на crates.io, `rust-version` в Cargo.toml нет; edition 2021 — [crates.io API](https://crates.io/api/v1/crates/nfsserve), [Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b455/Cargo.toml).
- Документация (статический подсчёт по клону): `///` есть у 46 из 156 `pub`-элементов (~29%). В `vfs.rs` — у 2 из 6, но основной трейт документирован подробно. docs.rs из сессии недоступен: статус сборки и покрытие там не проверены.
- В `vfs.rs` один `unsafe` (`static mut GENERATION_NUMBER`). Обработка ошибок внутри хендлеров — `anyhow::Error` — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs), [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs).
- Валидация входа: `let max_bytes_allowed = args.maxcount as usize - 128;` (nfs_handlers.rs:869) не проверяет `maxcount < 128`. В README признаётся: «The RPC message handling in `nfs_handlers.rs` leaves a lot to be desired. The response serialization is very manual» — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs), [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md).
- Примеры: `demo` — in-memory ФС с записью на порту 11111; `mirrorfs` — зеркало локального каталога. Оба за feature `demo` — [Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b455/Cargo.toml).
- Зависимости 0.11.0: `byteorder`, `num-traits`, `num-derive`, `tokio` (net, io-util, sync, fs, rt, macros), `tracing`, `anyhow`, `async-trait`, `filetime` — [Cargo.toml](https://github.com/huggingface/nfsserve/blob/327b455/Cargo.toml).

### Inferences
- Корректность протокола держится на «проверено руками с macOS- и Linux-клиентами» и на нагрузке в продакшене у hf-mount и cua. Регрессионной защиты нет, а любой рефакторинг (например, #49) рискован. Видимо, поэтому мейнтейнеры боятся мёржить.
- Целочисленное переполнение в `maxcount - 128`: в debug-сборке паника задачи, в release заворачивание (wraparound) и игнорирование лимита ответа. Это дефект устойчивости ко входу.

### Gaps
- Покрытие на docs.rs и результат сборки docs не проверены (docs.rs заблокирован). `cargo doc` и `cargo build` по скачанному коду не запускались — из соображений безопасности.

## 8. Уровень поддержки: сопровождение и сообщество / платформы и клиенты

### Takeaway
**Сопровождение — слабое.** В 2025 году не было ни одного коммита, в 2026-м — только релиз-переупаковка и CI-гигиена от HF. Ревью внешних PR практически остановилось с конца 2024 года. Bus factor исторически был равен 1 (ylow). Права на публикацию у HF Xet team, но активного ревьюера нет. **Платформы:** сервер работает на Linux и macOS (на Windows, судя по ветке HF, тоже). Клиенты: Linux и macOS — рабочие и массово используемые, Windows — «kinda works» и требует portmapper на 111, который не выпущен.

### Cited Findings
- Коммиты в `main` за 24 месяца (2024-10-30…2026-10-30) — всего 7:
  - 3 от ботов;
  - 1 от security-инженера HF (пиннинг Actions);
  - 2 от Assaf Vayner (зависимости, линт, метаданные);
  - 1 внешний (docstring).

  Функциональных изменений NFS-логики нет. В 2025 году — 0 коммитов — `git log --since=2024-10-30` по клону.
- Историческая скорость merge (2023–2024, от первого коммита PR до merge-коммита): #2 — тот же день, #6 — 1 день, #9 — 6 дней, #10 — ~12 дней, #11 — 1 день, #12 — 8 дней, #16 — тот же день, #28 — 7 дней, #31 — ~28 дней, #32 — 3 дня, #22 — ~79 дней — `git log --merges` по клону (оценка по датам коммитов).
- Скорость реакции в 2025–2026:
  - #35 — 17 месяцев без ревью;
  - #55, #57, #58 — без ревью;
  - #46 — ревью через 7 дней после пинга автора (открыт 2026-06-04, пинг 2026-06-11, ревью 2026-06-18), затем автор закрыл PR;
  - issue #38 — около 11 месяцев без ответа мейнтейнера.

  Источники — страницы [PR #35](https://github.com/huggingface/nfsserve/pull/35), [PR #46](https://github.com/huggingface/nfsserve/pull/46), [PR #57](https://github.com/huggingface/nfsserve/pull/57), [issue #38](https://github.com/huggingface/nfsserve/issues/38).
- GitHub на 2026-10-30: 770★, 98 форков, 13 watchers; последний коммит 2026-10-14; последний релиз 0.11.0 от 2026-04-01 — [репо](https://github.com/huggingface/nfsserve), `git log`, [crates.io API](https://crates.io/api/v1/crates/nfsserve).
- Контрибьюторов-людей 15. На ylow приходится ≈49% коммитов, и до 2024-09 все merge делал он. Права на crates.io: port8080, assafvayner, rajatarya и `github:huggingface:xet` — `git shortlog`, [owners API](https://crates.io/api/v1/crates/nfsserve/owners).
- Клиенты — README:
  - Linux: `mount.nfs … nolock,vers=3,tcp,…`;
  - macOS: `mount_nfs … nolocks,vers=3,tcp,…`;
  - Windows: только Pro (в Home нет NFS-клиента), «kinda works».

  [README](https://github.com/huggingface/nfsserve/blob/327b455/README.md). hf-mount на macOS использует `locallocks` — [hf-mount src/nfs.rs](https://github.com/huggingface/hf-mount/blob/cb84c3e/src/nfs.rs).
- hf-mount выпускает готовые бинарники только для Linux x86_64/aarch64 и macOS Apple Silicon. Windows-поддержка живёт в ветке, привязанной к невыпущенному PR #44 — [hf-mount](https://github.com/huggingface/hf-mount), [hf-mount feat/windows-nfs-support Cargo.toml](https://github.com/huggingface/hf-mount/blob/feat/windows-nfs-support/Cargo.toml).
- Реальное использование macOS-клиента: smfs («avoid by using NFSv3-over-localhost instead» macFUSE), cua-volume, hf-mount (через Homebrew на macOS только NFS) — [smfs Cargo.toml](https://github.com/supermemoryai/smfs/blob/f4a0f50/Cargo.toml), [hf-mount](https://github.com/huggingface/hf-mount).

### Inferences
- Для потребителя nfsserve это «замороженный» крейт с крупным спонсором. Что-то сломается — исправлять придётся в своём форке. Этим и объясняется обилие форков (98 на GitHub, а также отдельные крейты-форки).
- Сопровождение может ожить, если HF понадобится Windows для hf-mount (#44) или если активизируется ylow (#55). Но на 2026-10-30 ни одного сигнала merge нет.
- Поддержка платформ на практике шире, чем у nfs-mamont, за счёт macOS: его отлаживали на macOS-клиенте, а комментарии в коде прямо описывают особенности клиента Mac.

### Gaps
- Нет данных о поведении с клиентами FreeBSD, Solaris или ESXi и о соответствии тестам вроде pynfs/cthon — публичных отчётов не найдено.
- Время первого ответа считалось по датам коммитов и страницам PR, а не по событиям GitHub API.

## 9. Итог: чем nfsserve лучше и хуже nfs-mamont

### Takeaway
nfsserve выигрывает **признанием и экосистемой**: 1,42 млн загрузок против 80, спонсор HF, продакшен в hf-mount, cua и atomic-server. Ещё он выигрывает **macOS-ориентацией** и **простым API**. Проигрывает почти по всем инженерным параметрам: полнота RFC 1813, auth в VFS, тесты и фаззинг, лимиты на вход, zero-copy, корректность readdir и COMMIT, живость сопровождения. Позиционирование nfs-mamont против него — «корректный, протестированный и полный сервер NFSv3, а не localhost-эмулятор FUSE». Чтобы конкурировать в нише nfsserve, нужен macOS-сервер и простой адаптер.

### Cited Findings
- **Лучше у nfsserve:**
  - Признание: 1 415 987 загрузок против 80 у `nfs-mamont` (0.1.0 — 2026-10-09, 0.1.1 — 2026-10-12, 0.2.0 — 2026-10-26) на 2026-10-30 — [crates.io nfsserve](https://crates.io/api/v1/crates/nfsserve), [crates.io nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont).
  - Зависимые: около 51 репозитория на GitHub, среди них cua (31 357★), atomic-server, hf-mount (HF) и smfs — [dependents](https://github.com/huggingface/nfsserve/network/dependents).
  - Сервер собирается и используется на macOS, Linux и, судя по ветке HF, на Windows. nfs-mamont в README описан как Linux-only: «Platform: Linux» — [/home/user/nfs-mamont/README.md](/home/user/nfs-mamont/README.md), [PR #44](https://github.com/huggingface/nfsserve/pull/44).
  - Простой API: 15 обязательных методов по `fileid3`, удобные умолчания (`fsinfo`, `id_to_fh`/`fh_to_id`, `path_to_id`), сигнал mount/umount, режим `bind("auto:port")`, встроенный «фейковый» portmapper для libnfs — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs), [tcp.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/tcp.rs), [portmap_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/portmap_handlers.rs).
  - Спонсор — организация Hugging Face; права на публикацию у команды `github:huggingface:xet` — [owners API](https://crates.io/api/v1/crates/nfsserve/owners).
  - Пермиссивная лицензия BSD-3-Clause (у nfs-mamont MIT — паритет) — [LICENSE](https://github.com/huggingface/nfsserve/blob/327b455/LICENSE), [/home/user/nfs-mamont/README.md](/home/user/nfs-mamont/README.md).
- **Хуже у nfsserve** (по сравнению с локальным nfs-mamont):
  - Тесты и фаззинг. У nfsserve 0 тестов и CI только на Linux. У nfs-mamont 336 тестовых функций по репозиторию, workflow `fuzz.yml` с `cargo +nightly fuzz run`, CI на матрице `1.75.0` и `1.99.0` — `grep` по `/home/user/nfs-mamont`, [/home/user/nfs-mamont/.github/workflows/fuzz.yml](/home/user/nfs-mamont/.github/workflows/fuzz.yml), [/home/user/nfs-mamont/.github/workflows/rust-ci.yml](/home/user/nfs-mamont/.github/workflows/rust-ci.yml).
  - Полнота протокола. В nfsserve LINK, MKNOD и COMMIT отвечают `PROC_UNAVAIL`, MOUNT DUMP тоже; FSSTAT и PATHCONF — константы. nfs-mamont заявляет «full procedure set of RFC 1813», а в его `Vfs` есть `link` и `commit` — [nfs_handlers.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/nfs_handlers.rs), [/home/user/nfs-mamont/README.md](/home/user/nfs-mamont/README.md), [/home/user/nfs-mamont/nfs-mamont/src/vfs/link.rs](/home/user/nfs-mamont/nfs-mamont/src/vfs/link.rs), [/home/user/nfs-mamont/nfs-mamont/src/vfs/commit.rs](/home/user/nfs-mamont/nfs-mamont/src/vfs/commit.rs).
  - Auth. В nfsserve VFS не получает учётные данные, а ACCESS разрешает всё. В nfs-mamont методы `Vfs` принимают `cred: &Credential`, парсер поддерживает `AUTH_NONE`/`AUTH_SYS` с лимитами RFC 5531 — [/home/user/nfs-mamont/nfs-mamont/src/vfs/link.rs](/home/user/nfs-mamont/nfs-mamont/src/vfs/link.rs), [/home/user/nfs-mamont/nfs-mamont/src/consts/rpc.rs](/home/user/nfs-mamont/nfs-mamont/src/consts/rpc.rs), [/home/user/nfs-mamont/nfs-mamont/src/parser/parser_struct.rs](/home/user/nfs-mamont/nfs-mamont/src/parser/parser_struct.rs).
  - Память и копии. В nfsserve 3–4 копии на запрос, `Vec` на каждый ответ, unbounded-канал и нет проверки размера фрагмента. В nfs-mamont zero-copy аллокатор за публичным трейтом (опция `mlock`) и `MAX_FRAGMENT_SIZE` в парсере и сериализаторе — [rpcwire.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/rpcwire.rs), [/home/user/nfs-mamont/nfs-mamont/src/consts/rpc.rs](/home/user/nfs-mamont/nfs-mamont/src/consts/rpc.rs), [/home/user/nfs-mamont/README.md](/home/user/nfs-mamont/README.md).
  - Корректность семантики в nfsserve: cookie = fileid (потери и дубли в readdir), `FILE_SYNC` без COMMIT, handle становятся STALE после рестарта, нет graceful shutdown — разделы 4–5 выше.
  - Сопровождение и релизы. У nfsserve 2025 год без коммитов, 24 месяца между релизами, PR без ревью до 28 месяцев. nfs-mamont за октябрь 2026 выпустил 3 версии; последние коммиты — 2026-10-29 — [crates.io nfs-mamont](https://crates.io/api/v1/crates/nfs-mamont), `git log` в `/home/user/nfs-mamont`.
  - API-гигиена. У nfsserve `async_trait` (boxing) и нет MSRV. У nfs-mamont `trait-variant` и MSRV 1.75 — [vfs.rs](https://github.com/huggingface/nfsserve/blob/327b455/src/vfs.rs), [/home/user/nfs-mamont/Cargo.toml](/home/user/nfs-mamont/Cargo.toml).
  - NLM: у nfsserve его нет и он не планируется. У nfs-mamont NLM/NSM в roadmap, в дереве уже есть `src/parser/nlm` и `src/nlm/holder.rs` — [/home/user/nfs-mamont/README.md](/home/user/nfs-mamont/README.md), дерево `/home/user/nfs-mamont/nfs-mamont/src`.
- **Паритет или неясно:** оба только NFSv3 (у nfsserve NFSv4 только в TODO README); оба на tokio. Транспорт UDP в nfs-mamont в рамках этой задачи не проверялся.

### Inferences
- Главная угроза nfsserve для nfs-mamont — не техническая, а дистрибуционная. Он уже «по умолчанию» используется в проектах класса «смонтировать облако или агентную ФС на macOS без macFUSE» (hf-mount, smfs, cua, coven). У nfs-mamont нет macOS-сервера, поэтому он в этой нише не участвует вообще.
- Возможности nfs-mamont, вытекающие из слабостей nfsserve:
  1. корректная пагинация readdir с собственными cookie;
  2. реальные FSSTAT/PATHCONF и LINK/MKNOD/COMMIT;
  3. передача `Credential` в VFS;
  4. лимиты на вход и backpressure;
  5. живое сопровождение с тестами и фаззингом;
  6. «мост» или адаптер под `NFSFileSystem`-подобный простой API, чтобы облегчить миграцию — пользователи nfsserve уже упираются в #35, #49, #48, #55 и #57.
- Если HF всерьёз займётся сопровождением (вольёт #55 и #57, выпустит #44), разрыв по Windows и производительности сократится. Архитектурные долги (auth, cookie, копии, тесты) за один релиз не закрыть.

### Gaps
- Сравнительных бенчмарков nfsserve и nfs-mamont нет. Совместимость nfs-mamont с macOS- и Windows-клиентами и его поддержка UDP в этой задаче не проверялись. Детальный аудит nfs-mamont выходит за рамки этих заметок.
