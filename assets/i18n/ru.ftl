### The window's text in Russian. A message missing here shows in English, and
### `cargo xtask i18n ru` lists every such one; how to translate: docs/design/i18n.md.

## Shared by many panels

common-already-done = Уже сделано
common-cancel = Отмена
common-export = Экспорт
common-settings = Настройки
error-cannot-hand-over = не удалось передать { $name }
error-cannot-open = не удалось открыть { $path }
error-cannot-read = не удалось прочитать { $path }
error-cannot-write = не удалось записать { $path }
profiles-delete = Удалить
profiles-edit = Изменить
profiles-every = Все профили
profiles-shipped = Профиль, поставляемый вместе с программой, удалить нельзя.
status-exported = Экспортировано: { $path }
status-opened = Открыто: { $path }
status-saved = Сохранено: { $name }
table-profile = Профиль
unit-deg = град.
unit-degrees = °
unit-g-ml = г/мл
unit-h = ч
unit-m = мин
unit-mm = мм
unit-mm2 = мм²
unit-mm3 = мм³
unit-mm-min = мм/мин
unit-px = пикс.
unit-s = с
value-g = { $value } г
value-ml = { $value } мл
value-mm = { $value } мм
value-mm2 = { $value } мм²
value-s = { $value } с
value-um = { $value } мкм

## panels/settings/general.rs

general-language = Язык
general-language-system = Как в системе ({ $name })
general-language-untranslated = Интерфейс ещё не переведён на язык системы ({ $tag }), поэтому показан на английском.
general-start = Запуск
general-start-page = Открывать стартовую страницу
general-start-page-hint = Область для перетаскивания файлов, недавно открытые файлы и пробные печати смолы, пока нет модели. Если выключить, программа сразу откроется на платформе.
general-viewport = Область просмотра
general-invert-zoom = Прокрутка вниз приближает
general-sliced-files = Нарезанные файлы
general-sliced-dir-system = Диалог сохранения открывается там, где система его оставила в прошлый раз.
general-sliced-dir = Диалог сохранения открывается в папке { $dir }
general-choose-folder = Выбрать папку
general-forget-folder = Забыть папку
general-profiles = Профили
general-profiles-nowhere = В этой системе негде хранить ваши профили.
general-profiles-dir = Ваши принтеры, смолы и профили хранятся в { $dir }
general-open-folder = Открыть папку

## panels/inspector/cut.rs

cut-plane = Плоскость
cut-select-model = Выберите модель для разреза.
cut-height = Высота
cut-offset = Смещение
cut-measured-from-plate = Отсчёт от платформы.
cut-measured-from-centre = Отсчёт от центра масс модели.
cut-afterwards = После разреза
cut-lay-flat = Положить половины плашмя
cut-lay-flat-note = Каждая половина встаёт на грань, оставленную разрезом.
cut-joints = Соединить половины
cut-joints-note = Крепёж и гнёзда, чтобы половины склеивались точно.
cut-joint-dowels = Дюбели
cut-joint-pins = Штифты
cut-joint-magnets = Магниты
cut-joint-key = Шпонка
cut-joint-dowels-note = Гнёзда в обеих половинах и дюбели, напечатанные рядом, так что обе грани разреза остаются ровными под поддержки.
cut-joint-pins-note = Штифты стоят на нижней половине и входят в гнёзда верхней.
cut-joint-magnets-note = В каждой половине гнездо под каждый дисковый магнит, чтобы вклеить его.
cut-joint-key-note = Выступ по грани разреза нижней половины входит в паз верхней — для стенок, слишком тонких для штифта.
cut-per-piece = На кусок
cut-auto-size = Размер по сечению
cut-auto-size-note = Каждый кусок сечения получает штифты такой толщины, какая в нём помещается.
cut-diameter = Диаметр
cut-length = Длина
cut-thickness = Толщина
cut-width = Ширина
cut-clearance = Зазор
cut-keyed = Один уже
cut-keyed-note = Один штифт уже остальных, так что половины собираются только одним способом.
cut-cutting = Режу модель
cut-cancelled = Резка отменена
cut-parts = Части
cut-parts-mark = Указать часть
cut-parts-click = Кликните по части на модели
cut-parts-unmark = Убрать метку
cut-parts-mark-note = Если часть указана, «Разделить на части» отрезает её плоским срезом там, где она сужается к остальной модели, а остальную поверхность не трогает.
cut-parts-smallest = Наименьшая часть
cut-parts-smallest-note = Кусок меньше этого присоединяется к части, которой касается, или отбрасывается, если не касается ни одной.
cut-replaces = Одна модель заменяется двумя. Отмена вернёт её.
cut-cut = Разрезать
cut-split = Разделить на части

## cut.rs

cut-nothing-selected = для разреза ничего не выбрано
cut-no-direction = у плоскости разреза нет направления
cut-misses = плоскость не пересекает { $name }, резать нечего
cut-lower-name = { $name } (нижняя)
cut-upper-name = { $name } (верхняя)
cut-part-name = { $name } (часть { $index })
cut-dowel-name = { $name } (дюбель { $index })
cut-no-room-for-joint = На грани разреза нет места для соединения.
cut-joint-failed = Соединение не удалось встроить в половины, они оставлены без него: { $reason }
# Each starts with a space: it follows the cut's position directly.
cut-with-pins =
    { $count ->
        [one] На нижней половине стоит { $count } штифт.
        [few] На нижней половине стоят { $count } штифта.
       *[many] На нижней половине стоят { $count } штифтов.
    }
cut-with-dowels =
    { $count ->
        [one] В обеих половинах гнёзда под { $count } дюбель, он напечатан рядом.
        [few] В обеих половинах гнёзда под { $count } дюбеля, они напечатаны рядом.
       *[many] В обеих половинах гнёзда под { $count } дюбелей, они напечатаны рядом.
    }
cut-with-magnets =
    { $count ->
        [one] В каждой половине гнездо под { $count } магнит.
        [few] В каждой половине гнёзда под { $count } магнита.
       *[many] В каждой половине гнёзда под { $count } магнитов.
    }
cut-with-key =
    { $count ->
        [one] Шпонка идёт по { $count } куску грани разреза.
        [few] Шпонка идёт по { $count } кускам грани разреза.
       *[many] Шпонка идёт по { $count } кускам грани разреза.
    }
cut-bare =
    { $count ->
        [one] На { $count } куске грани разреза не нашлось места для крепежа.
        [few] На { $count } кусках грани разреза не нашлось места для крепежа.
       *[many] На { $count } кусках грани разреза не нашлось места для крепежа.
    }
cut-done = Разрез { $name } по { $axis } на { $position } мм
cut-done-open =
    { $open ->
        [one] Разрез { $name }; { $open } контур разреза не замкнут и остался без крышки
        [few] Разрез { $name }; { $open } контура разреза не замкнуты и остались без крышки
       *[many] Разрез { $name }; { $open } контуров разреза не замкнуты и остались без крышки
    }
cut-nothing-to-split = для разделения ничего не выбрано
cut-one-piece = { $name } — цельная деталь
cut-split-done =
    { $count ->
        [one] { $name } разделена на { $count } часть
        [few] { $name } разделена на { $count } части
       *[many] { $name } разделена на { $count } частей
    }
cut-cleaned = { $name } очищена и осталась цельной деталью
cut-split-joined =
    { $count ->
        [one] { $count } обрезок присоединён к части, которой касается.
        [few] { $count } обрезка присоединены к частям, которых касаются.
       *[many] { $count } обрезков присоединено к частям, которых касаются.
    }
cut-split-dropped =
    { $count ->
        [one] { $count } висящий обрезок отброшен.
        [few] { $count } висящих обрезка отброшены.
       *[many] { $count } висящих обрезков отброшено.
    }
cut-split-failed = модель не удалось разделить: { $reason }
cut-parted-off = Указанная часть отрезана от { $name } там, где она соединяется с остальным
cut-no-neck = от указанной точки нет узкого места, по которому можно разрезать
cut-mended =
    { $count ->
        [one] { $count } деталь залатана, чтобы её закрыть.
        [few] { $count } детали залатаны, чтобы их закрыть.
       *[many] { $count } деталей залатано, чтобы их закрыть.
    }

## panels/inspector/cut.rs

cut-keep-both = Обе
cut-keep-lower = Нижняя
cut-keep-upper = Верхняя

## workspace.rs

tool-select = Выбор
tool-measure = Замер
tool-hollow = Полость
tool-drain = Дренаж
tool-cut = Разрез
tool-relief = Рельеф
tool-supports = Поддержки
tool-paint = Разметка
tool-shape = Форма
tool-check = Проверка
tool-export = Экспорт
tool-print-settings = Параметры печати

## shortcuts.rs

action-undo = Отменить
action-redo = Повторить
action-select-all = Выбрать всё
action-deselect = Снять выбор
action-duplicate = Дублировать
action-remove = Убрать с платформы
action-next-view = Следующий вид
action-previous-view = Предыдущий вид
action-frame-view = Показать всё
action-settings = Настройки
action-sheet = Эта справка
action-find = Найти
action-layer-on = На слой вперёд
action-layer-back = На слой назад
action-ten-layers-on = На десять слоёв вперёд
action-ten-layers-back = На десять слоёв назад
action-play = Воспроизвести
action-new-plate = Новая платформа
action-new-project = Новый проект
action-open-project = Открыть проект
action-save-project = Сохранить проект
action-save-project-as = Сохранить проект как
action-import-model = Импортировать модель
action-slice = Нарезать
shortcut-group-tools = Инструменты, по порядку на панели
shortcut-group-plate = Платформа
shortcut-group-window = Окно
shortcut-group-layers = Слои
shortcut-group-files = Файлы
gesture-move-focus = Переход между элементами
gesture-move-focus-keys = Tab, Shift Tab
gesture-press-focus = Нажать элемент в фокусе
gesture-press-focus-keys = Enter или Пробел
gesture-leave-focus = Выйти из элемента в фокусе
gesture-leave-focus-keys = Esc
gesture-add-selection = Добавить к выбору
gesture-add-selection-keys = Cmd + щелчок
gesture-pick-between = Выбрать всё между, в списке
gesture-pick-between-keys = Shift + щелчок
gesture-orbit = Вращение
gesture-orbit-keys = Перетаскивание
gesture-pan = Сдвиг
gesture-pan-keys = Перетаскивание правой кнопкой
gesture-zoom = Масштаб
gesture-zoom-keys = Прокрутка
gesture-zoom-mask = Приблизить маску
gesture-zoom-mask-keys = Прокрутка или щипок
gesture-move-mask = Сдвинуть маску
gesture-move-mask-keys = Перетаскивание
gesture-whole-mask = Показать маску целиком
gesture-whole-mask-keys = Двойной щелчок
status-removed-models =
    { $count ->
        [one] Убрана { $count } модель
        [few] Убраны { $count } модели
       *[many] Убрано { $count } моделей
    }

## panels/shortcuts_sheet.rs

shortcuts-title = Горячие клавиши
shortcuts-note = Инструмент вызывается цифрой, соответствующей его месту на панели. Пока курсор в поле ввода, клавиши отключены.
shortcuts-or = или

## settings.rs

settings-page-supports = Поддержки
settings-page-hollow = Полость
settings-page-drain = Дренаж
settings-page-general = Общие
settings-page-updates = Обновления
settings-tab-resins = Смолы
settings-tab-machine = Принтер
settings-tab-network = Сеть
settings-new-resin = Новая смола
settings-new-printer = Новый принтер
settings-new-profile = Новый профиль
settings-copy-name = { $name } (копия)

