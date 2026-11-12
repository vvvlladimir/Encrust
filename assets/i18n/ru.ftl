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
profiles-shipped = Стандартный профиль удалить нельзя.
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
value-ml = { $value } мл
value-mm = { $value } мм
value-mm2 = { $value } мм²
value-s = { $value } с
value-um = { $value } мкм

## panels/settings/general.rs

general-language = Язык
general-language-system = Как в системе ({ $name })
general-language-untranslated = Интерфейс ещё не переведён на язык системы ({ $tag }), поэтому показан на английском.
general-start = Запуск
general-start-page = Открывать стартовую страницу
general-start-page-hint = Если выключено, окно открывается сразу на платформе.
general-appearance = Тема
general-appearance-system = Системная
general-appearance-light = Светлая
general-appearance-dark = Тёмная
general-appearance-light-sub = Сланец и белый
general-appearance-dark-sub = Графит
general-appearance-now-light = Сейчас светлая
general-appearance-now-dark = Сейчас тёмная
general-appearance-hint = Системная тема меняется вместе с темой ОС, а с ней и область просмотра.
settings-page-appearance = Оформление
settings-group-help = Помощь
settings-find = Найти настройку
settings-words-appearance = тема светлая тёмная системная язык анимация движение
settings-words-general = начальная страница область просмотра масштаб нарезанные файлы папка имя файла шаблон профили
settings-words-updates = обновление релиз выпуск версия
updates-this-build = Установлена версия { $version }
general-viewport = Область просмотра
general-invert-zoom = Прокрутка вниз приближает
general-motion = Анимация
general-reduce-motion = Уменьшить движение
general-sliced-files = Нарезанные файлы
general-sliced-dir-system = Здесь открывается окно сохранения. Если папку забыть, оно откроется там, где система открывала его в последний раз.
general-sliced-dir-none = Не выбрана
general-choose-folder = Выбрать
general-forget-folder = Забыть папку
general-file-name = Имя файла
general-file-name-reset = Вернуть имя модели
general-file-name-example = Файл будет сохранён как { $name }
general-file-name-unknown = Неизвестные подстановки, останутся как есть: { $tags }
general-file-name-help = Как назвать нарезанный файл. Расширение добавляется само, а символы, недопустимые в файловой системе, заменяются подчёркиванием: {"{"}model{"}"}, {"{"}ext{"}"} — тип файла, {"{"}printer{"}"}, {"{"}resin{"}"}, {"{"}layer{"}"} в мкм, {"{"}plate{"}"}, {"{"}date{"}"} и {"{"}time{"}"}.
general-profiles = Профили
general-profiles-nowhere = В этой системе негде хранить ваши профили.
general-profiles-dir = Здесь хранятся ваши принтеры, смолы и профили.
general-open-folder = Открыть папку

## panels/inspector/cut.rs

cut-plane = Плоскость
cut-fact = { $axis } { $height } мм
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
cut-joint-dowels-note = Гнёзда в обеих половинах и дюбели, напечатанные рядом: обе грани разреза остаются ровными, и на них можно ставить поддержки.
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
cut-keyed = Один тоньше
cut-keyed-note = Один штифт тоньше остальных, поэтому половины собираются только одним способом.
cut-cutting = Разрезание модели
cut-cancelled = Резка отменена
cut-parts = Части
cut-parts-mark = Указать часть
cut-parts-click = Щёлкните по части на модели
cut-parts-unmark = Убрать метку
cut-parts-mark-note = Если часть указана, «Разделить на части» отрезает её плоским срезом там, где она сужается к остальной модели, а остальную поверхность не трогает.
cut-parts-smallest = Наименьшая часть
cut-parts-smallest-note = Кусок меньше этого присоединяется к части, которой касается, или отбрасывается, если не касается ни одной.
cut-replaces = Вместо одной модели появятся две. Отмена вернёт исходную.
cut-cut = Разрезать
cut-split = Разделить

## cut.rs

cut-nothing-selected = для разреза ничего не выбрано
cut-no-direction = у плоскости разреза нет направления
cut-misses = плоскость не пересекает { $name }, резать нечего
cut-undrilled = отверстия в { $name } нельзя точно перенести на половины, и они бы пропали
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
cut-done = Разрезано: { $name }, { $axis } = { $position } мм
cut-done-open =
    { $open ->
        [one] Разрезано: { $name }; { $open } контур разреза не замкнут и оставлен без крышки
        [few] Разрезано: { $name }; { $open } контура разреза не замкнуты и оставлены без крышки
       *[many] Разрезано: { $name }; { $open } контуров разреза не замкнуты и оставлены без крышки
    }
cut-nothing-to-split = для разделения ничего не выбрано
cut-one-piece = { $name } — цельная деталь
cut-split-done =
    { $count ->
        [one] { $name }: разделено на { $count } часть
        [few] { $name }: разделено на { $count } части
       *[many] { $name }: разделено на { $count } частей
    }
cut-cleaned = { $name }: мусор убран, модель осталась цельной
cut-split-joined =
    { $count ->
        [one] { $count } обрезок присоединён к части, которой касается.
        [few] { $count } обрезка присоединены к частям, которых касаются.
       *[many] { $count } обрезков присоединено к частям, которых касаются.
    }
cut-split-dropped =
    { $count ->
        [one] { $count } лишний обрезок отброшен.
        [few] { $count } лишних обрезка отброшены.
       *[many] { $count } лишних обрезков отброшено.
    }
cut-split-failed = модель не удалось разделить: { $reason }
cut-parted-off = Указанная часть отрезана от { $name } там, где она переходит в остальную модель
cut-no-neck = от указанной точки нет узкого места, по которому можно разрезать
cut-mended =
    { $count ->
        [one] { $count } деталь пришлось залатать, чтобы замкнуть.
        [few] { $count } детали пришлось залатать, чтобы замкнуть.
       *[many] { $count } деталей пришлось залатать, чтобы замкнуть.
    }

## panels/inspector/cut.rs

cut-keep-both = Оставить обе
cut-keep-lower = Нижняя
cut-keep-upper = Верхняя

## workspace.rs

tool-select = Выбор
tool-measure = Замер
tool-repair = Починка
tool-hollow = Полость
tool-drain = Дренаж
tool-cut = Разрез
tool-relief = Рельеф
tool-supports = Опоры
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
action-sheet = Сочетания клавиш
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
shortcut-group-view = Вид
shortcut-group-pointer = Мышь и трекпад
gesture-move-focus = Переход между элементами
gesture-move-focus-keys = Tab, Shift Tab
gesture-press-focus = Нажать элемент в фокусе
gesture-press-focus-keys = Enter или Пробел
gesture-leave-focus = Убрать фокус с элемента
gesture-leave-focus-keys = Esc
gesture-add-selection = Добавить к выбору
gesture-add-selection-keys = Cmd + щелчок
gesture-pick-between = Выбрать диапазон в списке
gesture-pick-between-keys = Shift + щелчок
gesture-orbit = Вращение
gesture-orbit-keys = Перетаскивание
gesture-pan = Сдвиг
gesture-pan-keys = Перетаскивание правой кнопкой
gesture-zoom = Масштаб
gesture-zoom-keys = Прокрутка
gesture-zoom-mask = Приблизить маску
gesture-zoom-mask-keys = Прокрутка или сведение пальцев
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

shortcuts-title = Сочетания клавиш
shortcuts-find = Найти клавишу
shortcuts-all = Все клавиши
shortcuts-list-keyboard = Клавиатура
shortcuts-list-pointer = Указатель
shortcuts-changed =
    { $count ->
        [one] { $count } изменена
        [few] { $count } изменены
       *[many] { $count } изменено
    }
shortcuts-none-found = Нет клавиши с таким названием
shortcuts-taken = { $keys } уже вызывает «{ $action }»
shortcuts-taken-why = «Заменить» снимет её с «{ $action }», другие клавиши этого действия останутся.
shortcuts-replace = Заменить
shortcuts-keep = Отмена
shortcuts-reset-all = Сбросить все клавиши
shortcuts-reset-all-asked = Сбросить все клавиши?
shortcuts-reset-all-tooltip = Каждое действие вернётся к своим исходным клавишам
shortcuts-stop = Отменить запись

## ui/widgets/binding.rs

binding-change = Нажмите, чтобы записать другую клавишу
binding-listening = Нажмите клавиши
binding-done = Готово
binding-stop = Отменить запись
binding-add = Добавить клавишу
binding-clear = Убрать эту клавишу
binding-reset = Вернуть исходные клавиши

## settings.rs

settings-page-supports = Поддержки
settings-page-hollow = Полость
settings-page-drain = Дренаж
settings-page-recipes = Рецепты
settings-page-general = Общие
settings-page-updates = Обновления
settings-tab-resins = Смолы
settings-tab-machine = Принтер
settings-tab-network = Сеть
settings-new-resin = Новая смола
settings-new-printer = Новый принтер
settings-new-profile = Новый профиль
settings-new-recipe = Новый рецепт
settings-copy-name = { $name } (копия)

## supports.rs

supports-placing-place = Поставить
supports-placing-fill = Залить
supports-placing-block = Запретить
supports-placing-erase = Стереть
supports-nothing-visible = На платформе нет видимых моделей, которые нужно поддерживать.
supports-refused-collides = Касается модели
supports-refused-leans = Слишком большой наклон
supports-refused-no-footing = Основанию не на что встать
supports-refused-no-hold = Наконечнику не за что держаться
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
supports-auto-no-shape = { $name } строит поддержки по профилю { $id }, которого нет в каталоге
supports-no-automatic = В каталоге нет автоматического профиля поддержек
supports-no-profile = В каталоге нет профиля поддержек { $id }
supports-group-name = Группа { $index }
supports-cannot-grow = Не удалось построить поддержки: { $reason }
supports-growth-stopped = Построение поддержек прервалось без результата
surfaces-cannot-build = Не удалось подготовить поверхность для поддержек: { $reason }
surfaces-build-stopped = Подготовка поверхности для поддержек прервалась без результата

## measure.rs

measure-snapped-corner = на углу
measure-snapped-edge = на ребре
measure-snapped-free = свободно

## panels/inspector/supports.rs

