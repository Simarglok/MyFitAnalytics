# Техническая предпроверка packaged-native приёмки Plan 4

**Только для Codex/manager.** Этот документ содержит подробную техническую
подготовку, команды, проверки артефактов, защиту профиля, постановку фикстур,
проверки базы данных, очистку и оформление evidence. Его выполняет Codex в
изолированной session-owned среде; оператор не должен запускать команды из
этого файла.

Codex должен завершить эту предпроверку, подготовить и запустить точный
packaged app, зафиксировать динамические пути и значения в безопасной записи и
явно сообщить оператору, что можно начинать ручной UI-прогон. Операторский
сценарий находится в
`docs/testing/plan4-packaged-native-manual.md` и начинается только после этого
сигнала готовности. Статус ручной приёмки до фактического выполнения остаётся
**NOT RUN**.

---

## Подробный технический сценарий packaged-native приёмки Plan 4

Статус: утверждённая процедура приёмки, **NOT RUN** для текущей рабочей копии.

Этот сценарий является текущим критерием ручной приёмки Plan 4. Он отделён от
автоматизированных проверок Rust/web/package и от будущей задачи XCUITest.
Прогон нельзя считать пройденным, пока оператор не выполнит каждый применимый шаг,
не зафиксирует результат и не приложит только безопасные синтетические
скриншоты.

## Область и правила остановки

- Тестируйте только Plan 4: видимость локального импорта, страницы
  аналитики и dashboard, качество источников, настройки, выбор провайдера,
  фазовые события, упаковку и корректное завершение.
- Жизненный цикл рабочего стола Plan 5, поведение панели и фонового режима,
  автозапуск, публикация релиза, нотариальная заверка и телеметрия находятся
  вне области этой процедуры.
- Не реализуйте и не запускайте XCUITest в рамках этой процедуры.
- Не устанавливайте, не вызывайте, не настраивайте и не требуйте
  **CuaDriver**. Не заменяйте его другим сторонним daemon управления
  приложением на переднем плане.
  Для этого ручного прогона оператор может использовать только обычное
  видимое взаимодействие с macOS.
- Не запрашивайте и не выдавайте разрешения Accessibility, Screen Recording,
  Apple Events, Developer Tools или любые другие разрешения macOS. Если
  появится запрос разрешения, остановитесь, зафиксируйте `BLOCKED: permission
prompt`, по возможности безопасно завершите приложение, восстановите защиту
  профиля и получите явное разрешение до изменения permissions.
- Остановитесь, если выбран или обнаружен реальный экспорт, приватная запись,
  личное рабочее пространство, учётные данные, секрет или приватный лог. Не
  копируйте его в тестовый корень и не прикладывайте к материалам проверки.

## 1. Входные данные и временные корни

Используйте только следующие синтетические входные данные из репозитория:

| Назначение                                 | Фикстура в репозитории                                                 |
| ------------------------------------------ | ---------------------------------------------------------------------- |
| Успешный импорт MyNetDiary                 | `modules/sources/mynetdiary/tests/fixtures/valid-full.xls`             |
| Детерминированная ошибка/повтор MyNetDiary | `modules/sources/mynetdiary/tests/fixtures/missing-required-sheet.xls` |
| Измерения Hevy                             | `modules/sources/hevy/tests/fixtures/measurement_data.csv`             |
| Тренировки Hevy                            | `modules/sources/hevy/tests/fixtures/workout_data.csv`                 |

Фикстура с ошибкой необязательна для успешного сценария, но необходима для
проверки ограниченного поведения текущего `Attention`. Нельзя использовать
никакие другие фикстуры, экспорты или записи.

Из корня репозитория выполните приведённую ниже настройку в той же shell-сессии,
в которой будут выполняться остальные шаги сценария. Не вставляйте последующие
фрагменты в дочерние shell-сессии. Заранее подготовленный guard проверяется до
создания корня приёмки, а единственный обработчик `EXIT` устанавливается сразу
после задания корня и переменных guard, до выполнения любой последующей команды,
которая может завершиться ошибкой:

