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

# Second spike: what the first one refused (2026-11-25)

Question: can wheels, the Compose clock face, Material Components pickers, pickers in other languages, dates decades away and iOS pickers be set and read back, reliably enough to build on? Each was tried on a device and compared with the app's `picked` log line before any of it was kept (decisions D125, D126).

Rig: as above for Android, the testbed built under the id `dev.phonedriver.testbed.h1` with a new `MaterialPickers` activity (`MaterialDatePicker`, `MaterialTimePicker` in 12 and 24 hours and in keyboard mode) and a 24-hour Compose clock. For iOS, the Mac mini's Simulator `Bench iPhone 17` (iOS 26.5) and a new `pickers` screen of the iOS testbed (`-tab pickers`). A language was set for the Android testbed alone with `cmd locale set-app-locales`, so the emulator's own language never changed; the Simulator's language was set with its `AppleLanguages` and `AppleLocale` defaults and put back.

## What was found

| Picker | Finding | What is built |
|---|---|---|
| Android wheels | a wheel's field (`numberpicker_input`) takes typed text with submit; typing `Dec` over `Jan` turns the wheel back across the year's end and the year wheel follows (2030-12-31 confirmed for 2031-12-31 in the first run) | type each wheel, then read all of them in one snapshot and retype what is wrong, three rounds at most |
| Compose clock face | the taps of the first spike arrived while the face was turning from hours to minutes (the tree shows `1 minutes` to `12 minutes` for about 300 ms); a tap on a settled mark works; a tap between marks snaps to a five | wait for the minute marks; drag from the mark 45 minutes before to the minute's place and correct by half, then a quarter, of what the selector reads back |
| Material Components date | `month_navigation_next`, `month_navigation_previous`, `month_navigation_fragment_toggle` (the list of years), `confirm_button`; days as texts with the full date | month steps by id, the year list for far dates |
| Material Components time | `material_timepicker_mode_button`, chips for hour and minute, `material_clock_period_am_button`, `material_timepicker_ok_button`; the field under a chip reads `Hour` until focused | text input mode, type, read the chips after focus has left each |
| Lists of years | the platform's opens from `date_picker_header_year`, Compose's and Material's from the button that shows the month and year; scrolling the list by a year row did nothing, scrolling the screen moved it | open, scroll a page at a time until the year shows, tap it |
| Other languages | every label changes (`Nächster Monat`, `2026年1月`, `الساعة ١٢`); ids do not; Japanese date wheels show the month as a number and its twelve-hour clock counts from 0; Arabic time fields empty themselves when given `0` to `9`; `SimpleDateFormat` in the app's locale gives the month names the pickers show | names from the device, controls by id or by place |
| iOS compact date | a popover with `DatePicker.NextMonth`, `DatePicker.PreviousMonth`, `DatePicker.Show` (month and year wheels), days without a year, `PopoverDismissRegion`; a tap in the middle of the region turned a wheel behind it | month steps by id, wheels for far dates, close at a free point of the region's left edge |
| iOS wheels | a touch one row (34 pt) from the middle moves the wheel by one and the tree shows it; Arabic spells minute 1 and 2 in words | step a row at a time the shorter way, read each step, step blind from the last readable value when one is in words |

## Results against the app's log

Android, English (five runs; a failure was fixed with a failing test first and the kind rerun): platform date near and far (2041-02-28, 1990-05-01 in 13 s), date wheels 2031-12-31, 2026-01-31, 2026-01-01, 2028-02-29, 1999-12-01, time wheels 12:00, 23:59, 00:00, 11:00, 19:45, platform time 19:45 and 24-hour 06:07, Compose date near and far, the Compose clock in 12 and 24 hours with 17 of 17 odd minutes, Material date 2026-12-18, 1990-05-01, 2027-03-31, Material time 19:45, 24-hour 06:07, typed 00:30.

Android, German, Japanese and Arabic: 23 cases a language over every kind: the first run passed 40 of 51 and refused the other 11, none set wrongly. Each refusal became a failing test and a fix: Compose's button to its text input was tried as a month arrow and not found again after it moved (German); Compose writes `Nachmittags` or `PM` where the device says `PM` or `午後`; Japanese date wheels show the month as a number; Japanese twelve-hour wheels and fields count from 0; Compose's year button reads `2026年1月`; Arabic time fields take only Arabic-Indic digits. The second run found the one false success of the work: in Japanese the platform's time fields showed the 12 typed for 00:30, `OK` left the picker open under its error text, and the reply said `set`. A picker is now held to having closed. After the fixes: German 23 of 23; Japanese 21 of 23 and then the two that had failed (00:30, 12:05) with 19:45 again; Arabic 18 before the emulator lost its agent and then the five it had not reached. The emulator died twice under other builds on its host, so no single run shows all 69 green and the last English check ended at 4 of 7 with the other three lost to a disconnect.

iOS Simulator: `en_GB` 9 of 9, `en_US` 7 of 7, `ja_JP` 7 of 7, `ar_EG` 7 of 7, `de_DE` 7 of 7 (the calendar near and in 1990, the date wheel to 2028-02-29, the time wheel to 19:45 and 00:07, the compact time to 23:59 and 12:30).

## Timings (debug build)

Android, on a quiet host: a far date 11 to 15 s, a time 4 to 10 s. In the language runs, with other builds on the emulator's disk, everything took about twice that (a far date 17 to 28 s, wheels 13 to 23 s). iOS: a date in the shown month 4 s, 1990 from 2026 17 to 24 s (one touch a year), a time wheel 5 to 12 s.

The spike scripts ran from `~/pd-h1-spike` on the Mac mini and are not in the repo.

## Addendum, 2026-11-26: the Clock app's picker lists a field over its chip

Read on the API 37 emulator with `snapshot --json` after `Switch to text input mode for the time input.` on the Clock app's `Add alarm`: `button "Hour 9 o'clock"` (checkable, checked) and a text field with the value `09` and the hint `Hour`, both at 65.5, 260.6, 96 by 80; `button "Minute 0 minutes"` (checkable) alone at 185.5; `material_clock_period_am_button`, `material_clock_period_pm_button`, `material_timepicker_mode_button`, `material_timepicker_ok_button`. After the minute chip is tapped the field is over the minute and the hour is a chip alone. `select` took the chip for the part and failed (D135). The picker also opens in the mode it was last left in, with the keyboard up when that was the text input.