## supports.rs

supports-placing-place = Поставить
supports-placing-fill = Залить
supports-placing-block = Запретить
supports-placing-erase = Стереть
supports-nothing-visible = На платформе нет видимых моделей, которые нужно поддерживать.
supports-nothing-visible-error = на платформе нет видимых моделей, которые нужно поддерживать
supports-placed =
    { $count ->
        [0] На платформе ничего не нужно поддерживать
        [one] Поставлена { $count } поддержка
        [few] Поставлено { $count } поддержки
       *[many] Поставлено { $count } поддержек
    }
supports-cancelled = Расстановка отменена
supports-placed-sized = Поставлено поддержек: { $count }, из них лёгких { $light }, средних { $medium } и тяжёлых { $heavy }
supports-auto-no-shape = { $name } строит опоры по профилю { $id }, которого нет в каталоге
supports-group-name = Группа { $index }

## measure.rs

measure-snapped-corner = на углу
measure-snapped-edge = на ребре
measure-snapped-free = свободно

## panels/inspector/supports.rs

supports-where = Где
supports-picked = Выбрано
supports-already-done = Уже сделано
supports-pick-hint = Щёлкните по наконечнику, распорке, соединению, стволу или опоре, чтобы выбрать. Перетаскивание переносит, Shift добавляет, Alt убирает.
supports-picked-help = «Вырастить их заново» возвращает выбранные поддержки автоматике, и следующее выращивание строит их заново. «Переместить в текущую группу» строит их по профилю этой группы.
supports-stat-parts = Части
supports-stat-supports = Поддержки
supports-stat-standing = Стоят
supports-move-to-group = Переместить в текущую группу
supports-grow-these-again = Вырастить их заново
supports-remove-these = Удалить эти поддержки
supports-grow-again = Вырастить заново
supports-grow = Вырастить поддержки
supports-grow-tooltip = Сечёт { $scope }, чтобы найти то, что нависает над пустотой.
supports-placing = Расстановка поддержек
supports-no-room = Некоторым поддержкам негде встать, и они не будут напечатаны.
supports-sizes = Размеры
support-size-island = Остров для { $shape }
support-size-load = Нагрузка для { $shape }
supports-sizing-auto = Автоматически
supports-sizing-manual = Вручную
supports-remove-all = Удалить все
supports-remove-all-tooltip = Убрать все поддержки
supports-remove-group = Только { $name }
supports-remove-group-tooltip = Убрать поддержки текущей группы ({ $count }), остальные оставить

## panels/settings.rs

settings-title = Настройки
settings-shortcuts = Горячие клавиши
settings-import = Импорт
settings-cannot-export = не удалось экспортировать профиль
settings-cannot-delete = не удалось удалить профиль
settings-name = Название

## panels/settings/machines.rs

machines-title = Принтер и смола
machines-search = Ваши принтеры
machines-no-match = Принтера с таким названием нет.
machines-from-library = Выберите свой в библиотеке
machines-add = Добавить принтер
machines-from-file = Из файла профиля принтера, либо перетащите его сюда
machines-import = Импортировать принтер
machines-remove = Удалить этот принтер
machines-import-file = Импортировать принтер из файла
machines-pick = Выберите принтер или добавьте его из библиотеки.
machines-in-hand = Платформа печатается на этом принтере
machines-use = Использовать этот принтер
machines-cannot-add-printer = не удалось добавить принтер
machines-cannot-add-machine = не удалось добавить принтер
machines-cannot-add-a-resin = не удалось добавить смолу
machines-cannot-add-resin = не удалось добавить смолу
machines-cannot-duplicate-resin = не удалось дублировать смолу
machines-cannot-remove = не удалось удалить принтер
machines-cannot-take-resin-off = не удалось убрать смолу
machines-cannot-delete-resin = не удалось удалить смолу

## panels/inspector/check.rs

check-filter-all = Все
check-filter-islands = Острова
check-filter-levers = Рычаги
check-filter-peel = Отрыв
check-filter-cups = Присоски
check-filter-thin = Тонкие
check-verdict = Заключение
check-support-islands = Поддержать все острова
check-drill-cups = Просверлить присоски
check-issues = Проблемы
check-found =
    { $issues ->
        [one] { $issues } проблема
        [few] { $issues } проблемы
       *[many] { $issues } проблем
    } на { $layers ->
        [one] { $layers } слое
        [few] { $layers } слоях
       *[many] { $layers } слоях
    }
check-stood =
    { $count ->
        [0] Ни у одного острова не нашлось нижней стороны для поддержки
        [one] Поставлена { $count } поддержка в группе { $group }
        [few] Поставлено { $count } поддержки в группе { $group }
       *[many] Поставлено { $count } поддержек в группе { $group }
    }
check-fact-checking = проверка
check-fact-none = проблем нет
check-fact-issues =
    { $count ->
        [one] { $count } проблема
        [few] { $count } проблемы
       *[many] { $count } проблем
    }
check-checking = Проверка
check-reading = Чтение всех слоёв...
check-reads-layers = Проверка читает слои. Они нарезаются, как только на платформе появляется модель и принтер, для которого её можно нарезать.
check-measuring = замер
check-hardest-pull = Сильнейшая тяга
check-disc = Тянет как диск площадью
check-widest-step = Самый большой скачок
check-grows-by = Прирост
check-layer-lower = слой { $layer }
check-peel = Отрыв
check-nothing-found = Ничего, что привело бы к браку
check-nothing-found-why = Нет островов, нет перемычек, перегруженных сверх предела, и нет слоёв, которые слишком сильно тянут плёнку.
check-issues-help = Острова начинаются в воздухе и остаются на плёнке. Рычаги — тонкие перемычки, которые может сломать тяга вбок; присоски — закрытые полости, которые подъём тянет вакуумом; тонкие участки могут вовсе не напечататься. Щелчок по строке переходит к её слою.
check-peel-help = Каждый подъём отрывает свежий слой от плёнки, и чем шире слой, тем сильнее тяга. «Сильнейшая тяга» — худший слой, показанный как диск с той же силой отрыва; «Самый большой скачок» — где площадь сильнее всего растёт от слоя к слою. Наклон модели или полость уменьшают и то и другое.
check-worst = { $issue }, слой { $layer }
check-put-islands-back = Вернуть острова
check-remove-islands =
    { $count ->
        [one] Удалить { $count } остров
        [few] Удалить { $count } острова
       *[many] Удалить { $count } островов
    }
check-remove-islands-note = Стирает их из записываемого файла и проверяет снова. Уходят только острова; то, что на них стояло, остаётся и попадает в список для поддержки.
check-hanging-passes =
    { $count ->
        [one] И ещё { $count } слой того, что на них стояло.
        [few] И ещё { $count } слоя того, что на них стояло.
       *[many] И ещё { $count } слоёв того, что на них стояло.
    }
check-remove-next =
    { $count ->
        [one] Удалить следующий { $count } остров
        [few] Удалить следующие { $count } острова
       *[many] Удалить следующие { $count } островов
    }
check-remove-next-note = Снимает ещё по одному слою с каждой повисшей части, с того, с которого она начинается, и проверяет снова.
check-no-islands-to-take = Островов для удаления нет
check-erased-note = Стёрто из файла; всё, что осталось висеть, перечислено ниже.
check-layer = Слой { $layer }
check-more-layers =
    { $count ->
        [one] и ещё { $count } слой
        [few] и ещё { $count } слоя
       *[many] и ещё { $count } слоёв
    }
check-row-hanging = нависает над пустотой · { $area } мм²
check-row-islands =
    { $count ->
        [one] остров · { $area } мм²
        [few] { $count } острова · { $area } мм²
       *[many] { $count } островов · { $area } мм²
    }
check-row-lever = рычаг · { $lever } мм · { $stress } МПа
check-row-levers =
    { $count ->
        [one] { $count } рычаг · { $stress } МПа
        [few] { $count } рычага · { $stress } МПа
       *[many] { $count } рычагов · { $stress } МПа
    }
check-row-cup = присоска · { $area } мм²
check-row-cups =
    { $count ->
        [one] { $count } присоска · { $area } мм²
        [few] { $count } присоски · { $area } мм²
       *[many] { $count } присосок · { $area } мм²
    }
check-row-thin =
    { $count ->
        [one] тонкий участок · { $width } мм
        [few] { $count } тонких участка · { $width } мм
       *[many] { $count } тонких участков · { $width } мм
    }
check-row-peel = отрыв · { $force } Н
check-issue-island = Остров без поддержки
check-issue-hanging = Повисшая часть
check-issue-lever = Слабая перемычка
check-issue-peel = Тяжёлый отрыв
check-issue-cup = Присоска
check-issue-thin = Тонкий участок
check-explain-hanging = Остров под ним удалён, поэтому { $area } мм² здесь висят над пустотой, а вместе с ними и остальная часть. Поставьте поддержку.
check-explain-island = Остров площадью { $area } мм² засвечивается над пустотой: он прилипнет к плёнке и останется в ванне. Поставьте поддержку или удалите его.
check-explain-lever = Перемычка под ним ({ $neck } мм²) испытывает около { $stress } МПа от тяги со смещением в { $lever } мм в сторону. Поставьте поддержку ближе к её середине.
check-explain-peel = Плёнка тянет этот слой с силой около { $force } Н. Наклоните модель, сделайте её полой или поднимайте медленнее.
check-explain-cup = Полость площадью { $area } мм² и объёмом { $volume } мл закрыта снизу, поэтому при каждом подъёме на неё действует тяга. Дренажное отверстие впустит воздух.
check-explain-thin = Участок шириной { $width } мм возвышается больше чем на миллиметр. Он может не сформироваться; если это важно, сделайте его толще.
check-removed =
    { $count ->
        [one] { $count } остров удалён из файла
        [few] { $count } острова удалены из файла
       *[many] { $count } островов удалено из файла
    }

## panels/plate_summary.rs

summary-layer = Слой { $layer }
summary-this-plate = Эта платформа
summary-layer-range = от { $fewest } до { $most }
summary-height = Высота
summary-layers = Слои
summary-vertices = Вершины
summary-triangles = Треугольники
summary-model-supports = Модель/поддержки
summary-price = Стоимость
summary-z-height = Высота по Z
summary-exposure = Экспозиция
summary-cured-area = Площадь засветки
summary-not-priced = цена не задана
summary-volume = Объём
summary-weight = Масса
summary-measuring = замер
summary-time = Время
summary-block-bottom = нижние слои
summary-block-transition = переход
summary-block-body = основная часть

## ui/widgets.rs

duration-hours = { $hours } ч { $minutes } мин { $seconds } с
duration-minutes = { $minutes } мин { $seconds } с
ago-never = никогда
ago-just-now = только что
ago-minutes = { $count } мин назад
ago-hours = { $count } ч назад
ago-days =
    { $count ->
        [1] вчера
        [one] { $count } день назад
        [few] { $count } дня назад
       *[many] { $count } дней назад
    }

## ui/widgets/field.rs

field-step-more = Больше на { $step } { $unit }
field-step-less = Меньше на { $step } { $unit }
field-back-to = Вернуть { $value } { $unit }
field-printer-address = Адрес принтера

## ui/widgets/layout.rs

dialog-close = Закрыть  Esc

## ui/widgets/list.rs

list-hide = Скрыть в окне просмотра
list-show = Показать в окне просмотра

## panels/inspector/measure.rs