```bash
set -euo pipefail
export REPO_ROOT="$PWD"
export TMP_BASE="${TMPDIR:-/tmp}"
export TMP_BASE="${TMP_BASE%/}"
: "${PROFILE_GUARD:?set PROFILE_GUARD to a pre-provisioned session-owned helper}"
if [ ! -x "$PROFILE_GUARD" ]; then
  printf '%s\n' 'BLOCKED: pre-provisioned PROFILE_GUARD is unavailable' >&2
  exit 1
fi

ACCEPTANCE_ROOT="$(mktemp -d "$TMP_BASE/mfa-plan4-manual.XXXXXX")"
export ACCEPTANCE_ROOT
export TEST_HOME="$ACCEPTANCE_ROOT/home"
export WORKSPACE="$ACCEPTANCE_ROOT/workspace"
export MYNETDIARY_INBOX="$WORKSPACE/inbox/mynetdiary"
export HEVY_INBOX="$WORKSPACE/inbox/hevy"
export PROFILE_GUARD_ROOT="$ACCEPTANCE_ROOT"
export PROFILE_GUARD_MANIFEST="$ACCEPTANCE_ROOT/profile-guard.tsv"
export PROFILE_GUARD_BACKUP_MANIFEST="$ACCEPTANCE_ROOT/profile-guard-before.tsv"

record_blocked() {
  local reason="$1"
  printf 'BLOCKED: %s\n' "$reason" >&2
  if [ -n "${ACCEPTANCE_ROOT:-}" ] && [ -d "${ACCEPTANCE_ROOT:-}" ]; then
    printf 'BLOCKED\t%s\n' "$reason" \
      >"$ACCEPTANCE_ROOT/cleanup-status.tsv" || true
  fi
}

validate_profile_guard_manifest() {
  local expected_footer="$1"
  python3 - "$PROFILE_GUARD_MANIFEST" "$expected_footer" <<'PY'
import re
import sys

manifest, expected_footer = sys.argv[1:]
labels = [
    "application-support",
    "caches",
    "webkit",
    "preferences",
    "saved-application-state",
    "http-storages",
]


def fail(reason):
    print(f"BLOCKED: profile-guard.tsv {reason}", file=sys.stderr)
    raise SystemExit(1)


try:
    with open(manifest, encoding="utf-8", newline="") as handle:
        lines = handle.read().splitlines()
except OSError:
    fail("cannot be read")

if len(lines) != 8:
    fail("must contain one header, six root rows, and one footer")
if lines[0] != "label\tstate\tdigest\tfiles\trestored":
    fail("header is not exact")

for index, label in enumerate(labels, start=1):
    fields = lines[index].split("\t")
    if len(fields) != 5:
        fail(f"row {index} must have five TSV fields")
    if fields[0] != label:
        fail(f"row {index} has an unexpected label")
    if fields[1] not in {"present", "absent"}:
        fail(f"row {index} has an invalid state")
    if re.fullmatch(r"[0-9a-f]{64}", fields[2]) is None:
        fail(f"row {index} has an invalid lowercase SHA-256 digest")
    if re.fullmatch(r"[0-9]+", fields[3]) is None:
        fail(f"row {index} has an invalid file count")
    if fields[4] != expected_footer:
        fail(f"row {index} has the wrong restored flag")

if lines[-1] != f"restored={expected_footer}":
    fail("footer is not exact")
PY
}

compare_profile_guard_manifests() {
  python3 - "$PROFILE_GUARD_BACKUP_MANIFEST" "$PROFILE_GUARD_MANIFEST" <<'PY'
import sys

before_path, after_path = sys.argv[1:]


def read_rows(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return [line.split("\t") for line in handle.read().splitlines()]


try:
    before = read_rows(before_path)
    after = read_rows(after_path)
except OSError:
    print("BLOCKED: profile-guard backup comparison cannot be read", file=sys.stderr)
    raise SystemExit(1)

if len(before) != 8 or len(after) != 8:
    print("BLOCKED: profile-guard backup comparison has the wrong line count", file=sys.stderr)
    raise SystemExit(1)

for index in range(1, 7):
    if before[index][:4] != after[index][:4]:
        print(
            f"BLOCKED: profile-guard row {index} changed during restore",
            file=sys.stderr,
        )
        raise SystemExit(1)
PY
}

cleanup_acceptance() {
  local original_status="$?"
  local cleanup_status="$original_status"
  trap - EXIT

  finish_blocked() {
    local reason="$1"
    record_blocked "$reason"
    if [ "$cleanup_status" -eq 0 ]; then
      exit 1
    fi
    exit "$cleanup_status"
  }

  if [ -z "${ACCEPTANCE_ROOT:-}" ] || \
     [ -z "${PROFILE_GUARD_MANIFEST:-}" ]; then
    finish_blocked "acceptance root or profile-guard manifest is unset"
  fi
  if [ ! -f "$PROFILE_GUARD_MANIFEST" ]; then
    finish_blocked \
      "profile-guard backup manifest is missing; acceptance root retained"
  fi
  if ! "$PROFILE_GUARD" restore; then
    finish_blocked "profile-guard restore failed; acceptance root retained"
  fi
  if ! validate_profile_guard_manifest true; then
    finish_blocked \
      "profile-guard restore verification failed; acceptance root retained"
  fi
  if [ ! -f "$PROFILE_GUARD_BACKUP_MANIFEST" ]; then
    finish_blocked \
      "profile-guard backup record is missing; acceptance root retained"
  fi
  if ! compare_profile_guard_manifests; then
    finish_blocked \
      "profile-guard digest/state comparison failed; acceptance root retained"
  fi

  if ! test -n "$ACCEPTANCE_ROOT"; then
    finish_blocked "refusing to delete an empty acceptance root"
  fi
  case "$ACCEPTANCE_ROOT" in
    "$TMP_BASE"/mfa-plan4-manual.*) ;;
    *) finish_blocked "refusing to delete an unexpected acceptance root" ;;
  esac
  if ! rm -rf -- "$ACCEPTANCE_ROOT"; then
    finish_blocked "acceptance-root deletion failed; root retained"
  fi
  if [ -e "$ACCEPTANCE_ROOT" ]; then
    finish_blocked "acceptance root still exists after deletion"
  fi
  exit "$original_status"
}

trap cleanup_acceptance EXIT

mkdir -p "$TEST_HOME" "$MYNETDIARY_INBOX" "$HEVY_INBOX"
```

