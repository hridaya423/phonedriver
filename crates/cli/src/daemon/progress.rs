//! Progress lines for steps that are slow the first time (building the iOS runner, pushing the Android agent).
//!
//! A source says what it is doing while a step runs longer than a second. The daemon forwards each line to the
//! connections that asked for progress in `hello` and are waiting on a request, as a JSON-RPC notification
//! (`{"method": "progress", "params": {"message": ...}}`), and the CLI prints it on stderr when that is a terminal.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::broadcast;

const AFTER: Duration = Duration::from_secs(1);

fn channel() -> &'static broadcast::Sender<String> {
    static CHANNEL: OnceLock<broadcast::Sender<String>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(16).0)
}

pub fn subscribe() -> broadcast::Receiver<String> {
    channel().subscribe()
}

fn say(message: String) {
    eprintln!("{message}");
    let _ = channel().send(message);
}

/// Runs `work`; when it is still running after a second, says `message` once.
pub async fn after_a_second<T>(
    message: impl FnOnce() -> String,
    work: impl Future<Output = T>,
) -> T {
    tokio::pin!(work);
    tokio::select! {
        out = &mut work => return out,
        () = tokio::time::sleep(AFTER) => say(message()),
    }
    work.await
}
