mod support;

#[tokio::test]
#[ignore = "needs PHONEDRIVER_TEST_DEVICE"]
async fn shell_echo_returns_hi() {
    let out = support::adb()
        .shell(&support::serial(), "echo hi")
        .await
        .unwrap();
    assert_eq!(out.stdout_text(), "hi\n");
    assert_eq!(out.exit_code, 0);
}