Приложение необходимо запускать с `HOME="$TEST_HOME"`, а рабочее пространство
выбирать только из `$WORKSPACE`. Временный корень одноразовый и не должен
содержать копию обычного рабочего пространства пользователя.

## 2. Предполетная проверка и защита профиля

Перед запуском:

1. Проверьте рабочую копию и базовую версию артефактов:

   ```bash
   git status --short --untracked-files=all
   git rev-parse HEAD
   node scripts/fixtures/verify_fixture_privacy.mjs
   ```

   Статус должен быть чистым для принимаемой сборки. Проверка фикстур должна
   вывести точно следующий результат:
   `verified 7 BIFF fixtures and 2 CSV fixtures; privacy scan passed`.

2. До первого вызова `PROFILE_GUARD` настройте заранее подготовленный
   принадлежащий тестовой сессии helper для контроля хешей шести корней на
   `PROFILE_GUARD_ROOT` и `PROFILE_GUARD_MANIFEST`. Helper не является
   зависимостью репозитория, и его нельзя импровизированно создавать во время
   прогона. Он должен защищать следующие метки, не выводя их вычисленные
   абсолютные пути:
   `application-support`, `caches`, `webkit`, `preferences`,
   `saved-application-state` и `http-storages`.

   Полный точный контракт `$ACCEPTANCE_ROOT/profile-guard.tsv` определяется до
   первого вызова. Это UTF-8 TSV без пустых строк и дополнительных полей:

   - строка 1 — это в точности
     `label\tstate\tdigest\tfiles\trestored`;
   - строки 2–7 содержат ровно по одной строке для каждой метки в следующем
     порядке:
     `application-support`, `caches`, `webkit`, `preferences`,
     `saved-application-state`, `http-storages`;
   - каждая строка корня содержит пять полей: точная метка; `state`, равный
     `present` или `absent`; `digest`, равный строчному 64-символьному
     SHA-256 дайджесту детерминированного снимка корня; `files`, равный
     неотрицательному десятичному числу файлов; и `restored`, равный текущему
     состоянию восстановления;
   - сразу после успешного `backup` в каждой строке корня указано
     `restored=false`, а последняя строка — это в точности `restored=false`;
   - только после успешного `restore` и проверки после восстановления строки корней
     могут измениться на `restored=true`; последняя строка после этого должна
     быть в точности `restored=true`;
   - `label`, `state`, `digest` и `files` должны оставаться побайтно равными
     сохранённому манифесту до восстановления; изменяются только поля `restored`
     в строках корней и итоговая строка — с `false` на `true`;
   - всего в файле ровно восемь строк, без путей, исходных байтов и других
     полей. Функция `validate_profile_guard_manifest` из блока настройки проверяет этот
     контракт для обоих вариантов итоговой строки.

