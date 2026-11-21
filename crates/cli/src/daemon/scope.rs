//! The app scope as a layer over a device (decision D102): every read comes back without the windows of apps outside
//! the scope, and everything that touches the screen, the clipboard, the shell or another app is refused while one
//! of those windows is on screen.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use phonedriver_core::device::{
    App, AppId, AppOp, Button, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef, Gesture,
    Image, Key, LogReply, LogRequest, Quality, Recorded, Result, ScreenState, ShellReply,
    ShellRequest, TextInput, Url, WebPage,
};
use phonedriver_core::error::Error;
use phonedriver_core::geometry::Size;
use phonedriver_core::network::{LogWait, LogWaited, NetworkView};
use phonedriver_core::raw::RawTree;
use phonedriver_core::scope::{AppScope, Outside};
use phonedriver_core::settings::{SettingOp, SettingReply};
use tokio::sync::{broadcast, watch};

pub struct ScopedDevice {
    inner: Arc<dyn Device>,
    scope: Arc<AppScope>,
}

fn refused(detail: String) -> Error {
    Error::Refused { detail }
}

fn names(outside: &[Outside]) -> String {
    let mut packages: Vec<&str> = outside
        .iter()
        .map(|o| o.package.as_deref().unwrap_or("a window with no app name"))
        .collect();
    packages.dedup();
    packages.join(", ")
}

fn lift(scope: &AppScope) -> String {
    format!(
        "The scope is the person's setting: PHONEDRIVER_APP_SCOPE={}, read when the daemon starts.",
        scope.list()
    )
}

/// The refusal for something that touches the screen while windows of apps outside the scope are on it.
pub fn refusal(scope: &AppScope, outside: &[Outside], what: &str) -> Error {
    refused(format!(
        "{what} is refused: {} is on screen and outside the app scope ({}). `press back` or `press home` leaves it, and `launch` opens an app that is in scope. {}",
        names(outside),
        scope.list(),
        lift(scope)
    ))
}

/// Whether `url` is an Android `intent:` URL that names an activity (`component=`) or a selector (`SEL`): Android starts
/// that activity whatever package the call names, so the app the scope checked is not the app that opens.
fn names_a_component(url: &str) -> bool {
    url.get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("intent:"))
        && url
            .split(';')
            .any(|field| field == "SEL" || field.starts_with("component="))
}

impl ScopedDevice {
    pub fn new(inner: Arc<dyn Device>, scope: Arc<AppScope>) -> ScopedDevice {
        ScopedDevice { inner, scope }
    }

    fn lift(&self) -> String {
        lift(&self.scope)
    }

    fn screen(&self) -> Size {
        self.inner
            .current_screen()
            .unwrap_or(self.inner.info().screen)
    }

    async fn seen(&self) -> Result<RawTree> {
        Ok(self.scope.redact(self.inner.tree().await?, self.screen()))
    }

    async fn clear(&self, what: &str) -> Result<()> {
        let outside = self.seen().await?.outside;
        if outside.is_empty() {
            return Ok(());
        }
        Err(refusal(&self.scope, &outside, what))
    }

    fn in_scope(&self, app: &AppId, what: &str) -> Result<()> {
        if self.scope.allows(&app.0) {
            return Ok(());
        }
        Err(refused(format!(
            "{what} is refused: {} is outside the app scope ({}). {}",
            app.0,
            self.scope.list(),
            self.lift()
        )))
    }

    /// On iOS, `back` and `hide_keyboard` tap the front app's own back button or keyboard key, so they go through the
    /// scope like a touch. Android's are system keys.
    async fn front_is_in_scope(&self, what: &str) -> Result<()> {
        if self.windows_known() {
            return Ok(());
        }
        let outside = self.seen().await?.outside;
        if outside.is_empty() {
            return Ok(());
        }
        Err(refused(format!(
            "{what} is refused on an iPhone, iPad or Simulator while {} is in front and outside the app scope ({}): there it taps that app's own buttons. `press home` leaves it, and `launch` opens an app that is in scope. {}",
            names(&outside),
            self.scope.list(),
            self.lift()
        )))
    }

