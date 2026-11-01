### The window's text in Russian. A message missing here shows in English, and
### `cargo xtask i18n ru` lists every such one; how to translate: docs/design/i18n.md.

## Shared by many panels

common-already-done = Уже сделано
common-cancel = Отмена
common-export = Экспорт
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
unit-deg = °
unit-g = г
unit-ml = мл
unit-g-ml = г/мл
unit-h = ч
unit-m = мин
unit-mm = мм
unit-mm2 = мм²
unit-mm3 = мм³
unit-mm-min = мм/мин
unit-um = мкм
unit-px = пикс.
unit-s = с
unit-times = ×
value-deg = { $value }°
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
general-start-page-hint = Если выключено, окно открывается сразу на платформе.
general-appearance = Тема
general-appearance-system = Системное
general-appearance-light = Светлое
general-appearance-dark = Тёмное
general-appearance-light-sub = Сланец и белый
general-appearance-dark-sub = Графит
general-appearance-now-light = Сейчас светлая
general-appearance-now-dark = Сейчас тёмная
general-appearance-hint = Системная следует за рабочим столом, когда он меняется, вместе с областью просмотра.
settings-page-appearance = Оформление
settings-group-help = Помощь
settings-find = Найти настройку
settings-words-appearance = тема светлая тёмная системная язык анимация движение
settings-words-general = начальная страница область просмотра масштаб нарезанные файлы папка профили
settings-words-updates = обновление выпуск версия
updates-this-build = Это { $version }
general-viewport = Область просмотра
general-invert-zoom = Прокрутка вниз приближает
general-motion = Анимация
general-reduce-motion = Уменьшить движение
general-sliced-files = Нарезанные файлы
general-sliced-dir-system = Окно сохранения открывается здесь; если папку забыть, там, где система оставила его в прошлый раз.
general-sliced-dir-none = Не выбрана
general-choose-folder = Выбрать
general-forget-folder = Забыть папку
general-profiles = Профили
general-profiles-nowhere = В этой системе негде хранить ваши профили.
general-profiles-dir = Здесь хранятся ваши принтеры, смолы и профили.
general-open-folder = Открыть папку

## panels/inspector/cut.rs

cut-plane = Плоскость
cut-fact = { $axis } { $height } мм
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
cut-split = Разделить

## cut.rs

cut-nothing-selected = для разреза ничего не выбрано
cut-no-direction = у плоскости разреза нет направления
cut-misses = плоскость не пересекает { $name }, резать нечего
cut-undrilled = отверстия в { $name } не вырезаются точно, и половины потеряли бы их
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

cut-keep-both = Оставить обе
cut-keep-lower = Нижняя
cut-keep-upper = Верхняя

## workspace.rs

tool-select = Выбор
tool-measure = Замер
tool-repair = Ремонт
tool-hollow = Полость
tool-drain = Дренаж
tool-cut = Разрез
tool-relief = Рельеф
tool-supports = Поддержки
tool-paint = Разметка
tool-shape = Форма
tool-check = Проверка
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
shortcut-group-tools = Инструменты
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
shortcuts-find = Найти клавишу
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
settings-support-profile = Профиль поддержек
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
supports-placed-sized = Поставлено поддержек: { $count }, из них лёгких { $light }, средних { $medium }, тяжёлых { $heavy } и стабилизирующих { $steady }
supports-auto-no-shape = { $name } строит опоры по профилю { $id }, которого нет в каталоге
supports-group-name = Группа { $index }
supports-cannot-grow = Не удалось построить поддержки: { $reason }
supports-growth-stopped = Построение поддержек прервалось без ответа
surfaces-cannot-build = Не удалось подготовить поверхность для поддержек: { $reason }
surfaces-build-stopped = Подготовка поверхности для поддержек прервалась без ответа

## measure.rs

measure-snapped-corner = на углу
measure-snapped-edge = на ребре
measure-snapped-free = свободно

## panels/inspector/supports.rs

supports-grow-inside = Поддержать полость
supports-grow-inside-tooltip = Убирает поддержки внутри полостей полых моделей и выращивает их заново по профилю выше. «Вырастить поддержки» держит модель только снаружи.
supports-remove-inside = Удалить поддержки полости
supports-remove-inside-tooltip =
    { $count ->
        [one] Убирает { $count } поддержку внутри полостей полых моделей, в том числе исправленную вручную.
        [few] Убирает { $count } поддержки внутри полостей полых моделей, в том числе исправленные вручную.
       *[many] Убирает { $count } поддержек внутри полостей полых моделей, в том числе исправленные вручную.
    }
supports-removed-inside =
    { $count ->
        [one] Из полостей убрана { $count } поддержка
        [few] Из полостей убраны { $count } поддержки
       *[many] Из полостей убрано { $count } поддержек
    }