measure-settings = Настройки
measure-measure = Замер
measure-esc-clears = Esc сбрасывает.
measure-clear = Сбросить
measure-points = Точки
measure-planes = Плоскости
measure-snap-to = Привязка
measure-snap-corner = Угол
measure-snap-edge = Ребро
measure-snap-free = Свободно
measure-snap-corner-note = Щелчок захватывает ближайший угол, где поверхность меняет направление.
measure-snap-edge-note = Щелчок захватывает ближайшее ребро, где поверхность меняет направление.
measure-snap-free-note = Точно там, где пришёлся щелчок.
measure-free-while-alt = Свободно, пока зажат Alt.
measure-hold-alt = Удерживайте Alt для свободного замера.
measure-show-axes = Показывать dX, dY, dZ в окне просмотра
measure-second-point = Щёлкните по второй точке.
measure-two-points = Щёлкните по двум точкам на модели.
measure-distance = Расстояние
measure-second-face = Щёлкните по второй плоской грани.
measure-two-faces = Щёлкните по двум плоским граням модели.
measure-gap = Зазор
measure-angle = Угол
measure-between-centres = Между центрами
measure-not-parallel = Не параллельны, поэтому единого зазора между ними нет.

## panels/inspector/hollow.rs

hollow-shell = Оболочка
hollow-infill = Заполнение
hollow-keep-solid = Оставить сплошным
hollow-keep-solid-note = Щёлкните по модели, чтобы оставить стенку сплошной в этом месте; Alt + щелчок по блокиратору убирает его.
hollow-drainage = Дренаж
hollow-looking = Поиск запертой смолы
hollow-drains = Из всех полостей на платформе смола может вытечь.
hollow-trapped =
    { $count ->
        [one] Смола заперта в { $count } месте
        [few] Смола заперта в { $count } местах
       *[many] Смола заперта в { $count } местах
    }
hollow-trapped-note = Каждому месту нужно собственное отверстие. Включите рентген, чтобы увидеть заполняемый ею объём.
hollow-drain-trapped = Слить запертую смолу
hollow-inside = Внутрь
hollow-outside = Наружу
hollow-inside-note = Стенка растёт внутрь; снаружи ничего не меняется.
hollow-outside-note = Стенка растёт наружу, получается форма для отливки модели.
hollow-bottom-through = Сквозь дно
hollow-bottom-through-note = Стенка растёт внутрь, а дно, на котором стоит модель, вырезается, и смола стекает без отверстий. На платформе полость закрыта ею, пока отпечаток не снят, поэтому добавьте маленькое вентиляционное отверстие наверху.
hollow-shell-help = Полая модель расходует меньше смолы и слабее тянет плёнку. Стенка — толщина оболочки; около 2 мм подходит большинству деталей. Полости нужно дренажное отверстие, иначе запертая внутри смола может позже разорвать деталь.
hollow-wall = Стенка
hollow-precision = Точность
hollow-precision-note = Точность определяет гладкость полости на сетке { $lattice } мм. Это не высота слоя.
hollow-empty = Полость остаётся пустой.
hollow-hive = Соты
hollow-grid = Решётка
hollow-scaffold = Каркас
hollow-cell = Ячейка
hollow-cell-hive = Диаметр шестигранника
hollow-cell-grid = Ширина ячейки
hollow-density = Плотность
hollow-infill-walls-are = Стенки получаются толщиной { $mm } мм.
hollow-infill-struts-are = Стойки получаются толщиной { $mm } мм.
hollow-infill-thin = Каркас получается толщиной { $mm } мм, слишком тонким для печати: увеличьте ячейку или плотность.
hollow-walled-note = Ячейки стоят на платформе и соединены круглыми отверстиями ряд за рядом, поэтому смола перетекает из ячейки в ячейку к дренажному отверстию.
hollow-scaffold-note = Стойки по рёбрам куба, поставленного на вершину: все наклонены, ни одна не начинается в воздухе, и каркас держит нагрузку и вбок.
hollow-blocker-size = Размер блокиратора
hollow-clear-blockers =
    { $count ->
        [one] Убрать { $count } блокиратор
        [few] Убрать { $count } блокиратора
       *[many] Убрать { $count } блокираторов
    }
hollow-hollow = Выдолбить
hollow-hollowing = Выдалбливание
hollow-hollowed = Выдолблено
hollow-resin-saved = Сэкономлено смолы
hollow-coarsened = Этой модели потребовалось больше памяти, чем допускает лимит, поэтому полость вырезана на сетке { $lattice } мм.
hollow-stale = Числа с тех пор изменились. Выдолбите заново, чтобы обновить.
hollow-make-solid = Снова сделать сплошной

## panels/inspector/print_settings.rs

print-bottom-layers = Нижние слои
print-exposure = Экспозиция
print-bottom-exposure = Экспозиция нижних слоёв
print-light-off = Свет выключен
print-before-lift = До подъёма
print-after-lift = После подъёма
print-after-retract = После возврата
print-lift = Подъём
print-lift-speed = Скорость подъёма
print-retract-speed = Скорость возврата
print-this-session = в этом сеансе
print-from-resin = Из профиля смолы. Правки меняют только нарезку в этом сеансе; в профиле смолы остаются прежние значения.
print-exposure-help = Сколько экран засвечивает каждый слой. Мало — тонкие части не печатаются или слои расслаиваются; много — детали разбухают, а отверстия зарастают. Нижние слои засвечиваются дольше, чтобы печать держалась на платформе, а слои перехода плавно снижают время между ними.
print-waiting-help = Паузы, чтобы смола успокоилась. «Свет выключен» — одна пауза перед засветкой каждого слоя; время отдыха — паузы после засветки, наверху подъёма и после возврата платформы. Длинные паузы помогают густой смоле и большим слоям, но удлиняют печать.
print-motion-help = После каждого слоя платформа поднимается, отрывая его от плёнки, и возвращается. Медленный подъём отрывает мягче — это нужно большим и хрупким слоям, а высоты подъёма должно хватать, чтобы плёнка отпустила слой.
print-waiting = Паузы
print-motion = Движение
print-carried = Перенесено с { $exposure } с при { $from } мм на слой { $to } мм.
print-transition = Переход
print-transition-layers = Слои перехода
print-transition-type = Тип перехода
print-linear = Линейный
print-step = Ступенчато между слоями
print-resting = Отдых

## panels/inspector/relief.rs

relief-tile-size = Размер плитки
relief-angle = Угол
relief-depth = Глубина
relief-precision = Точность
relief-builtins = Текстуры
relief-texture-leather = Кожа
relief-texture-matte = Матовое зерно
relief-texture-carbon = Карбон
relief-texture-weave = Плетение
relief-texture-honeycomb = Соты
relief-texture-hex-blocks = Шестигранники
relief-texture-reptile = Рептилия
relief-texture-ripples = Волны
relief-texture-wood = Дерево
relief-texture-bark = Кора
relief-texture-stone = Камень
relief-texture-rock = Скала
relief-invert = Инвертировать картинку
relief-smoothing = Сглаживание
relief-symmetric = Симметрично от среднего серого
relief-hold = Оставить ровными
relief-hold-bottom = Грани вниз
relief-hold-top = Грани вверх
relief-hold-note = Грани, отклонённые от направления на платформу или прямо вверх не больше чем на столько градусов, сохраняют форму, и рельеф обрывается на их краю. Ноль не оставляет ни одной.
relief-paint = Где
relief-paint-brush = Рисовать, где выдавить
relief-paint-hint = Ведите по модели, чтобы закрасить; Alt — стереть.
relief-paint-note = Как только на модели закрашена хоть одна грань, текстура выдавливается только в закрашенные грани, остальная модель сохраняет свою сетку, и вид показывает текстуру только там.
relief-paint-clear = Очистить закрашенные области
relief-nothing-painted = Ничего не закрашено: закрасьте, где нужна текстура, или отключите кисть, чтобы выдавить на всю модель.
relief-image = Изображение
relief-image-note = Собственная текстура модели превращается в геометрию: белый выдвигает поверхность наружу, чёрный оставляет на месте. Либо выберите текстуру ниже или свою картинку, чтобы замостить ею любую модель.
relief-picture-size = { $width } × { $height } пикс.
relief-choose-another = Выбрать другую
relief-choose-picture = Выбрать картинку
relief-own = Собственные текстуры моделей
relief-own-none = На столе их нет: нажмите на квадрат, чтобы выбрать текстуру
relief-own-textures = Использовать собственные текстуры моделей
relief-tiling-note = Картинка накладывается с трёх сторон, на которые смотрит модель, и каждая грань читает её со стороны, на которую смотрит больше всего.
relief-lattice-note = Рельеф вырезается на сетке { $lattice } мм, и вся модель заново строится по ней. Отрицательная глубина вдавливает текстуру.
relief-no-texture = На платформе нет текстур. Текстура читается из файла, из которого открыта модель, и пропадает после вдавливания.
relief-textures =
    { $count ->
        [1] Текстура
        [one] { $count } текстура
        [few] { $count } текстуры
       *[many] { $count } текстур
    }
relief-pressing = Вдавливание...
relief-cancelled = Вдавливание отменено
relief-press = Вдавить
relief-pressing-status = Вдавливание рельефа
text-words = Текст
text-words-hint = Что написать; новая строка начинает второй ряд
text-reading-fonts = Читаю шрифты, установленные на этом компьютере...
text-no-family = Шрифта с таким названием нет
text-raised = Выпуклые
text-sunk = Вдавленные
text-height = Высота букв
text-depth = Глубина
text-angle = Угол
text-flat = Плоские
text-follow = По поверхности
text-shape-note = Буквы выступают из грани или уходят в неё на глубину, отсчитанную от грани в точке щелчка, и следуют за ней не дальше этой глубины; на кривой грани буквы остаются плоскими.
text-follow-note = Каждая буква ложится на грань под собой и выступает из неё или уходит в неё на всю глубину по всей площади, как бы грань ни изгибалась.
text-click-note = Щёлкните по модели, чтобы написать на ней текст; предпросмотр под курсором показывает, что получится. Щелчок с Shift или Cmd выбирает модель, ничего на ней не написав.
text-stamping = Ставлю надпись
text-cancelled = Надпись отменена
text-nothing-under = здесь нет модели, на которой писать
text-done = Надпись на { $name } готова
text-bad-font = этот шрифт не читается: { $reason }
text-missing-glyph = в этом шрифте нет буквы { $character }
text-blank = писать нечего
text-bad-size = буквам нужны высота и глубина
text-off-the-model = здесь надпись не встаёт на модель
relief-mode = Режим
relief-mode-texture = Текстура
relief-mode-text = Текст
relief-mode-combine = Объединение
relief-mode-texture-note = Текстура или картинка, вдавленная в поверхность моделей на плите.
relief-mode-text-note = Текст, выступающий из модели или вдавленный в неё там, где вы щёлкнете.
relief-mode-combine-note = Выбранные модели, сложенные в одну, вычтенные одна из другой или их общая часть.
text-lettering = Буквы
boolean-operation = Операция
boolean-models = Модели
boolean-none-picked = Пока ничего не выбрано.
boolean-models-note = В том порядке, в каком выбраны. Щёлкните модель, чтобы выбрать её, и с Shift или Cmd — чтобы добавить следующую.
boolean-role-kept = остаётся
boolean-role-taken-out = вычитается
boolean-role-joined = добавляется
boolean-role-shared = общая часть
boolean-union = Сложить
boolean-difference = Вычесть
boolean-intersection = Пересечь
boolean-union-note = Выбранные модели становятся одной, в которой есть всё, что было в любой из них.
boolean-difference-note = Остальные выбранные модели вычитаются из той, что выбрана первой.
boolean-intersection-note = Остаётся только то, что есть во всех выбранных моделях.
boolean-combine-none = Объединить
boolean-combine =
    { $count ->
        [one] Объединить { $count } модель
        [few] Объединить { $count } модели
       *[many] Объединить { $count } моделей
    }
