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

fn matcher(grep: &str) -> impl Fn(&str) -> bool {
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

fn segments(command: &str) -> Vec<Vec<String>> {
    let spaced = command
        .replace("&&", "\n")
        .replace("||", "\n")
        .replace([';', '|'], "\n");
    spaced
        .lines()
        .map(words)
        .filter(|w| !w.is_empty())
        .map(|mut w| {
            const WRAPPERS: &[&str] = &[
                "su", "sudo", "0", "2000", "root", "-c", "env", "nohup", "exec", "time", "busybox",
                "toybox",
            ];
            let skip = w
                .iter()
                .take_while(|t| WRAPPERS.contains(&t.as_str()))
                .count();
            w.drain(..skip);
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
                | "uname" | "df" | "date" | "grep" | "head" | "tail" | "wc" | "find" | "stat"
                | "du" | "uptime" | "printenv" | "readlink" | "pidof" | "sort" | "cut" | "true"
                | "sleep" => true,
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

/// Why a physical device should not run this command, or `None`. `mine` are the packages phone-use installed there.
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
                        "phone-use did not install that package on this physical device, so it will not clear or remove it"
                    );
                }
            }
            _ => {}
        }
    }
    None
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
            "cmd package uninstall -k com.example.other",
            "recovery --wipe_data",
            "am broadcast -a android.intent.action.MASTER_CLEAR",
            "reboot recovery",
            "ls; dd if=/dev/zero of=/dev/block/sda",
        ] {
            assert!(refuse_on_physical(c, &["com.mine".into()]).is_some(), "{c}");
        }
    }

    #[test]
    fn ordinary_commands_and_phone_use_installs_pass() {
        let mine = vec!["com.mine".to_string()];
        for c in [
            "rm -rf /sdcard/Download/old",
            "rm /sdcard/a.txt",
            "pm uninstall com.mine",
            "pm clear --user 0 com.mine",
            "pm list packages",
            "echo rm -rf /sdcard",
            "cmd uimode night yes",
            "settings put system screen_brightness 10",
        ] {
            assert_eq!(refuse_on_physical(c, &mine), None, "{c}");
        }
    }
}