supports-where = Где
supports-picked = Выбрано
supports-picked-help = «Вырастить заново» возвращает выбранные поддержки автоматике, и следующее выращивание строит их заново. «В группу» строит их по профилю текущей группы.
supports-stat-parts = Части
supports-stat-supports = Поддержки
supports-move-to-group = В группу
supports-grow-these-again = Вырастить заново
supports-remove-these = Удалить эти поддержки
supports-grow-again = Вырастить заново
supports-grow = Вырастить поддержки
supports-grow-tooltip = Сечёт { $scope }, чтобы найти то, что нависает над пустотой.
supports-placing = Расстановка поддержек
supports-sizes = Размеры
supports-place = Расстановка
support-size-island = Остров для { $shape }
support-size-load = Нагрузка для { $shape }
support-size-step = Уступ для { $shape }
support-group-steps = Уступы
support-group-steadying = Устойчивость
support-steps-help = Уступ — полка, выдвинутая за один слой; его размер — объём смолы, который он добавляет. Грани, поднимающиеся от края уступа, держатся вместе с ним до его досягаемости, если отклоняются от вертикали больше наклона у уступа.
support-step-height = Досягаемость для { $shape }
support-step-lean = Наклон у уступа
support-steadying-help = Деталь, поднимающуюся на гранях слишком пологих, чтобы провиснуть, держат несколько тонких поддержек, чтобы отрыв её не раскачивал. Наклон — насколько грань отклоняется от вертикали, прежде чем её подпирают; шаг — расстояние между такими поддержками; наименьший склон — какой должна быть грань, чтобы получить хоть одну.
support-steady-lean = Наклон
support-steady-spacing = Шаг
support-steady-area = Наименьший склон
supports-remove-all-tooltip = Убрать все поддержки
supports-on-plate = На платформе
supports-profiles = Профили
supports-edit-profiles = Изменить профили
supports-mode-pick = Выбрать поддержки
supports-mode-place = Ставить поддержки
supports-mode-area = Закрасить или запретить область
supports-mode-shape = Форма поддержек
supports-without-room =
    { $count ->
        [one] { $count } поддержке негде встать
        [few] { $count } поддержкам негде встать
       *[many] { $count } поддержкам негде встать
    }
supports-drop-preset =
    { $count ->
        [one] Убрать «{ $name }» и его { $count } поддержку
        [few] Убрать «{ $name }» и его { $count } поддержки
       *[many] Убрать «{ $name }» и его { $count } поддержек
    }
supports-clear-preset =
    { $count ->
        [one] Убрать { $count } поддержку
        [few] Убрать { $count } поддержки
       *[many] Убрать { $count } поддержек
    }
bin-click-again = Нажмите ещё раз, чтобы удалить

## panels/settings.rs

settings-title = Настройки
settings-group-profiles = Профили инструментов
settings-group-window = Приложение
settings-shortcuts = Сочетания клавиш
settings-import = Импорт
settings-cannot-export = не удалось экспортировать профиль
settings-cannot-delete = не удалось удалить профиль
settings-cannot-reset = не удалось сбросить нечитаемые профили
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
machines-maker-format = { $maker } · .{ $format }
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
check-support-islands = Поддержать
check-drill-cups = Просверлить
check-issues = Найдено
check-previous = Предыдущая проблема
check-next = Следующая проблема
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
check-layer = Слой { $layer }
check-layer-lower = слой { $layer }
check-peel = Отрыв
check-nothing-found = Ничего, что привело бы к браку
check-nothing-found-why = Нет островов, нет перемычек, перегруженных сверх предела, и нет слоёв, которые слишком сильно тянут плёнку.
check-issues-help = Острова начинаются в воздухе и остаются на плёнке. Рычаги — тонкие перемычки, которые может сломать тяга вбок; присоски — закрытые полости, которые подъём тянет вакуумом; тонкие участки могут вовсе не напечататься. Щелчок по строке переходит к её слою.
check-peel-help = Каждый подъём отрывает свежий слой от плёнки, и чем шире слой, тем сильнее тяга. «Сильнейшая тяга» — худший слой, показанный как диск с той же силой отрыва; «Самый большой скачок» — где площадь сильнее всего растёт от слоя к слою. Наклон модели или полость уменьшают и то и другое.
check-put-islands-back = Вернуть острова
check-remove-islands = Удалить
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
check-removed =
    { $count ->
        [one] { $count } остров удалён из файла
        [few] { $count } острова удалены из файла
       *[many] { $count } островов удалено из файла
    }

## panels/plate_summary.rs

summary-layer = Слой
summary-layer-range = от { $fewest } до { $most }
summary-layers = Слои
summary-vertices = Вершины
summary-triangles = Треугольники
summary-price = Стоимость
summary-exposure = Экспозиция
summary-cured-area = Площадь засветки
summary-volume = Объём
summary-weight = Масса
summary-time = Время
summary-resin = Смола
summary-block = Блок
summary-details = Подробнее
summary-model = Модель
summary-supports = Поддержки
summary-broken =
    { $count ->
        [one] { $count } модель сломана
        [few] { $count } модели сломаны
       *[many] { $count } моделей сломано
    }
summary-broken-why =
    { $count ->
        [one] То, что из неё нарезано, может не напечататься.
       *[other] То, что из них нарезано, может не напечататься.
    }
summary-estimate = ≈ { $value }
summary-block-bottom = Нижние слои
summary-block-transition = Переход
summary-block-body = Основная часть

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
ago-weeks =
    { $count ->
        [1] на прошлой неделе
        [one] { $count } неделю назад
        [few] { $count } недели назад
       *[many] { $count } недель назад
    }

## ui/widgets/field.rs

field-back-to = Вернуть { $value } { $unit }
field-printer-address = Адрес принтера

## ui/widgets/layout.rs

dialog-close = Закрыть  Esc

## ui/widgets/list.rs

list-hide = Скрыть в окне просмотра
list-show = Показать в окне просмотра

## panels/inspector/measure.rs