Резервная копия защиты профиля принадлежит тестовой сессии и действует
временно; она должна проверить дайджест резервной копии до запуска
приложения. Если helper отсутствует, не может быть настроен на этот корень и
манифест, `backup` завершается ошибкой, манифест нарушает этот контракт или
восстановление нельзя проверить, единственный обработчик `EXIT` записывает
`BLOCKED` и сохраняет корень приёмки; не импровизируйте замену и не меняйте
разрешения.

```bash
"$PROFILE_GUARD" backup
validate_profile_guard_manifest false
cp "$PROFILE_GUARD_MANIFEST" "$PROFILE_GUARD_BACKUP_MANIFEST"
```

3. Подготовьте только синтетические файлы:

   ```bash
   cp "$REPO_ROOT/modules/sources/mynetdiary/tests/fixtures/valid-full.xls" \
      "$MYNETDIARY_INBOX/valid-full.xls"
   cp "$REPO_ROOT/modules/sources/hevy/tests/fixtures/measurement_data.csv" \
      "$HEVY_INBOX/measurement_data.csv"
   cp "$REPO_ROOT/modules/sources/hevy/tests/fixtures/workout_data.csv" \
      "$HEVY_INBOX/workout_data.csv"
   ```

   Записывайте только имена фикстур и метку корня приёмки. Не записывайте
   вычисленные пути домашнего каталога или содержимое файлов.

## 3. Проверка свежих пакетов и приложения

Перед открытием приложения соберите пакеты:

```bash
cd "$REPO_ROOT"
command -v duckdb
duckdb --version
bash scripts/build-module-packages.sh
python3 scripts/verify_module_packages.py
shasum -a 256 \
  dist/modules/mynetdiary.mfasource \
  dist/modules/hevy.mfasource \
  dist/modules/base.mfadashboard
```

Ожидаемые детерминированные хеши пакетов на этой базовой версии Plan 4,
зафиксированные в репозитории:

| Пакет                  | Ожидаемый SHA-256                                                  |
| ---------------------- | ------------------------------------------------------------------ |
| `mynetdiary.mfasource` | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |
| `hevy.mfasource`       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
| `base.mfadashboard`    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |

Проверка должна показать ровно три production module packages из разрешённого
списка. Не добавляйте пакет в bundle и не редактируйте сгенерированный
манифест.

Соберите и проверьте свежие macOS-артефакты:

```bash
pnpm --dir web build
cargo tauri build --bundles app
cargo tauri build --bundles dmg -vv

export APP="$REPO_ROOT/target/release/bundle/macos/MyFitAnalytics.app"
export BIN="$APP/Contents/MacOS/myfitanalytics"
export DMG="$REPO_ROOT/target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg"
test -x "$BIN"
test -f "$DMG"
shasum -a 256 "$BIN" "$DMG"
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -d --entitlements :- "$APP"
hdiutil verify "$DMG"
```

Зафиксируйте свежие хеши исполняемого файла и DMG точно в том виде, в каком они
напечатаны. Предыдущие базовые значения приведены только для справки:
исполняемый файл
`0b66ca53e055dc6101815e9b7516689c44f12a51fa7d492fa88735894812a611` и DMG
`45a859450ebcb890c22bd91ac681955ad7363ccdd1d637f1370d048b9a64fdc4`. Новый
ручной прогон должен использовать собственный свежий результат и не должен
копировать эти значения, если артефакты отличаются.

## 4. Запуск и начальное состояние

