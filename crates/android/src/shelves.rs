//! The files of the shared folders a pull may read, from `find` and `stat` (decision D129).

use phonedriver_core::files::{Entry, Listing, PULL_FROM, shared};

/// The device's clock in seconds, then one line a file: when it was written, its size, its path. `folder` is what
/// `files::folder` made, so it holds no quote. A folder that is not there prints nothing.
pub fn command(folder: Option<&str>) -> String {
    let roots: Vec<String> = match folder {
        Some(folder) => vec![format!("'/sdcard/{folder}'")],
        None => PULL_FROM.iter().map(|f| format!("'/sdcard/{f}'")).collect(),
    };
    format!(
        "date +%s; find {} -type f ! -path '*/.*' -exec stat -c '%Y %s %n' {{}} + 2>/dev/null",
        roots.join(" ")
    )
}

/// The files the command printed. A file a pull would refuse (a quote or a control character in its name) is left
/// out, since it could not be asked for.
pub fn parse(out: &str) -> Option<Listing> {
    let mut lines = out.lines();
    let now = lines.next()?.trim().parse().ok()?;
    let files = lines
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let modified = parts.next()?.parse().ok()?;
            let bytes = parts.next()?.parse().ok()?;
            let path = shared(parts.next()?).ok()?;
            Some(Entry {
                path: path.strip_prefix("/sdcard/")?.to_string(),
                bytes,
                modified,
            })
        })
        .collect();
    Some(Listing { now, files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_reads_the_three_shelves_or_the_one_folder_and_skips_hidden_files() {
        assert_eq!(
            command(None),
            "date +%s; find '/sdcard/Download' '/sdcard/Pictures' '/sdcard/DCIM' -type f ! -path '*/.*' -exec stat -c '%Y %s %n' {} + 2>/dev/null"
        );
        assert!(command(Some("DCIM/Camera")).contains("find '/sdcard/DCIM/Camera' -type f"));
    }

    #[test]
    fn a_line_is_a_file_with_its_size_and_time_and_a_name_a_pull_refuses_is_left_out() {
        let out = "1795000000\n\
                   1794999000 9720 /sdcard/Pictures/my cat.png\n\
                   1794990000 48213 /sdcard/Download/report.pdf\n\
                   1794990000 5 /sdcard/Download/it's.txt\n\
                   stat: permission denied\n";
        assert_eq!(
            parse(out).unwrap(),
            Listing {
                now: 1_795_000_000,
                files: vec![
                    Entry {
                        path: "Pictures/my cat.png".into(),
                        bytes: 9720,
                        modified: 1_794_999_000
                    },
                    Entry {
                        path: "Download/report.pdf".into(),
                        bytes: 48213,
                        modified: 1_794_990_000
                    },
                ]
            }
        );
        assert_eq!(parse("1795000000\n").unwrap().files, vec![]);
        assert!(parse("").is_none());
        assert!(parse("/system/bin/sh: date: not found\n").is_none());
    }
}