measure-reading = Показания
measure-caught = Привязка
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
hollow-bottom-through = Открытое дно
hollow-fact = Стенка { $value } мм
hollow-nothing-done = Пока ничего не выдолблено
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
print-use-profiles = Взять настройки профилей
print-exposure-help = Сколько экран засвечивает каждый слой. Мало — тонкие части не печатаются или слои расслаиваются; много — детали разбухают, а отверстия зарастают. Нижние слои засвечиваются дольше, чтобы печать держалась на платформе, а слои перехода плавно снижают время между ними.
print-waiting-help = Паузы, чтобы смола успокоилась. «Свет выключен» — одна пауза перед засветкой каждого слоя; время отдыха — паузы после засветки, наверху подъёма и после возврата платформы. Длинные паузы помогают густой смоле и большим слоям, но удлиняют печать.
print-motion-help = После каждого слоя платформа поднимается, отрывая его от плёнки, и возвращается. Медленный подъём отрывает мягче — это нужно большим и хрупким слоям, а высоты подъёма должно хватать, чтобы плёнка отпустила слой.
print-motion-waits = Движение и паузы
print-motion = Движение
print-carried = Перенесено с { $exposure } с при { $from } мкм на слой { $to } мкм.
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
text-find-family = Найти шрифт
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
text-click-note = Щёлкните по модели, чтобы поставить текст.
text-stamping = Ставлю надпись
text-cancelled = Надпись отменена
text-nothing-under = здесь нет модели, на которой писать
text-done = Надпись на { $name } готова
text-bad-font = этот шрифт не читается: { $reason }
text-missing-glyph = в этом шрифте нет буквы { $character }
text-blank = писать нечего
text-bad-size = буквам нужны высота и глубина
text-off-the-model = здесь надпись не встаёт на модель
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
chip-layers = { $layer } мкм · { $exposure } с
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
bar-update-percent = Обновление · { $percent } %
update-sheet-title = Обновление
update-sheet-from-to = { $from } → { $to }
update-group-new = Новое
update-released = вышел { $when }
update-size = { $mb } МБ
update-later = Позже
update-downloading-of = Загрузка · { $got } из { $total } МБ
update-downloading = Загрузка · { $got } МБ
update-time-left = осталось около { $seconds } с
update-step-download = Загрузка
update-step-verify = Проверка подписи
update-step-install = Установка
update-cancel = Отмена
notice-release-out = Вышел Encrust { $version }
notice-release-and-more = { $first } и ещё { $count }
notice-what-is-new = Что нового
notice-release-later = Позже: значок останется в верхней панели
updates-release-found = Найден выпуск
updates-looking-block = Проверка
updates-every-change = Все изменения
updates-and-more = И ещё { $count }.
updates-last-looked = Последняя проверка { $when }
bar-alpha = Альфа
bar-alpha-tooltip = Encrust в альфа-версии: расскажите, что пошло не так
menu-new-plate = Новая платформа
menu-new-project = Новый проект
menu-open-project = Открыть проект...
menu-open-recent = Открыть недавние
menu-save-project = Сохранить
menu-save-project-as = Сохранить как...
menu-open-model = Добавить модель...
menu-open-test-print = Пробные печати
menu-import-machine = Профиль принтера...
menu-import-resin = Профиль смолы...
menu-machine-and-resin = Принтер и смола...
menu-settings = Настройки...
menu-quit = Выйти из Encrust
menu-nothing-recent = Пока ничего не открывалось
menu-arrange-plate = Расставить на платформе
menu-auto-orient-plate = Автоориентация платформы
menu-move-to-plate = Переместить на платформу
menu-frame-view = Показать всё
menu-plate-grid = Сетка платформы
menu-xray = Просвечивать модели
menu-orthographic = Ортогональный вид
menu-import = Импорт
menu-clear-recent = Очистить список
menu-slicing-already = Платформа уже нарезается
menu-pick-first = Сначала выберите модель
menu-plate-empty = Платформа пуста
menu-camera = Камера
menu-camera-perspective = Перспектива
menu-camera-orthographic = Ортогональная
menu-show = Показывать

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
support-longest-strut = Самая длинная ветка
support-tips-per-trunk = Кончиков на ствол
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
support-group-grid = Сетка
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
support-no-grid = Стволы стоят там, где сходятся кончики.
support-tip-help = Единственная часть, которая касается модели. Широкий контакт держит крепче, но оставляет след больше; врезка утапливает кончик в поверхность, чтобы он не оторвался во время печати. Длина — конус от кончика до колонны, а сфера держит тяжёлое нависание крепче плоскости.
support-body-help = Диаметр — толщина колонны: толще значит жёстче, но смолы уходит больше. Зазор — воздух между колонной и моделью, чтобы колонна не приросла к детали. Шапка ствола — наибольшая ширина ствола у основания.
support-foot-help = Основание держит поддержку на платформе. Шире и толще — держит крепче, но труднее снимается; у конусной и полоза есть край, под который заходит шпатель. Раструб — насколько колонна расширяется перед основанием.
support-branching-help = Соседние кончики сходятся в общие стволы, как дерево: меньше колонн, меньше смолы и следов на платформе. Больший угол ветвления объединяет больше, но ветви наклоняются сильнее; кончики дальше, чем «Объединять в пределах», стоят на своих колоннах.
support-grid-help = Каждый ствол стоит в узле квадратной сетки на платформе: кончик тянется к ближайшему узлу под 45°, кончики одного узла сходятся в один ствол. Лес получается ровным, равномерно связывается и срезается рядами. Кончик, которому не нашлось узла, стоит как без сетки.
support-grid-spacing = Шаг
support-raft-help = Плита, на которой поддержки стоят вместо платформы: она распределяет отрыв по одной площади и снимается одним куском. Решётка меньше присасывается к платформе, «Сплошная» соединяет площадки под каждым основанием. Отступ — насколько подложка выходит за модель, а наклонная стенка держит крепче и всё равно легко поддевается.
support-bracing-help = Распорки связывают высокие стволы, чтобы те не раскачивались при отрыве слоя. Пролёт — наибольшее расстояние между стволами, которое они соединяют; «Первая на» и «Каждые» задают высоту первой распорки и шаг между ними. По высоте каждой пары верхняя и нижняя зоны берут по нескольку уровней, а середина заполняет остальное: зигзаг — одна распорка на уровень, крест — две. Высокий ствол, свободный наверху, получает подпорки: пару, если он выше своей верхней распорки на «Пара выше», и одну, если выше второй на «Одна выше».
support-held-up-help = Нависание — насколько поверхность может отклониться от вертикали, прежде чем получит поддержки: 90° — потолок, меньшее число ставит больше поддержек. Шаг контактов — расстояние между кончиками под гранью, смотрящей прямо вниз, а шаг на крутых гранях — под гранью, наклонённой чуть дальше угла нависания; между ними шаг меняется вместе с углом. Плотность умножает их количество: 1 — столько, сколько Encrust считает достаточным.
support-lift-help = На какой высоте модель стоит над платформой. Поддержкам нужно место под ней, а первые слои прилипают к платформе сильнее всего, поэтому деталь, напечатанную прямо на ней, трудно снять.
support-raft = Подложка под опорами
support-raft-skates = Полозья
support-raft-grid = Решётка
support-raft-connected = Сплошная
support-raft-hull = Контур
support-raft-rectangle = Прямоугольник
support-bracing = Скреплять высокие стволы
support-bracing-fine-tune = Тонкая настройка: все числа вместо пресетов
support-bracing-preset-none = Эти числа — ваши собственные. Выберите пресет, чтобы начать с него заново.
support-bracing-standard = Обычный
support-bracing-strong = Прочный
support-bracing-tall = Высокий
support-bracing-small = Мелкий
support-bracing-standard-help = Зигзаг по всей высоте и один крест наверху. Хватает большинству деталей.
support-bracing-strong-help = Кресты по всей высоте, от самого основания. Для тяжёлых деталей.
support-bracing-tall-help = Зигзаг между двумя крестами наверху и одним внизу, распорки толстые, насколько позволяют стволы. Для высоких деталей.
support-bracing-small-help = Кресты по всей высоте, распорки тонкие. Для мелких деталей.
support-brace-top = Верх
support-brace-top-levels = Уровней сверху
support-brace-middle = Середина
support-brace-bottom = Низ
support-brace-bottom-levels = Уровней снизу
support-brace-zigzag = Зигзаг
support-brace-cross = Крест
support-buttress = Подпирать высокие стволы
support-buttress-pair-above = Пара выше
support-buttress-one-above = Одна выше