boolean-combining = Объединение моделей
boolean-cancelled = Объединение отменено
boolean-pick-two = Выберите на плите две модели или больше, удерживая Shift или Cmd.
boolean-not-solid = модель не замкнута, поэтому её нельзя вырезать точно; сначала почините её
boolean-nothing-left = после объединения от моделей ничего не остаётся
boolean-failed = модели не удалось объединить: { $reason }
boolean-done =
    { $count ->
        [one] { $count } модель объединена в { $name }
        [few] { $count } модели объединены в { $name }
       *[many] { $count } моделей объединено в { $name }
    }
boolean-done-sampled =
    { $count ->
        [one] { $count } модель объединена в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
        [few] { $count } модели объединены в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
       *[many] { $count } моделей объединено в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
    }
job-thread-stopped-boolean = поток объединения остановился, не закончив
job-thread-stopped-cut = поток разреза остановился, не закончив
job-thread-stopped-split = поток разделения остановился, не закончив
job-thread-stopped-plugs = поток вырезания пробок остановился, не закончив
job-thread-stopped-drill = поток вырезания отверстий остановился, не закончив
job-model-changed = пока шла работа, { $name } изменили, поэтому результат отброшен

## panels/title_bar.rs

menu-file = Файл
menu-edit = Правка
menu-view = Вид
chip-machine = Принтер: { $name }
chip-resin = Смола: { $name }
chip-machine-tooltip = Принтер, на котором печатается эта платформа
chip-resin-untuned = Смола, не откалибрована на этом принтере
chip-resin-tooltip = Смола
chip-layers = { $layer } мкм, { $exposure } с
find-button = Найти
find-tooltip = Найти инструмент, действие или модель
bar-source = Исходный код
bar-source-tooltip = Encrust — свободное ПО под лицензией AGPL-3.0; это код, который выполняется
bar-close = Закрыть
bar-restore = Восстановить
bar-maximise = Развернуть
bar-minimise = Свернуть
bar-restart-to-update = Перезапустить для обновления
bar-restart-tooltip = Закрыть окно и запустить новую сборку
bar-update-tooltip = Вышла новая версия: посмотрите, что изменилось
bar-alpha = Альфа
bar-alpha-tooltip = Encrust в альфа-версии: расскажите, что пошло не так
menu-new-plate = Новая платформа
menu-new-project = Новый проект
menu-open-project = Открыть проект...
menu-open-recent = Открыть недавние
menu-save-project = Сохранить проект
menu-save-project-as = Сохранить проект как...
menu-open-model = Открыть модель...
menu-open-test-print = Открыть пробную печать
menu-import-machine = Импортировать принтер...
menu-import-resin = Импортировать смолу...
menu-machine-and-resin = Принтер и смола...
menu-settings = Настройки...
menu-quit = Выход
menu-nothing-recent = Пока ничего не открывалось
menu-arrange-plate = Расставить на платформе
menu-auto-orient-plate = Автоориентация платформы
menu-move-to-plate = Переместить на платформу
menu-frame-view = Показать всё
menu-plate-grid = Сетка платформы
menu-xray = Просвечивать модели
menu-orthographic = Ортогональный вид

## panels/support_fields.rs

support-contact = Контакт
support-bite = Врезка
support-length = Длина
support-diameter = Диаметр
support-clearance = Зазор
support-trunk-cap = Шапка ствола
support-thickness = Толщина
support-flare = Раструб
support-upper-depth = Верхняя глубина
support-lower-depth = Нижняя глубина
support-max-length = Макс. длина
support-branch-angle = Угол ветвления
support-merge-within = Объединять в пределах
support-margin = Отступ
support-wall-slope = Наклон стенки
support-span = Пролёт
support-first-at = Первая на
support-every = Каждые
support-lift = Подъём
support-overhang = Нависание
support-contact-spacing = Шаг контактов
support-steep-contact-spacing = Шаг на крутых гранях
support-density = Плотность
support-group-top = Верх
support-group-pillar = Колонна
support-group-bottom = Низ
support-group-small-pillar = Малая колонна
support-group-branching = Ветвление
support-group-raft = Подложка
support-group-bracing = Перекрёстные связи
support-group-automatic = Автоматически
support-shape-plane = Плоскость
support-shape-sphere = Сфера
support-foot-round = Круглая
support-foot-tapered = Конусная
support-foot-prism = Призма
support-foot-cube = Куб
support-foot-skate = Полоз
support-land-on-model = Опираться на модель
support-land-on-model-off = Поддержка, под которой находится модель, отклоняется к платформе или отбрасывается. На детали ничего не остаётся засвеченным.
support-thin-down = Сделать их тоньше
support-thin-down-off = Поддержка, опирающаяся на модель, сохраняет ширину обычной колонны.
support-small-pillar-note = Применяется там, где поддержка идёт от одной части модели к другой. Оба конца уходят в поверхность; более длинный участок сохраняет обычную ширину.
support-no-branching = Каждый наконечник стоит на собственной колонне.
support-tip-help = Единственная часть, которая касается модели. Широкий контакт держит крепче, но оставляет след больше; врезка утапливает кончик в поверхность, чтобы он не оторвался во время печати. Длина — конус от кончика до колонны, а сфера держит тяжёлое нависание крепче плоскости.
support-body-help = Диаметр — толщина колонны: толще значит жёстче, но смолы уходит больше. Зазор — воздух между колонной и моделью, чтобы колонна не приросла к детали. Шапка ствола — наибольшая ширина ствола у основания.
support-foot-help = Основание держит поддержку на платформе. Шире и толще — держит крепче, но труднее снимается; у конусной и полоза есть край, под который заходит шпатель. Раструб — насколько колонна расширяется перед основанием.
support-branching-help = Соседние кончики сходятся в общие стволы, как дерево: меньше колонн, меньше смолы и следов на платформе. Больший угол ветвления объединяет больше, но ветви наклоняются сильнее; кончики дальше, чем «Объединять в пределах», стоят на своих колоннах.
support-raft-help = Плита, на которой поддержки стоят вместо платформы: она распределяет отрыв по одной площади и снимается одним куском. Решётка меньше присасывается к платформе, «Сплошная» соединяет площадки под каждым основанием. Отступ — насколько подложка выходит за модель, а наклонная стенка держит крепче и всё равно легко поддевается.
support-bracing-help = Распорки связывают высокие стволы, чтобы те не раскачивались при отрыве слоя. Пролёт — наибольшее расстояние между стволами, которое они соединяют; «Первая на» и «Каждые» задают высоту первой распорки и шаг между ними.
support-held-up-help = Нависание — насколько поверхность может отклониться от вертикали, прежде чем получит поддержки: 90° — потолок, меньшее число ставит больше поддержек. Шаг контактов — расстояние между кончиками под гранью, смотрящей прямо вниз, а шаг на крутых гранях — под гранью, наклонённой чуть дальше угла нависания; между ними шаг меняется вместе с углом. Плотность умножает их количество: 1 — столько, сколько Encrust считает достаточным.
support-lift-help = На какой высоте модель стоит над платформой. Поддержкам нужно место под ней, а первые слои прилипают к платформе сильнее всего, поэтому деталь, напечатанную прямо на ней, трудно снять.
support-raft = Подложка под опорами
support-raft-skates = Полозья
support-raft-grid = Решётка
support-raft-connected = Сплошная
support-raft-hull = Контур
support-raft-rectangle = Прямоугольник
support-bracing = Скреплять высокие стволы

## panels/settings/supports.rs

support-table-auto = Автоматический подбор
support-table-manual = Вручную
support-table-tip = Наконечник
support-table-spacing = Шаг
support-cannot-add = не удалось добавить профиль поддержек
support-pick-profile = Выберите профиль поддержек.
support-cannot-save = не удалось сохранить профиль поддержек

## panels/inspector/shape.rs

shape-part = Часть
shape-regular = Обычная
shape-raft-own = Подложка принадлежит этой модели, она пришла с её проектом.
shape-raft-plate = Подложка принадлежит платформе: на ней стоят все модели.
shape-use-plate-raft = Использовать подложку платформы
shape-use-profile-raft = Использовать подложку из «{ $name }»
shape-edited = { $name } (изменён)
shape-edit-profiles = Изменить профили поддержек
shape-save = Сохранить
shape-save-tooltip = Сохранить поверх профиля, с которого взяты эти значения
shape-save-as-new = Сохранить как новый
shape-save-as-new-tooltip = Сохранить эти значения как отдельный профиль
shape-saved = Профиль поддержек сохранён
shape-profile-help = Профиль — сохранённый набор размеров поддержек. Выбор другого меняет то, что вырастет дальше; уже стоящие поддержки сохраняют форму. Изменённые значения помечают профиль как изменённый, пока «Сохранить» не перезапишет его или «Сохранить как новый» не сохранит копию.

## panels/support_diagram.rs

diagram-tip = Наконечник
diagram-body = Тело
diagram-base = Основание
diagram-raft = Подложка
diagram-brace = Связь
diagram-no-branching = без ветвления
diagram-no-raft = без подложки
diagram-every = каждые { $value }
diagram-first = первая на { $value }
diagram-no-bracing = без связей
diagram-max = макс. { $value }
diagram-model = модель
diagram-plate = платформа

## panels/settings/presets.rs

presets-blocker = Блокиратор
presets-depth = Глубина
presets-taper = Конусность
presets-empty =
    { $kind ->
        [hollow] Профилей полости пока нет. Сохраните один из строки «Профиль» инструмента или импортируйте.
       *[drain] Профилей дренажа пока нет. Сохраните один из строки «Профиль» инструмента или импортируйте.
    }
presets-none = Нет
presets-cannot-rename = не удалось переименовать профиль

## panels/inspector/presets.rs

presets-unsaved = Не сохранено
presets-none-saved = Пока ничего не сохранено
presets-delete-named = Удалить «{ $name }»
presets-deleted =
    { $kind ->
        [hollow] Профиль полости удалён
       *[drain] Профиль дренажа удалён
    }
presets-name-hint = Назовите эти настройки

## panels/settings/machines/resins.rs

resins-type = Тип
resins-layer = Слой
resins-bottom = Нижние
resins-every = Все смолы
resins-by-hand = Экспозиция и слои с бутылки или сайта производителя смолы
resins-add = Добавить смолу
resins-from-file = Файл, нарезанный для этого принтера (.goo, .ctb, ...), или профиль смолы (.cfgx, .ini), либо перетащите его сюда
resins-import = Импортировать смолу
resins-all-types = Все типы
resins-from-pool = Из общего списка
resins-delete = Удалить эту смолу
resins-all-here = Все ваши смолы уже добавлены на этот принтер.
resins-none-here = На этом принтере пока нет смол. Добавьте смолу кнопкой «Добавить смолу» выше.
resins-row-in-hand = Платформа печатается этой смолой. Дважды щёлкните, чтобы изменить.
resins-row = Дважды щёлкните, чтобы изменить.
resins-take-off = Убрать с этого принтера
resins-use = Использовать эту смолу

## files.rs

