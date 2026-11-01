# English: the source

`en.ftl` is what every language translates, so it is written to be translated: a message
that reads well only in English costs each language a rewrite. The voice and names of the
window are `../ui-standards.md` (Voice, Names, Length); this page is what the source owes
the translations. How a catalogue works: [`../i18n.md`](../i18n.md).

## One meaning, one word

- One term a concept, the same in every message: the plate, a model, the printer, a
  support, its tip, foot and trunk, the raft, the peel, slice, hollow, repair. A new term
  goes into the English column of every `<code>-glossary.md` in the change that adds it.
- No synonyms for variety: "drop", "remove", "delete" and "take off" are four actions to a
  translator. Pick the one the window already uses for that act.
- A word that is both verb and noun or adjective (Open, Update, Set, Print, Cut) is two
  messages when it plays two roles, so each language can choose.

## Sentences a translator can turn

- Plain words, no idioms, puns, slang or cultural jokes; nothing that only works in English
  word order ("Grow them again", "the group in hand" read as riddles).
- A whole sentence a message, the objects it names as arguments: `Cut { $name } at …`,
  never "Cut " + name. The translation may move the argument, change its case or drop it.
- An argument stands where a noun stands, never as a verb, an article or an ending. Do not
  put quotes, articles or plural endings round it in English: each language adds its own.
- A count is a `$count` with a plural selector, `[one]` and `*[other]` at least; a variant
  with no number in it ("yesterday") selects on `[1]`, not `[one]`.
- A unit is `unit-*` or `value-*`, never typed into a sentence; numbers arrive formatted.
- A message whose use its words do not show — a fragment set into another, a leading
  space, a word in a tight button — gets a `#` comment above it in `en.ftl`.

## Form

- Sentence case everywhere; names and buttons without a final stop, sentences with one.
- Buttons and menu items are verbs ("Save the project"), tools and sections nouns
  ("Supports"), progress a present participle ("Slicing"), a result what happened
  ("Saved the support profile"), an error a lower-case clause the window prefixes
  ("cannot open { $path }").
- An ellipsis ends a menu item that asks for more before it acts, and a progress line still
  running; each language writes it its own way.
- Short is a design input, not a target: translations run up to 40 % longer
  (`../i18n.md`, Room for every language).