## panels/settings/supports.rs

support-table-auto = Автоматический подбор
support-table-manual = Вручную
support-table-tip = Наконечник
support-table-spacing = Шаг
support-cannot-add = не удалось добавить профиль поддержек
support-pick-profile = Выберите профиль поддержек.
support-reset = Сбросить к стандартным
support-reset-shipped-help = «Сбросить к стандартным» возвращает все числа, с которыми профиль поставляется, и сохраняет его имя; под поставочным именем он после этого обновляется вместе с программой. Число, выделенное янтарным, отличается от поставляемого; стрелка рядом возвращает только его. Сброс можно отменить.
support-reset-auto-help = «Сбросить к стандартным» возвращает все числа, с которыми поставляется автоматический профиль; после этого он обновляется вместе с программой. Число, выделенное янтарным, отличается от поставляемого; стрелка рядом возвращает только его. Сброс можно отменить.
support-reset-stock-help = «Сбросить к стандартным» возвращает числа, с которых начинается новый профиль, и сохраняет его имя. Сброс можно отменить.
support-cannot-save = не удалось сохранить профиль поддержек

## panels/inspector/shape.rs

shape-part = Часть
shape-regular = Обычная
shape-raft-own = Подложка принадлежит этой модели, она пришла с её проектом.
shape-raft-plate = Подложка принадлежит платформе: на ней стоят все модели.
shape-use-plate-raft = Использовать подложку платформы
shape-use-profile-raft = Использовать подложку из «{ $name }»
shape-save = Сохранить
shape-save-tooltip = Сохранить поверх профиля, с которого взяты эти значения
shape-save-as-new = Сохранить как новый
shape-saved = Профиль поддержек сохранён

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
diagram-brace-bottom = низ
diagram-brace-middle = середина
diagram-brace-top = верх
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
presets-open-tool =
    { $kind ->
        [hollow] Открыть инструмент «Полость»
       *[drain] Открыть инструмент «Дренаж»
    }
presets-none = Нет
presets-cannot-rename = не удалось переименовать профиль

## panels/inspector/presets.rs

presets-unsaved = Не сохранено
presets-edited = · изменено
presets-new-name = Мои настройки
presets-none-saved = Пока ничего не сохранено
presets-delete-named = Удалить «{ $name }»
presets-deleted =
    { $kind ->
        [hollow] Профиль полости удалён
       *[drain] Профиль дренажа удалён
    }

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
resin-bottom-retract = Возврат нижних слоёв
resin-second-stage = Вторая ступень
resin-buffer = Буфер
resin-buffer-help = Медленный участок хода, который прошивка принтера выполняет сама: на этой высоте у ванны стол идёт с этой скоростью, затем ждёт перед засветкой слоя. Пишется в файл; принтер начинает с чисел производителя.
resin-buffer-on = Использовать буфер
resin-buffer-height = Высота буфера
resin-buffer-speed = Скорость буфера
resin-buffer-light-off = Ожидание после буфера
resin-bottom-light-off = Свет выключен на нижних слоях
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
limit-takes-between = Эта машина принимает { $low }–{ $high } { $unit }.
limit-takes-from = Эта машина принимает от { $low } { $unit }.
limit-takes-up-to = Эта машина принимает не больше { $high } { $unit }.
limit-advised-between = Производитель советует { $low }–{ $high } { $unit }.
limit-advised-from = Производитель советует от { $low } { $unit }.
limit-advised-up-to = Производитель советует не больше { $high } { $unit }.
limit-why-thin-layer = Чем тоньше слой, тем больше слоёв и дольше печать.
limit-why-few-bottom-layers = С меньшим числом нижних слоёв модель может оторваться от платформы.

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
layers-levels-blur =
    { $count ->
        [one] { $count } уровень, размытие { $blur } пикс.
        [few] { $count } уровня, размытие { $blur } пикс.
       *[many] { $count } уровней, размытие { $blur } пикс.
    }
