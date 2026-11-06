//! The display-stream offer restricted to H.264 (decision D3, spike I3).

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use phone_use_core::error::Error;

pub const H264_PAYLOAD: u64 = 123;
pub const HEVC_PAYLOAD: u64 = 100;

const MEDIA_BLOB: &str = "avcMediaStreamNegotiatorMediaBlob";
const STREAM_GROUP: u64 = 5;
const PAYLOAD_BANK: u64 = 3;

fn bad(what: &str) -> Error {
    Error::Transport {
        detail: format!("the display offer is not what idevice builds: {what}"),
    }
}

struct Field {
    number: u64,
    wire: u64,
    bytes: Vec<u8>,
}

fn varint(buf: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        let byte = *buf.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        if shift > 63 {
            return None;
        }
    }
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

fn fields(buf: &[u8]) -> Option<Vec<Field>> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < buf.len() {
        let key = varint(buf, &mut at)?;
        let start = at;
        let bytes = match key & 7 {
            0 => {
                varint(buf, &mut at)?;
                buf[start..at].to_vec()
            }
            1 => {
                at += 8;
                buf.get(start..at)?.to_vec()
            }
            5 => {
                at += 4;
                buf.get(start..at)?.to_vec()
            }
            2 => {
                let len = usize::try_from(varint(buf, &mut at)?).ok()?;
                at = at.checked_add(len)?;
                buf.get(at - len..at)?.to_vec()
            }
            _ => return None,
        };
        out.push(Field {
            number: key >> 3,
            wire: key & 7,
            bytes,
        });
    }
    Some(out)
}

fn encode(fields: &[Field]) -> Vec<u8> {
    let mut out = Vec::new();
    for f in fields {
        put_varint(&mut out, f.number << 3 | f.wire);
        if f.wire == 2 {
            put_varint(&mut out, f.bytes.len() as u64);
        }
        out.extend_from_slice(&f.bytes);
    }
    out
}

fn payload_of(bank: &Field) -> Option<u64> {
    let body = fields(&bank.bytes)?;
    let first = body.first()?;
    let mut at = 0;
    varint(&first.bytes, &mut at)
}

/// Drops every payload bank except H.264 (RTP payload type 123) from an offer built by idevice's
/// `build_screen_video_offer`, so the device can only answer with H.264.
pub fn h264_only(offer: &[u8]) -> Result<Vec<u8>, Error> {
    let mut dict: plist::Dictionary = plist::from_bytes(offer).map_err(|_| bad("not a plist"))?;
    let compressed = dict
        .get(MEDIA_BLOB)
        .and_then(plist::Value::as_data)
        .ok_or_else(|| bad("no media blob"))?;
    let mut raw = Vec::new();
    ZlibDecoder::new(compressed)
        .read_to_end(&mut raw)
        .map_err(|_| bad("blob is not zlib"))?;
    let mut top = fields(&raw).ok_or_else(|| bad("blob is not protobuf"))?;
    let mut kept = 0;
    for group in top.iter_mut().filter(|f| f.number == STREAM_GROUP) {
        let mut inner = fields(&group.bytes).ok_or_else(|| bad("stream group"))?;
        inner.retain(|f| {
            let bank = f.number == PAYLOAD_BANK && f.wire == 2;
            let keep = !bank || payload_of(f) == Some(H264_PAYLOAD);
            kept += usize::from(bank && keep);
            keep
        });
        group.bytes = encode(&inner);
    }
    if kept == 0 {
        return Err(bad("no H.264 bank"));
    }
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(&encode(&top))
        .map_err(|_| bad("recompression"))?;
    let blob = zlib.finish().map_err(|_| bad("recompression"))?;
    dict.insert(MEDIA_BLOB.into(), plist::Value::Data(blob));
    let mut out = Vec::new();
    plist::to_writer_binary(&mut out, &plist::Value::Dictionary(dict))
        .map_err(|_| bad("plist encoding"))?;
    Ok(out)
}

#[cfg(test)]
fn banks(offer: &[u8]) -> Vec<u64> {
    let dict: plist::Dictionary = plist::from_bytes(offer).unwrap();
    let mut raw = Vec::new();
    ZlibDecoder::new(dict[MEDIA_BLOB].as_data().unwrap())
        .read_to_end(&mut raw)
        .unwrap();
    fields(&raw)
        .unwrap()
        .iter()
        .filter(|f| f.number == STREAM_GROUP)
        .flat_map(|g| fields(&g.bytes).unwrap())
        .filter(|f| f.number == PAYLOAD_BANK && f.wire == 2)
        .map(|f| payload_of(&f).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use idevice::core_device::{CallInfoBlob, build_screen_video_offer};

    use super::*;

    fn default_offer() -> Vec<u8> {
        let info = CallInfoBlob {
            call_id: 0,
            client_version: 1,
            device_type: "Mac17,7".into(),
            framework_version: "2205.3.1".into(),
            os_version: "25F71".into(),
            device_name: None,
            audio_device_uid: None,
        };
        build_screen_video_offer("0F0F0F0F-0000-0000-0000-000000000000", &info, 7).unwrap()
    }

    #[test]
    fn idevices_default_offer_lists_both_codecs() {
        assert_eq!(banks(&default_offer()), [H264_PAYLOAD, HEVC_PAYLOAD]);
    }

    #[test]
    fn the_restricted_offer_lists_only_h264() {
        let offer = h264_only(&default_offer()).unwrap();
        assert_eq!(banks(&offer), [H264_PAYLOAD]);
    }

    #[test]
    fn something_that_is_not_an_offer_is_a_transport_error() {
        assert!(matches!(h264_only(b"junk"), Err(Error::Transport { .. })));
    }
}