filter-model = Модель
filter-mesh = Меш
filter-sliced = Нарезанный файл
filter-model-or-project = Модель или проект
filter-project = Проект Encrust
filter-printer-profile = Профиль принтера
filter-resin = Профиль смолы или нарезанный файл
filter-picture = Изображение
filter-profile = Профиль

## panels/settings/resin.rs

resin-density = Плотность
resin-layer-height = Высота слоя
resin-retract = Возврат
resin-bottom-lift = Подъём нижних слоёв
resin-bottom-lift-speed = Скорость подъёма нижних слоёв
resin-bottom-retract-speed = Скорость возврата нижних слоёв
resin-resin = Смола
resin-exposure-on = Экспозиция на принтере { $printer }
resin-waits = Паузы
resin-compensation = Компенсация
resin-cannot-save = не удалось сохранить смолу
resin-cannot-rename = не удалось переименовать смолу
resin-colour = Цвет
resin-per = за
resin-per-kg = кг
resin-per-litre = л
resin-light-off-delay = Задержка выключения света
resin-rest-times = Время отдыха
resin-waits-note = Входит во время печати и записывается в файл.
resin-carried = Перенесено с { $exposure } с при { $from } мм на { $to } мм.
resin-light-power = Мощность света
resin-bottom-power = Мощность на нижних слоях
resin-transition-note = Линейный: каждый слой на { $step } с короче нижележащего.

## panels/settings/compensation.rs

comp-every-second-layer = Каждый второй слой
comp-unaccounted = Неучтённое время на слой
comp-shrinkage = Усадка
comp-from-measured = Вычислить по измеренной детали
comp-tolerance = Допуск
comp-bottom-tolerance = Допуск на нижних слоях
comp-print-time = Время печати
comp-from-print = Вычислить по завершённой печати
comp-shrinkage-title = Усадка по измеренной детали
comp-layer-time-title = Неучтённое время по печати
comp-shrinkage-note = Тестовая деталь, напечатанная в масштабе 100 %, досвеченная и измеренная в холодном виде.
comp-drawn = По модели
comp-measured = Измерено
comp-scale = Масштаб
comp-use-these = Применить
comp-limit = Поле принимает значения от { $least } до { $most } { $unit }, поэтому будет установлено { $kept }.
comp-same-print = Оба значения — из одной печати.
comp-predicted = Расчётное
comp-actual = Фактическое
comp-per-layer = На слой
comp-use-this = Применить
comp-hours = часы
comp-minutes = минуты
comp-seconds = секунды

## panels/inspector/layers.rs

layers-cusp = Выступ
layers-thinnest = Самый тонкий слой
layers-from = От
layers-to = До
layers-thickest = Самый толстый слой
layers-bands =
    { $count ->
        [one] { $count } полоса
        [few] { $count } полосы
       *[many] { $count } полос
    }
layers-by-height = По высоте
layers-levels =
    { $count ->
        [one] { $count } уровень
        [few] { $count } уровня
       *[many] { $count } уровней
    }
layers-edge-smoothing = Сглаживание краёв
layers-edge-note = Оттенки серого на краю маски, чтобы скрыть пиксельные ступеньки.
layers-look = Вид
layers-number = Число
layers-anti-aliased = Маски со сглаживанием
look-off = Выкл.
look-sharp = Чёткий
look-balanced = Сбалансированный
look-soft = Мягкий
look-off-line = Исходные маски
look-sharp-line = Чёткий текст
look-balanced-line = По умолчанию
look-soft-line = Плавные кривые
layers-by-hand = Значения заданы вручную. Выберите вид, чтобы заменить их.
layers-open-printer = Открыть настройки принтера
layers-planes = Плоскостей на слой
layers-planes-note = элемент тоньше слоя сохранится при таком кратном увеличении числа сечений
layers-height-help = Тонкие слои дают меньше ступенек на изгибах, но печать дольше: время растёт с числом слоёв. Адаптивная высота режет тонкие слои там, где поверхность изгибается, и толстые там, где она идёт прямо.
layers-round-grey = Округление серого на краю
layers-grey-levels = Оттенки серого
layers-edge-blur = Размытие края
layers-blur-note = сильное размытие затемняет и стенку внутри края; обычно хватает одного-двух пикселей
layers-carried = Экспозиция перенесена вместе со слоем: { $was } с при { $from } мм теперь { $now } с.
layers-thickest-reached = Самый толстый достигнутый слой — { $thickest } мм, а не { $asked } мм, поэтому обычный слой экспонируется { $exposure } с, а не { $resin } с.
layers-adaptive = Адаптивная высота
layers-adaptive-refused = Этот принтер двигает платформу на высоту из заголовка, поэтому у слоя не может быть собственной высоты.
layers-no-bands = Все слои экспонируются по значению смолы. Полоса заменяет его на участке высоты.
layers-bands-note = Нижние слои сохраняют экспозицию смолы, что бы ни говорила полоса.
layers-bands-refused = Этот принтер читает только заголовок, поэтому экспозицию нельзя менять по высоте.
layers-add-band = Добавить полосу
layers-band = Полоса { $index }
layers-remove-band = Удалить эту полосу

## panels/settings/printer.rs

printer-panel-width = Ширина экрана
printer-panel-height = Высота экрана
printer-lit-width = Ширина засвечиваемой области
printer-lit-height = Высота засвечиваемой области
printer-volume-x = Объём X
printer-volume-y = Объём Y
printer-volume-z = Объём Z
printer-per-layer = Настройки по слоям
printer-variable-height = Переменная высота слоя
printer-manufacturer = Производитель
printer-display = Экран
printer-build-volume = Область построения
printer-output = Вывод
printer-firmware = Прошивка
printer-browser-sends-nothing = Браузер не может достучаться до принтера в вашей сети. Нарезанный файл скачивается, и его нужно переносить на флешке или через приложение самого принтера.
printer-own-copy = Это ваша копия принтера, который поставляется и в этой сборке, и они различаются. Приоритет у вашей, даже если в новом выпуске исправлены поставляемые значения.
printer-use-shipped = Использовать поставляемый профиль
printer-cannot-put-back = не удалось вернуть поставляемый профиль
printer-cannot-save = не удалось сохранить принтер
printer-grey-floor = Порог серого
printer-grey-floor-off = записываются все оттенки серого; слишком тусклый для засветки пиксель оставляет жидкую смолу
printer-grey-floor-on = более тусклый пиксель записывается чёрным, поэтому край теряет долю пикселя
printer-pitch = { $x } × { $y } мкм на пиксель
printer-mirror-x = Зеркало по X
printer-mirror-y = Зеркало по Y
printer-mirror-note = Маска, напечатанная наоборот, даёт зеркальную деталь.
printer-per-layer-on = экспозиция и движение читаются из каждого слоя; в .goo это называют расширенным режимом
printer-per-layer-off = каждый слой печатается по значениям из заголовка, а нижние слои принтер плавно проходит сам
printer-variable-height-note = Если выключено, платформа двигается на высоту из заголовка, поэтому адаптивный набор слоёв отклоняется.
printer-output-note = Формат, в который нарезается для этого принтера, и для диалога, и для самого принтера.

## network.rs

reach-answered = { $name } ответил при последнем сканировании
reach-silent = { $name } не ответил при последнем сканировании
reach-unasked = { $name } пока не опрашивался
network-advice-app = Если принтер включён, возможно, macOS не пускает Encrust в локальную сеть: разрешите это в разделе «Системные настройки», «Конфиденциальность и безопасность», «Локальная сеть».
network-advice-terminal = Если принтер включён, возможно, macOS не пускает это окно в локальную сеть. При запуске из терминала у окна права этого терминала: запустите его из Terminal.app, разрешите свой терминал или редактор в разделе «Системные настройки», «Конфиденциальность и безопасность», «Локальная сеть» либо запустите его как приложение командой `cargo xtask app`.
network-not-reached = пока недоступен
network-printing = { $printer } печатает { $file }
network-busy = { $name } занят: { $state }
network-prusa-sl1 = Принтеры Prusa печатают .sl1; выберите этот формат как вывод для принтера.
network-sending = Отправка на { $name }
network-starting = Запуск { $file } на { $name }
network-sent = { $file } на { $printer }
network-cancelled = Передача отменена
network-usb-only = Только USB

## panels/settings/printer/connection.rs

connection-usb-note = Нарезанное сохраняется в файл и переносится на флешке.
connection-found = Найдены в сети
connection-scanning = Сканирование...
connection-scan-again = Сканировать снова
connection-add = Добавить
connection-address-hint = Добавить по адресу, 192.168.1.42
connection-listed = Принтеры, на которые отправляет этот принтер
connection-none-listed = Пока нет: отметьте их ниже или добавьте по адресу.
connection-speaks = Работает по { $theirs }, а не по { $ours }
connection-not-found = Не найден в сети
connection-not-looked = Пока не искали
connection-take-off = Убрать из списка этого принтера
connection-looking = Поиск принтеров в сети
connection-all-listed = Все найденные принтеры в списке.
connection-other-model = Другая модель, чем в этом профиле
connection-add-all =
    { $count ->
        [one] Добавить { $count } этой модели
        [few] Добавить все { $count } этой модели
       *[many] Добавить все { $count } этой модели
    }
connection-set-up-here = Настроить в этом окне
connection-forget-login = Забыть логин для { $host }
connection-set-up-another = Настроить другой
connection-login-note = Логин хранится в настройках этого окна, а не в профиле.
connection-password = пароль
connection-api-key = или ключ API

## panels/printer_card.rs

card-open-printer = Открыть принтер
card-search-failed = Поиск не удалось отправить ни в одну сеть
card-nothing-answered = Никто не ответил по адресу { $address }
card-destination-several = Принтер, на который отправляет этот профиль; нажмите, чтобы увидеть остальные в списке
card-destination = Принтер, на который отправляет этот профиль
card-not-answering = Не отвечает
card-connecting = Подключение
card-not-asked = Пока не опрашивался
card-paused = Пауза
card-ready = Готов
card-stopped-error = Остановлен: { $error }
card-finished = Завершено
card-stopped = Остановлено
card-last-print = Последняя печать
card-layer-of = Слой { $layer } из { $count }
card-light-source = Источник света
card-hood = Крышка
card-celsius = { $name } { $value } °C
card-almost-done = Почти готово
card-minutes-left = осталось { $minutes } мин
card-hours-left = осталось { $hours } ч
card-time-left = осталось { $hours } ч { $minutes } мин

## panels/slice.rs

slice-to = Нарезать в .{ $extension }
slice-tooltip = Растеризовать платформу в файл для печати
slice-more-tooltip = Нарезать все платформы или нарезать и отправить
slice-all = Нарезать все платформы ({ $count })
slice-send = Нарезать и отправить на { $name }
slice-broken =
    { $count ->
        [one] { $count } модель на этой платформе повреждена; нарезанное из неё может не напечататься
        [few] { $count } модели на этой платформе повреждены; нарезанное из них может не напечататься
       *[many] { $count } моделей на этой платформе повреждены; нарезанное из них может не напечататься
    }
slice-cannot-send = не удалось нарезать набор слоёв для отправки
slice-slicing = Нарезка
slice-all-started = Нарезка платформ ({ $count }) рядом с { $path }
slice-cannot-slice-into = не удалось нарезать в { $path }
slice-slicing-into = Нарезка в { $path }

## slicing.rs