layers-all-greys = Все оттенки
layers-all-greys-blur = Все оттенки, размытие { $blur } пикс.
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
layers-round-grey = Ограничить оттенки серого
layers-grey-levels = Оттенки серого
layers-edge-blur = Размытие края
layers-blur-note = сильное размытие затемняет и стенку внутри края; обычно хватает одного-двух пикселей
layers-thickest-reached = Самый толстый достигнутый слой — { $thickest } мкм, а не { $asked } мкм, поэтому обычный слой экспонируется { $exposure } с, а не { $resin } с.
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
printer-grey-min = Самый тусклый серый края
printer-grey-max = Самый яркий серый края
printer-grey-range-full = Серый края записывается как есть.
printer-grey-range-spread = Серый края поднимается до самого тусклого и растягивается до самого яркого; чёрный и сплошной белый не меняются.
printer-pitch = { $x } × { $y } мкм на пиксель
printer-mirror = Зеркало
printer-mirror-normal = Обычное
printer-mirror-lcd = Зеркало LCD
printer-mirror-dlp = Обычное DLP
printer-mirror-note = Большинство LCD-машин получают маски, отражённые по X; какое у машины — указано в настройках производителя.
printer-inset = Отступ от краёв стола
printer-inset-min-x = Отступ у −X
printer-inset-max-x = Отступ у +X
printer-inset-min-y = Отступ у −Y
printer-inset-max-y = Отступ у +Y
printer-inset-note = Модель в полосе отступа помечается, но нарезается целиком.
printer-per-layer-on = Экспозиция и движение читаются из каждого слоя; в .goo это называют расширенным режимом.
printer-per-layer-off = Каждый слой печатается по значениям из заголовка, а нижние слои принтер плавно проходит сам.
printer-variable-height-note = Если выключено, платформа двигается на высоту из заголовка, поэтому адаптивный набор слоёв отклоняется.
printer-output-note = Формат, в который нарезается для этого принтера, и для диалога, и для самого принтера.

## network.rs

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
connection-protocol = Протокол
connection-printers = Принтеры
connection-found = Найдены рядом
found-add = Добавить
connection-scanning = Сканирование...
connection-scan-again = Сканировать снова
connection-add = Добавить
connection-address-hint = Добавить по адресу, 192.168.1.42
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
card-destination-several = Принтер, на который отправляет этот профиль; нажмите, чтобы увидеть остальные в списке и искать снова
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
slice-send-tooltip = Растеризовать платформу и отправить файл на принтер
slice-all = Нарезать все платформы ({ $count }) в .{ $extension }
slice-send = Нарезать и отправить
slice-start-tooltip = Начать печать файла, загруженного на { $name }
slice-start-print = Начать печать
slice-saved-instead = { $why }; файл сохраняется на диск.
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
slicing-supports-growing = Поддержки ещё строятся.
slicing-steps-by-header = Этот принтер двигает платформу на высоту слоя из заголовка.
slicing-no-printer = профиль принтера не загружен
slicing-no-resin = смола не выбрана, а экспозиция подбирается, а не поставляется
slicing-nothing-visible-error = на платформе нет видимых моделей для нарезки
slicing-supports-growing-error = поддержки ещё строятся
slicing-no-format = в { $path } не указан формат нарезанного файла
slicing-clipped =
    { $count ->
        [one] , { $count } слой обрезан по краю экрана
        [few] , { $count } слоя обрезано по краю экрана
       *[many] , { $count } слоёв обрезано по краю экрана
    }
slicing-wrote = Записан { $path }: слоёв — { $layers }, { $volume } мл, { $weight } г{ $cost }{ $clipped }
slicing-cancelled = Нарезка отменена

## panels/destination.rs

slice-speaks-other = { $name } работает по { $theirs }, а этот профиль отправляет по { $ours }: настройте сеть в настройках принтера.

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
position-rotation = Повернуть
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
position-scale-to-fit = Вписать в объём
position-lift = Поднять
position-click-face = Щёлкните по грани
position-lay-on-face = Положить на грань
position-auto-orient = Автоориентация
orient-looking = Оценка вариантов положения
orient-proving = Пробная печать лучших вариантов
orient-cancelled = Автоориентация отменена
orient-preset-supports = Опоры
orient-preset-peel = Отрыв
orient-preset-shortest = Время
orient-preset-auto = Авто
orient-preset-supports-hint = Меньше всего смолы в поддержках, в каком бы положении ни стояла модель.
orient-preset-peel-hint = Самая слабая тяга на плёнку, без присосок; при усилии меньше 10 Н — меньше всего поддержек.
orient-preset-shortest-hint = Наименьшее число слоёв, при котором модель ещё печатается.
orient-preset-auto-hint = Баланс поддержек, тяги и времени печати: ни в чём не приходится сильно уступать.
position-mirror-across = Отразить по { $axis }
position-mirror-copy = Отражать копию
position-place-help = «Положить на грань» кладёт на платформу грань, по которой вы щёлкнете; «По центру» ставит модель над серединой платформы; «Отразить» переворачивает выбранные модели или их копии, поставленные рядом.

## panels/inspector/paint.rs

