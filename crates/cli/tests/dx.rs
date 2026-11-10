use assert_cmd::Command;

fn phonedriver(home: &tempfile::TempDir) -> Command {
    let mut c = Command::cargo_bin("phonedriver").unwrap();
    c.env("PHONEDRIVER_HOME", home.path())
        .env_remove("PHONEDRIVER_DEVICE")
        .env_remove("PHONEDRIVER_SOCKET");
    c
}

fn stderr_of(mut c: Command, code: i32) -> String {
    let out = c.assert().code(code);
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

#[test]
fn a_bad_scroll_direction_is_refused_by_the_cli_with_the_choices() {
    let home = tempfile::tempdir().unwrap();
    let mut c = phonedriver(&home);
    c.args(["scroll", "sideways"]);
    let stderr = stderr_of(c, 2);
    assert!(stderr.contains("up, down, left, right"), "{stderr}");
    assert!(!stderr.contains("daemon"), "{stderr}");
    assert!(
        !home.path().join("run").exists(),
        "no daemon was started for a usage error"
    );
}

#[test]
fn the_secret_error_shows_how_to_pipe_it_and_not_the_top_level_usage() {
    let home = tempfile::tempdir().unwrap();
    let mut c = phonedriver(&home);
    c.args(["type", "--secret", "hunter2"]);
    let stderr = stderr_of(c, 2);
    assert!(
        stderr.contains("echo -n \"$PW\" | phonedriver type --secret --into @e3"),
        "{stderr}"
    );
    assert!(!stderr.contains("Usage: phonedriver [OPTIONS]"), "{stderr}");
    assert!(!stderr.contains("hunter2"), "{stderr}");
}

fn daemon_env(c: &mut Command, home: &tempfile::TempDir) {
    c.env("PHONEDRIVER_HOME", home.path())
        .env("PHONEDRIVER_SOCKET", home.path().join("run").join("d.sock"))
        .env("ADB_SERVER_SOCKET", "tcp:127.0.0.1:1")
        .env("USBMUXD_SOCKET_ADDRESS", "127.0.0.1:1")
        .env_remove("PHONEDRIVER_DEVICE");
}

#[test]
fn daemon_stop_ends_a_running_daemon_and_succeeds_when_there_is_none() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let stop = |home: &tempfile::TempDir| {
        let mut c = Command::cargo_bin("phonedriver").unwrap();
        daemon_env(&mut c, home);
        c.args(["daemon", "stop"]);
        let out = c.assert().success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    };
    assert_eq!(stop(&home), "no daemon is running\n");

    let mut started = Command::cargo_bin("phonedriver").unwrap();
    daemon_env(&mut started, &home);
    let mut daemon = std::process::Command::new(started.get_program())
        .arg("daemon")
        .envs(started.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    BufReader::new(daemon.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    let pid = ready
        .trim()
        .strip_prefix("ready ")
        .expect(&ready)
        .to_string();

    let reaper = std::thread::spawn(move || {
        let started = Instant::now();
        daemon.wait().unwrap();
        started.elapsed()
    });
    let before = Instant::now();
    assert_eq!(stop(&home), format!("stopped the daemon (pid {pid})\n"));
    assert!(
        before.elapsed() < Duration::from_secs(3),
        "stop waited for the pid: {:?}",
        before.elapsed()
    );
    assert!(reaper.join().unwrap() < Duration::from_secs(3));
    assert_eq!(stop(&home), "no daemon is running\n");
}

fn walk(cmd: &clap::Command, path: &str, problems: &mut Vec<String>) {
    for sub in cmd.get_subcommands() {
        let name = format!("{path} {}", sub.get_name());
        if sub.get_about().is_none() && sub.get_name() != "help" {
            problems.push(format!("{name} has no description"));
        }
        for arg in sub.get_arguments() {
            if !arg.is_hide_set() && arg.get_help().is_none() && arg.get_id() != "help" {
                problems.push(format!("{name} {} has no description", arg.get_id()));
            }
        }
        walk(sub, &name, problems);
    }
}

#[test]
fn every_visible_flag_and_argument_has_a_description() {
    let mut problems = Vec::new();
    let mut cmd = phonedriver::commands::command();
    cmd.build();
    walk(&cmd, "phonedriver", &mut problems);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn a_command_s_help_lists_its_own_options_and_points_at_the_top_help_for_the_global_ones() {
    let home = tempfile::tempdir().unwrap();
    let help = |args: &[&str]| {
        let out = phonedriver(&home).args(args).assert().success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    };
    for args in [
        ["tap", "--help"],
        ["scroll", "--help"],
        ["snapshot", "--help"],
        ["wait", "--help"],
    ] {
        let text = help(&args);
        let (own, pointer) = text.rsplit_once("\n\n").unwrap();
        assert!(pointer.contains("`phonedriver --help`"), "{text}");
        for global in [
            "--device",
            "--session",
            "--json",
            "--no-settle",
            "--timeout",
            "--expect-change",
            "--share",
            "--display",
            "Global options:",
        ] {
            assert!(!own.contains(global), "{args:?} lists {global}:\n{text}");
        }
        assert!(text.lines().count() < 20, "{text}");
    }
    assert!(help(&["tap", "--help"]).contains("--role"));
    let top = help(&["--help"]);
    assert!(top.contains("Exit codes:"));
    for global in [
        "--device",
        "--session",
        "--json",
        "--timeout",
        "--expect-change",
    ] {
        assert!(top.contains(global), "{top}");
    }
}

#[test]
fn within_is_one_name_and_in_still_works() {
    let home = tempfile::tempdir().unwrap();
    let help = phonedriver(&home)
        .args(["scroll", "--help"])
        .assert()
        .success();
    let text = String::from_utf8(help.get_output().stdout.clone()).unwrap();
    assert!(
        text.contains("--within") && !text.contains("--in "),
        "{text}"
    );
    let mut c = phonedriver(&home);
    c.args(["scroll", "down", "--in", "@e1", "--device", "nope"]);
    let out = c.assert().failure();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert_ne!(out.get_output().status.code(), Some(2), "{stderr}");
    assert!(!stderr.contains("unexpected argument"), "{stderr}");
}
