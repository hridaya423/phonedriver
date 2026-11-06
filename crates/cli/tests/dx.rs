use assert_cmd::Command;

fn phone_use(home: &tempfile::TempDir) -> Command {
    let mut c = Command::cargo_bin("phone-use").unwrap();
    c.env("PHONE_USE_HOME", home.path())
        .env_remove("PHONE_USE_DEVICE")
        .env_remove("PHONE_USE_SOCKET");
    c
}

fn stderr_of(mut c: Command, code: i32) -> String {
    let out = c.assert().code(code);
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

#[test]
fn a_bad_scroll_direction_is_refused_by_the_cli_with_the_choices() {
    let home = tempfile::tempdir().unwrap();
    let mut c = phone_use(&home);
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
    let mut c = phone_use(&home);
    c.args(["type", "--secret", "hunter2"]);
    let stderr = stderr_of(c, 2);
    assert!(
        stderr.contains("echo -n \"$PW\" | phone-use type --secret --into @e3"),
        "{stderr}"
    );
    assert!(!stderr.contains("Usage: phone-use [OPTIONS]"), "{stderr}");
    assert!(!stderr.contains("hunter2"), "{stderr}");
}

fn daemon_env(c: &mut Command, home: &tempfile::TempDir) {
    c.env("PHONE_USE_HOME", home.path())
        .env("PHONE_USE_SOCKET", home.path().join("run").join("d.sock"))
        .env("ADB_SERVER_SOCKET", "tcp:127.0.0.1:1")
        .env("USBMUXD_SOCKET_ADDRESS", "127.0.0.1:1")
        .env_remove("PHONE_USE_DEVICE");
}

#[test]
fn daemon_stop_ends_a_running_daemon_and_succeeds_when_there_is_none() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let stop = |home: &tempfile::TempDir| {
        let mut c = Command::cargo_bin("phone-use").unwrap();
        daemon_env(&mut c, home);
        c.args(["daemon", "stop"]);
        let out = c.assert().success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    };
    assert_eq!(stop(&home), "no daemon is running\n");

    let mut started = Command::cargo_bin("phone-use").unwrap();
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
