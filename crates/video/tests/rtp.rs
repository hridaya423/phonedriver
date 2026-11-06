use phone_use_video::h264::Decoder;
use phone_use_video::rtp::{Depacketizer, Event};

const FIXTURE: &[u8] = include_bytes!("fixtures/ipad_testbed_h264.rtp");

fn packets() -> Vec<&'static [u8]> {
    let mut out = Vec::new();
    let mut rest = FIXTURE;
    while rest.len() >= 4 {
        let n = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        out.push(&rest[4..4 + n]);
        rest = &rest[4 + n..];
    }
    out
}

#[test]
fn the_fixture_has_two_hundred_packets() {
    assert_eq!(packets().len(), 200);
}

#[test]
fn every_access_unit_of_the_capture_decodes_into_a_frame_of_the_stream_size() {
    let mut depacketizer = Depacketizer::new();
    let mut decoder = Decoder::new().unwrap();
    let (mut units, mut frames, mut keyframes) = (0, 0, 0);
    for p in packets() {
        for event in depacketizer.push(p) {
            match event {
                Event::Unit(unit) => {
                    units += 1;
                    keyframes += usize::from(unit.keyframe);
                    if let Some(frame) = decoder.decode(&unit.data).unwrap() {
                        frames += 1;
                        assert_eq!((frame.width, frame.height), (2736, 2064));
                    }
                }
                Event::KeyframeNeeded => panic!("no packet was lost"),
            }
        }
    }
    assert!(keyframes >= 1, "the capture starts with a keyframe");
    assert!(units >= 10, "{units} access units");
    assert_eq!(frames, units, "every access unit is a picture");
}

#[test]
fn a_lost_packet_asks_for_a_keyframe_and_passes_on_no_broken_picture() {
    let mut depacketizer = Depacketizer::new();
    let mut decoder = Decoder::new().unwrap();
    let mut asked = 0;
    let mut since_ask = None;
    for (i, p) in packets().into_iter().enumerate() {
        if i % 50 == 49 {
            continue;
        }
        for event in depacketizer.push(p) {
            match event {
                Event::KeyframeNeeded => {
                    asked += 1;
                    since_ask = Some(0);
                }
                Event::Unit(unit) => {
                    if since_ask == Some(0) {
                        assert!(unit.keyframe, "after a loss the next unit is a keyframe");
                    }
                    since_ask = since_ask.map(|n| n + 1);
                    decoder.decode(&unit.data).unwrap();
                }
            }
        }
    }
    assert!(asked >= 1, "a keyframe was asked for");
}

#[test]
fn a_gap_across_the_sequence_number_wrap_is_still_a_gap() {
    let mut d = Depacketizer::new();
    let single = |seq: u16, ts: u32, marker: bool, nal: u8| {
        let mut p = vec![0x80, if marker { 0xfb } else { 0x7b }];
        p.extend_from_slice(&seq.to_be_bytes());
        p.extend_from_slice(&ts.to_be_bytes());
        p.extend_from_slice(&[0, 0, 0, 1]);
        p.extend_from_slice(&[nal, 0xaa, 0xbb]);
        p
    };
    assert!(matches!(
        d.push(&single(65535, 0, true, 0x65)).as_slice(),
        [Event::Unit(u)] if u.keyframe
    ));
    assert!(matches!(
        d.push(&single(0, 1, true, 0x41)).as_slice(),
        [Event::Unit(u)] if !u.keyframe
    ));
    assert!(matches!(
        d.push(&single(2, 2, true, 0x41)).as_slice(),
        [Event::KeyframeNeeded]
    ));
    assert!(d.push(&single(3, 3, true, 0x41)).is_empty());
    assert!(matches!(
        d.push(&single(4, 4, true, 0x65)).as_slice(),
        [Event::Unit(u)] if u.keyframe
    ));
}