supports-grow-inside = Поддержки в полости
supports-grow-inside-tooltip = Убирает поддержки внутри полостей полых моделей и строит их заново по профилю выше. «Построить поддержки» ставит их только снаружи.
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
supports-clean-surface-about = Закрасьте грани, которые должны быть на виду. Автоматика делает там шаг контактов шире и ставит самые лёгкие наконечники.
supports-picked = Выбрано
supports-picked-help = «Построить заново» возвращает выбранные поддержки автоматике, и при следующем построении они создаются заново. «В группу» строит их по профилю текущей группы. «Слить в одну» соединяет поддержки одной модели в одно дерево на общем стволе.
supports-stat-parts = Части
supports-stat-supports = Поддержки
supports-move-to-group = В группу
supports-grow-these-again = Построить заново
supports-merge-these = Слить в одну
supports-merge-refused = Эти поддержки не слить. { $why }.
supports-remove-these = Удалить эти поддержки
supports-grow-again = Построить заново
supports-grow = Построить поддержки
supports-grow-tooltip = Нарезает на слои { $scope } и ищет то, что нависает над пустотой.
supports-placing = Расстановка поддержек
supports-sizes = Размеры
support-sizes-help = Поддержка строится тяжелее там, где держит больше. Доля поддержки — объём модели над её наконечником под островом или нависанием, разделённый между ней и всеми поддержками выше; размер уступа — объём смолы, который он добавляет за один слой. Каждое значение — наименьшее, при котором поддержка строится в этой форме.
support-islands-help = Под малым островом наконечники стоят чаще, чем задаёт шаг контактов. От первой площади ниже они расходятся шире, от второй — встают с полным шагом.
support-island-wider = Шаг на острове шире от
support-island-full = Полный шаг на острове от
supports-place = Расстановка
support-size-share = Доля для { $shape }
support-size-step = Уступ для { $shape }
support-group-steps = Уступы
support-group-steadying = Стабилизация
support-steps-help = Уступ — полка, которая выступает за один слой; его размер — объём смолы, который она добавляет. Грани, поднимающиеся от края уступа, поддерживаются вместе с ним в пределах охвата, если отклоняются от вертикали больше, чем «Наклон у уступа».
support-step-height = Охват для { $shape }
support-step-lean = Наклон у уступа
support-steadying-help = Деталь, которая растёт на гранях, слишком пологих, чтобы провиснуть, подпирают несколько тонких поддержек, чтобы её не раскачивал отрыв. Наклон — насколько грань отклоняется от вертикали, прежде чем её подпирают; шаг — расстояние между такими поддержками; наименьший склон — какой площади должна быть грань, чтобы получить хоть одну.
support-steady-lean = Наклон
support-steady-spacing = Шаг
support-steady-area = Наименьший склон
supports-remove-all-tooltip = Убрать все поддержки
supports-on-plate = На платформе
supports-profiles = Профили
supports-edit-profiles = Изменить профили
supports-mode-pick = Выбрать поддержки
supports-mode-place = Расставить поддержки
supports-mode-area = Закрасить или запретить область
supports-mode-shape = Форма поддержек
supports-lost-head-blocked =
    { $count ->
        [one] { $count } поддержке не хватает места для наконечника
        [few] { $count } поддержкам не хватает места для наконечника
       *[many] { $count } поддержкам не хватает места для наконечника
    }
supports-lost-faces-up =
    { $count ->
        [one] { $count } поддержка стоит на грани, обращённой вверх
        [few] { $count } поддержки стоят на гранях, обращённых вверх
       *[many] { $count } поддержек стоят на гранях, обращённых вверх
    }
supports-lost-no-ground =
    { $count ->
        [one] { $count } поддержке не на что встать
        [few] { $count } поддержкам не на что встать
       *[many] { $count } поддержкам не на что встать
    }
