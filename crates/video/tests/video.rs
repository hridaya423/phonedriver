use std::time::Duration;

use phone_use_video::frame_store::{Frame, FrameStore, StreamState};
use phone_use_video::h264::{self, Decoder, WirePacket};
use phone_use_video::image::encode_png;
use phone_use_video::stream::VideoStream;

const DENSITY: f32 = 2.625;
const FIXTURE: &[u8] = include_bytes!("fixtures/settings_scroll.h264");

fn flat(w: u32, h: u32, luma: u8) -> Frame {
    let (w, h) = (w as usize, h as usize);
    Frame::from_i420(
        w as u32,
        h as u32,
        vec![luma; w * h],
        vec![128; w * h / 4],
        vec![128; w * h / 4],
    )
}

async fn fixture_frames() -> (h264::CodecHeader, Vec<Frame>) {
    let mut r = FIXTURE;
    let header = h264::read_header(&mut r).await.unwrap();
    let mut decoder = Decoder::new().unwrap();
    let mut frames = Vec::new();
    loop {
        match h264::read_packet(&mut r).await {
            Ok(WirePacket::Access(unit)) => {
                if let Some(f) = decoder.decode(&unit.data).unwrap() {
                    frames.push(f);
                }
            }
            Ok(WirePacket::Reconfigure(_)) => panic!("the fixture never changes size"),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => panic!("{e}"),
        }
    }
    (header, frames)
}

#[tokio::test]
async fn fixture_decodes_to_59_frames_of_the_header_size() {
    let (header, frames) = fixture_frames().await;
    assert_eq!((header.width, header.height), (1080, 2424));
    assert_eq!(frames.len(), 59);
    assert!(frames.iter().all(|f| (f.width, f.height) == (1080, 2424)));
}

#[test]
fn publish_increments_seq_and_the_watch_sees_only_the_newest() {
    let store = FrameStore::new(DENSITY);
    let mut rx = store.subscribe();
    assert_eq!(rx.borrow().seq, 0);
    assert!(store.latest().is_none());
    assert_eq!(store.publish(flat(108, 240, 50)).seq, 1);
    assert_eq!(store.publish(flat(108, 240, 50)).seq, 2);
    assert_eq!(store.publish(flat(108, 240, 50)).seq, 3);
    assert_eq!(rx.borrow_and_update().seq, 3);
    assert!(!rx.has_changed().unwrap());
    assert_eq!(store.latest().unwrap().seq, 3);
}

#[test]
fn identical_frames_change_nothing() {
    let store = FrameStore::new(DENSITY);
    store.publish(flat(1080, 2400, 90));
    let again = store.publish(flat(1080, 2400, 90));
    assert_eq!(again.changed_area_pt2, 0.0);
    assert_eq!(again.changed_box, None);
}

#[test]
fn a_region_change_reports_its_area_and_box_in_points() {
    let store = FrameStore::new(DENSITY);
    store.publish(flat(1080, 2400, 20));
    let mut y = vec![20u8; 1080 * 2400];
    for row in 600..1200 {
        for col in 100..700 {
            y[row * 1080 + col] = 200;
        }
    }
    let changed = store.publish(Frame::from_i420(
        1080,
        2400,
        y,
        vec![128; 1080 * 2400 / 4],
        vec![128; 1080 * 2400 / 4],
    ));
    let area_pt2 = (600.0 / DENSITY) * (600.0 / DENSITY);
    assert!(
        (changed.changed_area_pt2 - area_pt2).abs() < area_pt2 * 0.1,
        "{} vs {area_pt2}",
        changed.changed_area_pt2
    );
    let b = changed.changed_box.unwrap();
    assert!((b.x - 100.0 / DENSITY).abs() < 4.0 && (b.y - 600.0 / DENSITY).abs() < 4.0);
    assert!((b.w - 600.0 / DENSITY).abs() < 8.0 && (b.h - 600.0 / DENSITY).abs() < 8.0);
}

#[test]
fn a_size_change_counts_as_the_whole_screen_changing() {
    let store = FrameStore::new(DENSITY);
    store.publish(flat(1080, 2400, 20));
    let rotated = store.publish(flat(2400, 1080, 20));
    let b = rotated.changed_box.unwrap();
    assert_eq!((b.x, b.y), (0.0, 0.0));
    assert!((b.w - 2400.0 / DENSITY).abs() < 1.0 && (b.h - 1080.0 / DENSITY).abs() < 1.0);
    assert!(rotated.changed_area_pt2 > 4096.0);
}

