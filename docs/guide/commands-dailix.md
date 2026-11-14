# Commands: Dailix

*Made by `dailix commands`: the same text as `--help`. Don't edit it by hand.*

Bringing footage in and choosing it

## `dailix open`

The window, on the project at this folder

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |

## `dailix new-trip`

A new trip in the Trips place, in the fixed layout. With a title it's made and its folder printed; without, the window opens on the form

| Argument | What it is |
|---|---|
| `<TITLE>` |  |
| `--folder <FOLDER>` | Its folder's name (default: the month and the title in Latin) |
| `--description <DESCRIPTION>` |  |
| `--tags <TAGS>` | Comma-separated |

## `dailix add`

Videos added to a trip, each under the day it was filmed (its name's date, else the file's), whole or a span: `FILE`, `FILE@all`, `FILE@0:05-0:20`. The originals are only read

| Argument | What it is |
|---|---|
| `<TRIP>` |  |
| `<FILES>` |  |
| `--day <DAY>` | The day to put them under instead (YYYY-MM-DD) |

## `dailix sheet`

The contact sheet's marking (a project's `dailix/sheet.json`)

## `dailix sheet apply`

Applies ops (a JSON op or a list of them; `-` reads them from stdin) to a sheet: all or none, checked, written. A line per op: what it made, or why the whole run was refused

| Argument | What it is |
|---|---|
| `<NOTES>` |  |
| `<OPS>` |  |

## `dailix sheet check`

What's wrong with a sheet, a line each; nothing when it's sound

| Argument | What it is |
|---|---|
| `<NOTES>` |  |

## `dailix sheet describe`

A sheet as text: a line per day, rated clip and mark, what an AI reads in place of the JSON

| Argument | What it is |
|---|---|
| `<NOTES>` |  |
| `--day <DAY>` | One day only (YYYY-MM-DD) |

## `dailix sheet schema`

The JSON Schema of the sheet's file, or of the ops

| Argument | What it is |
|---|---|
| `<OF>` | (default: ops) (one of: ops, sheet) |

## `dailix voice`

Clips' cleaned voices: what's made, making one, measuring their loudness

## `dailix voice list`

A line per clip with voices: each one made, its loudness, the one the edit uses (*), and the recipes there are

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |

## `dailix voice make`

Clip `&lt;day>/&lt;file>`'s voice made by a recipe (dnn6 unless asked), from its original; made again if it's there

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `<CLIP>` |  |
| `--recipe <RECIPE>` | (default: dnn6) |

## `dailix voice measure`

The loudness of every voice not measured yet (made by footage-agent)

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |

## `dailix voice speech`

Where clip `&lt;day>/&lt;file>` holds speech, found in its cleaned voice: a line per stretch, seconds

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `<CLIP>` |  |

## `dailix voice use`

Clip `&lt;day>/&lt;file>`'s edit to use its voice made by RECIPE, or its own sound without one (the pane's «Use for the edit»)

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `<CLIP>` |  |
| `<RECIPE>` |  |

## `dailix places`

The three places (README): shows them, or sets those given

| Argument | What it is |
|---|---|
| `--tmp <TMP>` |  |
| `--projects <PROJECTS>` |  |
| `--sources <SOURCES>` | The originals' folder (a local, removable or mounted network one) |

## `dailix import-legacy`

A footage-agent project made into a Dailix one, in the Trips place; the old one and the archive are only read

| Argument | What it is |
|---|---|
| `<OLD>` |  |
| `--name <NAME>` | The new project's folder name (default: the old one's) |
| `--dry-run` | Say what it would copy, and copy nothing |

## `dailix install`

The icon and a desktop entry, in the menu and on the desktop (Linux)

## `dailix keys`

Every key and gesture of the window, as Markdown tables (the guide's reference page, the F1 list)

## `dailix commands`

Every command and its arguments, as Markdown (the guide's reference page; the same text as --help)

## `dailix trips`

The trips in the Trips place (or in the folder given) as the home view sums them up: a line per trip and per day

| Argument | What it is |
|---|---|
| `<PROJECTS>` |  |

## `dailix title`

A project's name as people say it: shows it, or sets it (an empty one gives back the folder's name). The folder isn't renamed

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `<TITLE>` |  |

## `dailix about`

A project's description and tags (as the home view edits them): shows them, or sets those given; an empty one is taken out

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--description <DESCRIPTION>` |  |
| `--tags <TAGS>` | Comma-separated |

## `dailix tidy-edits`

The edits lying loose in a project's old 04_project, each moved into a video of its own (videos/&lt;name>/edit.json); copies and empty ones to the trash, other files left. Says what it would do; does it with --apply

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--apply` |  |