supports-drop-preset =
    { $count ->
        [one] Убрать группу «{ $name }» и её { $count } поддержку
        [few] Убрать группу «{ $name }» и её { $count } поддержки
       *[many] Убрать группу «{ $name }» и её { $count } поддержек
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
machines-in-hand = Текущий принтер
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

check-filter-islands = Острова
check-support-islands = Поддержать
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
check-reading = Чтение всех слоёв…
check-quick-found = Найдено быстрой проверкой. Чтение каждого пикселя…
check-reads-layers = Проверка читает слои. Они нарезаются, как только на платформе появляется модель и принтер, для которого её можно нарезать.
check-measuring = измерение
check-hardest-pull = Наибольшее усилие
check-disc = Как диск площадью
check-widest-step = Самый большой скачок
check-grows-by = Прирост
check-layer = Слой { $layer }
check-layer-lower = слой { $layer }
check-peel = Отрыв
check-nothing-found = Ничего, что сорвало бы печать
check-nothing-found-why = Нет островов, нет перемычек, нагруженных сверх предела, и нет слоёв, которые слишком сильно тянут плёнку.
check-issues-help = Острова начинаются в воздухе и остаются на плёнке. Рычаги — тонкие перемычки, которые может сломать усилие вбок; присоски — закрытые полости, которые подъём тянет вакуумом; тонкие участки могут вовсе не напечататься. Щёлкните строку, чтобы перейти к её слою.
check-peel-help = Каждый подъём отрывает свежий слой от плёнки, и чем шире слой, тем больше усилие. «Наибольшее усилие» — худший слой, показанный как диск с тем же усилием отрыва; «Самый большой скачок» — где площадь сильнее всего растёт от слоя к слою. Наклон модели или полость уменьшают и то и другое.
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
check-section-cups = Присоски
check-section-levers = Слабые перемычки
check-drill-cups = Просверлить
check-support-levers = Поддержать
check-auto-orient = Автоориентация
check-least-peel = Уменьшить отрыв
check-lift-settings = Настройки подъёма
check-place = Переместить на свободное место
check-fit-volume = Подогнать по размеру
check-combine = Объединить
check-drill-drains = Просверлить дренаж
check-cups-vent-help = «Просверлить» делает вентиляционное отверстие в самой тонкой стенке у основания каждой присоски: внутрь попадает воздух, и присоска отпускает плёнку. Если присоска стоит на платформе или её стенки слишком толстые, поднимите модель на поддержки или поверните её. Автоориентация ищет положение без присосок.
check-necks-held-help = «Поддержать» ставит поддержки под часть, которую держит каждая слабая перемычка, — столько, сколько нужно, чтобы перемычка выдержала нагрузку. Автоориентация ищет положение, в котором нагрузка на перемычки меньше.
check-stood-levers =
    { $count ->
        [0] Ни у одной слабой перемычки не нашлось нижней стороны для поддержки
        [one] Поставлена { $count } поддержка в группе «{ $group }»
        [few] Поставлены { $count } поддержки в группе «{ $group }»
       *[many] Поставлено { $count } поддержек в группе «{ $group }»
    }
check-vented-cups =
    { $count ->
        [0] Ни одну присоску не удалось просверлить: стенки слишком толстые
        [one] Просверлена { $count } присоска
        [few] Просверлены { $count } присоски
       *[many] Просверлено { $count } присосок
    }
check-cups-on-the-plate =
    { $count ->
        [one] { $count } присоска стоит на платформе: поднимите модель на поддержки или поверните её
        [few] { $count } присоски стоят на платформе: поднимите модели на поддержки или поверните их
       *[many] { $count } присосок стоят на платформе: поднимите модели на поддержки или поверните их
    }
check-left-to-you = Осталось исправить вручную: { $count }
check-layers = В слоях
check-card-way = Размещение
check-card-resin = Полость и дренаж
check-card-thin = Тонкие участки
check-card-by-hand = вручную
check-card-layers = слои { $first }–{ $last }
check-thin-no-fix = Автоматически не исправить: утолщите эти участки в модели.
check-fact-fixing = исправление
check-undo-fix = Отменить исправление
check-fix-all = Исправить всё
check-fix-issues-tip = Отодвигает мешающие модели, сверлит дренажные отверстия и присоски, затем ставит поддержки — именно в таком порядке. Всё отменяется одним шагом.
check-place-tip = Сдвигает только мешающие модели, каждую на ближайшее свободное место; модель, которая иначе не помещается, поворачивает. Если места не осталось, заново расставляет всю платформу.
check-fit-volume-tip = Уменьшает модель, которая не помещается в область печати, до подходящего размера.
check-combine-tip = Открывает «Рельеф» с двумя касающимися моделями, чтобы объединить их в одну.
check-drill-drains-tip = Подбирает дренажные отверстия, через которые смола вытечет из каждой замкнутой модели.
check-support-islands-tip = Ставит поддержку под каждый остров — в отдельную группу.
check-put-islands-back-tip = Возвращает удалённые острова в файл.
check-least-peel-tip = Поворачивает модели так, чтобы усилие отрыва от плёнки было наименьшим.
check-lift-settings-tip = Открывает настройки подъёма.
check-plate = На платформе
check-plate-help = Находится сразу, как только модели перестают двигаться, ещё до чтения слоёв. Нажмите строку, чтобы перейти к месту.
check-row-past = вне области печати
check-row-inset = в отступе от краёв
check-row-meets = касается { $name }
check-row-sealed = полая, без дренажного отверстия
check-row-solid = стоит сделать полой
check-row-hanging = нависает над пустотой · { $area } мм²
check-row-islands =
    { $count ->
        [1] { $area } мм²
        [one] { $count } остров · { $area } мм²
        [few] { $count } острова · { $area } мм²
       *[many] { $count } островов · { $area } мм²
    }
check-row-lever = { $lever } мм · { $stress } МПа
check-row-levers =
    { $count ->
        [one] { $count } рычаг · { $stress } МПа
        [few] { $count } рычага · { $stress } МПа
       *[many] { $count } рычагов · { $stress } МПа
    }
check-row-cup = { $area } мм²
check-row-cups =
    { $count ->
        [one] { $count } присоска · { $area } мм²
        [few] { $count } присоски · { $area } мм²
       *[many] { $count } присосок · { $area } мм²
    }
check-row-thin =
    { $count ->
        [1] { $width } мм
        [one] { $count } тонкий участок · { $width } мм
        [few] { $count } тонких участка · { $width } мм
       *[many] { $count } тонких участков · { $width } мм
    }
check-row-peel = { $force } Н
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

## panels/prepare_row.rs

prepare-row-tooltip = Готовит к печати { $scope }.
prepare-row-recipe = Рецепт, по которому идёт подготовка к печати
prepare-row-built-in = Стандартные
prepare-row-yours = Ваши
prepare-row-edit-recipes = Изменить рецепты
prepare-row-continue = Перейти к следующему шагу
prepare-row-retry = Повторить шаг с ошибкой и продолжить
prepare-row-end = Остановиться и оставить платформу как сейчас
prepare-row-waits-at = Шаг { $number }: { $why }

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
summary-time-to-layer = Время до слоя
summary-details = Подробнее
summary-model = Модель
summary-supports = Поддержки
summary-broken =
    { $count ->
        [one] { $count } модель повреждена
        [few] { $count } модели повреждены
       *[many] { $count } моделей повреждено
    }
summary-broken-why =
    { $count ->
        [one] То, что из неё нарезано, может не напечататься.
       *[other] То, что из них нарезано, может не напечататься.
    }

## ui/widgets.rs

duration-hours = { $hours }ч{ $minutes }мин{ $seconds }с
duration-minutes = { $minutes }мин{ $seconds }с
ago-never = никогда
ago-just-now = только что
ago-minutes = { $count } мин назад
ago-hours = { $count } ч назад
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

## ui/widgets/field/condition.rs

condition-insert = Вставить величину

## ui/widgets/layout.rs

dialog-close = Закрыть  Esc

## ui/widgets/list.rs

list-hide = Скрыть в области просмотра
list-show = Показать в области просмотра

## ui/widgets/colour.rs

colour-swatch = { $name } { $hex }
colour-opacity = Непрозрачность { $percent }%

## panels/inspector/measure.rs

measure-reading = Результат
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
measure-show-axes = Показывать dX, dY, dZ в области просмотра
measure-along-surface = Измерять по поверхности
measure-on-surface = По поверхности
measure-walk-finding = Поиск кратчайшего пути по поверхности…
measure-walk-apart = Точки не на одной модели, поэтому поверхность их не соединяет.
measure-walk-unreachable = Точки на разных оболочках модели, поэтому поверхность их не соединяет.
measure-walk-stopped = Отменено до того, как путь по поверхности был найден.
measure-walk-failed = Путь по поверхности не найден: { $why }.
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
hollow-keep-thin = Не делать полыми тонкие части
hollow-keep-thin-note = Там, где модель тоньше этого значения, она остаётся сплошной: тонкие части вроде пальцев и лезвий не становятся полыми.
hollow-thinner-than = Тоньше
hollow-keep-solid-clicks = Щёлкните по модели, чтобы оставить стенку сплошной в этом месте; Shift + щелчок тянет стержень от последнего блокиратора, Alt + щелчок по блокиратору убирает его.
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
hollow-fact = Стенка { $value } мм
hollow-nothing-done = Полостей пока нет
hollow-bottom-through-note = Стенка растёт внутрь, а дно, на котором стоит модель, вырезается, поэтому смола стекает без отверстий. Пока модель на платформе, дно закрыто, поэтому добавьте небольшое вентиляционное отверстие вверху.
hollow-shell-help = Полая модель расходует меньше смолы и слабее тянет плёнку. Стенка — толщина оболочки; около 2 мм подходит большинству деталей. Полости нужно дренажное отверстие, иначе запертая внутри смола может позже разорвать деталь.
hollow-wall = Стенка
hollow-precision = Точность
hollow-precision-note = Точность определяет гладкость полости на сетке { $lattice } мм. Это не высота слоя.
hollow-empty = Полость остаётся пустой.
hollow-hive = Соты
hollow-grid = Решётка
hollow-scaffold = Каркас
hollow-cell = Ячейка
hollow-cell-hive = Диаметр шестигранника
hollow-cell-grid = Ширина ячейки
hollow-density = Плотность
hollow-infill-walls-are = Стенки получаются толщиной { $mm } мм.
hollow-infill-struts-are = Стойки получаются толщиной { $mm } мм.
hollow-infill-thin = Каркас получается толщиной { $mm } мм — слишком тонко для печати: увеличьте ячейку или плотность.
hollow-walled-note = Ячейки стоят на платформе и соединены круглыми отверстиями ряд за рядом, поэтому смола перетекает из ячейки в ячейку к дренажному отверстию.
hollow-scaffold-note = Стойки по рёбрам куба, поставленного на вершину: все наклонены, ни одна не начинается в воздухе, и каркас держит нагрузку и вбок.
hollow-blocker-fit = По толщине
hollow-blocker-fixed = Свой размер
hollow-blocker-fit-note = Каждый блокиратор заполняет модель там, где вы щёлкнули, во всю её толщину.
hollow-blocker-reach = За стенкой
hollow-blocker-reach-note = Насколько каждый блокиратор заходит сквозь стенку в полость.
hollow-clear-blockers =
    { $count ->
        [one] Убрать { $count } блокиратор
        [few] Убрать { $count } блокиратора
       *[many] Убрать { $count } блокираторов
    }
hollow-hollow = Сделать полой
hollow-hollowing = Создание полости
hollow-hollowed = Объём полостей
hollow-resin-saved = Сэкономлено смолы
hollow-coarsened = Этой модели потребовалось больше памяти, чем допускает лимит, поэтому полость вырезана на сетке { $lattice } мм.
hollow-stale = Числа с тех пор изменились. Сделайте полость заново, чтобы обновить её.
hollow-make-solid = Снова сделать сплошной

## panels/inspector/print_settings.rs

print-bottom-layers = Нижние слои
print-exposure = Экспозиция
print-bottom-exposure = Экспозиция нижних слоёв
print-light-off = Пауза без света
print-before-lift = До подъёма
print-after-lift = После подъёма
print-after-retract = После возврата
print-lift = Подъём
print-lift-speed = Скорость подъёма
print-retract-speed = Скорость возврата
print-use-profiles = Вернуть настройки профилей
print-exposure-help = Сколько экран засвечивает каждый слой. Слишком мало — тонкие части не печатаются или слои расслаиваются; слишком много — детали разбухают, а отверстия зарастают. Нижние слои засвечиваются дольше, чтобы модель держалась на платформе, а слои перехода плавно снижают время между ними.
print-waiting-help = Паузы, чтобы смола успокоилась. «Пауза без света» — одна пауза перед засветкой каждого слоя; время отдыха — паузы после засветки, в верхней точке подъёма и после возврата платформы. Длинные паузы помогают густой смоле и большим слоям, но удлиняют печать.
print-motion-help = После каждого слоя платформа поднимается, отрывая его от плёнки, и возвращается. Медленный подъём отрывает мягче — это нужно большим и хрупким слоям, а высоты подъёма должно хватать, чтобы плёнка отпустила слой.
print-motion-waits = Движение и паузы
print-motion = Движение
print-carried = Пересчитано из { $exposure } с для слоя { $from } мкм на слой { $to } мкм.
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
relief-symmetric = Симметрично относительно среднего серого
relief-hold = Оставить ровными
relief-hold-bottom = Грани вниз
relief-hold-top = Грани вверх
relief-hold-note = Грани, отклонённые от направления на платформу или прямо вверх не больше чем на столько градусов, сохраняют форму, и рельеф обрывается на их краю. Ноль не оставляет ни одной.
relief-paint = Где
relief-paint-brush = Закрасить, где вдавить
relief-paint-note = Как только на модели закрашена хоть одна грань, текстура вдавливается только в закрашенные грани, остальная модель сохраняет свою сетку, и в области просмотра текстура видна только там.
relief-paint-clear = Очистить закрашенные области
relief-nothing-painted = Ничего не закрашено: закрасьте, где нужна текстура, или уберите кисть, чтобы вдавить текстуру во всю модель.
relief-image = Изображение
relief-image-note = Собственная текстура модели превращается в геометрию: белый выдвигает поверхность наружу, чёрный оставляет на месте. Либо выберите текстуру ниже или свою картинку, чтобы замостить ею любую модель.
relief-picture-size = { $width } × { $height } пикс.
relief-choose-another = Выбрать другую
relief-choose-picture = Выбрать картинку
relief-own = Собственные текстуры моделей
relief-own-none = На платформе их нет: нажмите на квадрат, чтобы выбрать текстуру
relief-own-textures = Использовать собственные текстуры моделей
relief-tiling-note = Картинка накладывается с трёх сторон, на которые смотрит модель, и каждая грань читает её со стороны, на которую смотрит больше всего.
relief-lattice-note = Рельеф вырезается на сетке { $lattice } мм, и вся модель заново строится по ней. Отрицательная глубина вдавливает текстуру.
relief-no-texture = На платформе нет текстур. Текстура читается из файла, из которого открыта модель, и пропадает после вдавливания.
relief-textures =
    { $count ->
        [1] Текстура
        [one] { $count } текстура
        [few] { $count } текстуры
       *[many] { $count } текстур
    }
relief-pressing = Вдавливание…
relief-cancelled = Вдавливание отменено
relief-press = Вдавить
relief-pressing-status = Вдавливание рельефа
text-words = Текст
text-words-hint = Что написать; новая строка начинает второй ряд
text-reading-fonts = Чтение шрифтов, установленных на этом компьютере…
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
text-click-note = Щёлкните по модели, чтобы разместить надпись.
text-stamping = Нанесение надписи
text-cancelled = Надпись отменена
text-nothing-under = здесь нет модели, на которой писать
text-done = Надпись на { $name } готова
text-bad-font = этот шрифт не читается: { $reason }
text-missing-glyph = в этом шрифте нет символа «{ $character }»
text-blank = писать нечего
text-bad-size = буквам нужны высота и глубина
text-off-the-model = здесь надпись не встаёт на модель
relief-mode-texture = Текстура
relief-mode-text = Текст
relief-mode-combine = Объединение
relief-mode-texture-note = Текстура или картинка, вдавленная в поверхность моделей на платформе.
relief-mode-text-note = Текст, выступающий из модели или вдавленный в неё там, где вы щёлкнете.
relief-mode-combine-note = Выбранные модели, сложенные в одну, вычтенные одна из другой или их общая часть.
text-lettering = Надпись
boolean-operation = Операция
boolean-models = Модели
boolean-none-picked = Пока ничего не выбрано.
boolean-models-note = В том порядке, в каком выбраны. Щёлкните модель, чтобы выбрать её, и с Shift или Cmd — чтобы добавить следующую.
boolean-role-kept = остаётся
boolean-role-taken-out = вычитается
boolean-role-joined = добавляется
boolean-role-shared = общая часть
boolean-union = Сложение
boolean-difference = Вычитание
boolean-intersection = Пересечение
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
boolean-pick-two = Выберите на платформе две модели или больше, удерживая Shift или Cmd.
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
        [one] { $count } модель объединена в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
        [few] { $count } модели объединены в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
       *[many] { $count } моделей объединено в { $name } на решётке { $lattice } мм, потому что одна из них не замкнута
    }
job-thread-stopped-boolean = поток объединения остановился, не закончив
job-thread-stopped-cut = поток разреза остановился, не закончив
job-thread-stopped-split = поток разделения остановился, не закончив
job-thread-stopped-measure-walk = поток измерения остановился, не закончив
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
chip-resin-untuned = Смола не откалибрована для этого принтера
chip-resin-tooltip = Смола
chip-layers = { $layer } мкм · { $exposure } с
chip-settings = { $tool }, { $summary }
find-button = Найти
find-tooltip = Найти инструмент, действие или модель
bar-source = Исходный код
bar-source-tooltip = Encrust — свободное ПО под лицензией AGPL-3.0; здесь исходный код этой версии
bar-close = Закрыть
bar-restore = Восстановить
bar-maximise = Развернуть
bar-minimise = Свернуть
bar-restart-to-update = Перезапустить и обновить
bar-restart-tooltip = Закрыть окно и запустить новую сборку
bar-update-tooltip = Вышел новый релиз: посмотрите, что изменилось
bar-update-percent = Обновление · { $percent } %
update-sheet-title = Обновление
update-sheet-from-to = { $from } → { $to }
update-group-new = Новое
update-released = вышел { $when }
update-size = { $mb } МБ
update-later = Позже
update-downloading-of = Скачивание · { $got } из { $total } МБ
update-downloading = Скачивание · { $got } МБ
update-time-left = осталось около { $seconds } с
update-step-download = Скачивание
update-step-verify = Проверка подписи
update-step-install = Установка
update-cancel = Отмена
notice-release-out = Вышел Encrust { $version }
app-version = Encrust { $version }
name-numbered = { $name } { $number }
name-and-detail = { $name }, { $detail }
value-percent = { $value }%
printer-last-print = { $how }: { $file }
notice-release-and-more = { $first } и ещё { $count }
notice-what-is-new = Что нового
notice-release-later = Позже: значок останется в верхней панели
updates-release-found = Найден релиз
updates-looking-block = Проверка
updates-every-change = Все изменения
updates-and-more = И ещё { $count }.
updates-last-looked = Последняя проверка: { $when }
bar-alpha = Альфа
bar-alpha-tooltip = Encrust в альфа-версии: расскажите, что пошло не так
menu-new-plate = Новая платформа
menu-new-project = Новый проект
menu-open-project = Открыть проект…
menu-open-recent = Открыть недавние
menu-save-project = Сохранить
menu-save-project-as = Сохранить как…
menu-open-model = Добавить модель…
menu-open-test-print = Пробные печати
menu-import-machine = Профиль принтера…
menu-import-resin = Профиль смолы…
menu-machine-and-resin = Принтер и смола…
menu-settings = Настройки…
menu-quit = Выйти из Encrust
menu-nothing-recent = Пока ничего не открывалось
menu-arrange-plate = Расставить на платформе
menu-auto-orient-plate = Автоориентация платформы
menu-prepare = Подготовить к печати
menu-move-to-plate = Переместить на платформу
menu-frame-view = Показать всё
menu-plate-grid = Сетка платформы
menu-xray = Просвечивать модели
menu-render = Показать как напечатается
menu-depth-of-field = Глубина резкости в виде «как напечатается»
menu-orthographic = Ортогональный вид
menu-import = Импорт
menu-clear-recent = Очистить список
menu-slicing-already = Платформа уже нарезается
menu-send-file = Отправить нарезанный файл…
menu-send-file-to = Файл уйдёт на { $name } без изменений.
menu-pick-first = Сначала выберите модель
menu-plate-empty = Платформа пуста
menu-camera = Камера
menu-camera-perspective = Перспектива
menu-camera-orthographic = Ортогональная
menu-show = Показывать