    fn windows_known(&self) -> bool {
        let id = &self.inner.info().id;
        !(id.starts_with("ios:") || id.starts_with("sim:"))
    }
}

#[async_trait]
impl Device for ScopedDevice {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn hidden_display(&self) -> Option<u32> {
        self.inner.hidden_display()
    }

    async fn tree(&self) -> Result<RawTree> {
        self.seen().await
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        self.inner.frames()
    }

    fn frames_follow(&self) -> bool {
        self.inner.frames_follow()
    }

    async fn screenshot(&self, q: Quality) -> Result<Image> {
        if !self.windows_known() {
            return Err(refused(format!(
                "a screenshot is refused under an app scope: on an iPhone or iPad phonedriver cannot see a notification banner or another app's window over the front app, so it cannot tell that the picture holds nothing from outside the scope. `snapshot` works. {}",
                self.lift()
            )));
        }
        self.clear("a screenshot").await?;
        let image = self.inner.screenshot(q).await?;
        self.clear("a screenshot").await?;
        Ok(image)
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.inner.events()
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        self.clear("a touch").await?;
        self.inner.gesture(g).await
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.clear("a key").await?;
        self.inner.key(k).await
    }

    async fn button(&self, b: Button) -> Result<()> {
        match b {
            Button::Home | Button::Back | Button::Lock => {}
            Button::Recents | Button::Notifications => {
                return Err(refused(format!(
                    "this button is refused under an app scope: it opens a screen of other apps' windows or notifications. `press home` and `press back` work. {}",
                    self.lift()
                )));
            }
            _ => self.clear("this button").await?,
        }
        self.inner.button(b).await
    }

    async fn open_url(&self, url: &Url, app: Option<&AppId>) -> Result<Option<AppId>> {
        let Some(app) = app else {
            return Err(refused(format!(
                "`open` needs an app under an app scope, because a link can open any app: `open <url> --app <package>` with one of {}. {}",
                self.scope.list(),
                self.lift()
            )));
        };
        self.in_scope(app, "`open`")?;
        if !self.windows_known() {
            return Err(refused(format!(
                "`open` is refused under an app scope on an iPhone, iPad or Simulator: iOS opens a link in whatever app claims it, not in the app named, so phonedriver cannot keep it in the scope. `launch {}` opens the app. {}",
                app.0,
                self.lift()
            )));
        }
        if names_a_component(&url.0) {
            return Err(refused(format!(
                "`open` is refused under an app scope for an `intent:` URL that names a component or a selector: Android starts that activity whatever app is named. Leave out `component=` and `SEL`, or use `launch`. {}",
                self.lift()
            )));
        }
        self.inner.open_url(url, Some(app)).await
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        self.clear("typing").await?;
        self.inner.text(t).await
    }

    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>> {
        if matches!(op, ClipboardOp::Get) {
            return Err(refused(format!(
                "reading the clipboard is refused under an app scope: it may hold what was copied in another app. {}",
                self.lift()
            )));
        }
        self.clear("the clipboard").await?;
        self.inner.clipboard(op).await
    }

    async fn apps(&self) -> Result<Vec<App>> {
        let mut apps = self.inner.apps().await?;
        apps.retain(|a| self.scope.allows(&a.id.0));
        Ok(apps)
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        self.in_scope(app, "`launch`")?;
        self.inner.launch(app).await
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        self.inner.foreground().await
    }

    async fn grant(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.in_scope(app, "this change")?;
        self.inner.grant(app, permissions).await
    }

    async fn revoke(&self, app: &AppId, permissions: &[String]) -> Result<()> {
        self.in_scope(app, "this change")?;
        self.inner.revoke(app, permissions).await
    }

    async fn push(&self, app: &AppId, payload: &str) -> Result<()> {
        self.in_scope(app, "this notification")?;
        self.inner.push(app, payload).await
    }

    async fn record_start(&self, _path: &Path) -> Result<()> {
        Err(refused(format!(
            "recording is refused under an app scope: a video shows every window, and phonedriver cannot cut other apps out of it. Screenshots work while only apps in the scope are on screen. {}",
            self.lift()
        )))
    }