paint-brush = Кисть
paint-surface-angle = Угол поверхности
paint-inside = Внутри
paint-hint-point = Щелчок — поставить, Shift + щелчок — линия от последней, Alt + щелчок — убрать.
paint-hint-paint = Перетаскивайте, чтобы закрасить; Cmd + Shift + щелчок по поверхности; Alt — стереть.
paint-hint-block = Перетаскивайте, чтобы запретить; Cmd + Shift + щелчок по поверхности; Alt — стереть.
paint-hint-erase = Перетаскивайте, чтобы стереть разметку и запреты; Cmd + Shift + щелчок по поверхности.
paint-fill-help = Как держится залитая область: «Кайма» — по её краю, «Внутри» — по остальной площади. «Отступ» — насколько глубже начинается каждая часть: кайма от края, середина от каймы. Вторая кайма встаёт внутри первой — для больших плоских областей. Середина укладывается сеткой, кольцами вдоль каймы или вразброс. Каждую часть можно строить своим профилем или текущей группой. Меньше шаг — гуще и надёжнее, но следов больше.
paint-rim = Кайма
paint-line-spacing = Шаг линии
paint-offset = Отступ
paint-second-rim = Вторая кайма
paint-second-rim-gap = Внутрь от первой
paint-pattern-grid = Сетка
paint-pattern-rings = Кольца
paint-pattern-random = Вразброс
paint-profile = Профиль
paint-profile-in-hand = Текущая группа
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
paint-dropped =
    { $count ->
        [one] Удалена «{ $name }» и { $count } поддержка
        [few] Удалена «{ $name }» и { $count } поддержки
       *[many] Удалена «{ $name }» и { $count } поддержек
    }

## panels/inspector/drain.rs

drain-hole-size = Размер отверстия
drain-trapped-resin = Запертая смола
drain-opening = Отверстие
drain-opening-note = Щёлкните по модели, её поддержкам или подложке, чтобы поставить отверстие, Alt + щелчок по отверстию убирает его. Каждое вырезается сразу при установке, полая модель или нет, а щелчок по дну отверстия сверлит дальше сквозь него.
drain-fact = заперто: { $count }
drain-fact-unchecked = Не проверено
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
drain-cancelled = Проверка дренажа отменена
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
drain-rehollowing = Повторное выдалбливание вокруг каналов и отверстий со стенкой
drain-nothing-trapped = Ничего не заперто: смола может вытечь из каждой модели на платформе.
drain-not-checked = Ещё не проверялось
drain-not-checked-why = Проверка находит полости, из которых смола не вытечет.
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
update-error-cancelled = загрузка отменена
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

updates-by-store = Microsoft Store поддерживает эту копию в актуальном состоянии.
updates-store-link = Encrust в Microsoft Store
updates-daily = Искать новый выпуск раз в сутки
updates-privacy = Один запрос в сутки к GitHub за самым новым выпуском. В нём только версия этой сборки, ничего больше, но GitHub видит ваш адрес. Ничего не скачивается и не устанавливается, пока вы не попросите.
updates-check-now = Проверить сейчас
updates-looking = Поиск нового выпуска
updates-newest = Это самый новый выпуск.
updates-installing = Загрузка и проверка подписи
updates-restart-now = Перезапустить сейчас
updates-in-store = Обновить в Microsoft Store
updates-install = Загрузить и установить
updates-skip = Пропустить эту версию
updates-release-page = Страница выпуска

## panels/start.rs

start-title = Новая платформа
start-setup = { $machine } · { $resin } · { $layer } мкм
start-zone = Откройте модель или проект
start-drop-here = Перетащите модель или проект
start-drop-formats = STL, OBJ, 3MF, нарезанный файл или проект .encrust
start-open-model = Открыть модель
start-open-project = Открыть проект
start-recent = Недавние
start-test-prints = Пробные печати
start-clear-recent = Очистить список
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

## panels/unreadable_profiles.rs

notice-unreadable =
    { $count ->
        [one] { $count } ваш профиль не читается и пропущен
        [few] { $count } ваших профиля не читаются и пропущены
       *[many] { $count } ваших профилей не читаются и пропущены
    }
notice-unreadable-file = { $path }: { $reason }
notice-reset-profiles = Сбросить на стандартные
notice-delete-profiles = Удалить их

## panels/stage_notice.rs

notice-cutting-preview = Нарезка слоёв для предпросмотра
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
notice-no-resin-why = Для нарезки нужны времена засветки.
notice-no-printer-why = Для нарезки нужны экран и объём машины.
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
section-of = из { $count }
section-no-layers = Нет слоёв
section-no-layers-yet = Слоёв пока нет
section-height-of = { $height } из { $top } мм
section-type-layer = Введите номер слоя для перехода
section-layer-field = Слой { $layer }, введите номер слоя для перехода
section-go-to-issue = { $issue } на слое { $layer }: нажмите, чтобы перейти
section-slider = Слой, всего { $count }
section-nothing-to-cut = На платформе пока нечего сечь
section-layers-range = Слои с { $first } по { $layer }

## panels/settings/confirm.rs

confirm-cannot-undo = Это нельзя отменить.
confirm-click-again = Нажмите ещё раз, чтобы продолжить
confirm-asked-delete = Удалить?
confirm-asked-take-off = Убрать?
confirm-asked-stop = Остановить?
confirm-machine-stays = Принтер остаётся в библиотеке, поэтому его можно добавить снова со значениями, поставляемыми в этой сборке.
confirm-untouched-resin = В неё ничего не вводилось, поэтому она не сохраняется в общем списке.
confirm-stays-in-pool = Она остаётся в общем списке, и этот или любой другой принтер может взять её обратно.
confirm-stays-measured = Она остаётся в общем списке, откалибрована на: { $printers }.
confirm-comes-off = Она также убирается с: { $printers }.
confirm-reset-unreadable = Сбросить нечитаемые профили
confirm-delete-unreadable = Удалить нечитаемые профили
confirm-reset-named = Удалить ваши копии профилей { $names } и использовать те, что поставляются с этой версией?
confirm-reset = Сбросить
confirm-reset-own-gone = Ваши собственные нечитаемые профили тоже будут удалены, потому что заменить их нечем: { $names }.
confirm-delete-own-named = Удалить { $names }? Это ваши собственные профили, они не читаются, и заменить их нечем.
confirm-delete = Удалить

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

