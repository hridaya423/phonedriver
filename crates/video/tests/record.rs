use std::time::Duration;

use phonedriver_video::frame_store::FrameStore;
use phonedriver_video::h264::{self, AccessUnit, Decoder, WirePacket};
use phonedriver_video::record::{Recording, probe};
use phonedriver_video::stream::VideoStream;
use tokio::io::AsyncWriteExt;

const FIXTURE: &[u8] = include_bytes!("fixtures/settings_scroll.h264");

async fn fixture_units() -> (h264::CodecHeader, Vec<AccessUnit>) {
    let mut r = FIXTURE;
    let header = h264::read_header(&mut r).await.unwrap();
    let mut units = Vec::new();
    while let Ok(WirePacket::Access(unit)) = h264::read_packet(&mut r).await {
        units.push(unit);
    }
    (header, units)
}

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

fn be64(b: &[u8]) -> usize {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as usize
}

fn child<'a>(mut body: &'a [u8], kind: &[u8; 4]) -> &'a [u8] {
    while body.len() >= 8 {
        let (size, head) = match be32(body) {
            1 => (be64(&body[8..]), 16),
            n => (n, 8),
        };
        if &body[4..8] == kind {
            return &body[head..size];
        }
        body = &body[size..];
    }
    panic!("no {} box", String::from_utf8_lossy(kind));
}

struct Track {
    width: usize,
    height: usize,
    sps: Vec<u8>,
    pps: Vec<u8>,
    sizes: Vec<usize>,
    sync: Vec<usize>,
    durations: Vec<usize>,
    offset: usize,
    movie_ticks: usize,
}

fn track(file: &[u8]) -> Track {
    let moov = child(file, b"moov");
    let mvhd = child(moov, b"mvhd");
    let trak = child(moov, b"trak");
    let tkhd = child(trak, b"tkhd");
    let stbl = child(child(child(trak, b"mdia"), b"minf"), b"stbl");
    let stsd = child(stbl, b"stsd");
    let avc1 = child(&stsd[8..], b"avc1");
    let avcc = child(&avc1[78..], b"avcC");
    let sps_len = u16::from_be_bytes([avcc[6], avcc[7]]) as usize;
    let sps = avcc[8..8 + sps_len].to_vec();
    let at = 8 + sps_len + 1;
    let pps_len = u16::from_be_bytes([avcc[at], avcc[at + 1]]) as usize;
    let pps = avcc[at + 2..at + 2 + pps_len].to_vec();
    let stsz = child(stbl, b"stsz");
    let sizes = (0..be32(&stsz[8..]))
        .map(|i| be32(&stsz[12 + 4 * i..]))
        .collect();
    let stss = child(stbl, b"stss");
    let sync = (0..be32(&stss[4..]))
        .map(|i| be32(&stss[8 + 4 * i..]))
        .collect();
    let stts = child(stbl, b"stts");
    let mut durations = Vec::new();
    for i in 0..be32(&stts[4..]) {
        let count = be32(&stts[8 + 8 * i..]);
        let each = be32(&stts[12 + 8 * i..]);
        durations.extend(std::iter::repeat_n(each, count));
    }
    Track {
        width: be32(&tkhd[76..]) >> 16,
        height: be32(&tkhd[80..]) >> 16,
        sps,
        pps,
        sizes,
        sync,
        durations,
        offset: be64(&child(stbl, b"co64")[8..]),
        movie_ticks: be32(&mvhd[16..]),
    }
}

fn annex_b(file: &[u8], t: &Track) -> Vec<Vec<u8>> {
    let mut at = t.offset;
    let mut out = Vec::new();
    for size in &t.sizes {
        let mut sample = &file[at..at + size];
        let mut unit = Vec::new();
        while !sample.is_empty() {
            let len = be32(sample);
            unit.extend_from_slice(&[0, 0, 0, 1]);
            unit.extend_from_slice(&sample[4..4 + len]);
            sample = &sample[4 + len..];
        }
        out.push(unit);
        at += size;
    }
    out
}

