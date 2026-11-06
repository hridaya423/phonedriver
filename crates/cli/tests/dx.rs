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