slicing-cannot-read-profiles = не удалось прочитать ваши профили в { $dir }
slicing-cannot-read-profiles-browser = не удалось прочитать ваши профили в этом браузере
slicing-select-resin = Выберите смолу
slicing-pick-machine = Выберите принтер, чтобы узнать, под какой экран нарезать.
slicing-add-resin = Добавьте смолу для этого принтера: экспозиция подбирается на вашем принтере.
slicing-nothing-visible = На платформе нет видимых моделей для нарезки.
slicing-steps-by-header = Этот принтер двигает платформу на высоту слоя из заголовка.
slicing-no-printer = профиль принтера не загружен
slicing-no-resin = смола не выбрана, а экспозиция подбирается, а не поставляется
slicing-nothing-visible-error = на платформе нет видимых моделей для нарезки
slicing-no-format = в { $path } не указан формат нарезанного файла
slicing-clipped =
    { $count ->
        [one] , { $count } слой обрезан по краю экрана
        [few] , { $count } слоя обрезано по краю экрана
       *[many] , { $count } слоёв обрезано по краю экрана
    }
slicing-wrote = Записан { $path }: слоёв — { $layers }, { $volume } мл, { $weight } г{ $cost }{ $clipped }
slicing-cancelled = Нарезка отменена

## panels/inspector/export.rs

export-on = на { $printer }
export-on-machine = Файл на принтере. Рядом с ним пока никого нет.
export-start-print = Начать печать
export-slice-all-to = Нарезать все платформы ({ $count }) в .{ $extension }
export-the-file = Файл
export-file-name = Имя файла
export-print-settings = Слои, экспозиция и края
export-all-plates = Все платформы ({ $count })
export-this-plate = Эта платформа
export-send-to-printer = Отправить на принтер
export-look-again = Искать снова
export-usb-only = Профиль этого принтера принимает файлы только с флешки; выберите его сеть в его настройках, чтобы отправлять отсюда.
export-none-listed = Принтеров этой модели для отправки пока нет в списке.
export-choose-them = Выберите их в сетевых настройках
export-speaks-other = { $name } работает по { $theirs }, а этот профиль отправляет по { $ours }: настройте сеть в настройках принтера.

## hollow.rs

hollow-nothing-visible = На платформе нет видимых моделей, которые можно выдолбить.
hollow-nothing-visible-error = на платформе нет видимых моделей, которые можно выдолбить
hollow-done =
    { $count ->
        [0] На платформе нечего выдалбливать
        [one] Выдолблена { $count } модель, сэкономлено { $saved } мл
        [few] Выдолблено { $count } модели, сэкономлено { $saved } мл
       *[many] Выдолблено { $count } моделей, сэкономлено { $saved } мл
    }
hollow-cancelled = Выдалбливание отменено

## orient.rs

orient-nothing-visible = На платформе нет видимых моделей для ориентации.
orient-nothing-visible-error = на платформе нет видимых моделей для ориентации
orient-pose = Положение { $index } из { $count }
orient-stepped =
    { $count ->
        [0] Других хороших положений нет: измените пресет, поддержки или подъём, чтобы искать заново
        [one] { $count } модель поставлена в следующее хорошее положение
        [few] { $count } модели поставлены в следующее хорошее положение
       *[many] { $count } моделей поставлено в следующее хорошее положение
    }
orient-turned =
    { $count ->
        [0] Все модели уже стоят так, как печатаются лучше всего
        [one] Повёрнута { $count } модель
        [few] Повёрнуты { $count } модели
       *[many] Повёрнуто { $count } моделей
    }
orient-seated =
    { $count ->
        [one] { $turned }; { $count } стоит на платформе
        [few] { $turned }; { $count } стоят на платформе
       *[many] { $turned }; { $count } стоят на платформе
    }
orient-others =
    { $count ->
        [one] { $turned }; запустите автоориентацию снова, чтобы увидеть ещё { $count } хорошее положение
        [few] { $turned }; запустите автоориентацию снова, чтобы увидеть ещё { $count } хороших положения
       *[many] { $turned }; запустите автоориентацию снова, чтобы увидеть ещё { $count } хороших положений
    }
orient-supports-ml = { $value } мл на поддержки
orient-pull = тяга { $value } Н
orient-tall = высота { $value } мм

## relief.rs

relief-nothing-visible = На платформе нет видимых моделей, в которые можно вдавить текстуру.
relief-no-textures = Ни у одной из этих моделей нет текстуры. Выберите картинку, которую нужно замостить по ним, либо откройте OBJ с изображением рядом или 3MF с изображением внутри.
relief-nothing-to-press = на платформе нет модели с текстурой для вдавливания
relief-done =
    { $count ->
        [one] Вдавлено { $depth } мм рельефа в { $count } модель
        [few] Вдавлено { $depth } мм рельефа в { $count } модели
       *[many] Вдавлено { $depth } мм рельефа в { $count } моделей
    }
relief-done-coarsened =
    { $count ->
        [one] Вдавлено { $depth } мм рельефа в { $count } модель; { $coarsened } получились на более грубой сетке, чем запрошено точностью
        [few] Вдавлено { $depth } мм рельефа в { $count } модели; { $coarsened } получились на более грубой сетке, чем запрошено точностью
       *[many] Вдавлено { $depth } мм рельефа в { $count } моделей; { $coarsened } получились на более грубой сетке, чем запрошено точностью
    }
relief-picked-picture = { $name } выбрана для вдавливания

## panels/inspector/position.rs

position-global = Глобально
position-local = Локально
position-move = Перемещение
position-move-reset = В центр платформы
position-move-along = Сдвиг по { $axis }
position-rotation = Поворот
position-rotation-reset = Как при импорте модели
position-rotation-about = Поворот вокруг { $axis }
position-scale = Масштаб
position-scale-reset = К размеру при импорте модели
position-size = Размер
position-ratio = Коэффициент
position-keep-proportions = Сохранять пропорции
position-axis-size = Размер по { $axis }
position-axis-ratio = Коэффициент по { $axis }
position-place = Размещение
position-on-plate = На платформе
position-centre = По центру
position-scale-to-fit = Вписать
position-lift = Поднять
position-click-face = Щёлкните по грани
position-lay-on-face = Положить на грань
position-auto-orient = Автоориентация
orient-preset-supports = Меньше поддержек
orient-preset-peel = Меньше усилие отрыва
orient-preset-shortest = Самая быстрая печать
orient-preset-auto = Авто
orient-preset-supports-hint = Меньше всего смолы в поддержках, в каком бы положении ни стояла модель.
orient-preset-peel-hint = Самая слабая тяга на плёнку, без присосок; при усилии меньше 10 Н — меньше всего поддержек.
orient-preset-shortest-hint = Наименьшее число слоёв, при котором модель ещё печатается.
orient-preset-auto-hint = Баланс поддержек, тяги и времени печати: ни в чём не приходится сильно уступать.
position-mirror = Зеркало
position-across = По { $axis }
position-keep-original = Сохранить оригинал
position-keep-original-note = Вместо этого зеркальная копия рядом.
position-place-help = «На платформе» ставит модель на платформу, «По центру» переносит её в середину, а «Вписать» увеличивает до размеров области печати. «Автоориентация» поворачивает модель так, как ей лучше печататься; «Положить на грань» кладёт на платформу грань, по которой вы щёлкнете.

## panels/inspector/paint.rs

paint-brush = Кисть
paint-surface-angle = Угол поверхности
paint-inside = Внутри
paint-hint-point = Щелчок — поставить, Alt + щелчок — убрать.
paint-hint-paint = Перетаскивайте, чтобы закрасить; Cmd + Shift + щелчок по поверхности; Alt — стереть.
paint-hint-block = Перетаскивайте, чтобы запретить; Cmd + Shift + щелчок по поверхности; Alt — стереть.
paint-hint-erase = Перетаскивайте, чтобы стереть разметку и запреты; Cmd + Shift + щелчок по поверхности.
paint-brush-help = «Поставить» ставит одну поддержку по щелчку. «Залить» закрашивает область, которая получит поддержки по кнопке «Залить закрашенные области»; «Запретить» закрашивает места, где поддержкам стоять нельзя. Угол поверхности — насколько далеко расходится Cmd + Shift + щелчок: по всем граням, повёрнутым меньше чем на этот угол от выбранной.
paint-fill-help = Расстояние между поддержками в залитой области: «Кайма» — по её краю, «Внутри» — по остальной площади. Меньше — гуще и надёжнее, но следов больше.
paint-groups-help = Группа — набор поддержек одного профиля: на одной модели могут стоять толстые поддержки под тяжёлыми частями и тонкие под деталями. Новые поддержки попадают в текущую группу.
paint-rim = Кайма
paint-groups = Группы
paint-fill = Залить закрашенные области
paint-filled =
    { $count ->
        [0] В закрашенных областях негде поставить поддержку
        [one] Закрашенные области заполнены { $count } поддержкой
        [few] Закрашенные области заполнены { $count } поддержками
       *[many] Закрашенные области заполнены { $count } поддержками
    }
paint-clear-blocked = Очистить запрещённые области
paint-clear-both = Очистить закрашенные и запрещённые области
paint-clear-painted = Очистить закрашенные области
paint-drop-group = Удалить эту группу
paint-build-to = Строить эту группу до
paint-group-holds =
    { $count ->
        [one] В группе «{ $name }» { $count } поддержка, она удалится вместе с группой.
        [few] В группе «{ $name }» { $count } поддержки, они удалятся вместе с группой.
       *[many] В группе «{ $name }» { $count } поддержек, они удалятся вместе с группой.
    }
paint-drop-the-group = Удалить группу
paint-dropped =
    { $count ->
        [one] Удалена «{ $name }» и { $count } поддержка
        [few] Удалена «{ $name }» и { $count } поддержки
       *[many] Удалена «{ $name }» и { $count } поддержек
    }
paint-keep-it = Оставить
paint-new-group = Новая группа

## panels/inspector/drain.rs

drain-hole-size = Размер отверстия
drain-trapped-resin = Запертая смола
drain-opening = Отверстие
drain-opening-note = Щёлкните по модели, чтобы поставить отверстие, Alt + щелчок по отверстию убирает его. Каждое вырезается сразу при установке, полая модель или нет.
drain-fact = заперто: { $count }
drain-dig = Прорыть канал (точек: { $count })
drain-dig-tooltip = Под каждой точкой проходит трубка с размером отверстия, открытая только на двух концах.
drain-dug =
    { $count ->
        [one] Прорыт { $count } канал
        [few] Прорыто { $count } канала
       *[many] Прорыто { $count } каналов
    }