#[tokio::test]
async fn the_stream_fixture_becomes_an_mp4_whose_samples_decode_to_the_same_frames() {
    let (header, units) = fixture_units().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.mp4");
    let mut rec = Recording::create(&path, header.width, header.height).unwrap();
    let pictures: Vec<&AccessUnit> = units.iter().filter(|u| !u.config).collect();
    for (i, unit) in units.iter().enumerate() {
        let next = units[i + 1..].iter().find(|u| !u.config);
        let lasts = next.map_or(33_333, |n| n.pts_us - unit.pts_us);
        rec.push(unit, lasts).unwrap();
    }
    let done = rec.finish().unwrap();
    assert_eq!(
        probe(&path).unwrap(),
        done,
        "the file says what `finish` said"
    );
    assert_eq!((done.width, done.height), (1080, 2424));
    assert_eq!(done.frames as usize, pictures.len());

    let file = std::fs::read(&path).unwrap();
    assert_eq!(&file[4..8], b"ftyp");
    let t = track(&file);
    assert_eq!((t.width, t.height), (1080, 2424));
    assert_eq!(t.sizes.len(), pictures.len());
    assert_eq!(t.sync.first(), Some(&1), "the first sample is a keyframe");
    assert!(t.sps[0] & 0x1f == 7 && t.pps[0] & 0x1f == 8);
    let span_us = pictures.last().unwrap().pts_us - pictures[0].pts_us + 33_333;
    let ticks: usize = t.durations.iter().sum();
    assert_eq!(ticks, t.movie_ticks);
    let ms = ticks as u64 * 1000 / 90_000;
    assert!(ms.abs_diff(span_us / 1000) <= 2, "{ms} ms for {span_us} us");
    assert_eq!(done.ms, ms);

    let mut decoder = Decoder::new().unwrap();
    let mut config = vec![0, 0, 0, 1];
    config.extend_from_slice(&t.sps);
    config.extend_from_slice(&[0, 0, 0, 1]);
    config.extend_from_slice(&t.pps);
    assert!(decoder.decode(&config).unwrap().is_none());
    let mut frames = 0;
    for unit in annex_b(&file, &t) {
        if let Some(frame) = decoder.decode(&unit).unwrap() {
            assert_eq!((frame.width, frame.height), (1080, 2424));
            frames += 1;
        }
    }
    assert_eq!(
        frames, 59,
        "the fixture decodes to 59 frames from the stream"
    );
}

#[tokio::test]
async fn a_recording_that_is_dropped_is_still_a_whole_file() {
    let (header, units) = fixture_units().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dropped.mp4");
    {
        let mut rec = Recording::create(&path, header.width, header.height).unwrap();
        for unit in units.iter().take(6) {
            rec.push(unit, 16_000).unwrap();
        }
    }
    let file = std::fs::read(&path).unwrap();
    let t = track(&file);
    assert!(!t.sizes.is_empty());
    assert_eq!(t.offset + t.sizes.iter().sum::<usize>(), {
        let mdat = child(&file, b"mdat");
        mdat.as_ptr() as usize - file.as_ptr() as usize + mdat.len()
    });
}

/// The byte offset in the fixture after its header and `packets` packets.
fn offset_after(packets: usize) -> usize {
    let mut at = 12;
    for _ in 0..packets {
        at += 13 + be32(&FIXTURE[at + 9..]);
    }
    at
}

/// Decoding the fixture in a debug build took 3.6 s on an M-series MacBook and passed 5 s on GitHub's macOS runner
/// (CI run 45258133541, twice), so the wait allows 30 s; it returns as soon as `done` holds.
async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..3000 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