Запустите точный упакованный исполняемый файл напрямую с изолированным
домашним каталогом. Не используйте автоматизацию Finder или управляющий daemon:

```bash
HOME="$TEST_HOME" "$BIN" >"$ACCEPTANCE_ROOT/app.stdout" 2>"$ACCEPTANCE_ROOT/app.stderr" &
export APP_PID=$!
```

Осмотрите окно обычными средствами видимого взаимодействия с macOS. Первое
окно должно иметь заголовок `MyFitAnalytics`, размер примерно 1200 x 800 и
оставаться видимым. Записывайте только, видно ли окно и произошёл ли сбой.
Никогда не прикладывайте raw stdout, stderr, логи WebKit или пути; удалите эти
файлы во время очистки.

Ожидаемое начальное состояние нового профиля:

- верхняя навигация содержит `Overview`, `Body`, `Nutrition`, `Activity`,
  `Strength`, `Sources & quality`, `Phase events` и `Settings`;
- статус равен `Healthy` или `Not configured`, пока не выбрано рабочее
  пространство;
- нигде не отображаются необработанный JSON, `[[object Object]]`, путь источника
  или приватные данные;
- `Overview` не должен ошибочно показывать `Ready` до импорта данных.

## 5. Настройка синтетического рабочего пространства и Settings

Выполните следующие UI-шаги в точности:

1. Откройте `Settings` через верхнюю навигацию Analytics.
2. Активируйте `Choose Workspace...` и выберите только свежий `$WORKSPACE`.
3. Для источника MyNetDiary активируйте `Choose inbox` и выберите только
   `$MYNETDIARY_INBOX`.
4. Для источника Hevy активируйте `Choose inbox` и выберите только
   `$HEVY_INBOX`.
5. Убедитесь, что Settings показывает рабочее пространство и оба выбранных
   каталога inbox источников, не раскрывая байты источников и посторонние
   расположения профиля.
6. Убедитесь, что видимы установленные встроенные модули источников и базовый
   dashboard. Убедитесь, что control активного provider задан явно; не делайте
   вывод о provider только на основании наличия capability.
7. Если у установленного модуля отображается `Update`, нажмите только явный
   control `Update` этого модуля и дождитесь перезагрузки каталога. Убедитесь,
   что модуль видимо показывает выбранную встроенную версию и состояние
   `selected/active`; запишите точный текст version и state, отображаемый UI. Не
   утверждайте, что UI показывает SHA-256 hash: проверка package SHA-256 — это
   отдельное evidence в разделе 3. Убедитесь, что остальные modules, отключённые
   modules, custom packages и provider selections не изменились.
   Никогда не выполняйте глобальное или автоматическое обновление.
8. Если на этой базовой версии свежих артефактов ни у одного встроенного модуля
   нет `Update`, запишите `N/A — no update candidate advertised` и опирайтесь
   на автоматизированные тесты явного обновления; не утверждайте, что
   обновление было выполнено.
9. Если каталог модулей сообщает `Incompatible`, `Error` или неожиданную
   `selected version/state`, остановите приёмочный прогон и запишите
   типизированное состояние.

## 6. Начальное состояние dashboard и видимость импорта без перезапуска

Перед первым обновлением вернитесь в `Overview` и запишите состояние неготовности:
`Waiting for data` или точное typed non-ready state. Не обновляйте и не
перезапускайте приложение между настройкой и импортом.

1. Один раз активируйте `Refresh data`.
2. Пока операция выполняется, наблюдайте `Refreshing data…` и, если он виден,
   положительный счётчик активных jobs.
3. Не перезапуская приложение, дождитесь завершения обновления после изменения
   данных.
4. Ожидаемый результат: тот же процесс переходит в `Healthy`, `0 active jobs`,
   `0 attention items`; три синтетических inbox-файла потребляются и
   архивируются; dashboard и метаданные навигации обновляются без перезапуска.
5. Ожидаемая начальная дата наблюдения — `2026-02-03` из фикстур репозитория.
   Начальный диапазон должен быть включительным 31-дневным диапазоном,
   принадлежащим backend: `2026-01-04`–`2026-02-03`, а не датой, придуманной
   frontend.