drain-check-again = Проверить снова
drain-check = Проверить дренаж
drain-check-tooltip = Сечёт платформу и прослеживает путь воздуха, ища смолу без выхода.
drain-checking = Проверка дренажа
drain-hole = Отверстие
drain-channel = Канал
drain-channel-note = Каждый щелчок добавляет точку; канал повторяет ширину и контур отверстия и проходит под всеми ними и открытая только на двух концах.
drain-sizes-note = Глубина отсчитывается от поверхности внутрь. Конусность меньше 1 делает отверстие конусом.
drain-round = Круглое
drain-polygon = Многоугольник
drain-sides = Стороны
drain-turn = Поворот
drain-polygon-note = Углы лежат на размере отверстия, поворот отсчитывается от горизонтали. В боковой стенке поверните его так, чтобы угол оказался сверху: острый свод печатается без провисания.
drain-plugs = Пробки
drain-plugs-note = Для каждого отверстия, как только оно вырезано, рядом с моделью на платформе появляется пробка: то, что отверстие вынуло из стенки, уже на зазор со всех сторон; её внешняя грань повторяет поверхность, поэтому закрывает отверстие заподлицо.
drain-plug-gap = Зазор на сторону
drain-plug-drift = Компенсация допусков у смолы выключена, поэтому пробка рисуется на { $drift } мм на сторону шире, чтобы напечататься с этим зазором.
drain-plug-name = { $name } (пробка { $index })
drain-plugs-failed = Не удалось вырезать пробки для дренажных отверстий.
drill-refused = Отверстия в { $name } вырезаны приближённо: модель не удалось прочитать как тело даже после починки, поэтому они могут прорезать и то, что стоит в них.
drain-plugs-closed = { $count ->
    [one] { $count } отверстие осталось без пробки: зазор на сторону не меньше его устья.
    [few] { $count } отверстия остались без пробки: зазор на сторону не меньше их устья.
   *[other] { $count } отверстий остались без пробки: зазор на сторону не меньше их устья.
}
drain-plugs-sampled = { $count ->
    [one] { $count } пробка повторяет поверхность лишь приблизительно: модель не удалось прочитать как тело даже после починки.
    [few] { $count } пробки повторяют поверхность лишь приблизительно: модель не удалось прочитать как тело даже после починки.
   *[other] { $count } пробок повторяют поверхность лишь приблизительно: модель не удалось прочитать как тело даже после починки.
}
drain-plugs-crowded = { $count ->
    [one] { $count } пробке не нашлось свободного места на столе, она стоит там, где её положили, рядом с моделью.
    [few] { $count } пробкам не нашлось свободного места на столе, они стоят там, где их положили, рядом с моделью.
   *[other] { $count } пробкам не нашлось свободного места на столе, они стоят там, где их положили, рядом с моделью.
}
drain-none-cut = Ни одного отверстия и ни одного канала ещё не вырезано.
drain-clear-cuts =
    { $count ->
        [one] Убрать { $count } вырез
        [few] Убрать { $count } выреза
       *[many] Убрать { $count } вырезов
    }
drain-rehollowing = Повторное выдалбливание вокруг каналов
drain-nothing-trapped = Ничего не заперто: смола может вытечь из каждой модели на платформе.
drain-not-checked = Ещё не проверялось.
drain-held = { $volume } мл не могут вытечь
drain-held-note = Она засвечивается внутри и добавляет вес, который должен преодолеть отрыв. Каждый карман на рентгене закрашен красным, и каждому нужно собственное отверстие.
drain-holes-placed =
    { $count ->
        [one] Поставлено { $count } отверстие
        [few] Поставлено { $count } отверстия
       *[many] Поставлено { $count } отверстий
    }
drain-drill-each = Просверлить отверстие в каждом
drain-lowest-point = Каждое отверстие ставится в самой нижней точке кармана.
drain-pocket = карман { $index }
drain-drill = Просверлить
drain-drill-tooltip = Отверстие в самой нижней точке этого кармана

## updates.rs

update-error-fetch = не удалось получить { $url }
update-error-feed = источник обновлений нечитаем
update-error-version = `{ $version }` не является версией выпуска
update-error-foreign = источник указывает за пределы выпусков этого проекта: { $url }
update-error-signature = загрузка не подписана ключом выпусков Encrust
update-error-signed = подпись относится к `{ $found }`, а не к `{ $expected }`
update-error-archive = не удалось распаковать загруженный архив
update-error-missing = в загруженном архиве нет `{ $name }`
update-error-replace = не удалось заменить { $path }
update-error-store = Microsoft Store не ответил
update-no-build = В этом выпуске нет сборки для этой платформы.
update-cannot-check = не удалось проверить обновления
update-installed = Encrust { $version } установлен
update-cannot-install = не удалось установить Encrust { $version }
update-cannot-open-store = не удалось открыть Microsoft Store

## updates/install.rs

update-dev-build = Сборка для разработки обновляется пересборкой.
update-no-exe = Не удалось определить расположение запущенной программы.
update-by-store = Эта копия обновляется через Microsoft Store.
update-by-package = Эта копия обновляется загрузкой нового пакета.

## panels/settings/updates.rs

updates-checking = Проверка
updates-by-store = Microsoft Store поддерживает эту копию в актуальном состоянии.
updates-store-link = Encrust в Microsoft Store
updates-daily = Искать новый выпуск раз в сутки
updates-privacy = Один запрос в сутки к GitHub за самым новым выпуском. В нём только версия этой сборки, ничего больше, но GitHub видит ваш адрес. Ничего не скачивается и не устанавливается, пока вы не попросите.
updates-check-now = Проверить сейчас
updates-this-is = Это { $version }, последняя проверка: { $when }
updates-looking = Поиск нового выпуска
updates-newest = Это самый новый выпуск.
updates-installing = Загрузка и проверка подписи
updates-installed = Установлено. Заработает при следующем запуске.
updates-restart-now = Перезапустить сейчас
updates-store-has-it = В Microsoft Store для этой копии есть этот выпуск.
updates-in-store = Обновить в Microsoft Store
updates-install = Установить и перезапустить
updates-skip = Пропустить эту версию
updates-release-page = Страница выпуска
updates-more-notes = И ещё больше на странице выпуска.

## panels/start.rs

start-title = Новая платформа
start-subtitle = Откройте модель или продолжите с того места, где остановились.
start-zone = Откройте модель или проект
start-drop-here = Перетащите модель сюда
start-drop-formats = STL, OBJ, 3MF или проект .encrust
start-open-model = Открыть модель
start-open-project = Открыть проект
start-recent = Недавние
start-test-prints = Пробные печати
start-files =
    { $count ->
        [one] { $count } файл
        [few] { $count } файла
       *[many] { $count } файлов
    }
start-models =
    { $count ->
        [one] { $count } модель
        [few] { $count } модели
       *[many] { $count } моделей
    }
start-nothing-recent = Пока ничего не открывалось. Открытые модели и проекты будут показаны здесь.
start-test-print = Пробная печать
start-gone = { $path } больше не существует
start-project = Проект

## panels/stage_notice.rs

notice-cutting-preview = Нарезка слоёв для предпросмотра
notice-start-print = Начать печать
notice-broken-one = { $name } повреждена
notice-broken-several =
    { $count ->
        [one] { $count } модель на этой платформе повреждена
        [few] { $count } модели на этой платформе повреждены
       *[many] { $count } моделей на этой платформе повреждены
    }
notice-repair = Починить
notice-repair-all = Починить все
notice-keep-as-is = Оставить как есть
notice-no-printer-resin = Принтера и смолы пока нет
notice-no-printer = Принтера пока нет
notice-no-resin = На этом принтере пока нет смолы
notice-set-up = Настроить
notice-put-away = Убрать

## project.rs

project-save-and-close = Сохранить и закрыть
project-save-and-new = Сохранить и начать новый проект
project-close-without-saving = Закрыть без сохранения
project-new-without-saving = Начать новый проект без сохранения
project-cannot-open = не удалось открыть проект { $path }
project-opened-note = Открыто: { $path }; { $note }
project-cannot-draw = не удалось нарисовать изображение проекта
project-cannot-write = не удалось записать проект { $path }
project-unsaved = На этой платформе есть изменения, которых нет в файле проекта.
project-keep-working = Продолжить работу

## project/state.rs

project-missing-printer = принтера { $printer }, который в нём указан, нет среди ваших, поэтому остаётся ваш
project-missing-resin = смолы { $resin }, которая в нём указана, нет среди ваших, поэтому остаётся ваша
project-missing-both = принтера { $printer } и смолы { $resin }, которые в нём указаны, нет среди ваших, поэтому остаются ваши

## panels/section.rs

view-model = Модель
view-side-by-side = Модель и слой рядом
view-mask = Маска слоя
section-pause = Пауза
section-no-layers = Нет слоёв
section-no-layers-yet = Слоёв пока нет
section-height-of = { $height } из { $top } мм
section-type-layer = Введите номер слоя для перехода
section-layer-field = Слой { $layer }, введите номер слоя для перехода
section-slider = Слой, всего { $count }
section-nothing-to-cut = На платформе пока нечего сечь
section-layers-range = Слои с { $first } по { $layer }

## panels/settings/confirm.rs

confirm-cannot-undo = Это нельзя отменить.
confirm-remove-machine = Удалить «{ $name }» и всё, что вы на нём изменили?
confirm-remove = Удалить
confirm-machine-stays = Принтер остаётся в библиотеке, поэтому его можно добавить снова со значениями, поставляемыми в этой сборке.
confirm-take-resin-off = Убрать эту смолу
confirm-delete-named = Удалить «{ $name }»?
confirm-untouched-resin = В неё ничего не вводилось, поэтому она не сохраняется в общем списке.
confirm-take-off-named = Убрать «{ $name }» с этого принтера?
confirm-take-off = Убрать
confirm-stays-in-pool = Она остаётся в общем списке, и этот или любой другой принтер может взять её обратно.
confirm-stays-measured = Она остаётся в общем списке, откалибрована на: { $printers }.
confirm-delete-resin = Удалить эту смолу
confirm-delete-resin-named = Удалить «{ $name }» и подобранные для неё экспозиции?
confirm-comes-off = Она также убирается с: { $printers }.
confirm-delete-profile = Удалить этот профиль

## panels/report.rs

report-nothing-sent = Encrust находится в альфа-версии, а отчёт — способ исправить ошибку. Отсюда ничего не отправляется: то, что вы видите ниже, остаётся на этом компьютере, пока вы сами не передадите это дальше.
report-no-model = Ваша модель в отчёт никогда не попадает. Прикрепите её к обращению сами, только если вы вправе ею делиться: небольшая форма, повторяющая ту же ошибку, лучше для всех.
report-what-happened = Что произошло и чего вы ожидали
report-steps = Как воспроизвести
report-title = Сообщить о проблеме
report-include-profiles = Профили принтера и смолы
report-include-settings = Настройки панелей инструментов
report-include-plate = Сколько стоит на платформе
report-the-report = Отчёт
report-open-issue = Создать обращение
report-copied-for-issue = Отчёт в буфере обмена: вставьте его в обращение
report-copy = Скопировать отчёт
report-copied = Отчёт в буфере обмена
report-save = Сохранить в файл
report-cannot-write = не удалось записать отчёт в { $path }
report-cannot-hand-over = не удалось передать отчёт

## sliced.rs

sliced-no-panel = в файле не указан размер экрана, а принтер с таким экраном неизвестен; сначала выберите принтер, для которого файл нарезан
sliced-no-layers = в { $name } нет слоёв
sliced-layer-unreachable = слой { $layer } из { $name } находится там, куда принтер не дотягивается
sliced-cannot-decode = не удалось декодировать слой { $layer } из { $name }
sliced-no-pixel = { $name } не засвечивает ни одного пикселя

## scene.rs

defect-open = Поверхность не замкнута: у { $count } рёбер нет граней с другой стороны
defect-degenerate = граней без площади: { $count }
defect-duplicate = граней, нарисованных дважды: { $count }
note-folded = рёбер, где грани сложены друг на друга, нарезается как есть: { $count }
repaired-faces-removed = Починено: удалено схлопнутых или удвоенных граней — { $count }
repaired-flipped = Починено: граней, повёрнутых правильной стороной, — { $count }
repaired-inverted = Починено: оболочек, вывернутых правильной стороной, — { $count }
scene-plate-name = Платформа { $index }
scene-copy-numbered = { $name } (копия { $count })
# The scope-* messages are what a tool works on, set into the tooltips of the Hollow,
# Supports and Relief buttons as their { $scope }.
scope-selected-one = выбранная модель
scope-selected = выбранные модели ({ $count })
scope-plate-one = единственная модель на платформе
scope-plate = все модели на платформе ({ $count })

