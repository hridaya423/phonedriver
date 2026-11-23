//! What `shell` does around the device's own shell: trimming output for an agent's context, recognising commands that
//! only read, and refusing the few that would hurt a person's physical device (decision D21).

pub const DEFAULT_HEAD: usize = 40;
const LINE_CHARS: usize = 300;

pub struct Tidy {
    pub text: String,
    pub omitted: usize,
}

fn clean(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(n) = chars.next() {
                    if n == '\u{7}' {
                        break;
                    }
                    if n == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn lines_of(raw: &str) -> Vec<String> {
    let cleaned = clean(raw);
    let mut lines: Vec<String> = cleaned
        .split('\n')
        .map(|l| {
            let l = l.trim_end_matches('\r');
            let l = l.rsplit('\r').next().unwrap_or(l);
            let l = l.trim_end();
            if l.chars().count() > LINE_CHARS {
                let cut: String = l.chars().take(LINE_CHARS).collect();
                format!("{cut}…")
            } else {
                l.to_string()
            }
        })
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

pub fn matcher(grep: &str) -> impl Fn(&str) -> bool {
    let alternatives: Vec<String> = grep
        .split('|')
        .filter(|a| !a.is_empty())
        .map(str::to_lowercase)
        .collect();
    move |line: &str| {
        let line = line.to_lowercase();
        alternatives.iter().any(|a| line.contains(a.as_str()))
    }
}

/// Standard output, then standard error under a `stderr:` line, with ANSI codes and carriage returns removed, only
/// the lines containing one of the `|`-separated words of `grep` (ignoring case), and at most `head` lines
/// (0 keeps all). `omitted` counts the lines that did not fit.
pub fn tidy(stdout: &str, stderr: &str, grep: Option<&str>, head: usize) -> Tidy {
    let mut out = lines_of(stdout);
    let mut err = lines_of(stderr);
    let mut note = None;
    if let Some(g) = grep.filter(|g| !g.is_empty()) {
        let keep = matcher(g);
        let total = out.len() + err.len();
        out.retain(|l| keep(l));
        err.retain(|l| keep(l));
        if out.is_empty() && err.is_empty() && total > 0 {
            note = Some(format!("no line of {total} matches \"{g}\""));
        }
    }
    let mut all = out;
    if !err.is_empty() {
        all.push("stderr:".into());
        all.extend(err);
    }
    let mut omitted = 0;
    if head > 0 && all.len() > head {
        omitted = all.len() - head;
        all.truncate(head);
    }
    let mut text = all.join("\n");
    if omitted > 0 {
        text.push_str(&format!(
            "\n… {omitted} more lines (narrow it with grep, or set head to 0 for all)"
        ));
    }
    if let Some(n) = note {
        text = n;
    }
    Tidy { text, omitted }
}

fn words(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|w| w.trim_matches(['\'', '"', '(', ')', '`']).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn file_name(word: &str) -> &str {
    word.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(word)
}

fn segments(command: &str) -> Vec<Vec<String>> {
    let spaced = command
        .replace("$(", "\n")
        .replace(['&', '|', ';', '`'], "\n");
    spaced
        .lines()
        .map(words)
        .filter(|w| !w.is_empty())
        .map(|mut w| {
            const WRAPPERS: &[&str] = &[
                "su", "sudo", "0", "2000", "root", "-c", "env", "nohup", "exec", "time", "busybox",
                "toybox", "sh", "bash", "xargs", "eval", "command", "nice",
            ];
            let skip = w
                .iter()
                .take_while(|t| WRAPPERS.contains(&file_name(t)))
                .count();
            w.drain(..skip);
            if let Some(word) = w.first_mut() {
                *word = file_name(word).to_string();
            }
            w
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// True when every part of the command only reads state, so the screen cannot have changed and `shell` skips the
/// settle wait.
pub fn only_reads(command: &str) -> bool {
    if command.contains('>') {
        return false;
    }
    let parts = segments(command);
    !parts.is_empty()
        && parts.iter().all(|w| {
            let second = w.get(1).map(String::as_str);
            match w[0].as_str() {
                "echo" | "cat" | "ls" | "getprop" | "dumpsys" | "ps" | "id" | "whoami"
                | "uname" | "df" | "date" | "grep" | "head" | "tail" | "wc" | "stat" | "du"
                | "uptime" | "printenv" | "readlink" | "pidof" | "sort" | "cut" | "true"
                | "sleep" => true,
                "find" => !w
                    .iter()
                    .any(|t| t == "-delete" || FIND_RUNS.contains(&t.as_str())),
                "defaults" => second == Some("read"),
                "settings" => matches!(second, Some("get" | "list")),
                "pm" => matches!(
                    second,
                    Some("list" | "path" | "dump" | "resolve-activity" | "get-install-location")
                ),
                "logcat" => w.iter().any(|t| t == "-d"),
                "wm" => w.len() == 2 && matches!(second, Some("size" | "density")),
                _ => false,
            }
        })
}

macro_rules! refuse {
    ($why:literal) => {
        Some(concat!(
            "refused on a physical device: ",
            $why,
            ". The device belongs to a person; pass force (--force) only if the user asked for exactly this."
        ))
    };
}

const SDCARD_ROOTS: &[&str] = &[
    "",
    "/sdcard",
    "/storage",
    "/storage/emulated",
    "/storage/emulated/0",
    "/storage/self/primary",
    "/mnt/sdcard",
    "/data",
    "/data/data",
    "/data/media",
    "/data/media/0",
    "/data/user",
    "/data/user/0",
];

/// The `find` expressions that run another program on what it found.
const FIND_RUNS: &[&str] = &["-exec", "-execdir", "-ok", "-okdir"];

/// The input sources `input` takes before its command (`input touchscreen tap …`).
const INPUT_SOURCES: &[&str] = &[
    "keyboard",
    "mouse",
    "joystick",
    "touchnavigation",
    "touchpad",
    "trackball",
    "stylus",
    "dpad",
    "gesture",
    "touchscreen",
    "gamepad",
    "rotaryencoder",
];

/// The keys `press` and `button` send (and DPAD_CENTER and NUMPAD_ENTER, which confirm a focused button as Enter
/// does), as Android names them and by number.
const GUARDED_KEYS: &[(&str, &str)] = &[
    ("HOME", "3"),
    ("BACK", "4"),
    ("DPAD_UP", "19"),
    ("DPAD_DOWN", "20"),
    ("DPAD_LEFT", "21"),
    ("DPAD_RIGHT", "22"),
    ("DPAD_CENTER", "23"),
    ("VOLUME_UP", "24"),
    ("VOLUME_DOWN", "25"),
    ("TAB", "61"),
    ("ENTER", "66"),
    ("DEL", "67"),
    ("ESCAPE", "111"),
    ("NUMPAD_ENTER", "160"),
    ("APP_SWITCH", "187"),
    ("SLEEP", "223"),
];

/// Whether an `input` command does what tap, type, swipe, press or button would do with the approval guard
/// watching. A key those cannot send (media keys, the camera) has no guarded route, so it runs.
fn input_has_a_guarded_route(args: &[&str]) -> bool {
    let mut args = args.iter();
    let mut command = None;
    while let Some(a) = args.next() {
        if matches!(*a, "-d" | "--display") {
            args.next();
        } else if !a.starts_with('-') && !INPUT_SOURCES.contains(a) {
            command = Some(*a);
            break;
        }
    }
    match command {
        None => false,
        Some("keyevent" | "keycombination") => args.filter(|a| !a.starts_with('-')).any(|k| {
            let name = k.strip_prefix("KEYCODE_").unwrap_or(k);
            GUARDED_KEYS
                .iter()
                .any(|(n, code)| name == *n || name == *code)
        }),
        Some(_) => true,
    }
}

/// Why a physical device should not run this command, or `None`. `mine` are the packages phonedriver installed there
/// and nobody has installed again since.
pub fn refuse_on_physical(command: &str, mine: &[String]) -> Option<&'static str> {
    if ["MASTER_CLEAR", "FACTORY_RESET", "--wipe_data"]
        .iter()
        .any(|m| command.contains(m))
    {
        return refuse!("it wipes the device (factory reset)");
    }
    for w in segments(command) {
        let rest: Vec<&str> = w[1..].iter().map(String::as_str).collect();
        match w[0].as_str() {
            "input" if input_has_a_guarded_route(&rest) => {
                return refuse!(
                    "`input` sends touches and keys the approval guard does not see; tap, type, swipe, press and button do the same and check what they touch"
                );
            }
            "find"
                if find_deletes(&rest)
                    && find_paths(&rest)
                        .any(|a| SDCARD_ROOTS.contains(&a.trim_end_matches('/'))) =>
            {
                return refuse!(
                    "it deletes or rewrites the user's files from the storage root or /data"
                );
            }
            "am" if rest.iter().any(|a| places_a_call(a)) => {
                return refuse!("a CALL intent dials without a tap");
            }
            "wipe" | "format" | "fastboot" => return refuse!("it wipes or reformats the device"),
            "mkfs" | "newfs_msdos" => return refuse!("it reformats a partition"),
            "dd" if rest.iter().any(|a| a.starts_with("of=/dev/")) => {
                return refuse!("it overwrites a partition");
            }
            "reboot"
                if rest
                    .iter()
                    .any(|a| matches!(*a, "recovery" | "bootloader" | "fastboot" | "sideload")) =>
            {
                return refuse!(
                    "it reboots into recovery or the bootloader, where the device can be wiped"
                );
            }
            "rm" => {
                let recursive = rest.iter().any(|a| {
                    *a == "--recursive"
                        || (a.starts_with('-') && !a.starts_with("--") && a.contains(['r', 'R']))
                });
                let hits_root = rest.iter().filter(|a| !a.starts_with('-')).any(|a| {
                    let p = a.trim_end_matches('*').trim_end_matches('/');
                    SDCARD_ROOTS.contains(&p)
                });
                if recursive && hits_root {
                    return refuse!(
                        "it deletes the user's files recursively from the storage root or /data"
                    );
                }
            }
            first @ ("pm" | "cmd") => {
                let rest = if first == "cmd" {
                    if rest.first() != Some(&"package") {
                        continue;
                    }
                    &rest[1..]
                } else {
                    &rest[..]
                };
                if !matches!(rest.first(), Some(&("uninstall" | "clear"))) {
                    continue;
                }
                let mut args = rest[1..].iter();
                let mut package = None;
                while let Some(a) = args.next() {
                    if matches!(*a, "--user" | "--versionCode") {
                        args.next();
                    } else if !a.starts_with('-') {
                        package = Some(*a);
                        break;
                    }
                }
                if let Some(p) = package
                    && !mine.iter().any(|m| m == p)
                {
                    return refuse!(
                        "phonedriver did not install that package on this physical device, or it was installed again since, so it will not clear or remove it"
                    );
                }
            }
            _ => {}
        }
    }
    None
}

/// The paths a `find` starts from: the words before its first expression, past the options `-H`, `-L`, `-P`,
/// `-D <debug>` and `-O<level>` that come first.
fn find_paths<'a>(args: &'a [&'a str]) -> impl Iterator<Item = &'a str> {
    let mut i = 0;
    while let Some(a) = args.get(i) {
        match *a {
            "-H" | "-L" | "-P" => i += 1,
            "-D" => i += 2,
            o if o.starts_with("-O") => i += 1,
            _ => break,
        }
    }
    args[i.min(args.len())..]
        .iter()
        .copied()
        .take_while(|a| !a.starts_with('-') && !matches!(*a, "!" | "("))
}

/// Whether an intent action dials a number (`CALL_BUTTON` only opens the dialer).
pub fn places_a_call(action: &str) -> bool {
    matches!(
        action,
        "android.intent.action.CALL"
            | "android.intent.action.CALL_PRIVILEGED"
            | "android.intent.action.CALL_EMERGENCY"
    )
}

/// Whether a `find` deletes what it finds: `-delete`, or `-exec` and its kin running `rm`.
fn find_deletes(args: &[&str]) -> bool {
    args.iter().enumerate().any(|(i, a)| {
        *a == "-delete"
            || (FIND_RUNS.contains(a)
                && args
                    .get(i + 1)
                    .is_some_and(|p| p.rsplit('/').next() == Some("rm")))
    })
}

/// The action an Android `intent:` link names (`#Intent;…;action=…;end`), or `None` for another link or one that
/// names none.
pub fn intent_action(url: &str) -> Option<&str> {
    let fragment = url.strip_prefix("intent:")?.split_once("#Intent;")?.1;
    fragment
        .split(';')
        .find_map(|part| part.strip_prefix("action="))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_codes_and_carriage_returns_are_removed() {
        let t = tidy(
            "\u{1b}[1;32mgreen\u{1b}[0m\r\nprogress 10%\rprogress 100%\r\n",
            "",
            None,
            40,
        );
        assert_eq!(t.text, "green\nprogress 100%");
        assert_eq!(t.omitted, 0);
    }

    #[test]
    fn long_output_is_cut_with_a_count_of_what_was_left_out() {
        let out: String = (1..=100).map(|n| format!("line {n}\n")).collect();
        let t = tidy(&out, "", None, 5);
        assert_eq!(t.omitted, 95);
        assert!(t.text.starts_with("line 1\nline 2"));
        assert!(
            t.text
                .ends_with("… 95 more lines (narrow it with grep, or set head to 0 for all)")
        );
        assert_eq!(tidy(&out, "", None, 0).omitted, 0);
    }

    #[test]
    fn grep_keeps_matching_lines_from_both_streams_ignoring_case() {
        let t = tidy(
            "mCurrentFocus=Window{x}\nmOther\n",
            "ERROR focus lost\nfine\n",
            Some("focus|nomatch"),
            40,
        );
        assert_eq!(t.text, "mCurrentFocus=Window{x}\nstderr:\nERROR focus lost");
        let none = tidy("a\nb\n", "", Some("zzz"), 40);
        assert_eq!(none.text, "no line of 2 matches \"zzz\"");
    }

    #[test]
    fn stderr_follows_stdout_under_its_own_line_and_long_lines_are_cut() {
        let t = tidy("ok\n", "bad\n", None, 40);
        assert_eq!(t.text, "ok\nstderr:\nbad");
        let long = "x".repeat(1000);
        assert_eq!(
            tidy(&long, "", None, 40).text.chars().count(),
            LINE_CHARS + 1
        );
    }

    #[test]
    fn reading_commands_are_recognised_and_changing_ones_are_not() {
        for c in [
            "echo hi",
            "settings get system screen_brightness",
            "dumpsys window | grep mCurrentFocus",
            "pm list packages -3",
            "getprop ro.build.version.sdk; id",
            "wm size",
        ] {
            assert!(only_reads(c), "{c}");
        }
        for c in [
            "cmd uimode night yes",
            "settings put system screen_brightness 10",
            "echo hi > /sdcard/a",
            "am start -a android.intent.action.VIEW -d https://example.com",
            "input keyevent 3",
            "wm size 1080x1920",
            "dumpsys window | grep a && input tap 1 1",
        ] {
            assert!(!only_reads(c), "{c}");
        }
    }

    #[test]
    fn destructive_commands_are_refused_on_a_physical_device() {
        for c in [
            "rm -rf /sdcard",
            "rm -rf /sdcard/*",
            "rm -fr /data/",
            "su 0 rm -r /storage/emulated/0",
            "pm uninstall com.android.chrome",
            "pm clear --user 0 com.google.android.gm",
            "/system/bin/pm uninstall com.example.other",
            "bash -c \"pm clear com.example.other\"",
            "sh -c 'pm clear com.example.other'",
            "/system/bin/sh -c \"ls; pm uninstall com.example.other\"",
            "cmd package uninstall -k com.example.other",
            "recovery --wipe_data",
            "am broadcast -a android.intent.action.MASTER_CLEAR",
            "reboot recovery",
            "ls; dd if=/dev/zero of=/dev/block/sda",
        ] {
            assert!(refuse_on_physical(c, &["com.mine".into()]).is_some(), "{c}");
        }
    }

    /// The approval guard sees the taps, text and keys phonedriver sends, not the ones `input` sends; a CALL intent
    /// dials without a tap; `find -delete` or `-exec rm` from the storage root empties it.
    #[test]
    fn touches_calls_and_mass_deletes_the_guard_cannot_see_are_refused_on_a_physical_device() {
        for c in [
            "input tap 540 1650",
            "input touchscreen swipe 100 100 100 900",
            "input text 4111111111111111",
            "sh -c 'input keyevent 66'",
            "input keyevent KEYCODE_ENTER",
            "input keyevent --longpress KEYCODE_BACK",
            "dumpsys window | grep a && input tap 1 1",
            "sleep 0 & input tap 540 1650",
            "echo $(input keyevent 66)",
            "echo `input tap 1 1`",
            "echo 1 1 | xargs input tap",
            "eval input tap 1 1",
            "am start -a android.intent.action.CALL -d tel:5551234",
            "am start -a android.intent.action.CALL_PRIVILEGED -d tel:5551234",
        ] {
            assert!(refuse_on_physical(c, &[]).is_some(), "{c}");
        }
        for c in [
            "find /sdcard -delete",
            "find /storage/emulated/0/ -name '*' -delete",
            "find /data -exec rm -rf {} +",
            "find /sdcard -type f -execdir /system/bin/rm {} ;",
            "find -L /sdcard -delete",
            "find -H -D tree -O2 /storage/emulated/0 -name '*' -delete",
        ] {
            assert!(refuse_on_physical(c, &[]).is_some(), "{c}");
            assert!(!only_reads(c), "{c}");
        }
    }

    /// What has no guarded equivalent and can be undone runs: a key `press` cannot send, an install, an intent that
    /// only opens a screen, and `find -exec` with a program that reads.
    #[test]
    fn what_the_guard_has_no_equivalent_for_and_can_be_undone_runs_on_a_physical_device() {
        for c in [
            "input",
            "input keyevent KEYCODE_MEDIA_PLAY_PAUSE",
            "input keyevent 27",
            "input keyevent KEYCODE_WAKEUP",
            "ime list -s",
            "pm install /data/local/tmp/x.apk",
            "pm install-create -S 1000",
            "cmd package install-existing com.example.other",
            "am start -a android.intent.action.VIEW -d https://example.com",
            "am start -a android.settings.WIFI_SETTINGS",
            "am start -a android.intent.action.CALL_BUTTON",
            "logcat -d 2>&1 | grep input",
            "find /sdcard -name '*.jpg' -exec ls -l {} +",
            "find /sdcard/Download -name '*.tmp' -delete",
        ] {
            assert_eq!(refuse_on_physical(c, &[]), None, "{c}");
        }
    }

    #[test]
    fn ordinary_commands_and_phonedriver_installs_pass() {
        let mine = vec!["com.mine".to_string()];
        for c in [
            "rm -rf /sdcard/Download/old",
            "rm /sdcard/a.txt",
            "pm uninstall com.mine",
            "pm clear --user 0 com.mine",
            "/system/bin/pm clear com.mine",
            "sh -c 'pm clear com.mine'",
            "/system/bin/echo pm clear com.other",
            "pm list packages",
            "echo rm -rf /sdcard",
            "cmd uimode night yes",
            "settings put system screen_brightness 10",
        ] {
            assert_eq!(refuse_on_physical(c, &mine), None, "{c}");
        }
    }
}