6. Убедитесь, что нигде в status или dashboard не отображаются путь источника,
   raw export row, SQL, credential или raw JSON.

## 7. Идемпотентность Refresh и актуальное состояние Attention

Этот раздел выполняется при открытом приложении. Используйте только
синтетическую фикстуру ошибки из репозитория.

1. Скопируйте `modules/sources/mynetdiary/tests/fixtures/missing-required-sheet.xls`
   в `$MYNETDIARY_INBOX/missing-required-sheet.xls`.
2. Активируйте `Refresh data` и дождитесь, пока число активных jobs вернётся к
   нулю.
3. Ожидаемый результат: status равен `Attention` (или точному typed
   import-error state), текущий счётчик Attention для этого failed asset равен
   одному, а отображаемый счётчик failure codes детерминирован. Причина и
   identity должны быть privacy-safe и не должны раскрывать абсолютный путь
   inbox/archive или байты источника.
4. Снова активируйте `Refresh data`, не изменяя failed synthetic file.
5. Ожидаемый результат: тот же текущий элемент Attention остаётся одним
   элементом; счётчик и счётчик failure codes не увеличиваются до двух. Это
   проверка идемпотентности. Приложение не должно создавать дубликат текущего
   Attention только из-за повторного Refresh.
6. Откройте `Sources & quality`. Убедитесь, что для failed asset видимо
   действие `Retry import`, а retry возвращает typed queued/result state, а не
   raw error text. Если фикстура по-прежнему недействительна, зафиксируйте
   ожидаемый typed failure и не заявляйте о recovery.
7. Убедитесь, что `Open settings` предлагается только для применимого
   module-update condition, а не вместо причины failure. Не заявляйте об
   успешном clearing для artifact, который нельзя исправить через flow
   фикстуры из репозитория; invariant успешного clearing покрыт automated
   ingestion correction test.

## 8. Страницы dashboard, пробелы данных, provider и фазовые события

В том же запущенном приложении посетите все страницы в указанном порядке и
запишите заголовок, состояние доступности, видимое сообщение о gap/readiness и
отсутствие raw payloads:

| Страница            | Ожидаемые проверки                                                                                                                                              |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Overview`          | заголовок, блоки body-weight/nutrition/trend/quality, coverage и freshness; недостаточное coverage явно обозначено и никогда не представлено выдуманными нулями |
| `Body`              | raw weights, дневная медиана, trailing mean, trend, необязательный ряд body-fat, phase overlay и status; `null` остаются gaps                                   |
| `Nutrition`         | calories, macros, trailing mean, TDEE, trend, quality и coverage отсутствующих/неполных дней                                                                    |
| `Activity`          | steps, events, heart rate, water, trend, gaps с сохранением `null` и status                                                                                     |
| `Strength`          | session windows, duration, working sets, e1RM, calendar, контролируемые exercise keys и status                                                                  |
| `Sources & quality` | состояния modules, варианты providers, active snapshots, surface quality/retry и status                                                                         |

Для базовой версии с импортированными фикстурами сравните семантические значения
с `web/e2e/fixtures/expected-dashboard.json`: body weights `81.4` и `81.1` kg,
trailing mean `81.25` kg, nutrition item count `2`, activity steps `6400`,
accepted event count `1` и strength working sets `2`. Записывайте только
семантические значения и state; не копируйте raw fixture rows в evidence.

Шаги для provider и phase events:

1. В `Settings` используйте явный provider control для `body.weight` и
   выберите `hevy`, если control предлагает такой вариант. Убедитесь, что
   выбранный provider отображается после перезагрузки settings и что ни один
   невыбранный вариант не становится активным неявно.
2. Откройте `Phase events`, создайте synthetic event с type `cut`, датами
   `2026-01-15`–`2026-01-16`, description `synthetic phase` и установленным
   флагом `Exclude from TDEE`. Сохраните его.
3. Убедитесь, что event появляется в списке без перезапуска. Вернитесь в
   `Overview` или `Nutrition` и убедитесь, что phase overlay и состояние
   исключённых из TDEE дней обновились после свежего command query.
4. Если появится подтверждение удаления, сначала выполните cancel и убедитесь,
   что event остался. Затем явно подтвердите удаление и проверьте, что он
   исчез. Записывайте только type/date/result события, но не database paths или
   private text.

## 9. Корректное завершение и проверки базы данных в режиме read-only

1. Завершите packaged app через видимый application quit control.
2. Дождитесь завершения точного packaged process. Проверяйте только process
   acceptance app, а не unrelated processes:

   ```bash
   pgrep -f "$BIN" || true
   ```

   Ожидаемый результат — matching packaged process отсутствует. Не завершайте
   unrelated process; если точный process остаётся, запишите `FAIL: process
