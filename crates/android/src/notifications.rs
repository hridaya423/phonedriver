//! The active notifications, read from `dumpsys notification --noredact` (decision D117).

use phonedriver_core::notice::{Notice, Notices};

/// The device's clock in seconds, then the dump: a notification's time is on that clock.
pub const COMMAND: &str = "date +%s; dumpsys notification --noredact";

/// The notifications of the dump's `Notification List`, newest first. A group's summary with no text of its own is
/// left out.
pub fn parse(out: &str) -> Notices {
    let mut lines = out.lines().peekable();
    let Some(now) = lines.next().and_then(|l| l.trim().parse::<u64>().ok()) else {
        return Notices::default();
    };
    let mut notices: Vec<Notice> = Vec::new();
    let mut summary = false;
    let mut listed = false;
    while let Some(line) = lines.next() {
        if !listed {
            listed = line.trim_end() == "  Notification List:";
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let body = line.trim_start();
        if indent == 2 && !body.is_empty() {
            break;
        }
        if indent == 4 && body.starts_with("NotificationRecord(") {
            drop_empty(&mut notices, summary);
            summary = false;
            notices.push(Notice {
                key: between(body, " key=", ": Notification(")
                    .unwrap_or_default()
                    .to_string(),
                package: between(body, "pkg=", " ").unwrap_or_default().to_string(),
                title: None,
                text: None,
                posted_ms: 0,
                ongoing: false,
            });
            continue;
        }
        let Some(n) = notices.last_mut() else {
            continue;
        };
        if indent == 6
            && let Some(flags) = body.strip_prefix("flags=")
        {
            n.ongoing = flags.split('|').any(|f| f == "ONGOING_EVENT");
            summary = flags.split('|').any(|f| f == "GROUP_SUMMARY");
        } else if let Some(when) = body.strip_prefix("when=") {
            n.posted_ms = when
                .rsplit('/')
                .next()
                .and_then(|t| t.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = body.strip_prefix("android.title=") {
            n.title = value(rest, &mut lines);
        } else if let Some(rest) = body.strip_prefix("android.text=") {
            n.text = value(rest, &mut lines).or(n.text.take());
        } else if let Some(rest) = body.strip_prefix("android.bigText=")
            && n.text.is_none()
        {
            n.text = value(rest, &mut lines);
        }
    }
    drop_empty(&mut notices, summary);
    notices.sort_by_key(|n| std::cmp::Reverse(n.posted_ms));
    Notices {
        now_ms: now * 1000,
        notices,
    }
}

/// A group's summary that says nothing of its own stands for the notifications listed beside it.
fn drop_empty(notices: &mut Vec<Notice>, summary: bool) {
    if summary
        && notices
            .last()
            .is_some_and(|n| n.title.is_none() && n.text.is_none())
    {
        notices.pop();
    }
}

fn between<'a>(line: &'a str, from: &str, to: &str) -> Option<&'a str> {
    let rest = &line[line.find(from)? + from.len()..];
    Some(&rest[..rest.find(to)?])
}

/// The text of `String (the text)`, which a line break in the text carries onto the lines after it; `null` and an
/// empty text are nothing.
fn value<'a>(
    rest: &str,
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let mut text = rest.split_once(" (")?.1.to_string();
    while !text.ends_with(')') {
        let next = lines.peek()?;
        if next.trim_start().starts_with("android.") || next.trim() == "}" {
            break;
        }
        text.push('\n');
        text.push_str(lines.next()?);
    }
    let text = text.strip_suffix(')').unwrap_or(&text).trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const API37: &str = include_str!("../tests/fixtures/dumpsys_notification_api37.txt");

    #[test]
    fn the_list_of_an_api_37_dump_is_read_newest_first_with_titles_and_texts() {
        let seen = parse(API37);
        assert_eq!(seen.now_ms, 1_795_551_184_000);
        let titles: Vec<_> = seen
            .notices
            .iter()
            .map(|n| n.title.as_deref().unwrap_or("-"))
            .collect();
        assert_eq!(
            titles,
            [
                "Shopping",
                "Two lines",
                "Team",
                "Your code",
                "Ada Lovelace",
                "Set a screen lock",
                "Serial console enabled"
            ]
        );
        let by = |title: &str| {
            seen.notices
                .iter()
                .find(|n| n.title.as_deref() == Some(title))
                .unwrap()
        };
        assert_eq!(by("Your code").text.as_deref(), Some("Your code is 482913"));
        assert_eq!(by("Your code").package, "com.android.shell");
        assert_eq!(by("Your code").key, "0|com.android.shell|2020|pdtag2|2000");
        assert_eq!(by("Your code").posted_ms, 1_795_551_084_919);
        assert_eq!(by("Team").text.as_deref(), Some("Bea: ship it (really)"));
        assert_eq!(
            by("Two lines").text.as_deref(),
            Some("first line\nsecond line")
        );
        assert!(by("Serial console enabled").ongoing && !by("Team").ongoing);
        assert_eq!(by("Serial console enabled").package, "android");
    }

    #[test]
    fn a_dump_with_no_list_or_no_clock_reads_as_nothing() {
        assert_eq!(parse(""), Notices::default());
        assert_eq!(
            parse("1795551184\nCurrent Notification Manager state:\n").notices,
            []
        );
    }
}