    async fn record_stop(&self) -> Result<Option<Recorded>> {
        self.inner.record_stop().await
    }

    async fn logs(&self, req: &LogRequest) -> Result<LogReply> {
        self.in_scope(&req.app, "reading this log")?;
        self.inner.logs(req).await
    }

    async fn log_wait(&self, req: &LogWait) -> Result<LogWaited> {
        let app = match &req.app {
            Some(app) => app.clone(),
            None => match self.scope.only() {
                Some(only) => AppId(only.to_string()),
                None => {
                    return Err(refused(format!(
                        "`wait log` needs an app under an app scope that names several: `--app <package>` with one of {}. {}",
                        self.scope.list(),
                        self.lift()
                    )));
                }
            },
        };
        self.in_scope(&app, "waiting on this log")?;
        self.inner
            .log_wait(&LogWait {
                app: Some(app),
                patterns: req.patterns.clone(),
                timeout: req.timeout,
            })
            .await
    }

    async fn network(&self) -> Result<NetworkView> {
        let mut view = self.inner.network().await?;
        let hide = |owner: &mut Option<String>| {
            if owner.as_deref().is_some_and(|o| !self.scope.allows(o)) {
                *owner = Some("an app outside the app scope".into());
            }
        };
        for link in &mut view.networks {
            hide(&mut link.owner);
        }
        hide(&mut view.always_on_vpn);
        Ok(view)
    }

    async fn page(&self, _app: Option<&AppId>) -> Result<WebPage> {
        let seen = self.seen().await?;
        if !seen.outside.is_empty() {
            return Err(refusal(&self.scope, &seen.outside, "reading the page"));
        }
        let approved = seen
            .windows
            .iter()
            .filter(|w| w.kind == "application")
            .find_map(|w| w.package.clone().filter(|p| self.scope.allows(p)))
            .ok_or_else(|| refused(format!("no app in the app scope is in front. {}", self.lift())))?;
        self.inner.page(Some(&AppId(approved))).await
    }

    async fn setting(&self, op: SettingOp) -> Result<SettingReply> {
        self.inner.setting(op).await
    }

    async fn install(&self, path: &Path) -> Result<Vec<AppId>> {
        self.inner.install(path).await
    }

    async fn installed_at(&self, app: &AppId) -> Result<Option<String>> {
        self.inner.installed_at(app).await
    }

    async fn hardware_serial(&self) -> Result<Option<String>> {
        self.inner.hardware_serial().await
    }

    async fn app_op(&self, op: AppOp, app: &AppId) -> Result<()> {
        self.in_scope(app, "this change")?;
        self.inner.app_op(op, app).await
    }

    async fn shell(&self, _req: &ShellRequest) -> Result<ShellReply> {
        Err(refused(format!(
            "`shell` is refused under an app scope: a command such as `am start`, `input` or `content query` goes around every other rule of it. To use it, stop the daemon (`phonedriver daemon stop`) and start it without PHONEDRIVER_APP_SCOPE. {}",
            self.lift()
        )))
    }

    async fn screen_state(&self) -> Result<ScreenState> {
        self.inner.screen_state().await
    }

    async fn wake(&self) -> Result<()> {
        self.inner.wake().await
    }

    fn current_screen(&self) -> Option<Size> {
        self.inner.current_screen()
    }

    fn secure_content_hidden(&self, tree: &RawTree) -> bool {
        self.inner.secure_content_hidden(tree)
    }

    async fn capture_tree(&self) -> Result<Option<String>> {
        Err(refused(format!(
            "saving the device's own tree is refused under an app scope: it holds every window. {}",
            self.lift()
        )))
    }

    async fn back(&self) -> Result<Option<&'static str>> {
        self.front_is_in_scope("`press back`").await?;
        self.inner.back().await
    }

    async fn hide_keyboard(&self) -> Result<Option<&'static str>> {
        self.front_is_in_scope("hiding the keyboard").await?;
        self.inner.hide_keyboard().await
    }
}