models-broken = Повреждена
models-title = Модели
models-selected = Выбрано: { $count }
models-picked = выбрано: { $count }
models-project = · { $name }
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

inspector-about-select = Выберите модель, затем перемещайте, поворачивайте, масштабируйте, кладите и зеркальте её.
inspector-about-measure = Щёлкните по двум точкам или двум плоским граням модели.
inspector-about-hollow = Вырежьте полость, чтобы сэкономить смолу и облегчить отрыв.
inspector-about-drain = Дайте смоле вытечь из полой модели.
inspector-about-cut = Разделите модель плоскостью.
inspector-about-relief = Вдавите текстуру в поверхность модели, напишите на ней текст или объедините модели в одну.
inspector-about-supports = Поддержите то, что нависает над пустотой.
inspector-about-repair = Что не так с поверхностью моделей, и починка, которая её закрывает, разворачивает или оборачивает.
inspector-about-check = Что не получится на принтере, с указанием слоя, на котором это происходит.
inspector-about-print-settings = Как эта платформа нарезается и экспонируется. Значения берутся из настроек принтера и смолы, а изменения здесь действуют только для этой платформы: при открытии проекта, создании нового, добавлении платформы или перезапуске они снова берутся из профилей. Чтобы изменить исходные значения для всех платформ, отредактируйте принтер или смолу в настройках.

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

controls-stop-cost = Остановленную печать нельзя возобновить.
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
repair-stitched-seams =
    { $count ->
        [one] сшит { $count } шов
        [few] сшито { $count } шва
       *[many] сшито { $count } швов
    }
repair-dropped-shells =
    { $count ->
        [one] отброшена { $count } соринка, ничего не охватывающая
        [few] отброшено { $count } соринки, ничего не охватывающие
       *[many] отброшено { $count } соринок, ничего не охватывающих
    }

repair-cancelled = Починка отменена
repair-failed = { $name } не удалось починить: { $reason }
repair-turned-faces =
    { $count ->
        [one] развёрнута { $count } грань
        [few] развёрнуто { $count } грани
       *[many] развёрнуто { $count } граней
    }
repair-wrapped = обёрнуто одной поверхностью
repair-not-wrapped = не обёрнуто: оболочки не объединяются точно

## panels/inspector/repair.rs

repair-mode-model = Модель: найти, что не так, и починить
repair-mode-faces = Грани: развернуть, удалить и соединить вручную
repair-shells-title = Оболочки
repair-shells-one = Выберите одну модель, чтобы увидеть её оболочки.
repair-shells-note = Каждая часть модели, грани которой соединены друг с другом. Нажатие выбирает оболочку и подводит к ней камеру; глаз скрывает её из вида, пока открыт этот список, чтобы увидеть, что за ней или внутри; корзина удаляет её сразу. Кнопки в заголовке выбирают все оболочки площадью меньше сотой доли самой большой или обращают выбор.
repair-listing = Перечисляем оболочки
repair-shell = Оболочка { $number }
repair-shell-faces =
    { $count ->
        [one] { $count } грань
        [few] { $count } грани
       *[many] { $count } граней
    }
repair-shell-faces-open =
    { $count ->
        [one] { $count } грань, открыта
        [few] { $count } грани, открыта
       *[many] { $count } граней, открыта
    }
repair-drop-shell = Удалить эту оболочку
repair-drop-shells =
    { $count ->
        [one] Удалить { $count } оболочку
        [few] Удалить { $count } оболочки
       *[many] Удалить { $count } оболочек
    }
repair-pick-small = Выбрать мелкие оболочки
repair-pick-invert = Обратить выбор
repair-pick = Выбор
repair-pick-faces = Грани
repair-pick-edges = Рёбра
repair-pick-faces-note = Проведите кистью по модели, чтобы выбрать грани, с Alt — чтобы снять выбор; Cmd и Shift вместе выбирают всю поверхность, пока она поворачивает не больше угла. «Развернуть» переворачивает их, «Удалить» убирает.
repair-pick-edges-note = Щёлкните по двум открытым рёбрам вокруг дыр, и «Соединить» положит между ними грани, обойдённые как соседние. Третий щелчок снимает первый.
repair-brush = Кисть
repair-brush-radius = Радиус
repair-flood-angle = Угол поверхности
repair-picked = Выбрано
repair-picked-model = Модель
repair-picked-faces = Грани
repair-picked-edges = Открытые рёбра
repair-of-two = { $count } из 2
repair-nothing-picked = Ничего не выбрано.
repair-no-edge-near = Рядом с указателем нет открытого ребра: щёлкните по контуру дыры
repair-no-open-edges = Открытых рёбер нет: модели замкнуты, соединять нечего
repair-finding-edges = Ищем открытые рёбра
repair-clear-picks = Снять выбор
repair-flip-faces = Развернуть
repair-delete-faces = Удалить
repair-bridge = Соединить
repair-pick-faces-first = Сначала выберите грани кистью.
repair-pick-two-edges = Сначала выберите два открытых ребра.
repair-editing = Меняем грани
edit-editing = Меняем грани { $name }
edit-flipped =
    { $count ->
        [one] Развёрнута { $count } грань { $name }
        [few] Развёрнуто { $count } грани { $name }
       *[many] Развёрнуто { $count } граней { $name }
    }
edit-removed =
    { $count ->
        [one] Удалена { $count } грань { $name }
        [few] Удалено { $count } грани { $name }
       *[many] Удалено { $count } граней { $name }
    }
