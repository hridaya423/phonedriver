# Spike: setting date and time pickers (2026-11-24)

Question: on the API 37 emulator, can the standard date and time pickers be set to a given value and the value read back, reliably enough to build on?

Rig: Mac mini to the win-pc emulator `Pixel_API_37` (Android 17, English). The testbed got a `pickers` screen (`testbeds/android`, `Pickers.java`, `ComposePickers.kt`, `assets/pickers.html`; open it with `am start -n dev.phonedriver.testbed/.MainActivity --es tab pickers`) with one button for each kind, each opened on 2026-01-15 or 09:05, and each logs `{"ev":"picked","field":…,"value":…}` when confirmed. The Material (Views) time picker was taken from the Clock app's `Add alarm`. Every value below was compared with that log line, not with phonedriver's reply.

## What each picker gives the accessibility tree

| Picker | What the tree shows |
|---|---|
| Platform `DatePickerDialog`, calendar (also a web page's `<input type=date>`) | year `2026`, `Thu, Jan 15`, `Previous month`, `Next month`, one button a day labelled `15 January 2026` with `selected` on the chosen one, `CANCEL`, `OK` (`CLEAR`, `SET` for a web page) |
| Platform `DatePickerDialog`, wheels (Holo theme) | three wheels, each a text field with the value between two buttons: `Dec` `Jan` `Feb`, `14` `15` `16`, `2025` `2026` `2027` |
| Platform `TimePickerDialog`, clock (also `<input type=time>`) | `9`, `05`, radios `AM` `PM` (none in 24-hour), the hours as buttons, `Switch to text input mode for the time input.`; after it: two text fields (hour, minute), a menu `AM`, `Switch to clock mode for the time input.` |
| Platform `TimePickerDialog`, wheels | three wheels like the date's: `8` `9` `10`, `04` `05` `06`, `AM` `PM` |
| Compose Material 3 `DatePicker` | `Current selection: Thursday, January 15, 2026`, `Change to previous month`, `Change to next month`, a text a day labelled `Thursday, January 15, 2026` with on or off, `Switch to text input mode` (a field `Date, MM/DD/YYYY`) |
| Compose Material 3 `TimePicker` | `9 o'clock`, `Select minutes` `5 minutes`, buttons `AM` `PM`, the hours as buttons `7 o'clock`; after an hour is tapped, minutes in fives as buttons `45 minutes` |
| Material Components time picker (Clock) | `9 o'clock`, `0 minutes`, radios `AM` `PM`, `Switch to text input mode for the time input.`; after it: a text field for the hour, a button `Minute 0 minutes` that turns into the field when tapped |

## Setting them by hand with the CLI

| Picker | Steps | Result (app's log) |
|---|---|---|
| Platform date, calendar | `Next month` or `Previous month` until the month shows, tap the day, check `selected`, `OK` | 2026-12-18, 2025-11-03: right, 2 of 2 |
| Web date | the same, `SET` | 2026-03-09: right |
| Platform time, 12-hour, text input | type the hour and the minute into the two fields, choose `AM` or `PM` from the menu, `OK` | 19:45, 00:30, 22:05, 01:00, 23:59: right, 5 of 5 |
| Platform time, 24-hour | the same, no menu | 06:07, 23:59, 00:00: right, 3 of 3 |
| Web time | the same, `SET` | 19:45, 00:30, 03:07: right when the fields were read again between the two texts; 00:05 for 00:30 once, when the minute was typed straight after the hour (the dialog moved as the keyboard came up; the reply said `NOT verified: @e585 shows nothing`) |
| Compose date | `Change to next month` until the month shows, tap the day, check on, `OK` | 2026-12-18: right |
| Compose time | tap `7 o'clock`, tap `45 minutes`, `OK` | 07:05 for 07:45: the tap on `45 minutes` was reported as a tap and changed nothing. Not reliable |
| Clock's time picker | text input, type the hour, tap `Minute 0 minutes`, type the minute, tap `PM` | chips read `Hour 7 o'clock`, `Minute 45 minutes`, `PM` on: right |
| Wheels (date and time) | not set: the fields take text only after a tap puts the wheel in edit mode, and no step was found that the tree confirms | not built |

## What was built from it

`tap` with `select` (decision D122) sets a calendar date picker by stepping months and tapping the dated day, and a time picker through its text input, reads the value back from the picker, and only then taps `OK` or `SET`. The pickers it does not know (wheels, the Compose clock, any picker in another language than English) are refused with the screen and left open.

The first live run of the built code changed it twice. Compose heads its calendar with `Current selection: Thursday, January 15, 2026`, which the code read as the first day cell: it tapped the heading in place of the day and thought the month never changed. A day cell is now a label that says a date and nothing else. A wheel time picker has text fields too, so the code typed an hour into a wheel before it saw there was no minute field. A time picker is now one that shows `Switch to clock mode` after `Switch to text input mode` is tapped.

## Timings (debug build, the emulator's software renderer)

A month step is one settled tap, about 0.9 s: 2026-01 to 2026-12 took 11 to 18 s, a date in the shown month 3.5 s, a time 5 to 9 s. The same by hand is one call a step, each with a model turn.