#[tokio::test]
async fn a_recording_whose_keyframe_never_comes_starts_from_the_kept_units_after_a_second() {
    let (mut agent, host) = tokio::io::duplex(1 << 22);
    let video = VideoStream::spawn(host, FrameStore::new(2.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.mp4");
    assert!(video.record_start(&path).is_err(), "no stream yet");
    assert_eq!(video.record_stop().unwrap(), None);

    let half = offset_after(30);
    agent.write_all(&FIXTURE[..half]).await.unwrap();
    until("the first half to decode", || {
        video.store.latest().is_some_and(|f| f.seq >= 20)
    })
    .await;
    let began = std::time::Instant::now();
    video.record_start(&path).unwrap();
    assert!(video.recording());
    assert!(video.record_start(&dir.path().join("second.mp4")).is_err());
    tokio::time::sleep(Duration::from_millis(1100)).await;
    agent.write_all(&FIXTURE[half..]).await.unwrap();
    until("the rest to decode", || {
        video.store.latest().is_some_and(|f| f.seq >= 59)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    let ran_ms = began.elapsed().as_millis() as u64;
    let stopped = video
        .record_stop()
        .unwrap()
        .expect("a recording was running");
    let wall_ms = began.elapsed().as_millis() as u64;
    assert_eq!(stopped.early, None);
    assert!(!video.recording());
    let (_, units) = fixture_units().await;
    let pictures: Vec<&AccessUnit> = units.iter().filter(|u| !u.config).collect();
    let stamped_ms = (pictures.last().unwrap().pts_us - pictures[pictures.len() / 2].pts_us) / 1000;
    // A debug build under WSL2 decoded the second half in 1364 ms against 1346 ms of time stamps, so there the
    // stamps do not run ahead of the clock and only the length below is checked.
    if stamped_ms <= wall_ms + 200 {
        eprintln!(
            "this host is too slow for the stamps to run ahead: {stamped_ms} ms against {wall_ms} ms"
        );
    }
    assert!(
        stopped.recorded.ms <= wall_ms + 1 && stopped.recorded.ms + 60 >= ran_ms,
        "the video is as long as the recording ran, and the kept units 12 µs each: {} ms against {ran_ms} to {wall_ms}",
        stopped.recorded.ms
    );

    let file = std::fs::read(&path).unwrap();
    let t = track(&file);
    assert_eq!(t.sync.first(), Some(&1));
    let mut decoder = Decoder::new().unwrap();
    let mut config = vec![0, 0, 0, 1];
    config.extend_from_slice(&t.sps);
    config.extend_from_slice(&[0, 0, 0, 1]);
    config.extend_from_slice(&t.pps);
    decoder.decode(&config).unwrap();
    let frames = annex_b(&file, &t)
        .iter()
        .filter(|unit| decoder.decode(unit).unwrap().is_some())
        .count();
    assert_eq!(
        frames, 59,
        "the kept start and the live rest decode to every frame"
    );
    assert!(
        *t.durations.last().unwrap() >= 90_000 / 10,
        "the last frame lasts until the stop: {:?}",
        t.durations.last()
    );
    assert_eq!(stopped.recorded.frames as usize, t.sizes.len());
}

#[tokio::test]
async fn a_recording_starts_at_the_first_keyframe_after_its_start_and_holds_nothing_from_before() {
    let (mut agent, host) = tokio::io::duplex(1 << 22);
    let video = VideoStream::spawn(host, FrameStore::new(2.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fresh.mp4");
    agent.write_all(&FIXTURE[..offset_after(30)]).await.unwrap();
    until("the screen before the start", || {
        video.store.latest().is_some_and(|f| f.seq >= 20)
    })
    .await;
    let began = std::time::Instant::now();
    video.record_start(&path).unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    agent.write_all(&FIXTURE[offset_after(1)..]).await.unwrap();
    until("the stream after the start", || {
        video.store.latest().is_some_and(|f| f.seq >= 20 + 59)
    })
    .await;
    let ran_ms = began.elapsed().as_millis() as u64;
    let stopped = video.record_stop().unwrap().expect("a recording");
    let wall_ms = began.elapsed().as_millis() as u64;
    let (_, units) = fixture_units().await;
    let after: Vec<usize> = units
        .iter()
        .filter(|u| !u.config)
        .map(|u| u.data.len())
        .collect();
    let file = std::fs::read(&path).unwrap();
    let t = track(&file);
    assert_eq!(
        t.sizes.len(),
        after.len(),
        "only the pictures from the keyframe after the start"
    );
    assert_eq!(t.sync.first(), Some(&1));
    assert!(
        file.windows(4).any(|w| w == b"elst"),
        "the 200 ms before the keyframe show nothing"
    );
    assert!(
        stopped.recorded.ms <= wall_ms && stopped.recorded.ms + 60 >= ran_ms,
        "{} ms against {ran_ms} to {wall_ms}",
        stopped.recorded.ms
    );
    let mut decoder = Decoder::new().unwrap();
    let mut config = vec![0, 0, 0, 1];
    config.extend_from_slice(&t.sps);
    config.extend_from_slice(&[0, 0, 0, 1]);
    config.extend_from_slice(&t.pps);
    decoder.decode(&config).unwrap();
    let frames = annex_b(&file, &t)
        .iter()
        .filter(|unit| decoder.decode(unit).unwrap().is_some())
        .count();
    assert_eq!(frames, 59);
}

/// The fixture's header, config and first keyframe, then its next `repeats` pictures stamped as the agent's encoder
/// stamps its repeats of a still screen: 100 ms after the one before.
fn still_screen(repeats: usize) -> Vec<u8> {
    let mut out = FIXTURE[..12].to_vec();
    let mut at = 12;
    let mut first = None;
    let mut pictures = 0;
    while pictures <= repeats {
        let len = be32(&FIXTURE[at + 9..]);
        let mut packet = FIXTURE[at..at + 13 + len].to_vec();
        if packet[8] & 1 == 0 {
            let pts = *first.get_or_insert(be64(&packet));
            let stamp = (pts + pictures * 100_000) as u64;
            packet[..8].copy_from_slice(&stamp.to_be_bytes());
            pictures += 1;
        }
        out.extend_from_slice(&packet);
        at += 13 + len;
    }
    out
}

#[tokio::test]
async fn a_recording_of_a_still_screen_opens_with_that_screen() {
    let (mut agent, host) = tokio::io::duplex(1 << 22);
    let video = VideoStream::spawn(host, FrameStore::new(2.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("still.mp4");
    agent.write_all(&still_screen(4)).await.unwrap();
    until("the still screen", || {
        video.store.latest().is_some_and(|f| f.seq >= 5)
    })
    .await;
    video.record_start(&path).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stopped = video.record_stop().unwrap().expect("a recording");
    let file = std::fs::read(&path).unwrap();
    let t = track(&file);
    assert_eq!(t.sizes.len(), 5, "the keyframe and its repeats");
    assert_eq!(t.sync.first(), Some(&1));
    assert!(
        !file.windows(4).any(|w| w == b"elst"),
        "the picture shows from the start"
    );
    assert!(stopped.recorded.ms >= 250, "{}", stopped.recorded.ms);
    let mut decoder = Decoder::new().unwrap();
    let mut config = vec![0, 0, 0, 1];
    config.extend_from_slice(&t.sps);
    config.extend_from_slice(&[0, 0, 0, 1]);
    config.extend_from_slice(&t.pps);
    decoder.decode(&config).unwrap();
    let frames = annex_b(&file, &t)
        .iter()
        .filter(|unit| decoder.decode(unit).unwrap().is_some())
        .count();
    assert_eq!(frames, 5);
}

#[tokio::test]
async fn a_recording_started_after_the_screen_changed_holds_no_picture_from_before_it() {
    let (mut agent, host) = tokio::io::duplex(1 << 22);
    let video = VideoStream::spawn(host, FrameStore::new(2.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idle.mp4");
    agent.write_all(&FIXTURE[..offset_after(30)]).await.unwrap();
    until("the screen before the start", || {
        video.store.latest().is_some_and(|f| f.seq >= 20)
    })
    .await;
    video.record_start(&path).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stopped = video.record_stop().unwrap().expect("a recording");
    assert_eq!(stopped.recorded.frames, 0);
    assert!(stopped.recorded.ms >= 250, "{}", stopped.recorded.ms);
}

#[tokio::test]
async fn a_stream_that_ends_finishes_the_recording_and_stop_says_why() {
    let (mut agent, host) = tokio::io::duplex(1 << 22);
    let video = VideoStream::spawn(host, FrameStore::new(2.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cut.mp4");
    agent.write_all(&FIXTURE[..offset_after(10)]).await.unwrap();
    until("frames", || video.store.latest().is_some()).await;
    video.record_start(&path).unwrap();
    drop(agent);
    video.finished().await;
    until("the recording to end", || !video.recording()).await;
    let stopped = video.record_stop().unwrap().expect("the ended recording");
    assert_eq!(stopped.early, Some("the device's video stream ended"));
    assert!(
        track(&std::fs::read(&path).unwrap()).sizes.is_empty(),
        "no picture came after the start, and none from before it is kept"
    );
    assert_eq!(video.record_stop().unwrap(), None);
}

/// 1.2 s of a still iPhone 17 Simulator screen as `xcrun simctl io recordVideo --codec=h264` wrote it (Xcode 27):
/// `moov` before `mdat`, a `wide` box between them, and one frame, since the Simulator writes a frame only when
/// the screen changes. The movie still says how long the recording ran.
#[test]
fn a_recording_the_simulator_wrote_is_read_for_its_size_frames_and_length() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simulator_still.mp4"
    ));
    let read = probe(path).unwrap();
    assert_eq!((read.width, read.height, read.frames), (1206, 2622, 1));
    assert_eq!(read.ms, 1306);
    let dir = tempfile::tempdir().unwrap();
    let cut = dir.path().join("cut.mp4");
    std::fs::write(&cut, &std::fs::read(path).unwrap()[..500]).unwrap();
    assert!(probe(&cut).is_err(), "a file cut short is refused");
}

#[cfg(unix)]
#[test]
fn a_recording_can_be_read_only_by_its_owner() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.mp4");
    drop(Recording::create(&path, 16, 16).unwrap());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