## panels/support_fields.rs

support-contact = Контакт
support-bite = Заглубление
support-length = Длина
support-diameter = Диаметр
support-clearance = Зазор
support-trunk-cap = Предел ширины ствола
support-thickness = Толщина
support-flare = Раструб
support-upper-depth = Верхняя глубина
support-lower-depth = Нижняя глубина
support-max-length = Макс. длина
support-branch-angle = Угол ветвления
support-merge-within = Объединять в пределах
support-longest-strut = Макс. длина ветви
support-tips-per-trunk = Наконечников на ствол
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
support-land-on-model-off = Поддержка, под которой находится модель, отклоняется к платформе или убирается. К детали ничего не прирастает.
support-thin-down = Сделать их тоньше
support-thin-down-off = Поддержка, опирающаяся на модель, сохраняет ширину обычной колонны.
support-small-pillar-note = Применяется там, где поддержка идёт от одной части модели к другой. Оба конца уходят в поверхность; более длинный участок сохраняет обычную ширину.
support-no-branching = Каждый наконечник стоит на собственной колонне.
support-no-grid = Стволы стоят там, где сходятся наконечники, где бы это ни было.
support-no-raft = Основания стоят прямо на платформе.
support-no-bracing = Каждый ствол стоит сам по себе.
support-tip-help = Единственная часть, которая касается модели. Широкий контакт держит крепче, но оставляет след больше; заглубление утапливает наконечник в поверхность, чтобы он не оторвался во время печати. Длина — конус от наконечника до колонны, а сфера держит тяжёлое нависание крепче плоскости.
support-body-help = Диаметр — толщина колонны: толще значит жёстче, но смолы уходит больше. Зазор — воздух между колонной и моделью, чтобы колонна не приросла к детали. Предел ширины ствола — наибольшая ширина, до которой ствол может расшириться у основания.
support-foot-help = Основание держит поддержку на платформе. Шире и толще — держит крепче, но труднее снимается; у конусной и полоза есть край, под который заходит шпатель. Раструб — насколько колонна расширяется перед основанием.
support-branching-help = Соседние наконечники сходятся в общие стволы, как дерево: меньше колонн, меньше смолы и следов на платформе. Больший угол ветвления объединяет больше, но ветви наклоняются сильнее; наконечники дальше друг от друга, чем «Объединять в пределах», стоят на своих колоннах.
support-grid-help = Каждый ствол стоит в узле квадратной сетки на платформе: наконечник тянется к ближайшему узлу под 45°, наконечники одного узла сходятся в один ствол. Лес получается ровным, равномерно связывается и срезается рядами. Наконечник, которому не нашлось узла, стоит как без сетки.
support-grid-spacing = Шаг
support-raft-help = Подложка — пластина, на которой поддержки стоят вместо платформы: она распределяет отрыв по одной площади и снимается одним куском. «Решётка» меньше присасывается к платформе, «Сплошная» соединяет площадки под основаниями. Отступ — насколько подложка выходит за модель, а наклонная стенка держит крепче, но всё равно легко поддевается.
support-bracing-help = Распорки связывают высокие стволы, чтобы те не раскачивались при отрыве слоя. Пролёт — наибольшее расстояние между стволами, которое они соединяют; «Первая на» и «Каждые» задают высоту первой распорки и шаг между ними. По высоте каждой пары верхняя и нижняя зоны берут по нескольку уровней, а середина заполняет остальное: зигзаг — одна распорка на уровень, крест — две. Высокий ствол, свободный наверху, получает подпорки: пару, если он выше своей верхней распорки на «Пара выше», и одну, если выше второй на «Одна выше».
support-held-up-help = Нависание — насколько поверхность может отклониться от вертикали, прежде чем получит поддержки: 90° — потолок, меньшее число ставит больше поддержек. Шаг контактов — расстояние между наконечниками под гранью, смотрящей прямо вниз, а шаг на крутых гранях — под гранью, наклонённой чуть дальше угла нависания; между ними шаг меняется вместе с углом. Плотность умножает их количество: 1 — столько, сколько Encrust считает достаточным.
support-lift-help = На какой высоте модель стоит над платформой. Поддержкам нужно место под ней, а первые слои прилипают к платформе сильнее всего, поэтому деталь, напечатанную прямо на ней, трудно снять.
support-raft = Подложка под основаниями
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
support-tab-off = ВЫКЛ
settings-save = Сохранить
support-save-first = Сначала сохраните профиль: берётся последняя сохранённая версия.
unsaved-title = Сохранить изменения в профиле «{ $name }»?
unsaved-what = Правки этого профиля ещё не сохранены.
unsaved-discard = Не сохранять
support-table-manual = Вручную
support-table-tip = Наконечник
support-table-spacing = Шаг
support-cannot-add = не удалось добавить профиль поддержек
support-pick-profile = Выберите профиль поддержек.
support-reset = Сбросить к стандартным
support-reset-shipped-help = «Сбросить к стандартным» возвращает стандартные значения профиля и сохраняет его имя; пока имя стандартное, профиль обновляется вместе с программой. Число, выделенное янтарным, ещё не сохранено; стрелка рядом возвращает сохранённое. Сброс можно отменить.
support-reset-auto-help = «Сбросить к стандартным» возвращает стандартные значения автоматического профиля; после этого он обновляется вместе с программой. Число, выделенное янтарным, ещё не сохранено; стрелка рядом возвращает сохранённое. Сброс можно отменить.
support-reset-stock-help = «Сбросить к стандартным» возвращает числа, с которых начинается новый профиль, и сохраняет его имя. Сброс можно отменить.
support-cannot-save = не удалось сохранить профиль поддержек

## panels/inspector/shape.rs

shape-part = Часть
shape-regular = Обычная
shape-raft-own = Это собственная подложка модели, она пришла из её проекта.
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

## settings/recipes.rs

recipes-nothing-open = не открыт ни один рецепт
recipes-built-in-why = Стандартный рецепт нельзя изменить или удалить. Нажмите «Дублировать», чтобы получить свою копию.
recipes-cannot-save = не удалось сохранить рецепт
recipes-unreadable-why = в файле рецепта есть ошибки
recipes-copy-name = { $name } — копия
recipes-new-name = Новый рецепт
recipes-new-name-numbered = Новый рецепт { $number }

## panels/settings/recipes.rs

recipes-table-recipe = Рецепт
recipes-table-kept = Тип
recipes-table-steps = Шаги
recipes-kept-built-in = Стандартный
recipes-kept-yours = Ваш
recipes-kept-broken = С ошибками
recipes-fix-first = Перед сохранением исправьте то, что отмечено красным.
recipes-saved = Рецепт { $recipe } сохранён

## panels/settings/recipes/form.rs

recipes-built-in = Стандартный рецепт
recipes-built-in-how = Нажмите «Дублировать», чтобы получить свою копию и менять её.
recipes-block-recipe = Рецепт
recipes-block-recipe-help = Название показывается в списке «Подготовить к печати». Переменные — это ваши числа, которые можно использовать в условиях и настройках шагов. Например, сколько смолы должна сэкономить полость, чтобы модель стоило делать полой.
recipes-does-not-run = Рецепт не запустится, пока в нём есть ошибки
recipes-var-value = Значение
recipes-var-remove = Удалить переменную
recipes-var-add = Добавить переменную
recipes-step-add = Добавить шаг
recipes-no-steps = В рецепте пока нет шагов.

## panels/settings/recipes/chain.rs

recipes-step-drag = Перетащите, чтобы переместить шаг
recipes-summary-off = Выключен
recipes-summary-when = Когда { $condition }
recipes-summary-skips = Пропускается при ошибке
recipes-summary-set = { $count ->
    [one] { $count } настройка изменена
    [few] { $count } настройки изменены
   *[many] { $count } настроек изменено
}
recipes-summary-rules = { $count ->
    [one] { $count } правило
    [few] { $count } правила
   *[many] { $count } правил
}

## panels/settings/recipes/step.rs

recipes-step-help = Шаги выполняются по порядку, каждый — с моделью в том виде, в каком её оставили предыдущие. «Когда» — условие для модели, например saves_ml >= 30; если поле пустое, шаг выполняется всегда. «При ошибке» решает, что делать, если шаг не удался: остановить подготовку или пропустить шаг. Если настройку не задать, берётся значение по умолчанию; число или переключатель можно также вычислить выражением. «Правила» меняют настройки, когда выполняется их условие; срабатывает первое подходящее правило.
recipes-step-up = Переместить шаг выше
recipes-step-down = Переместить шаг ниже
recipes-step-remove = Удалить шаг
recipes-when = Когда
recipes-stop-if = Остановить, если
recipes-message = Сообщение
recipes-on-fail = При ошибке
recipes-on-fail-stop = Остановить
recipes-on-fail-skip = Пропустить
recipes-retry = Добавить поддержки к найденным островам
recipes-retry-while = Пока
recipes-retry-max = Попыток, макс.
recipes-choose = Правила
recipes-rule-remove = Удалить правило
recipes-rule-add = Добавить правило
recipes-when-empty = Всегда
recipes-stop-if-empty = Никогда
recipes-retry-while-empty = например, islands > 0
recipes-rule-when-empty = например, height_mm > 120
recipes-conditions-help = В условии можно сравнивать величины и числа (< <= > >= == !=), объединять сравнения через && (и), || (или) и ! (не), считать с + - * / и функциями min, max, abs и clamp. Кнопка рядом с условием показывает все доступные величины.
recipes-step-help-repair = Починка исправляет сетку, чтобы модель можно было нарезать: закрывает дыры и разворачивает грани наружу. «Метод» задаёт, насколько тщательно.
recipes-step-help-orient = Ориентация поворачивает модель на платформе. «Цель» — что свести к минимуму: поддержки, усилие отрыва от плёнки или время печати. «Наклон, макс.» ограничивает, насколько сильно модель можно наклонить.
recipes-step-help-hollow = Полость убирает материал внутри модели и оставляет стенку заданной толщины, чтобы расходовать меньше смолы.
recipes-step-help-drain = Дренажные отверстия выпускают смолу из полой модели: туда, где поиск найдёт, что смола вытечет, или в заданные точки.
recipes-step-help-supports = Шаг «Поддержки» строит поддержки под моделью по профилю поддержек.
recipes-step-help-check = Проверка нарезает платформу и ищет острова, запертую смолу и сильный отрыв от плёнки. Величины islands, peel_n и support_ml доступны в условиях только после проверки.
recipes-step-help-arrange = Расстановка раскладывает модели по платформе с заданным расстоянием между ними.
recipes-step-help-save = Сохранение нарезает платформу в файл в указанной папке и может сразу отправить его на принтер.
recipes-step-help-gate = Условие остановки останавливает подготовку и показывает своё сообщение, если выполняется «Остановить, если».
recipes-step-help-pause = Пауза показывает своё сообщение и ждёт, пока вы не нажмёте «Продолжить».

