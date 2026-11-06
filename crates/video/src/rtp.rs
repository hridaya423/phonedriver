//! RTP packets from the iOS display stream to H.264 access units (RFC 6184; architecture 7.2 and 8).

const START_CODE: [u8; 4] = [0, 0, 0, 1];
const NAL_IDR: u8 = 5;
const NAL_STAP_A: u8 = 24;
const NAL_FU_A: u8 = 28;
const SEQ_HALF: u16 = 0x8000;

/// One RTP packet (RFC 3550), its payload borrowed from the datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet<'a> {
    pub payload_type: u8,
    pub marker: bool,
    pub seq: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: &'a [u8],
}

impl<'a> Packet<'a> {
    /// `None` for anything that is not an RTP media packet. The device multiplexes RTCP sender reports onto the
    /// media socket (RFC 5761): their second byte is 192 to 223.
    pub fn parse(buf: &'a [u8]) -> Option<Packet<'a>> {
        if buf.len() < 12 || buf[0] >> 6 != 2 || (192..=223).contains(&buf[1]) {
            return None;
        }
        let csrc = usize::from(buf[0] & 0x0f);
        let mut start = 12 + 4 * csrc;
        if buf[0] & 0x10 != 0 {
            let words = u16::from_be_bytes([*buf.get(start + 2)?, *buf.get(start + 3)?]);
            start += 4 + 4 * usize::from(words);
        }
        let mut end = buf.len();
        if buf[0] & 0x20 != 0 {
            end = end.checked_sub(usize::from(*buf.last()?))?;
        }
        Some(Packet {
            payload_type: buf[1] & 0x7f,
            marker: buf[1] & 0x80 != 0,
            seq: u16::from_be_bytes([buf[2], buf[3]]),
            timestamp: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            ssrc: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            payload: buf.get(start..end)?,
        })
    }
}

/// One picture in Annex-B form, ready for `h264::Decoder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub timestamp: u32,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Unit(Unit),
    /// A packet was lost. Pictures are held back until the next keyframe; the receiver should ask for one.
    KeyframeNeeded,
}

#[derive(Default)]
pub struct Depacketizer {
    next_seq: Option<u16>,
    timestamp: Option<u32>,
    data: Vec<u8>,
    keyframe: bool,
    fragment: Option<Vec<u8>>,
    waiting_for_keyframe: bool,
}

impl Depacketizer {
    pub fn new() -> Depacketizer {
        Depacketizer::default()
    }

    /// Feeds one datagram and returns what it completed. Datagrams that are not RTP are ignored.
    pub fn push(&mut self, datagram: &[u8]) -> Vec<Event> {
        let Some(p) = Packet::parse(datagram) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(expected) = self.next_seq
            && p.seq != expected
        {
            if p.seq.wrapping_sub(expected) >= SEQ_HALF {
                return out;
            }
            self.data.clear();
            self.keyframe = false;
            self.fragment = None;
            self.waiting_for_keyframe = true;
            out.push(Event::KeyframeNeeded);
        }
        self.next_seq = Some(p.seq.wrapping_add(1));
        if self.timestamp != Some(p.timestamp) {
            self.flush(&mut out);
            self.timestamp = Some(p.timestamp);
        }
        self.payload(p.payload);
        if p.marker {
            self.flush(&mut out);
        }
        out
    }

    fn payload(&mut self, payload: &[u8]) {
        let Some(&first) = payload.first() else {
            return;
        };
        if first & 0x80 != 0 {
            for nal in sample_description_parameter_sets(payload) {
                self.nal(nal);
            }
            return;
        }
        match first & 0x1f {
            1..=23 => self.nal(payload),
            NAL_STAP_A => {
                let mut rest = &payload[1..];
                while rest.len() >= 2 {
                    let n = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
                    let Some(nal) = rest.get(2..2 + n) else {
                        return;
                    };
                    self.nal(nal);
                    rest = &rest[2 + n..];
                }
            }
            NAL_FU_A if payload.len() >= 2 => {
                let header = payload[1];
                if header & 0x80 != 0 {
                    let mut nal = vec![(first & 0xe0) | (header & 0x1f)];
                    nal.extend_from_slice(&payload[2..]);
                    self.fragment = Some(nal);
                } else if let Some(nal) = self.fragment.as_mut() {
                    nal.extend_from_slice(&payload[2..]);
                }
                if header & 0x40 != 0
                    && let Some(nal) = self.fragment.take()
                {
                    self.nal(&nal);
                }
            }
            _ => {}
        }
    }

    fn nal(&mut self, nal: &[u8]) {
        if nal.first().map(|b| b & 0x1f) == Some(NAL_IDR) {
            self.keyframe = true;
        }
        self.data.extend_from_slice(&START_CODE);
        self.data.extend_from_slice(nal);
    }

