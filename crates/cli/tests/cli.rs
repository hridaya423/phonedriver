use assert_cmd::Command;

#[test]
fn version_prints_name_and_version() {
    Command::cargo_bin("phone-use")
        .unwrap()
        .arg("version")
        .assert()
        .success()
        .stdout("phone-use 0.1.0\n");
}

#[test]
fn no_args_prints_usage_and_exits_2() {
    let out = Command::cargo_bin("phone-use").unwrap().assert().code(2);
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("usage: phone-use"), "{stderr}");
}