## panels/settings/recipes/setting.rs

recipes-setting-set = Задать
recipes-setting-default = Вернуть значение по умолчанию
recipes-setting-expression = Вычислить выражением
recipes-setting-left-out = По умолчанию
recipes-point-remove = Удалить точку
recipes-point-add = Добавить точку
recipes-set-method = Метод
recipes-set-goal = Цель
recipes-set-max-tilt = Наклон, макс.
recipes-set-clean-surface = Чистая поверхность
recipes-set-wall = Стенка
recipes-set-mode = Режим
recipes-set-infill = Заполнение
recipes-set-keep-solid = Сплошное, если тоньше
recipes-set-at = Размещение
recipes-set-points = Точки
recipes-set-diameter = Диаметр
recipes-set-depth = Глубина
recipes-set-vent = Вентиляционное отверстие
recipes-set-plugs = Пробки
recipes-set-profile = Профиль поддержек
recipes-set-cavity = Поддержки в полости
recipes-set-spacing = Расстояние
recipes-set-only-in-the-way = Двигать только мешающие модели
recipes-set-drill-cups = Сверлить присоски
recipes-set-hold-levers = Поддерживать слабые перемычки
recipes-set-folder = Папка
recipes-set-name = Имя файла
recipes-set-send = Отправить на принтер
recipes-expression-empty = например, height_mm / 50
recipes-word-repair = Починка
recipes-word-close-holes = Закрыть дыры
recipes-word-wrap = Обернуть
recipes-word-deep = Глубокая починка
recipes-word-auto = Авто
recipes-word-fewest-supports = Меньше всего поддержек
recipes-word-least-peel = Слабый отрыв
recipes-word-shortest-print = Быстрая печать
recipes-word-inside = Внутрь
recipes-word-outside = Наружу
recipes-word-open-floor = Открытое дно
recipes-word-points = В точках

## panels/settings/recipes/names.rs

recipes-names-model = Модель
recipes-names-measured = После проверки
recipes-names-yours = Ваши переменные
recipes-names-functions = Функции
recipes-names-your-value = Ваша переменная, { $value }
recipes-names-min = Меньшее из двух
recipes-names-max = Большее из двух
recipes-names-abs = Модуль числа
recipes-names-clamp = Ограничить диапазоном
recipes-names-height = Высота, мм
recipes-names-width = Ширина, мм
recipes-names-depth = Глубина, мм
recipes-names-volume = Объём, мл
recipes-names-surface = Площадь поверхности, см²
recipes-names-faces = Число граней
recipes-names-sound = В сетке нет дефектов
recipes-names-defects = Число дефектов сетки
recipes-names-turned = Модель повёрнута вручную
recipes-names-hollow = Модель полая
recipes-names-saves-ml = Сколько смолы сэкономит полость, мл
recipes-names-saves-share = Какую долю смолы сэкономит полость
recipes-names-worth-hollowing = Полость имеет смысл
recipes-names-fits = Помещается в область печати
recipes-names-trapped = Карманы запертой смолы с учётом дренажа
recipes-names-islands = Острова без поддержки
recipes-names-peel = Наибольшее усилие отрыва от плёнки, Н
recipes-names-support-ml = Смола на поддержки, мл

## panels/settings/presets.rs

presets-depth = Глубина
presets-taper = Конусность
presets-empty =
    { $kind ->
        [hollow] Профилей полости пока нет. Сохраните профиль в строке «Профиль» инструмента или импортируйте его.
       *[drain] Профилей дренажа пока нет. Сохраните профиль в строке «Профиль» инструмента или импортируйте его.
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
resins-by-hand = Экспозиция и слои — с этикетки или сайта производителя смолы
resins-add = Добавить смолу
resins-from-file = Файл, нарезанный для этого принтера (.goo, .ctb, …), или профиль смолы (.cfgx, .ini), либо перетащите его сюда
resins-import = Импортировать смолу
resins-all-types = Все типы
resins-from-pool = Из общего списка
resins-delete = Удалить эту смолу
resins-all-here = Все ваши смолы уже добавлены на этот принтер.
resins-none-here = На этом принтере пока нет смол. Добавьте смолу кнопкой «Добавить смолу» выше.
resins-row-in-hand = Текущая смола. Дважды щёлкните, чтобы изменить.
resins-row = Дважды щёлкните, чтобы изменить.
resins-take-off = Убрать с этого принтера
resins-use = Использовать эту смолу

## files.rs

filter-model = Модель
filter-mesh = Полигональная сетка
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
resin-buffer-help = Медленный участок хода, который прошивка принтера выполняет сама: на этой высоте у ванны платформа движется с этой скоростью, затем ждёт перед засветкой слоя. Записывается в файл; принтер начинает со значений производителя.
resin-buffer-on = Использовать буфер
resin-buffer-height = Высота буфера
resin-buffer-speed = Скорость буфера
resin-buffer-light-off = Ожидание после буфера
resin-bottom-light-off = Пауза без света на нижних слоях
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
resin-light-off-delay = Пауза без света
resin-rest-times = Время отдыха
resin-waits-note = Входит во время печати и записывается в файл.
resin-carried = Пересчитано из { $exposure } с для { $from } мм на { $to } мм.
resin-light-power = Мощность света
resin-bottom-power = Мощность на нижних слоях
resin-transition-note = Линейный: каждый слой на { $step } с короче нижележащего.
limit-takes-between = Этот принтер поддерживает { $low }–{ $high } { $unit }.
limit-takes-from = Этот принтер поддерживает от { $low } { $unit }.
limit-takes-up-to = Этот принтер поддерживает не больше { $high } { $unit }.
limit-advised-between = Производитель рекомендует { $low }–{ $high } { $unit }.
limit-advised-from = Производитель рекомендует от { $low } { $unit }.
limit-advised-up-to = Производитель рекомендует не больше { $high } { $unit }.
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
comp-shrinkage-note = Тестовая деталь, напечатанная в масштабе 100 %, досвеченная и измеренная в холодном виде.
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

layers-cusp = Ступенька
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
        [one] { $count } уровень, размытие { $blur } пикс.
        [few] { $count } уровня, размытие { $blur } пикс.
       *[many] { $count } уровней, размытие { $blur } пикс.
    }
layers-all-greys = Все оттенки
layers-all-greys-blur = Все оттенки, размытие { $blur } пикс.
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
layers-planes-note = во столько раз больше сечений — и элемент тоньше слоя не пропадёт
layers-height-help = Тонкие слои дают меньше ступенек на изгибах, но печать дольше: время растёт с числом слоёв. Адаптивная высота режет тонкие слои там, где поверхность изгибается, и толстые там, где она идёт прямо.
layers-round-grey = Ограничить оттенки серого
layers-grey-levels = Оттенки серого
layers-edge-blur = Размытие края
layers-blur-note = сильное размытие затемняет и стенку внутри края; обычно хватает одного-двух пикселей
layers-thickest-reached = Самый толстый получившийся слой — { $thickest } мкм, а не { $asked } мкм, поэтому обычный слой экспонируется { $exposure } с, а не { $resin } с.
layers-adaptive = Адаптивная высота
layers-adaptive-refused = Этот принтер двигает платформу на высоту из заголовка, поэтому у слоя не может быть собственной высоты.
layers-no-bands = Все слои экспонируются по значению смолы. Полоса заменяет его на участке высоты.
layers-bands-note = Нижние слои сохраняют экспозицию смолы независимо от полос.
layers-bands-refused = Этот принтер читает только заголовок, поэтому экспозицию нельзя менять по высоте.
layers-add-band = Добавить полосу
layers-band = Полоса { $index }
layers-remove-band = Удалить эту полосу

## panels/settings/printer.rs

printer-panel-width = Ширина экрана
printer-panel-height = Высота экрана
printer-lit-width = Ширина засвечиваемой области
printer-lit-height = Высота засвечиваемой области
printer-volume-x = Размер по X
printer-volume-y = Размер по Y
printer-volume-z = Размер по Z
printer-per-layer = Настройки по слоям
printer-variable-height = Переменная высота слоя
printer-manufacturer = Производитель
printer-display = Экран
printer-build-volume = Область печати
printer-output = Формат файла
printer-firmware = Прошивка
printer-browser-sends-nothing = Браузер не может подключиться к принтеру в вашей сети. Нарезанный файл скачивается, а на принтер его нужно перенести на флешке или через приложение самого принтера.
printer-edited-copy = Вы изменили принтер, который есть и в стандартной библиотеке. Изменённые поля сохраняют ваши значения, остальные обновляются с каждым релизом.
printer-use-shipped = Использовать стандартный профиль
printer-cannot-put-back = не удалось вернуть стандартный профиль
printer-cannot-save = не удалось сохранить принтер
printer-grey-min = Самый тёмный серый на краю
printer-grey-max = Самый светлый серый на краю
printer-grey-range-full = Оттенки серого на краях записываются как есть.
printer-grey-range-spread = Серый на краю поднимается до самого тёмного и растягивается до самого светлого; чёрный и чистый белый не меняются.
printer-pitch = { $x } × { $y } мкм на пиксель
printer-mirror = Отражение
printer-mirror-normal = Без отражения
printer-mirror-lcd = Отражение по X (LCD)
printer-mirror-dlp = Без отражения (DLP)
printer-mirror-note = Большинство LCD-принтеров ждут маски, отражённые по X; какое отражение нужно, указано в настройках производителя.
printer-inset = Отступ от краёв платформы
printer-inset-min-x = Отступ с края −X
printer-inset-max-x = Отступ с края +X
printer-inset-min-y = Отступ с края −Y
printer-inset-max-y = Отступ с края +Y
printer-inset-note = Модель в полосе отступа помечается, но нарезается целиком.
printer-per-layer-on = Экспозиция и движение читаются из каждого слоя; в .goo это называют расширенным режимом.
printer-per-layer-off = Каждый слой печатается по значениям из заголовка, а переход нижних слоёв принтер выполняет сам.
printer-variable-height-note = Если выключено, платформа двигается на высоту из заголовка, поэтому адаптивная высота слоя недоступна.
printer-output-note = В какой формат нарезать для этого принтера — и при сохранении, и при отправке.