#[tokio::test]
async fn scrolled_fixture_frames_exceed_the_significant_area_and_the_box_covers_the_list() {
    let (_, frames) = fixture_frames().await;
    let store = FrameStore::new(DENSITY);
    let mut best = None;
    let mut previous = None;
    for f in frames {
        let r = store.publish(f);
        if best.is_none_or(|b: phone_use_core::device::FrameRef| {
            r.changed_area_pt2 > b.changed_area_pt2
        }) {
            best = Some(r);
        }
        previous = Some(r);
    }
    assert!(previous.is_some());
    let best = best.unwrap();
    assert!(best.changed_area_pt2 > 4096.0, "{best:?}");
    let b = best.changed_box.unwrap();
    let (screen_w, screen_h) = (1080.0 / DENSITY, 2424.0 / DENSITY);
    assert!(b.w > screen_w * 0.5, "{b:?}");
    assert!(b.h > screen_h * 0.3, "{b:?}");
    assert!(b.y > 100.0, "the box starts below the status bar: {b:?}");
    assert!(b.y + b.h < screen_h, "{b:?}");
}

#[tokio::test]
async fn first_and_last_fixture_frames_differ_by_more_than_a_region() {
    let (_, frames) = fixture_frames().await;
    let store = FrameStore::new(DENSITY);
    store.publish(frames[0].clone());
    let last = store.publish(frames[frames.len() - 1].clone());
    assert!(last.changed_area_pt2 > 4096.0, "{last:?}");
}

#[tokio::test(start_paused = true)]
async fn no_frame_for_one_second_is_stalled() {
    let store = FrameStore::new(DENSITY);
    assert_eq!(store.state(), StreamState::Live);
    tokio::time::advance(Duration::from_millis(999)).await;
    assert_eq!(store.state(), StreamState::Live);
    tokio::time::advance(Duration::from_millis(2)).await;
    assert_eq!(store.state(), StreamState::Stalled);
    store.publish(flat(108, 240, 10));
    assert_eq!(store.state(), StreamState::Live);
    tokio::time::advance(Duration::from_millis(1001)).await;
    assert_eq!(store.state(), StreamState::Stalled);
}

#[test]
fn png_at_scale_one_over_density_has_the_point_size() {
    let frame = flat(1080, 2400, 120);
    let png = encode_png(&frame, 1.0 / DENSITY).unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(&png));
    let info = decoder.read_info().unwrap();
    let (w, h) = (info.info().width as i64, info.info().height as i64);
    assert!((w - 412).abs() <= 1 && (h - 915).abs() <= 1, "{w}x{h}");
}

#[test]
fn png_pixels_follow_the_luma_and_chroma() {
    let (w, h) = (64usize, 64usize);
    let y = vec![235u8; w * h];
    let u = vec![128u8; w * h / 4];
    let v = vec![128u8; w * h / 4];
    let png = encode_png(&Frame::from_i420(64, 64, y, u, v), 1.0).unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(&png))
        .read_info()
        .unwrap();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut buf).unwrap();
    assert!(
        buf.iter().all(|&b| b >= 253),
        "white expected, got {:?}",
        &buf[..6]
    );
}

#[tokio::test]
async fn the_pipeline_decodes_every_packet_without_dropping_any() {
    let stream = VideoStream::spawn(FIXTURE, FrameStore::new(DENSITY));
    stream.finished().await;
    assert_eq!(stream.store.latest().unwrap().seq, 59);
    assert_eq!(stream.store.decode_errors(), 0);
    let f = stream.store.latest().unwrap();
    assert_eq!((f.width, f.height), (1080, 2424));
}

#[test]
fn a_region_is_black_when_every_sample_in_it_is_at_the_black_level() {
    use phone_use_core::geometry::Rect;
    let store = FrameStore::new(DENSITY);
    let region = Rect {
        x: 50.0,
        y: 100.0,
        w: 300.0,
        h: 400.0,
    };
    assert!(!store.region_is_black(region), "no frame yet");
    let mut y = vec![16u8; 1080 * 2400];
    store.publish(Frame::from_i420(
        1080,
        2400,
        y.clone(),
        vec![128; 1080 * 2400 / 4],
        vec![128; 1080 * 2400 / 4],
    ));
    assert!(store.region_is_black(region));
    for row in 600..700 {
        for col in 400..700 {
            y[row * 1080 + col] = 220;
        }
    }
    store.publish(Frame::from_i420(
        1080,
        2400,
        y,
        vec![128; 1080 * 2400 / 4],
        vec![128; 1080 * 2400 / 4],
    ));
    assert!(
        !store.region_is_black(region),
        "a bright patch inside the region"
    );
    let elsewhere = Rect {
        x: 5.0,
        y: 5.0,
        w: 60.0,
        h: 60.0,
    };
    assert!(
        store.region_is_black(elsewhere),
        "the patch is outside this region"
    );
}

#[test]
fn a_downscaled_frame_reports_its_change_in_screen_points() {
    let store = FrameStore::fitted(480.0);
    store.publish(flat(108, 240, 50));
    let (w, h) = (108usize, 240usize);
    let mut y = vec![50u8; w * h];
    for row in 120..240 {
        for col in 54..108 {
            y[row * w + col] = 200;
        }
    }
    let changed = store.publish(Frame::from_i420(
        108,
        240,
        y,
        vec![128; w * h / 4],
        vec![128; w * h / 4],
    ));
    let b = changed.changed_box.expect("a change");
    assert_eq!((b.x, b.y, b.w, b.h), (108.0, 240.0, 108.0, 240.0));
}