    fn flush(&mut self, out: &mut Vec<Event>) {
        self.fragment = None;
        if self.data.is_empty() {
            return;
        }
        let data = std::mem::take(&mut self.data);
        let keyframe = std::mem::take(&mut self.keyframe);
        if self.waiting_for_keyframe && !keyframe {
            return;
        }
        self.waiting_for_keyframe = false;
        out.push(Event::Unit(Unit {
            timestamp: self.timestamp.unwrap_or_default(),
            keyframe,
            data,
        }));
    }
}

/// The SPS and PPS inside the device's own parameter packet. iOS 27 sends the H.264 parameter sets as an
/// ISO sample description (an `avc1` entry holding an `avcC` record) behind a 4-byte header whose first byte has
/// the NAL forbidden bit set, instead of as SPS and PPS NAL units (observed on an iPad Pro, iOS 27.0, spike I3).
fn sample_description_parameter_sets(payload: &[u8]) -> Vec<&[u8]> {
    let Some(at) = payload.windows(4).position(|w| w == b"avcC") else {
        return Vec::new();
    };
    let record = &payload[at + 4..];
    let mut out = Vec::new();
    let mut pos = 5;
    for _ in 0..2 {
        let Some(&count) = record.get(pos) else {
            return out;
        };
        pos += 1;
        let count = if out.is_empty() { count & 0x1f } else { count };
        for _ in 0..count {
            let Some(len) = record.get(pos..pos + 2) else {
                return out;
            };
            let len = usize::from(u16::from_be_bytes([len[0], len[1]]));
            let Some(nal) = record.get(pos + 2..pos + 2 + len) else {
                return out;
            };
            out.push(nal);
            pos += 2 + len;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_extension_and_padding_are_not_payload() {
        let mut p = vec![0xb0, 0xe4, 0, 7, 0, 0, 0, 9, 0, 0, 0, 1];
        p.extend_from_slice(&[0x93, 0x11, 0, 1, 0xde, 0xad, 0xbe, 0xef]);
        p.extend_from_slice(&[0x65, 1, 2, 0, 0, 3]);
        let parsed = Packet::parse(&p).unwrap();
        assert_eq!(parsed.payload, &[0x65, 1, 2]);
        assert_eq!((parsed.seq, parsed.timestamp, parsed.marker), (7, 9, true));
        assert_eq!(parsed.payload_type, 100);
    }

    #[test]
    fn the_sample_description_yields_its_sps_and_pps() {
        let mut payload = vec![0x92, 0xe6, 0xc0, 0xa3, 0, 0, 0, 0x2c];
        payload.extend_from_slice(b"avcC");
        payload.extend_from_slice(&[1, 0x64, 0, 0x34, 0xff, 0xe1, 0, 3, 0x27, 0x64, 0]);
        payload.extend_from_slice(&[1, 0, 2, 0x28, 0xee]);
        assert_eq!(
            sample_description_parameter_sets(&payload),
            vec![&[0x27, 0x64, 0][..], &[0x28, 0xee][..]]
        );
    }

    #[test]
    fn a_stap_a_packet_becomes_one_unit_of_its_nals() {
        let mut d = Depacketizer::new();
        let mut p = vec![0x80, 0xfb, 0, 1, 0, 0, 0, 5, 0, 0, 0, 1, 0x18];
        p.extend_from_slice(&[0, 2, 0x67, 0xaa, 0, 2, 0x68, 0xbb, 0, 2, 0x65, 0xcc]);
        let out = d.push(&p);
        let [Event::Unit(u)] = out.as_slice() else {
            panic!("{out:?}")
        };
        assert!(u.keyframe);
        assert_eq!(
            u.data,
            [
                0, 0, 0, 1, 0x67, 0xaa, 0, 0, 0, 1, 0x68, 0xbb, 0, 0, 0, 1, 0x65, 0xcc
            ]
        );
    }

    #[test]
    fn a_sender_report_on_the_media_socket_is_not_a_media_packet() {
        let report = hex("81c8000c694d2e40ee986fbb99cbc800000059a60000006f000155dcc573ccbc");
        assert_eq!(Packet::parse(&report), None);
        let mut d = Depacketizer::new();
        let media = |seq: u16| {
            vec![
                0x80,
                0xfb,
                (seq >> 8) as u8,
                seq as u8,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0x41,
                0xaa,
            ]
        };
        assert_eq!(d.push(&media(10)).len(), 1);
        assert!(d.push(&report).is_empty());
        assert_eq!(d.push(&media(11)).len(), 1);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn a_late_duplicate_is_dropped_without_asking_for_a_keyframe() {
        let mut d = Depacketizer::new();
        let p = |seq: u16| {
            let mut p = vec![0x80, 0xfb];
            p.extend_from_slice(&seq.to_be_bytes());
            p.extend_from_slice(&[0, 0, 0, seq as u8, 0, 0, 0, 1, 0x41, 0xaa]);
            p
        };
        assert_eq!(d.push(&p(10)).len(), 1);
        assert_eq!(d.push(&p(11)).len(), 1);
        assert!(d.push(&p(10)).is_empty());
        assert_eq!(d.push(&p(12)).len(), 1);
    }
}