## network.rs

network-advice-app = Если принтер включён, возможно, macOS не пускает Encrust в локальную сеть: разрешите это в разделе «Системные настройки», «Конфиденциальность и безопасность», «Локальная сеть».
network-advice-terminal = Если принтер включён, возможно, macOS не пускает это окно в локальную сеть. При запуске из терминала у окна права этого терминала: запустите его из Terminal.app, разрешите свой терминал или редактор в разделе «Системные настройки», «Конфиденциальность и безопасность», «Локальная сеть» либо запустите его как приложение командой `cargo xtask app`.
network-not-reached = пока недоступен
network-printing = { $printer } печатает { $file }
network-busy = { $name } занят: { $state }
network-prusa-sl1 = Принтеры Prusa печатают .sl1; выберите его в поле «Формат файла» этого принтера.
network-sending = Отправка на { $name }
network-not-answering = { $name } не отвечает
network-starting = Запуск { $file } на { $name }
network-sent = { $file } передан на { $printer }
network-cancelled = Передача отменена
network-usb-only = Только USB

## panels/settings/printer/connection.rs

connection-usb-note = Нарезанное сохраняется в файл и переносится на флешке.
connection-protocol = Протокол
connection-printers = Принтеры
connection-found = Найдены рядом
found-add = Добавить
connection-scanning = Сканирование…
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
connection-other-model = Модель не совпадает с профилем
connection-add-all = Добавить все принтеры этой модели ({ $count })
connection-set-up-here = Настроить в этом окне
connection-forget-login = Забыть логин для { $host }
connection-set-up-another = Настроить другой
connection-login-note = Логин хранится в настройках этого окна, а не в профиле.
connection-password = пароль
connection-api-key = или ключ API

## panels/printer_card.rs

card-open-printer = Открыть принтер
card-search-failed = Не удалось начать поиск ни в одной сети
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
card-celsius = { $name } { $value } °C
card-almost-done = Почти готово
card-minutes-left = осталось { $minutes } мин
card-hours-left = осталось { $hours } ч
card-time-left = осталось { $hours } ч { $minutes } мин

## panels/slice.rs

slice-to = Нарезать в .{ $extension }
slice-tooltip = Нарезать платформу в файл для печати
slice-send-tooltip = Нарезать платформу и отправить файл на принтер
slice-all = Нарезать все платформы ({ $count }) в .{ $extension }
slice-send = Нарезать и отправить
slice-start-tooltip = Начать печать файла, загруженного на { $name }
slice-start-print = Начать печать
slice-saved-instead = { $why }; вместо отправки файл будет сохранён на диск.
slice-cannot-send = не удалось нарезать набор слоёв для отправки
slice-slicing = Нарезка
slice-all-started = Нарезка платформ ({ $count }) рядом с { $path }
slice-cannot-slice-into = не удалось нарезать в { $path }
slice-slicing-into = Нарезка в { $path }

## slicing.rs

slicing-cannot-read-profiles = не удалось прочитать ваши профили в { $dir }
slicing-cannot-read-profiles-browser = не удалось прочитать ваши профили в этом браузере
slicing-select-resin = Выберите смолу
slicing-pick-machine = Выберите принтер: без него неизвестно, под какой экран нарезать.
slicing-add-resin = Добавьте смолу для этого принтера: экспозиция подбирается на вашем принтере.
slicing-nothing-visible = На платформе нет видимых моделей для нарезки.
slicing-supports-growing = Поддержки ещё строятся.
slicing-steps-by-header = Этот принтер двигает платформу на высоту слоя из заголовка.
slicing-no-printer = профиль принтера не загружен
slicing-no-resin = смола не выбрана, а готовой экспозиции нет — её подбирают на своём принтере
slicing-nothing-visible-error = на платформе нет видимых моделей для нарезки
slicing-supports-growing-error = поддержки ещё строятся
slicing-no-format = по имени { $path } не понять формат нарезанного файла
slicing-clipped =
    { $count ->
        [one] , { $count } слой обрезан по краю экрана
        [few] , { $count } слоя обрезано по краю экрана
       *[many] , { $count } слоёв обрезано по краю экрана
    }
slicing-wrote = Записан { $path }: слоёв — { $layers }, { $volume } мл, { $weight } г{ $cost }{ $clipped }
slicing-cancelled = Нарезка отменена

## panels/destination.rs

slice-speaks-other = { $name } работает по { $theirs }, а этот профиль отправляет по { $ours }: настройте сеть в настройках принтера.

## hollow.rs

hollow-nothing-visible = На платформе нет видимых моделей, в которых можно сделать полость.
hollow-nothing-visible-error = на платформе нет видимых моделей, в которых можно сделать полость
hollow-done =
    { $count ->
        [0] На платформе нет моделей, в которых можно сделать полость
        [one] Полость сделана в { $count } модели, сэкономлено { $saved } мл
        [few] Полости сделаны в { $count } моделях, сэкономлено { $saved } мл
       *[many] Полости сделаны в { $count } моделях, сэкономлено { $saved } мл
    }
hollow-cancelled = Создание полости отменено

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
orient-supports-ml = { $value } мл на поддержки
orient-pull = отрыв { $value } Н
orient-tall = высота { $value } мм

## prepare.rs

prepare-nothing-visible = На платформе нет видимых моделей, которые можно подготовить.
prepare-busy = Дождитесь, пока закончится текущая операция на платформе.
prepare-running-why = Дождитесь окончания подготовки к печати.
prepare-step-orient = Ориентация
prepare-step-repair = Починка
prepare-step-hollow = Создание полости
prepare-step-drain = Поиск дренажных отверстий
prepare-step-supports = Построение поддержек
prepare-step-cavity = Построение поддержек в полости
prepare-step-scan = Проверка дренажа
prepare-step-check = Проверка слоёв
prepare-step-arrange = Расстановка на платформе
prepare-step-save = Сохранение файла
prepare-step-pause = Ожидание действия
prepare-paused = Подготовка к печати приостановлена
prepare-check-unread = Не удалось проверить слои
prepare-save-no-printer-to-send = Нет принтера для отправки файла
prepare-cancelled = Подготовка к печати отменена, платформа возвращена как была
prepare-ended = Подготовка к печати остановлена, сделанные изменения сохранены
prepare-gate-held = Рецепт остановлен на этом шаге
prepare-step-cannot-run-yet = Шаг «{ $step }» здесь пока не работает
prepare-done =
    { $count ->
        [one] К печати подготовлена { $count } модель
        [few] К печати подготовлены { $count } модели
       *[many] К печати подготовлено { $count } моделей
    }

## prepare/recipes.rs

prepare-recipe-missing = рецепт { $recipe } не найден
prepare-recipe-cannot-run = рецепт «{ $recipe }» нельзя выполнить
recipe-kind-repair = Починка
recipe-kind-orient = Ориентация
recipe-kind-hollow = Полость
recipe-kind-drain = Дренаж
recipe-kind-supports = Поддержки
recipe-kind-check = Проверка
recipe-kind-arrange = Расстановка
recipe-kind-save = Сохранение
recipe-kind-gate = Условие остановки
recipe-kind-pause = Пауза

## relief.rs

relief-nothing-visible = На платформе нет видимых моделей, в которые можно вдавить текстуру.
relief-no-textures = Ни у одной из этих моделей нет текстуры. Выберите картинку, которую нужно замостить по ним, либо откройте OBJ с изображением рядом или 3MF с изображением внутри.
relief-nothing-to-press = на платформе нет модели с текстурой для вдавливания
relief-done =
    { $count ->
        [one] Вдавлено { $depth } мм рельефа в { $count } модель
        [few] Вдавлено { $depth } мм рельефа в { $count } модели
       *[many] Вдавлено { $depth } мм рельефа в { $count } моделей
    }
relief-done-coarsened =
    { $count ->
        [one] Вдавлено { $depth } мм рельефа в { $count } модель; { $coarsened } получились на более грубой сетке, чем задано «Точностью»
        [few] Вдавлено { $depth } мм рельефа в { $count } модели; { $coarsened } получились на более грубой сетке, чем задано «Точностью»
       *[many] Вдавлено { $depth } мм рельефа в { $count } моделей; { $coarsened } получились на более грубой сетке, чем задано «Точностью»
    }
relief-picked-picture = Картинка для вдавливания: { $name }

## panels/inspector/position.rs

position-global = Глобально
position-local = Локально
position-move = Перемещение
position-move-reset = В центр платформы
position-move-along = Сдвиг по { $axis }
position-rotation = Поворот
position-rotation-reset = Вернуть исходный поворот
position-rotation-about = Поворот вокруг { $axis }
position-scale = Масштаб
position-scale-reset = Вернуть исходный размер
position-size = Размер
position-ratio = Коэффициент
position-keep-proportions = Сохранять пропорции
position-axis-size = Размер по { $axis }
position-axis-ratio = Коэффициент по { $axis }
position-place = Размещение
position-on-plate = На платформе
position-centre = По центру
position-scale-to-fit = Подогнать по размеру
position-lift = Поднять над платформой
position-click-face = Щёлкните по грани
position-lay-on-face = Положить на грань
position-auto-orient = Автоориентация
position-limit-tilt = Ограничить наклон
position-limit-tilt-note = Поворот не дальше этого числа градусов от того, как модель стоит сейчас; 0 оставляет её как есть.
position-max-tilt = Наклон, макс.
position-clean-surface = Чистая поверхность
position-clean-surface-about = Убирает поддержки с того, что на виду, а затем ищет по пресету среди этих положений. Положение, где на виду ещё несколько касаний, но пресет выигрывает гораздо больше, предлагается рядом с выбранным. Если ничего не закрашено, видимое определяется по форме: открытые детализированные места.
position-paint-shown = Закрасить
position-paint-shown-clear = Очистить
orient-looking = Оценка вариантов положения
orient-proving = Пробная нарезка лучших вариантов
orient-cancelled = Автоориентация отменена
orient-preset-supports = Поддержки
orient-preset-peel = Отрыв
orient-preset-shortest = Время
orient-preset-auto = Авто
orient-preset-supports-hint = Меньше всего смолы в поддержках, в каком бы положении ни стояла модель.
orient-preset-peel-only-hint = Самое слабое усилие отрыва от плёнки; присоски и острова не учитываются.
orient-preset-shortest-only-hint = Наименьшее число слоёв; присоски и острова не учитываются.
orient-preset-auto-hint = Баланс поддержек, отрыва и времени печати: ничем не жертвуют ради остального.
position-mirror-across = Отразить по { $axis }
position-mirror-copy = Отражать копию
position-place-about = «На платформе» ставит модель на платформу; «По центру» переносит её к середине платформы; «Положить на грань» кладёт на платформу грань, по которой вы щёлкнете; «Отразить» переворачивает выбранные модели или их копии, поставленные рядом.

## panels/inspector/paint.rs

