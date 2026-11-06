//! `CoreDevice`: the `Device` port over the developer services of an iOS 27 device, nothing installed on it
//! (architecture 7.2).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use idevice::ReadWrite;
use idevice::RsdService;
use idevice::core_device::{
    AppServiceClient, ButtonState, CallInfoBlob, DisplayServiceClient, IndigoHidClient,
    MainKeyboardService, PasteboardServiceClient, ReportBlock, RtpPacket,
    UniversalHidServiceClient, build_frame_ack, build_keyframe_request, build_liveness, build_rctl,
    build_screen_audio_offer, build_screen_video_offer, build_start_audio_parameters,
    build_start_video_parameters, is_rtcp,
};
use idevice::core_device::{GENERAL_PASTEBOARD, KeyboardUsage};
use idevice::tcp::handle::UdpSocketHandle;
use phone_use_core::device::{
    App, AppId, Button, Capabilities, ClipboardOp, Device, DeviceEvent, DeviceInfo, FrameRef,
    Gesture, Image, ImageFormat, Key, Quality, Result, TextInput, Url,
};
use phone_use_core::error::{Error, Op};
use phone_use_core::geometry::{Rect, Size};
use phone_use_core::raw::{RawTree, RawWindow};
use phone_use_video::frame_store::FrameStore;
use phone_use_video::h264::Decoder;
use phone_use_video::image::{encode_png, scaled_size};
use phone_use_video::rtp::{Depacketizer, Event};
use tokio::sync::{Mutex, broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::offer;
use crate::touch::{self, MOD_GUI, MOD_SHIFT, USAGE_V};
use crate::transport::{self, Tunnel, Wired};

type Socket = Box<dyn ReadWrite>;

const CLIENT_SUPPORTED_FEATURES: u64 = 140;
const FIRST_FRAME_WAIT: Duration = Duration::from_secs(5);
const RTCP_INTERVAL: Duration = Duration::from_secs(1);
const RCTL_INTERVAL: Duration = Duration::from_millis(50);
const EVENT_BACKLOG: usize = 16;
const DATAGRAM_QUEUE: usize = 512;
const KEY_HOLD: Duration = Duration::from_millis(25);
const HID_USAGE_PAGE_CONSUMER: u64 = 0x0C;

fn button_usage(b: Button) -> Option<u64> {
    Some(match b {
        Button::Home => 0x40,
        Button::Lock => 0x30,
        Button::VolumeUp => 0xE9,
        Button::VolumeDown => 0xEA,
        _ => return None,
    })
}

struct Keyboard {
    service: Option<MainKeyboardService>,
}

pub struct CoreDevice {
    info: DeviceInfo,
    screen: Size,
    density: f32,
    store: Arc<FrameStore>,
    hid: Mutex<UniversalHidServiceClient<Socket>>,
    keyboard: Mutex<Keyboard>,
    indigo: Mutex<IndigoHidClient<Socket>>,
    pasteboard: Mutex<PasteboardServiceClient<Socket>>,
    apps: Mutex<AppServiceClient<Socket>>,
    events: broadcast::Sender<DeviceEvent>,
    ended: watch::Receiver<bool>,
    tasks: Vec<JoinHandle<()>>,
    _tunnel: Tunnel,
    _display: DisplayServiceClient<Socket>,
}

/// `RsdHandshake::connect`, spelled out so the socket type is `'static`: through `connect_rsd` the daemon's
/// `Send` check fails with "implementation of `RsdService` is not general enough".
async fn rsd_client<C: RsdService>(
    adapter: &mut idevice::tcp::handle::AdapterHandle,
    rsd: &idevice::rsd::RsdHandshake,
) -> std::result::Result<C, idevice::IdeviceError> {
    let port = rsd
        .services
        .get(&C::rsd_service_name().to_string())
        .ok_or(idevice::IdeviceError::ServiceNotFound)?
        .port;
    let stream = adapter.connect(port).await?;
    let socket: Socket = Box::new(stream);
    C::from_stream(socket).await
}

fn call_info() -> CallInfoBlob {
    CallInfoBlob {
        call_id: 0,
        client_version: 1,
        device_type: "Mac17,7".into(),
        framework_version: "2205.3.1".into(),
        os_version: "25F71".into(),
        device_name: None,
        audio_device_uid: None,
    }
}

fn uuid_text() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

fn stream_error(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Transport {
        detail: format!("display stream {what}: {e}"),
    }
}

impl CoreDevice {
    pub const CAPABILITIES: Capabilities = Capabilities::FRAMES
        .union(Capabilities::SCREENSHOT)
        .union(Capabilities::GESTURE)
        .union(Capabilities::MULTI_TOUCH)
        .union(Capabilities::KEY)
        .union(Capabilities::BUTTON)
        .union(Capabilities::TEXT_PASTE)
        .union(Capabilities::TEXT_KEYS)
        .union(Capabilities::CLIPBOARD)
        .union(Capabilities::APPS)
        .union(Capabilities::LAUNCH);

    /// Opens the tunnel, starts the stream and connects the services. The work runs on a blocking thread under
    /// `Handle::block_on` because rustc cannot prove the `Send` bound of idevice's `RsdService` futures for the
    /// daemon's `async_trait` caller ("implementation of `RsdService` is not general enough").
    pub async fn connect(device: &Wired, model: &str) -> Result<CoreDevice> {
        let (device, model) = (device.clone(), model.to_string());
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || handle.block_on(CoreDevice::open(&device, &model)))
            .await
            .map_err(|e| Error::Transport {
                detail: format!("the connect task failed: {e}"),
            })?
    }

    async fn open(device: &Wired, model: &str) -> Result<CoreDevice> {
        let id = format!("ios:{}", device.udid);
        let lost = |e: idevice::IdeviceError| transport::classify(&id, &e);
        let mut tunnel = Tunnel::open(device).await.map_err(lost)?;
        let Tunnel { adapter, rsd } = &mut tunnel;

        let mut display = rsd_client::<DisplayServiceClient<Socket>>(adapter, rsd)
            .await
            .map_err(lost)?;
        let audio = adapter
            .bind_udp(0)
            .await
            .map_err(|e| stream_error("audio socket", e))?;
        let video = Arc::new(
            adapter
                .bind_udp(0)
                .await
                .map_err(|e| stream_error("video socket", e))?,
        );
        let (host, peer) = (adapter.host_ip().to_string(), adapter.peer_ip().to_string());
        let session = uuid::Uuid::new_v4();
        let audio_offer = build_screen_audio_offer(&uuid_text(), &call_info()).map_err(lost)?;
        Box::pin(display.start_media_stream(build_start_audio_parameters(
            &host,
            audio.local_port(),
            &peer,
            50000,
            audio_offer,
            CLIENT_SUPPORTED_FEATURES,
            session,
        )))
        .await
        .map_err(lost)?;
        let our_ssrc = uuid::Uuid::new_v4().as_u128() as u32;
        let video_offer = offer::h264_only(
            &build_screen_video_offer(&uuid_text(), &call_info(), our_ssrc).map_err(lost)?,
        )?;
        let answer = Box::pin(display.start_media_stream(build_start_video_parameters(
            &host,
            video.local_port(),
            &peer,
            50001,
            video_offer,
            CLIENT_SUPPORTED_FEATURES,
            1,
            session,
        )))
        .await
        .map_err(lost)?;
        let device_port = answer
            .as_dictionary()
            .and_then(|d| d.get("connection"))
            .and_then(|c| c.as_dictionary())
            .and_then(|c| c.get("streamConfig"))
            .and_then(|c| c.as_dictionary())
            .and_then(|c| c.get("SourcePort"))
            .and_then(|p| p.as_signed_integer())
            .and_then(|p| u16::try_from(p).ok())
            .ok_or_else(|| stream_error("answer", "no SourcePort"))?;

        let hid = rsd_client::<UniversalHidServiceClient<Socket>>(adapter, rsd)
            .await
            .map_err(lost)?;
        let indigo = rsd_client::<IndigoHidClient<Socket>>(adapter, rsd)
            .await
            .map_err(lost)?;
        let pasteboard = rsd_client::<PasteboardServiceClient<Socket>>(adapter, rsd)
            .await
            .map_err(lost)?;
        let apps = rsd_client::<AppServiceClient<Socket>>(adapter, rsd)
            .await
            .map_err(lost)?;

        let density = if model.starts_with("iPad") { 2.0 } else { 3.0 };
        let store = Arc::new(FrameStore::new(density));
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let (ended_tx, ended) = watch::channel(false);
        let (datagrams, queue) = mpsc::channel::<Vec<u8>>(DATAGRAM_QUEUE);
        let (feedback, wanted) = mpsc::unbounded_channel::<Feedback>();
        let media_ssrc = Arc::new(AtomicU32::new(0));
        let highest_seq = Arc::new(AtomicU32::new(0));

        let decoder_store = store.clone();
        let decoder_feedback = feedback.clone();
        std::thread::spawn(move || {
            decode(queue, &decoder_store, &decoder_feedback);
        });

        let mut tasks = Vec::new();
        {
            let (video, media_ssrc, highest_seq, events) = (
                video.clone(),
                media_ssrc.clone(),
                highest_seq.clone(),
                events.clone(),
            );
            tasks.push(tokio::spawn(async move {
                loop {
                    match video.recv().await {
                        Ok(dg) if is_rtcp(&dg.data) => {}
                        Ok(dg) => {
                            if let Some(p) = RtpPacket::parse(&dg.data) {
                                media_ssrc.store(p.ssrc, Ordering::Relaxed);
                                highest_seq.store(u32::from(p.sequence_number), Ordering::Relaxed);
                            }
                            if datagrams.send(dg.data).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = ended_tx.send(true);
                let _ = events.send(DeviceEvent::Gone);
            }));
        }
        tasks.push(tokio::spawn(rtcp(
            video,
            device_port,
            our_ssrc,
            media_ssrc,
            highest_seq,
            wanted,
        )));

        let mut first = store.subscribe();
        tokio::time::timeout(FIRST_FRAME_WAIT, first.wait_for(|f| f.seq > 0))
            .await
            .map_err(|_| Error::Timeout {
                waited_ms: FIRST_FRAME_WAIT.as_millis() as u32,
                condition: "the first video frame".into(),
            })?
            .map_err(|_| Error::DeviceGone { id: id.clone() })?;
        let frame = store
            .latest()
            .ok_or_else(|| Error::DeviceGone { id: id.clone() })?;
        let screen = Size {
            w: frame.width as f32 / density,
            h: frame.height as f32 / density,
        };
        Ok(CoreDevice {
            info: DeviceInfo {
                id,
                model: model.to_string(),
                os: "iOS 27".into(),
                screen,
                density,
                capabilities: Self::CAPABILITIES,
            },
            screen,
            density,
            store,
            hid: Mutex::new(hid),
            keyboard: Mutex::new(Keyboard { service: None }),
            indigo: Mutex::new(indigo),
            pasteboard: Mutex::new(pasteboard),
            apps: Mutex::new(apps),
            events,
            ended,
            tasks,
            _tunnel: tunnel,
            _display: display,
        })
    }

    fn lost(&self, e: impl std::fmt::Display) -> Error {
        Error::Transport {
            detail: format!("{}: {e}", self.info.id),
        }
    }

    /// Presses the usages one report at a time, in order, and releases them in reverse: a modifier sent in the
    /// same report as its key was ignored (the pasted text arrived as a plain "V", Shift+A as "a"; iPad, iOS 27.0).
    async fn press_usages(&self, usages: &[u16]) -> Result<()> {
        let mut keyboard = self.keyboard.lock().await;
        let mut hid = self.hid.lock().await;
        if keyboard.service.is_none() {
            keyboard.service = Some(hid.create_main_keyboard().await.map_err(|e| self.lost(e))?);
        }
        let service = keyboard.service.as_mut().expect("created above");
        let keys: Vec<KeyboardUsage> = usages
            .iter()
            .map(|u| KeyboardUsage::new(*u).map_err(|e| self.lost(e)))
            .collect::<Result<_>>()?;
        for held in (1..=keys.len()).chain((0..keys.len()).rev()) {
            hid.set_main_keyboard_usages(service, keys[..held].iter().copied())
                .await
                .map_err(|e| self.lost(e))?;
            tokio::time::sleep(KEY_HOLD).await;
        }
        Ok(())
    }
}

enum Feedback {
    Keyframe,
    FrameDecoded(u32),
}

fn decode(
    mut queue: mpsc::Receiver<Vec<u8>>,
    store: &FrameStore,
    feedback: &mpsc::UnboundedSender<Feedback>,
) {
    let Ok(mut decoder) = Decoder::new() else {
        return;
    };
    let mut depacketizer = Depacketizer::new();
    while let Some(datagram) = queue.blocking_recv() {
        for event in depacketizer.push(&datagram) {
            match event {
                Event::KeyframeNeeded => {
                    let _ = feedback.send(Feedback::Keyframe);
                }
                Event::Unit(unit) => match decoder.decode(&unit.data) {
                    Ok(frame) => {
                        if let Some(frame) = frame {
                            store.publish(frame.rotated_cw());
                        }
                        let _ = feedback.send(Feedback::FrameDecoded(unit.timestamp));
                    }
                    Err(_) => {
                        store.record_decode_error();
                        let _ = feedback.send(Feedback::Keyframe);
                    }
                },
            }
        }
    }
}

/// The receiver reports the device's encoder expects: a liveness report each second, and, as Apple's Device Hub
/// sends them, an acknowledgment per decoded frame and a receiver-control report about every 50 ms. Without the
/// acknowledgments the encoder's long-term references drift away from ours: the status bar updated while the app
/// area stayed on the screen of the first frame (observed on the iPad, 2026-11-06).
async fn rtcp(
    socket: Arc<UdpSocketHandle>,
    device_port: u16,
    our_ssrc: u32,
    media_ssrc: Arc<AtomicU32>,
    highest_seq: Arc<AtomicU32>,
    mut wanted: mpsc::UnboundedReceiver<Feedback>,
) {
    let mut liveness = tokio::time::interval(RTCP_INTERVAL);
    let mut control = tokio::time::interval(RCTL_INTERVAL);
    let started = Instant::now();
    let (mut fir_seq, mut frames, mut base_seq) = (0u8, 0u16, None);
    loop {
        let packet = tokio::select! {
            _ = liveness.tick() => {
                let block = ReportBlock {
                    source_ssrc: media_ssrc.load(Ordering::Relaxed),
                    fraction_lost: 0,
                    cumulative_lost: 0,
                    highest_seq: highest_seq.load(Ordering::Relaxed),
                    jitter: 0,
                    lsr: 0,
                    dlsr: 0,
                };
                build_liveness(our_ssrc, "phone-use", &[block])
            }
            _ = control.tick() => {
                let highest = highest_seq.load(Ordering::Relaxed) as u16;
                let base = *base_seq.get_or_insert(highest);
                build_rctl(our_ssrc, started.elapsed().as_millis() as u16, frames, highest.wrapping_sub(base))
            }
            Some(feedback) = wanted.recv() => match feedback {
                Feedback::Keyframe => {
                    fir_seq = fir_seq.wrapping_add(1);
                    build_keyframe_request(our_ssrc, "phone-use", media_ssrc.load(Ordering::Relaxed), &[], fir_seq)
                }
                Feedback::FrameDecoded(timestamp) => {
                    frames = frames.wrapping_add(1);
                    build_frame_ack(our_ssrc, timestamp)
                }
            },
        };
        if socket.send_to(device_port, packet).await.is_err() {
            return;
        }
    }
}

impl Drop for CoreDevice {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

#[async_trait]
impl Device for CoreDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// No tree without the runner: one window and no nodes, so the session reads the screen with OCR.
    async fn tree(&self) -> Result<RawTree> {
        Ok(RawTree {
            windows: vec![RawWindow {
                id: 0,
                kind: "application".into(),
                package: None,
                layer: 0,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: self.screen.w,
                    h: self.screen.h,
                },
            }],
            nodes: Vec::new(),
        })
    }

    fn current_screen(&self) -> Option<Size> {
        Some(self.screen)
    }

    fn frames(&self) -> Option<watch::Receiver<FrameRef>> {
        Some(self.store.subscribe())
    }

    async fn screenshot(&self, _q: Quality) -> Result<Image> {
        if *self.ended.borrow() {
            return Err(Error::DeviceGone {
                id: self.info.id.clone(),
            });
        }
        let frame = self.store.latest().ok_or_else(|| Error::Transport {
            detail: "no video frame".into(),
        })?;
        let scale = 1.0 / self.density;
        let (width, height) = scaled_size(frame.width, frame.height, scale);
        let bytes = tokio::task::spawn_blocking(move || encode_png(&frame, scale))
            .await
            .map_err(|e| self.lost(e))?
            .map_err(|e| self.lost(e))?;
        Ok(Image {
            width,
            height,
            format: ImageFormat::Png,
            bytes,
        })
    }

    fn events(&self) -> broadcast::Receiver<DeviceEvent> {
        self.events.subscribe()
    }

    async fn gesture(&self, g: Gesture) -> Result<()> {
        let steps = touch::schedule(&g, self.screen);
        let start = Instant::now();
        let mut hid = self.hid.lock().await;
        for step in steps {
            tokio::time::sleep_until(start + Duration::from_millis(u64::from(step.at_ms))).await;
            hid.send_multitouch(&step.contacts, None)
                .await
                .map_err(|e| transport::classify(&self.info.id, &e))?;
        }
        Ok(())
    }

    async fn key(&self, k: Key) -> Result<()> {
        self.press_usages(&[touch::key_usage(k)]).await
    }

    async fn button(&self, b: Button) -> Result<()> {
        let Some(code) = button_usage(b) else {
            return match b {
                Button::Back => self.back().await.map(|_| ()),
                _ => Err(Error::Unsupported {
                    op: Op::Button,
                    reason: "iOS has no such button",
                }),
            };
        };
        let mut indigo = self.indigo.lock().await;
        let lost = |e| transport::classify(&self.info.id, &e);
        indigo
            .send_button(HID_USAGE_PAGE_CONSUMER, code, ButtonState::Down)
            .await
            .map_err(lost)?;
        tokio::time::sleep(KEY_HOLD).await;
        indigo
            .send_button(HID_USAGE_PAGE_CONSUMER, code, ButtonState::Up)
            .await
            .map_err(lost)
    }

    async fn open_url(&self, _url: &Url) -> Result<()> {
        Err(Error::Unsupported {
            op: Op::OpenUrl,
            reason: "the CoreDevice tier cannot open a link yet",
        })
    }

    async fn text(&self, t: TextInput) -> Result<()> {
        match t {
            TextInput::Paste(text) | TextInput::PasteSecret(text) => {
                self.pasteboard
                    .lock()
                    .await
                    .set_text(&text, GENERAL_PASTEBOARD)
                    .await
                    .map_err(|e| transport::classify(&self.info.id, &e))?;
                self.press_usages(&[MOD_GUI, USAGE_V]).await
            }
            TextInput::Keystrokes(text) => {
                for c in text.chars() {
                    let (usage, shift) = touch::char_usage(c).ok_or(Error::Unsupported {
                        op: Op::Text,
                        reason: "keystrokes type US-layout characters only; paste other text",
                    })?;
                    if shift {
                        self.press_usages(&[MOD_SHIFT, usage]).await?;
                    } else {
                        self.press_usages(&[usage]).await?;
                    }
                }
                Ok(())
            }
            TextInput::SetValue(_) => Err(Error::Unsupported {
                op: Op::Text,
                reason: "the CoreDevice tier cannot set a field's value; use paste or keys",
            }),
        }
    }

    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>> {
        let mut pasteboard = self.pasteboard.lock().await;
        let lost = |e| transport::classify(&self.info.id, &e);
        match op {
            ClipboardOp::Set(text) => {
                pasteboard
                    .set_text(&text, GENERAL_PASTEBOARD)
                    .await
                    .map_err(lost)?;
                Ok(None)
            }
            ClipboardOp::Get => Ok(pasteboard
                .get(GENERAL_PASTEBOARD)
                .await
                .map_err(lost)?
                .text()),
        }
    }

    async fn apps(&self) -> Result<Vec<App>> {
        let listed = self
            .apps
            .lock()
            .await
            .list_apps(false, true, false, false, true)
            .await
            .map_err(|e| transport::classify(&self.info.id, &e))?;
        let mut apps: Vec<App> = listed
            .into_iter()
            .map(|a| App {
                id: AppId(a.bundle_identifier),
                name: a.name,
                system: a.is_first_party,
            })
            .collect();
        apps.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        Ok(apps)
    }

    async fn launch(&self, app: &AppId) -> Result<()> {
        self.apps
            .lock()
            .await
            .launch_application(app.0.clone(), &[], false, false, None, None, None)
            .await
            .map_err(|e| transport::classify(&self.info.id, &e))?;
        Ok(())
    }

    async fn foreground(&self) -> Result<Option<AppId>> {
        Ok(None)
    }
}