## panels/scene_panel.rs

models-broken = повреждена
models-title = Модели
models-selected = Выбрано: { $count }
models-arrange = Расставить на платформе
models-duplicate = Дублировать моделей: { $count }
models-array = Массив
models-columns = Столбцы
models-rows = Строки
models-lay-out = Разложить
models-show = Показать { $name }
models-import-found = Что нашёл импорт

## samples.rs

sample-detail = Тест детализации
sample-detail-about = Шипы, уступы под тремя углами и нависания надписи, всё без поддержек.
sample-exposure = Проверка экспозиции
sample-exposure-about = Штифты и рёбра ломаются при недоэкспонировании, пазы и отверстия заплывают при переэкспонировании.
sample-surface = Качество поверхности
sample-surface-about = Купол и колонна показывают пиксельные ступеньки и сглаживание, пандус — слои.
sample-fit = Калибр посадки
sample-fit-about = Штифты от 0,3 мм меньше до 0,1 мм больше отверстия в 6 мм, самый короткий — самый маленький: какие войдут.

## job.rs

job-rasterising = Растеризация
job-thread-stopped = поток нарезки остановился, не закончив
job-cancelling = Отмена
job-progress = { $stage }: слой { $done } из { $total }
job-working = { $stage }...

## job/pipeline.rs

job-cannot-start-threads = не удалось запустить потоки нарезки
job-cannot-slice = не удалось нарезать модель
job-cannot-measure = не удалось измерить набор слоёв

## panels/inspector.rs

inspector-parallel = параллельно
inspector-about-select = Выберите модель, затем перемещайте, поворачивайте, масштабируйте, кладите и зеркальте её.
inspector-about-measure = Щёлкните по двум точкам или двум плоским граням модели.
inspector-about-hollow = Вырежьте полость, чтобы сэкономить смолу и облегчить отрыв.
inspector-about-drain = Дайте смоле вытечь из полой модели.
inspector-about-cut = Разделите модель плоскостью.
inspector-about-relief = Вдавите текстуру в поверхность модели, напишите на ней текст или объедините модели в одну.
inspector-about-supports = Поддержите то, что нависает над пустотой.
inspector-about-paint = Укажите поддержкам, где им стоять, а где нет.
inspector-about-shape = Форма одной поддержки, часть за частью.
inspector-about-check = Что не получится на принтере, с указанием слоя, на котором это происходит.
inspector-about-export = От платформы до файла и от файла до принтера.
inspector-about-print-settings = Как эта платформа нарезается и экспонируется.

## web/files.rs

web-cannot-open-dialog = не удалось открыть диалог выбора файла: { $error }
web-cannot-hand-over = не удалось передать { $name }: { $error }
web-cannot-read = не удалось прочитать { $name }: { $error }
web-nothing-to-open = среди выбранных файлов нечего открывать; выберите .{ $extensions }

## preview/picture.rs

preview-no-stack = нет нарезанного набора слоёв для предпросмотра
preview-no-plate = нет платформы для предпросмотра
preview-cannot-cut-under = не удалось нарезать слой под просматриваемым
preview-not-in-stack = слоя { $index } нет в наборе
preview-cannot-rasterise = не удалось растеризовать слой на z = { $z } мм

## panels/printer_card/controls.rs

controls-stop-question = Остановить печать? Её нельзя будет возобновить.
controls-stop = Остановить печать
controls-keep-printing = Продолжить печать
controls-film = Освобождение плёнки: подъём { $lifts } из { $max }
controls-copy = Копировать
controls-camera-off = Выключить камеру
controls-rtsp = Поток по протоколу RTSP: откройте адрес в проигрывателе, который его поддерживает.
controls-camera = Поток с камеры

## import.rs

import-opening = Открытие { $path }
import-and-more = { $first } и ещё { $count }
import-no-extension = у { $name } нет расширения файла
import-cannot-load = не удалось загрузить { $name }
import-no-vertices = в { $name } нет вершин

## repair.rs

repair-gone = { $name } больше нет на платформе
repair-kept = { $name } не удалось починить без новых повреждений, поэтому она осталась как есть
repair-nothing = в { $name } больше нечего чинить
repair-no-patch = у { $name } границ без подходящей заплатки: { $count }
repair-done = Починено { $name }: { $what }
repair-done-open = Починено { $name }: { $what }; границ, которые не удалось закрыть: { $count }
repair-closed-holes =
    { $count ->
        [one] закрыта { $count } дыра
        [few] закрыто { $count } дыры
       *[many] закрыто { $count } дыр
    }
repair-dropped-faces =
    { $count ->
        [one] отброшена { $count } дважды нарисованная грань
        [few] отброшено { $count } дважды нарисованные грани
       *[many] отброшено { $count } дважды нарисованных граней
    }

## panels/view_cube.rs

cube-top = Сверху
cube-bottom = Снизу
cube-front = Спереди
cube-back = Сзади
cube-right = Справа
cube-left = Слева
cube-home = Исходный вид

## panels/mask_pane.rs

mask-pick-machine = Выберите принтер, чтобы узнать, на каком экране рисовать слои.
mask-no-stack = Набора слоёв пока нет.
mask-zoomed = ×{ $zoom }   двойной щелчок — вписать
mask-how = прокрутка — масштаб, перетаскивание — сдвиг
mask-caption = Маска слоя   1 : { $scale }   { $hint }
mask-cannot-draw = не удалось нарисовать слой
mask-cannot-draw-full = не удалось нарисовать слой целиком

## network/errands.rs

errand-pausing = Постановка на паузу
errand-resuming = Возобновление
errand-stopping = Остановка
errand-camera-on = Включение камеры
errand-camera-off = Выключение камеры
errand-no-answer = поручение завершилось без ответа

## app.rs

app-cannot-build-preview = не удалось построить предпросмотр слоя

## job/hollow.rs

job-thread-stopped-hollowing = поток выдалбливания остановился, не закончив
job-hollow-stopping = Остановка...
job-hollow-progress = Выдалбливание модели { $index } из { $total }
job-step-preparing = Проверка модели
job-step-seeding = Замер поверхности
job-step-carrying = Построение поля
job-step-refining = Уточнение поля
job-step-surfacing = Построение поверхности
job-step-shaping = Завершение
job-step-reading = Чтение моделей
job-step-combining = Объединение
job-step-finishing = Сборка результата
job-hollow-preparing = Проверка модели
job-hollow-seeding = Замер поверхности
job-hollow-carrying = Проход через стенку
job-hollow-refining = Расчёт стенки
job-hollow-surfacing = Построение внутренней поверхности
job-hollow-shaping = Завершение оболочки

## job/import.rs

job-thread-stopped-import = поток импорта остановился, не закончив
import-stage-reading = Чтение
import-stage-repairing = Починка
import-stage-indexing = Индексация
import-stage = { $stage } { $name }

## job/measure.rs

job-thread-stopped-measuring = поток замера остановился, не закончив

## job/orient.rs

job-thread-stopped-orientation = поток ориентации остановился, не закончив
job-orient-none = поиск не нашёл ни одной ориентации

## job/place.rs

job-thread-stopped-support = поток поддержек остановился, не закончив
job-placing = Расстановка поддержек, { $percent } %
job-cutting-plate = Нарезка платформы...

## job/preview.rs

job-thread-stopped-preview = поток предпросмотра остановился, не закончив

## job/relief.rs

job-thread-stopped-relief = поток рельефа остановился, не закончив
job-relief-no-texture = у модели нет текстуры для вдавливания

## job/send.rs

job-thread-stopped-network = сетевой поток остановился, не закончив
job-send-starting = Запуск { $printer }
job-send-connecting = Подключение к { $printer }

## job/trapped.rs

job-thread-stopped-drainage = поток проверки дренажа остановился, не закончив

## job/repair.rs

job-thread-stopped-repair = поток починки { $name } остановился, не закончив
job-repairing = Починка { $name }

## arrange.rs

arrange-no-footprint = у модели на платформе нет основания для расстановки
arrange-nothing = На платформе нет видимых моделей для расстановки
arrange-done =
    { $count ->
        [one] Расставлена { $count } модель
        [few] Расставлено { $count } модели
       *[many] Расставлено { $count } моделей
    }
arrange-short =
    { $count ->
        [one] Платформа расставлена; для { $count } модели не нашлось места
        [few] Платформа расставлена; для { $count } моделей не нашлось места
       *[many] Платформа расставлена; для { $count } моделей не нашлось места
    }

## drain.rs

drain-nothing-visible = на платформе нет видимых моделей для проверки
drain-found =
    { $count ->
        [0] Запертой смолы нет: всё может стечь
        [one] { $count } карман с запертой смолой
        [few] { $count } кармана с запертой смолой
       *[many] { $count } карманов с запертой смолой
    }

## exchange.rs

exchange-cannot-import = не удалось импортировать { $path }
exchange-imported = Импортировано: { $path }
exchange-choose-printer-file = сначала выберите принтер, для которого нарезан этот файл
exchange-choose-printer-resin = сначала выберите принтер, для которого настроена эта смола

## panels/find.rs

find-nothing = Под таким названием ничего нет

## panels/inspector/scope.rs

scope-keeping =
    { $count ->
        [one] У { $count } модели свои собственные значения.
        [few] У { $count } моделей свои собственные значения.
       *[many] У { $count } моделей свои собственные значения.
    }

## panels/inspector/select.rs

select-selection = Выбор
select-on-plate = на платформе: { $count }
select-how = Щёлкните по модели, чтобы выбрать её. Cmd + щелчок добавляет модель к выбору, а Shift + щелчок выбирает всё между ними; затем любой инструмент работает с выбранным.

## panels/plate_strip.rs

plates-gather = Переместить сюда выбранное
plates-remove = Удалить вместе с содержимым

## panels/printer_card/history.rs

history-none = Печатей в памяти нет.
history-read-again = Прочитать снова
history-at-layer-of = на слое { $layer } из { $total }
history-at-layer = на слое { $layer }
history-finished = Завершено { $when }
history-stopped = Остановлено { $at }, { $when }
history-failed = Сбой { $at }, { $when }

## panels/printer_card/ring.rs

ring-printers = Принтеры в сети
ring-open = Открыть этот принтер

## panels/printer_card/sheet.rs

sheet-recent-prints = Последние печати
sheet-firmware = прошивка { $version }

## panels/settings/machines/library.rs

library-all-brands = Все производители
library-custom = Свой принтер
library-search = Искать среди всех принтеров
library-machines =
    { $count ->
        [one] { $count } принтер
        [few] { $count } принтера
       *[many] { $count } принтеров
    }

## panels/stage.rs

stage-empty = На платформе нет модели
stage-empty-hint = Откройте модель или проект либо перетащите их в окно.
stage-open = Открыть...

## plate.rs

plate-select-printer = Выберите принтер

## preview/job.rs

preview-nothing-visible = на платформе нет видимых моделей для предпросмотра
preview-cannot-cut = не удалось нарезать слои для предпросмотра

## relief.rs

relief-no-pixels = в изображении нет пикселей

## web/opfs.rs

web-no-storage = в этом браузере нет закрытого файлового хранилища

## profiles.rs

picker-no-machine = Принтеров пока нет. Добавьте принтер в окне «Принтер и смола».
picker-no-resin = Для этого принтера пока нет смол. Добавьте их в окне «Принтер и смола».
picker-edit-resin = Изменить эту смолу...