paint-brush = Кисть
paint-surface-angle = Угол поверхности
paint-inside = Внутри
paint-hint-point = Щелчок — поставить, Shift + щелчок — линия от последней, Alt + щелчок — убрать.
paint-hint-paint = Ведите, чтобы закрасить; Cmd + Shift + щелчок — вся поверхность; с Alt — стереть.
paint-hint-block = Ведите, чтобы запретить; Cmd + Shift + щелчок — вся поверхность; с Alt — стереть.
paint-hint-erase = Ведите, чтобы стереть разметку и запреты; Cmd + Shift + щелчок — вся поверхность.
paint-fill-help = Как держится залитая область: «Кайма» — по её краю, «Внутри» — по остальной площади. «Отступ» — насколько глубже начинается каждая часть: кайма от края, середина от каймы. Вторая кайма встаёт внутри первой — для больших плоских областей. Середина укладывается сеткой, кольцами вдоль каймы или вразброс. Каждую часть можно строить своим профилем или текущей группой. Меньше шаг — гуще и надёжнее, но следов больше.
paint-rim = Кайма
paint-line-spacing = Шаг в линии
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
        [one] Группа «{ $name }» удалена вместе с { $count } поддержкой
        [few] Группа «{ $name }» удалена вместе с { $count } поддержками
       *[many] Группа «{ $name }» удалена вместе с { $count } поддержками
    }

## panels/inspector/drain.rs

drain-hole-size = Размер отверстия
drain-trapped-resin = Запертая смола
drain-opening = Отверстие
drain-opening-note = Щёлкните по модели, её поддержкам или подложке, чтобы поставить отверстие, Alt + щелчок по отверстию убирает его. Каждое вырезается сразу при установке, полая модель или нет, а щелчок по дну отверстия сверлит дальше сквозь него.
drain-fact = заперто: { $count }
drain-fact-unchecked = Не проверено
drain-dig = Прорыть канал (точек: { $count })
drain-dig-tooltip = Под каждой точкой проходит трубка размером с отверстие, открытая только на двух концах.
drain-dug =
    { $count ->
        [one] Прорыт { $count } канал
        [few] Прорыто { $count } канала
       *[many] Прорыто { $count } каналов
    }
drain-check-again = Проверить снова
drain-check = Проверить дренаж
drain-check-tooltip = Нарезает платформу на слои и прослеживает путь воздуха, чтобы найти смолу без выхода.
drain-checking = Проверка дренажа
drain-cancelled = Проверка дренажа отменена
drain-auto = Авто
drain-auto-tooltip = Ищет дренажное и вентиляционное отверстия для каждого кармана запертой смолы в выбранных моделях и сверлит те, через которые, по повторной проверке, смола действительно вытекает.
drain-auto-looking = Поиск дренажных отверстий
drain-hole = Отверстие
drain-channel = Канал
drain-channel-note = Каждый щелчок добавляет точку; канал повторяет ширину и контур отверстия и проходит под всеми точками, открытый только на двух концах.
drain-sizes-note = Глубина отсчитывается от поверхности внутрь. Конусность меньше 1 делает отверстие конусом.
drain-round = Круглое
drain-polygon = Многоугольник
drain-sides = Стороны
drain-turn = Поворот
drain-polygon-note = Углы лежат на размере отверстия, поворот отсчитывается от горизонтали. В боковой стенке поверните его так, чтобы угол оказался сверху: острый свод печатается без провисания.
drain-plugs = Пробки
drain-plugs-note = Для каждого отверстия, как только оно вырезано, рядом с моделью на платформе появляется пробка: то, что отверстие вынуло из стенки, уже на зазор со всех сторон; её внешняя грань повторяет поверхность, поэтому закрывает отверстие заподлицо.
drain-plug-gap = Зазор на сторону
drain-plug-drift = Компенсация допусков у смолы выключена, поэтому пробка рисуется на { $drift } мм на сторону шире, чтобы после печати зазор был таким.
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
    [one] { $count } пробке не нашлось свободного места на платформе, она стоит там, где её положили, рядом с моделью.
    [few] { $count } пробкам не нашлось свободного места на платформе, они стоят там, где их положили, рядом с моделью.
   *[other] { $count } пробкам не нашлось свободного места на платформе, они стоят там, где их положили, рядом с моделью.
}
drain-none-cut = Ни одного отверстия и ни одного канала ещё не вырезано.
drain-clear-cuts =
    { $count ->
        [one] Убрать { $count } вырез
        [few] Убрать { $count } выреза
       *[many] Убрать { $count } вырезов
    }
drain-rehollowing = Повторное создание полости вокруг каналов и отверстий со стенкой
drain-nothing-trapped = Ничего не заперто: смола может вытечь из каждой модели на платформе.
drain-not-checked = Ещё не проверялось
drain-not-checked-why = Проверка находит полости, из которых смола не вытечет.
drain-held = { $volume } мл смолы не вытекут
drain-held-note = Она отвердеет внутри и добавит вес, который придётся отрывать. Каждый карман на рентгене закрашен красным, и каждому нужно собственное отверстие.
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
update-error-feed = не удалось прочитать источник обновлений
update-error-version = `{ $version }` — не номер релиза
update-error-foreign = источник указывает за пределы релизов этого проекта: { $url }
update-error-signature = скачанный файл не подписан ключом релиза Encrust
update-error-signed = подпись относится к `{ $found }`, а не к `{ $expected }`
update-error-archive = не удалось распаковать скачанный архив
update-error-missing = в скачанном архиве нет `{ $name }`
update-error-replace = не удалось заменить { $path }
update-error-cancelled = скачивание отменено
update-error-store = Microsoft Store не ответил
update-no-build = В этом релизе нет сборки для этой платформы.
update-cannot-check = не удалось проверить обновления
update-installed = Encrust { $version } установлен
update-cannot-install = не удалось установить Encrust { $version }
update-cannot-open-store = не удалось открыть Microsoft Store

## updates/install.rs

update-dev-build = Сборка для разработки обновляется пересборкой.
update-no-exe = Не удалось определить расположение запущенной программы.
update-by-store = Эта копия обновляется через Microsoft Store.
update-by-package = Эта копия обновляется скачиванием нового пакета.

## panels/settings/updates.rs

updates-by-store = Microsoft Store поддерживает эту копию в актуальном состоянии.
updates-store-link = Encrust в Microsoft Store
updates-daily = Проверять новый релиз раз в сутки
updates-privacy = Один запрос в сутки к GitHub за самым новым релизом. В нём только версия этой сборки, ничего больше, но GitHub видит ваш адрес. Ничего не скачивается и не устанавливается, пока вы не попросите.
updates-check-now = Проверить сейчас
updates-looking = Поиск нового релиза
updates-newest = Установлен последний релиз.
updates-installing = Скачивание и проверка подписи
updates-restart-now = Перезапустить сейчас
updates-in-store = Обновить в Microsoft Store
updates-install = Скачать и установить
updates-skip = Пропустить эту версию
updates-release-page = Страница релиза

## panels/start.rs

start-title = Новая платформа
start-setup = { $machine } · { $resin } · { $layer } мкм
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
start-gone = { $path } не найден
start-download = Скачать
start-downloading = Скачивается
start-download-write = не удалось записать { $path }
start-download-failed = «{ $name }» не скачан: { $error }
start-project = Проект

## panels/unreadable_profiles.rs

notice-unreadable =
    { $count ->
        [one] { $count } ваш профиль не читается и пропущен
        [few] { $count } ваших профиля не читаются и пропущены
       *[many] { $count } ваших профилей не читаются и пропущены
    }
notice-unreadable-file = { $path }: { $reason }
notice-reset-profiles = Сбросить к стандартным
notice-delete-profiles = Удалить их

## panels/stage_notice.rs

notice-cutting-preview = Нарезка слоёв для предпросмотра
notice-finding-walk = Поиск кратчайшего пути по поверхности
notice-drain-reading = Чтение модели для отверстий и пробок
notice-drain-drilling = Сверление отверстий
notice-drain-plugs = { $count ->
    [one] Вырезание { $count } пробки
    [few] Вырезание { $count } пробок
   *[other] Вырезание { $count } пробок
}
notice-drain-stopped = Остановлено: отверстия остаются, вырезанные приближённо, а новые остаются без пробок.
notice-preparing = Подготовка к печати: { $step }
notice-prepare-continue = Продолжить
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
notice-no-resin-why = Для нарезки нужно время экспозиции.
notice-no-printer-why = Для нарезки нужны экран и область печати принтера.
notice-set-up = Настроить
notice-put-away = Скрыть

## project.rs

project-save-and-close = Сохранить и закрыть
project-save-and-new = Сохранить и начать новый проект
project-close-without-saving = Закрыть без сохранения
project-new-without-saving = Начать новый проект без сохранения
project-cannot-open = не удалось открыть проект { $path }
project-opened-note = Открыто: { $path }; { $note }
project-cannot-draw = не удалось создать миниатюру проекта
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
section-height-of = { $height } из { $top } мм
section-type-layer = Введите номер слоя для перехода
section-layer-field = Слой { $layer }, введите номер слоя для перехода
section-go-to-issue = { $issue } на слое { $layer }: щёлкните, чтобы перейти
section-slider = Слой (всего { $count })
section-nothing-to-cut = На платформе пока нечего нарезать
section-layers-range = Слои с { $first } по { $layer }

## panels/settings/confirm.rs

confirm-cannot-undo = Это нельзя отменить.
confirm-click-again = Нажмите ещё раз, чтобы продолжить
confirm-asked-delete = Удалить?
confirm-asked-take-off = Убрать?
confirm-asked-stop = Остановить?
confirm-machine-stays = Принтер остаётся в библиотеке, поэтому его можно снова добавить со стандартными значениями этой версии.
confirm-untouched-resin = В неё ничего не вводилось, поэтому она не сохраняется в общем списке.
confirm-stays-in-pool = Она остаётся в общем списке, и этот или любой другой принтер может взять её обратно.
confirm-stays-measured = Она остаётся в общем списке, откалиброванная для: { $printers }.
confirm-comes-off = Она также будет убрана с: { $printers }.
confirm-reset-unreadable = Сбросить нечитаемые профили
confirm-delete-unreadable = Удалить нечитаемые профили
confirm-reset-named = Заменить ваши копии профилей { $names } стандартными из этой версии?
confirm-reset = Сбросить
confirm-reset-own-gone = Ваши собственные нечитаемые профили тоже будут удалены, потому что заменить их нечем: { $names }.
confirm-delete-own-named = Удалить { $names }? Это ваши собственные профили, они не читаются, и заменить их нечем.
confirm-delete = Удалить

## panels/report.rs

report-nothing-sent = Encrust пока в альфа-версии, и отчёты помогают исправлять ошибки. Отсюда ничего не отправляется: то, что вы видите ниже, остаётся на этом компьютере, пока вы сами его не передадите.
report-no-model = Ваша модель никогда не попадает в отчёт. Прикрепляйте её к обращению сами и только если вправе ею делиться: небольшая модель, на которой воспроизводится та же ошибка, подойдёт ещё лучше.
report-what-happened = Что произошло и чего вы ожидали
report-steps = Как воспроизвести
report-title = Сообщить о проблеме
report-include-profiles = Профили принтера и смолы
report-include-settings = Настройки панелей инструментов
report-include-plate = Сколько стоит на платформе
report-the-report = Отчёт
report-open-issue = Создать обращение
report-copied-for-issue = Отчёт в буфере обмена: вставьте его в обращение
report-copy = Копировать отчёт
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