edit-bridged =
    { $count ->
        [one] Соединено { $count } гранью на { $name }
       *[other] Соединено гранями ({ $count }) на { $name }
    }
edit-dropped-shells =
    { $count ->
        [one] Удалена { $count } оболочка { $name }
        [few] Удалено { $count } оболочки { $name }
       *[many] Удалено { $count } оболочек { $name }
    }
edit-bridge-not-open = Одно из рёбер уже не открыто, соединять нечего
edit-bridge-same = Ребро нельзя соединить с самим собой
edit-bridge-in-line = Рёбра лежат на одной прямой, грань между ними не ляжет
job-thread-stopped-edit = поток правки граней остановился, не закончив
repair-found = Найдено
repair-found-note = Что импорт нашёл в моделях, с которыми работает инструмент: выбранных или всех на платформе. Нажатие на строку отмечает найденное на моделях и подводит камеру к первому месту, каждое следующее — к следующему, а нажатие после последнего убирает отметки. Пересекающиеся грани считаются по лупе: на большой модели это занимает время.
repair-nothing-found = Чинить нечего.
repair-count-crossings = Посчитать пересекающиеся грани
repair-nothing-on-plate = На платформе нет моделей.
repair-holes = Дыры
repair-open-edges = Открытые рёбра
repair-flat-faces = Грани без площади
repair-doubled-faces = Дважды нарисованные грани
repair-shells = Оболочки
repair-crossing-faces = Пересекающиеся грани
repair-uncounted = –
repair-counting = …
repair-method = Метод
repair-method-repair = Починка
repair-method-close-holes = Закрыть дыры
repair-method-close-small-holes = Закрыть мелкие дыры
repair-method-turn = Развернуть грани
repair-method-wrap = Обернуть
repair-method-deep = Глубокая починка
repair-method-repair-note = Сшивает стыки, убирает дважды нарисованные грани и соринки без объёма, закрывает все дыры и разворачивает заплатки наружу. Оставляет модель как была, если стало бы хуже.
repair-method-close-holes-note = Закрывает все дыры и разворачивает заплатки наружу, и больше ничего.
repair-method-close-small-holes-note = Закрывает только дыры от одной недостающей грани и ничего не разворачивает.
repair-method-turn-note = Разворачивает неверно обойдённые грани, чтобы каждая оболочка смотрела наружу, и ничего не добавляет.
repair-method-wrap-note = Чинит, затем оборачивает модель одной внешней поверхностью: перекрытые оболочки объединяются точно, а деталь, целиком лежащая внутри другой, пропадает в ней. Полость внутри сохраняется. Где оболочки объединить нельзя, например если одна пересекает саму себя, остаётся только починка.
repair-method-deep-note = Чинит, оборачивает и чинит снова — для модели, которую другие методы оставляют сломанной. Как и при обёртке, деталь целиком внутри другой пропадает в ней.
repair-settings = Настройки
repair-settings-note = Вершина ближе допуска стыка к открытому ребру вшивается в него. Оболочка без объёма с площадью меньше наименьшей соринки удаляется.
repair-seam-tolerance = Допуск стыка
repair-smallest-shell = Наименьшая соринка
repair-repair = Починить
repair-repairing = Починка

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
mask-pixels = { $width } × { $height } пикс.
zoom-out = Уменьшить
zoom-in = Увеличить
zoom-whole = Показать целиком, или двойной щелчок
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
job-step-mending = Починка сетки
job-step-crossing = Поиск самопересечений
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

job-thread-stopped-repair = поток починки остановился, не закончив
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

find-group-tools = Инструменты
find-group-actions = Действия
find-group-models = Модели
find-move = выбрать
find-open = открыть
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
library-import = Импортировать профиль
library-custom = Свой принтер
library-search = Искать среди всех принтеров
library-machines =
    { $count ->
        [one] { $count } принтер
        [few] { $count } принтера
       *[many] { $count } принтеров
    }

## panels/stage.rs

stage-empty = На платформе ничего нет
stage-empty-how = Перетащите модель в окно
stage-open = Открыть модель

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
picker-your-machines = Ваши принтеры
picker-machine-detail = { $maker } · .{ $extension }
picker-add-machine = Добавить принтер
picker-on-machine = На { $name }
picker-resin-detail = { $kind } · { $layer } мкм
picker-add-resin = Добавить смолу

hint-click = Щелчок
hint-drag = Перетаскивание
hint-shift = Shift
hint-esc = Esc
hint-modifier-click = { $key } + щелчок
hint-modifier-drag = { $key } + перетаскивание
hint-clear = сбросить
hint-remove = убрать
hint-paint = закрасить
hint-erase = стереть
hint-measure-two-points = по двум точкам
hint-measure-second-point = по второй точке
hint-measure-two-faces = по двум плоским граням
hint-measure-second-face = по второй плоской грани
hint-measure-freely = мерить свободно
hint-drain-hole = по модели, чтобы прорезать отверстие
hint-drain-channel = по модели, чтобы добавить точку канала
hint-relief-write = по модели, чтобы написать там
hint-pick-without-writing = выбрать, не создавая надпись
hint-supports-pick = по части опоры, чтобы выбрать её
hint-supports-carry = перенести выбранное
hint-supports-add = добавить к выбранному
hint-hollow-solid = по модели, чтобы оставить там сплошную стенку
hint-area-paint = где опоры нужны
hint-area-block = где опор быть не должно
hint-repair-faces = выбрать грани
hint-repair-edge = по открытому краю, чтобы выбрать его
hint-let-go = отпустить
hint-supports-place = по модели, чтобы поставить опору
hint-supports-line = поставить их линией
hint-cut-part = по части, чтобы отметить её
hint-gizmo-steps = за ручку шагами по 1 мм и 15°
