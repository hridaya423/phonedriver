use phonedriver_android::agent::{read_reply, write_frame};
use serde_json::json;

#[tokio::test]
async fn pushed_events_before_the_reply_are_skipped() {
    let (mut agent, mut host) = tokio::io::duplex(4096);
    write_frame(&mut agent, &json!({"ev": "window_changed", "w": 1}))
        .await
        .unwrap();
    write_frame(&mut agent, &json!({"id": 7, "ok": true, "api": 37}))
        .await
        .unwrap();
    let reply = read_reply(&mut host, 7).await.unwrap();
    assert_eq!(reply["api"], 37);
}

#[tokio::test]
async fn a_reply_for_another_request_is_not_ours() {
    let (mut agent, mut host) = tokio::io::duplex(4096);
    write_frame(&mut agent, &json!({"id": 3, "ok": true}))
        .await
        .unwrap();
    write_frame(&mut agent, &json!({"id": 7, "ok": true, "api": 26}))
        .await
        .unwrap();
    assert_eq!(read_reply(&mut host, 7).await.unwrap()["api"], 26);
}