defect-open = Поверхность не замкнута: рёбер без соседней грани — { $count }
defect-degenerate = граней без площади: { $count }
defect-duplicate = граней, нарисованных дважды: { $count }
note-folded = рёбер со сложенными друг на друга гранями (нарезаются как есть): { $count }
repaired-faces-removed = Починено: удалено вырожденных или повторяющихся граней — { $count }
repaired-flipped = Починено: развёрнуто граней — { $count }
repaired-inverted = Починено: вывернуто оболочек — { $count }
scene-plate-name = Платформа { $index }
scene-copy-numbered = { $name } (копия { $count })
# The scope-* messages are what a tool works on, set into the tooltips of the Hollow,
# Supports and Relief buttons as their { $scope }.
scope-selected-one = выбранную модель
scope-selected =
    { $count ->
        [one] { $count } выбранную модель
        [few] { $count } выбранные модели
       *[many] { $count } выбранных моделей
    }
scope-plate-one = единственную модель на платформе
scope-plate =
    { $count ->
        [one] { $count } модель на платформе
        [few] все { $count } модели на платформе
       *[many] все { $count } моделей на платформе
    }

## panels/scene_panel.rs

models-broken = Повреждена
models-meets = Касается
models-meets-names = Касается: { $names }. Нажмите, чтобы перейти к месту касания.
models-title = Модели
models-selected = Выбрано: { $count }
models-picked = выбрано: { $count }
models-project = · { $name }
models-arrange = Расставить на платформе
models-duplicate = Дублировать модели ({ $count })
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
sample-fit-about = Штифты под отверстие 6 мм — от −0,3 до +0,1 мм к его размеру, самый короткий — самый тонкий: проверьте, какие войдут.
sample-crustling = Crustling
sample-crustling-about = Маскот проекта — маленькая каменная черепаха шириной 30 мм, из панциря которой растут кристаллы. Коготки, острые грани кристаллов и щербатые пластины панциря покажут, какие мелкие детали удержат ваша смола и поддержки. Скачивается из репозитория проекта при первом нажатии, около 9 МБ.

## job.rs

job-rasterising = Растеризация
job-thread-stopped = поток нарезки остановился, не закончив
job-cancelling = Отмена…
job-progress = { $stage }: слой { $done } из { $total }
job-working = { $stage }…

## job/pipeline.rs

job-cannot-start-threads = не удалось запустить потоки нарезки
job-cannot-slice = не удалось нарезать модель
job-cannot-measure = не удалось измерить набор слоёв

## panels/inspector.rs

inspector-about-select = Выберите модель, затем перемещайте, поворачивайте, масштабируйте, кладите на грань и отражайте её.
inspector-about-measure = Щёлкните по двум точкам или двум плоским граням модели.
inspector-about-hollow = Вырежьте полость, чтобы сэкономить смолу и облегчить отрыв.
inspector-about-drain = Дайте смоле вытечь из полой модели.
inspector-about-cut = Разделите модель плоскостью.
inspector-about-relief = Вдавите текстуру в поверхность модели, напишите на ней текст или объедините модели в одну.
inspector-about-supports = Поддержите то, что нависает над пустотой.
inspector-about-repair = Что не так с поверхностью моделей и как её починить: закрыть, развернуть или обернуть.
inspector-about-check = Что может не напечататься и на каком слое.
inspector-about-print-settings = Как эта платформа нарезается и экспонируется. Значения берутся из настроек принтера и смолы, а изменения здесь действуют только для этой платформы: при открытии проекта, создании нового, добавлении платформы или перезапуске они снова берутся из профилей. Чтобы изменить исходные значения для всех платформ, измените принтер или смолу в настройках.

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
preview-cannot-rasterise = не удалось растеризовать слой на z = { $z } мм

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
repair-kept = { $name } не удалось починить, не повредив ещё сильнее, поэтому модель оставлена как есть
repair-nothing = в { $name } больше нечего чинить
repair-no-patch = у { $name } границ без подходящей заплатки: { $count }
repair-done = { $name } — починено: { $what }
repair-done-open = { $name } — починено: { $what }; не удалось закрыть границ: { $count }
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
        [one] отброшена { $count } соринка без объёма
        [few] отброшено { $count } соринки без объёма
       *[many] отброшено { $count } соринок без объёма
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
repair-listing = Поиск оболочек
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
repair-pick-invert = Инвертировать выбор
repair-pick = Выбор
repair-pick-faces = Грани
repair-pick-edges = Рёбра
repair-pick-faces-note = Проведите кистью по модели, чтобы выбрать грани, с Alt — чтобы снять выбор; Cmd и Shift вместе выбирают всю поверхность, пока её изгиб не превышает «Угол поверхности». «Развернуть» переворачивает грани, «Удалить» убирает их.
repair-pick-edges-note = Щёлкните по двум открытым рёбрам вокруг дыр, и «Соединить» положит между ними грани с той же ориентацией, что у соседних. Третий щелчок снимает выбор с первого.
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
repair-finding-edges = Поиск открытых рёбер
repair-clear-picks = Снять выбор
repair-flip-faces = Развернуть
repair-delete-faces = Удалить
repair-bridge = Соединить
repair-pick-faces-first = Сначала выберите грани кистью.
repair-pick-two-edges = Сначала выберите два открытых ребра.
repair-editing = Изменение граней
edit-editing = Изменение граней: { $name }
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
repair-found-note = Что импорт нашёл в моделях, с которыми работает инструмент: выбранных или всех на платформе. Нажатие на строку отмечает найденное на моделях и подводит камеру к первому месту, каждое следующее — к следующему, а нажатие после последнего убирает отметки. Пересекающиеся грани подсчитываются кнопкой с лупой; на большой модели это занимает время.
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
repair-method-deep-note = Чинит, оборачивает и чинит снова — для модели, которую другие методы оставляют повреждённой. Как и при обёртке, деталь целиком внутри другой пропадает в ней.
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

mask-pick-machine = Выберите принтер: без него неизвестно, под какой экран рисовать слои.
mask-no-stack = Набора слоёв пока нет.
mask-pixels = { $width } × { $height } пикс.
zoom-out = Уменьшить
zoom-in = Увеличить
zoom-whole = Показать целиком (или двойной щелчок)
mask-cannot-draw = не удалось нарисовать слой
mask-cannot-draw-full = не удалось нарисовать слой целиком

## network/errands.rs

errand-pausing = Приостановка
errand-resuming = Возобновление
errand-stopping = Остановка
errand-camera-on = Включение камеры
errand-camera-off = Выключение камеры
errand-no-answer = команда завершилась без ответа

## app.rs

app-cannot-build-preview = не удалось построить предпросмотр слоя

## job/hollow.rs

job-thread-stopped-hollowing = поток создания полости остановился, не закончив
job-hollow-stopping = Остановка…
job-hollow-progress = Создание полости: модель { $index } из { $total }
job-step-preparing = Проверка модели
job-step-seeding = Измерение поверхности
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
job-hollow-seeding = Измерение поверхности
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

job-thread-stopped-measuring = поток измерения остановился, не закончив

## job/orient.rs

job-thread-stopped-orientation = поток ориентации остановился, не закончив
job-orient-none = поиск не нашёл ни одной ориентации

## job/place.rs

job-thread-stopped-support = поток поддержек остановился, не закончив
job-placing = Расстановка поддержек, { $percent } %
printer-printing-file = { $state }, { $file }, { $percent }%
job-cutting-plate = Нарезка платформы…

## job/preview.rs

job-thread-stopped-preview = поток предпросмотра остановился, не закончив

## job/relief.rs

job-thread-stopped-relief = поток рельефа остановился, не закончив
job-relief-no-texture = у модели нет текстуры для вдавливания

## job/send.rs

job-thread-stopped-network = сетевой поток остановился, не закончив
job-send-starting = Запуск печати на { $printer }
job-send-connecting = Подключение к { $printer }

## job/drains.rs

job-thread-stopped-drain-search = поток поиска дренажных отверстий остановился, не закончив

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
arrange-cleared =
    { $count ->
        [one] Отодвинута { $count } модель
        [few] Отодвинуты { $count } модели
       *[many] Отодвинуто { $count } моделей
    }
arrange-turned =
    { $count ->
        [one] Повёрнута { $count } модель, чтобы она поместилась на платформе
        [few] Повёрнуты { $count } модели, чтобы они поместились на платформе
       *[many] Повёрнуто { $count } моделей, чтобы они поместились на платформе
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

## drain/auto.rs

drain-auto-cancelled = Поиск дренажных отверстий отменён
drain-auto-drilled = 
    { $count ->
        [0] Отверстия не просверлены
        [one] Просверлено { $count } отверстие
        [few] Просверлено { $count } отверстия
       *[many] Просверлено { $count } отверстий
    }
drain-auto-trapped = 
    { $count ->
        [one] Для { $count } кармана не нашлось отверстия для слива, он остаётся красным.
       *[many] Для { $count } карманов не нашлось отверстия для слива, они остаются красными.
    }
drain-auto-moved = 
    { $count ->
        [one] { $count } модель изменилась во время поиска и осталась без отверстий.
        [few] { $count } модели изменились во время поиска и остались без отверстий.
       *[many] { $count } моделей изменились во время поиска и остались без отверстий.
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

history-none = История печати пуста.
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

web-no-storage = в этом браузере нет личного хранилища файлов

## profiles.rs

picker-no-machine = Принтеров пока нет. Добавьте принтер в окне «Принтер и смола».
picker-no-resin = Для этого принтера пока нет смол. Добавьте их в окне «Принтер и смола».
picker-your-machines = Ваши принтеры
picker-machine-detail = { $maker } · .{ $extension }
picker-add-machine = Добавить принтер
picker-on-machine = На { $name }
picker-resin-detail = { $kind } · { $layer } мкм
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
hint-paint-shown = закрасить видимое
hint-erase = стереть
hint-measure-two-points = по двум точкам
hint-measure-second-point = по второй точке
hint-measure-two-faces = по двум плоским граням
hint-measure-second-face = по второй плоской грани
hint-measure-freely = измерять без привязки
hint-drain-hole = по модели, чтобы прорезать отверстие
hint-drain-channel = по модели, чтобы добавить точку канала
hint-relief-write = по модели, чтобы сделать надпись
hint-pick-without-writing = выбрать, не создавая надпись
hint-supports-pick = по части поддержки, чтобы выбрать её
hint-supports-carry = перенести выбранное
hint-supports-level = перенести по платформе
hint-supports-add = добавить к выбранному
hint-hollow-solid = по модели, чтобы оставить там сплошную стенку
hint-hollow-rod = стержень от последнего блокиратора
hint-area-paint = где нужны поддержки
hint-area-block = где поддержек быть не должно
hint-repair-faces = выбрать грани
hint-repair-edge = по открытому ребру, чтобы выбрать его
hint-let-go = отпустить
hint-supports-place = по модели, чтобы поставить поддержку
hint-supports-line = поставить их линией
hint-cut-part = по части, чтобы отметить её
hint-gizmo-steps = за ручку шагами по 1 мм и 15°

## panels/send_file.rs

send-file-not-sliced = { $file } нельзя отправить: это не нарезанный файл