remains` и остановитесь.

3. Найдите изолированную application database в test profile, не выводя
   вычисленный путь. Откройте её в режиме read-only с помощью DuckDB:

   ```bash
   command -v duckdb
   duckdb --version
   export DB_PATH="$TEST_HOME/Library/Application Support/com.simarglok.myfitanalytics/myfitanalytics.duckdb"
   test -f "$DB_PATH"
   duckdb -readonly "$DB_PATH" -c "PRAGMA database_size;"
   duckdb -readonly "$DB_PATH" -c "
   SELECT table_name, row_count
   FROM (
     SELECT 'source_asset' table_name, COUNT(*) row_count FROM source_asset
     UNION ALL SELECT 'source_receipt', COUNT(*) FROM source_receipt
     UNION ALL SELECT 'ingestion_attempt', COUNT(*) FROM ingestion_attempt
     UNION ALL SELECT 'logical_snapshot', COUNT(*) FROM logical_snapshot
     UNION ALL SELECT 'active_snapshot', COUNT(*) FROM active_snapshot
     UNION ALL SELECT 'nutrition_item', COUNT(*) FROM nutrition_item
     UNION ALL SELECT 'body_measurement', COUNT(*) FROM body_measurement
     UNION ALL SELECT 'activity_day', COUNT(*) FROM activity_day
     UNION ALL SELECT 'heart_rate_observation', COUNT(*) FROM heart_rate_observation
     UNION ALL SELECT 'workout_session', COUNT(*) FROM workout_session
     UNION ALL SELECT 'exercise_set', COUNT(*) FROM exercise_set
     UNION ALL SELECT 'user_phase_event', COUNT(*) FROM user_phase_event
   ) ORDER BY table_name;"
   duckdb -readonly "$DB_PATH" -c "
   SELECT COUNT(*) AS broken_active_snapshots
   FROM active_snapshot a
   LEFT JOIN logical_snapshot l ON l.logical_snapshot_key = a.logical_snapshot_key
                                AND l.snapshot_id = a.snapshot_id
   WHERE l.snapshot_id IS NULL;"
   ```

   Последний запрос должен вернуть `0`. После успешного импорта и сохранённого
   одного phase event ожидаемые aggregate counts таковы: по `3` для
   `source_asset`, `source_receipt`, `ingestion_attempt`, `logical_snapshot` и
   `active_snapshot`; `2` nutrition items; `2` body measurements; `1` activity
   day; `1` heart-rate observation; `1` workout session; `4` exercise sets; и
   `0` user phase events после подтверждённого удаления в разделе 8. Если
   проверка ошибки создаёт дополнительный receipt/attempt, запишите фактическое
   количество и объясните его; никогда не удаляйте строки, чтобы получить
   ожидаемый результат.

   Все проверки базы данных выполняются в режиме read-only. Не запускайте
   `CHECKPOINT`, `DELETE`, `UPDATE`, `VACUUM` или любую recovery command для
   acceptance database.

## 10. Очистка и проверка приватности

Установленный в разделе 1 обработчик `EXIT` функции `cleanup_acceptance` —
единственный путь очистки. Он устанавливается до создания каталогов, backup
profile, запуска приложения и любой другой последующей команды, выполняемой с
`set -e`; поэтому и обычный `exit 0`, и ранняя ошибка попадают в один и тот же
обработчик. Не добавляйте другой trap, не вызывайте `restore` вручную, не
сбрасывайте trap в другом месте и не запускайте вторую последовательность
`rm -rf`.

При каждом завершении обработчик выполняет действия в следующем порядке:

1. сохраняет исходный exit status и отключает только собственный trap, чтобы
   избежать рекурсии;
2. требует, чтобы acceptance root и profile-guard manifest были заданы, и
   проверяет наличие manifest; иначе выводит и записывает `BLOCKED`, сохраняя
   root;
3. вызывает `restore` guard, затем запускает
   `validate_profile_guard_manifest true`, которая требует все строки корней и
   точный footer `restored=true`; если любой из шагов завершается ошибкой,
   выводит и записывает `BLOCKED`, сохраняя root;
4. выполняет существующие проверки непустого acceptance root и точного
   префикса `$TMP_BASE/mfa-plan4-manual.*`;
5. только после успешных restore/verification и обеих проверок пути удаляет
   acceptance root и проверяет его отсутствие; при ошибке удаления или проверки
   отсутствия выводит и записывает `BLOCKED` и не заявляет об успешной очистке.

Если backup guard завершается ошибкой до создания manifest, обработчик не
пытается выполнять непроверенное восстановление; он записывает `BLOCKED` и
сохраняет root. Любая другая ранняя ошибка при наличии manifest по-прежнему
обрабатывается в порядке restore, проверки manifest, проверок пути и условного
удаления.

Перед окончательным оформлением записи:

- убедитесь, что сохранённая acceptance record содержит уже завершённый
  результат profile guard: у каждого root указано `restored=true`, а digest до
  и после совпадает точно. Сам manifest удаляется вместе с temporary root;
- убедитесь, что точный packaged app process отсутствует;
- удалите stdout/stderr, копии database, байты archive, не прикладываемые
  screenshots и все временные копии fixtures;
- убедитесь, что обычные workspace и profile не выбирались и не изменялись;
- проверьте acceptance record и attachments на наличие home paths, source paths,
  exports, health records, credentials, secrets, raw rows и private logs;
- сохраняйте только exit codes команд, детерминированные hashes, counts, state
  labels, семантические aggregates, безопасные screenshots и таблицу PASS/FAIL
  ниже.

## 11. Запись результатов PASS/FAIL

Эту запись должен заполнить reviewer по результатам того же прогона. До
реального ручного выполнения единственно допустимое значение — `NOT RUN`.

| Проверка                                            | Результат (`PASS`/`FAIL`/`N/A`/`BLOCKED`) | Ссылка на evidence |
| --------------------------------------------------- | ----------------------------------------- | ------------------ |
| Предполетная проверка и приватность фикстур         | `NOT RUN`                                 |                    |
| Хеши свежих module packages и allowlist             | `NOT RUN`                                 |                    |
| Хеши свежих app/DMG, signing и проверка DMG         | `NOT RUN`                                 |                    |
| Backup profile и хеш-идентичное restore             | `NOT RUN`                                 |                    |
| Свежий видимый запуск packaged app                  | `NOT RUN`                                 |                    |
| Настройка Settings/workspace/inbox                  | `NOT RUN`                                 |                    |
| Видимость импорта без перезапуска                   | `NOT RUN`                                 |                    |
| Идемпотентность Refresh и текущий Attention         | `NOT RUN`                                 |                    |
| Явное поведение bundled update                      | `NOT RUN`                                 |                    |
| Все страницы dashboard и семантические gaps         | `NOT RUN`                                 |                    |
| Явный выбор provider                                | `NOT RUN`                                 |                    |
| Сохранение/overlay/удаление phase event             | `NOT RUN`                                 |                    |
| Корректное завершение и отсутствие packaged process | `NOT RUN`                                 |                    |
| Целостность DB и aggregates в режиме read-only      | `NOT RUN`                                 |                    |
| Граница cleanup/privacy                             | `NOT RUN`                                 |                    |

Итоговый статус для текущего checkout: **NOT RUN**. Не переводите его в
`PASS` только на основании автоматизированных lower-level gates. Запрос
разрешения, несинтетический input, несоответствие profile guard, раскрытие
raw/private data или необъяснимое несоответствие artifact/hash — это условие
остановки; его необходимо записать как `BLOCKED` или `FAIL`, не заявляя о
нативной приёмке Plan 4.

После заполнения записи PASS/FAIL выполните эту команду последней в той же
shell-сессии, чтобы единственный handler выполнил проверенную очистку:

```bash
exit 0
```
