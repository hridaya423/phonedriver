//! Text recognition in screenshots with Apple Vision (architecture section 9).
//!
//! Vision hands back normalized boxes with the origin at the bottom left. `boundingBox` and `setRevision` are
//! the Vision calls `objc2-vision` declares `unsafe`, so this module allows unsafe code for those two and nothing else.

#![cfg_attr(target_os = "macos", allow(unsafe_code))]

use phone_use_core::error::Error;
use phone_use_core::error::Op;
use phone_use_core::geometry::Size;
use phone_use_core::ocr::TextBox;

/// Recognizes text in `png`, a screenshot of a screen `screen` points large.
#[cfg(target_os = "macos")]
pub fn recognize(png: &[u8], screen: Size) -> Result<Vec<TextBox>, Error> {
    use objc2::AnyThread;
    use objc2::rc::{Retained, autoreleasepool};
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequestTextRecognitionLevel,
    };
    use phone_use_core::geometry::Rect;

    autoreleasepool(|_| {
        let request = VNRecognizeTextRequest::new();
        // Revisions 1 and 3 run the Accurate model on the Neural Engine, which on macOS 27.0.1 takes 18 to 36 s and
        // often fails with an e5rt error; revision 2 recognizes the same text in about 45 ms.
        unsafe { request.setRevision(2) };
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(false);
        let data = NSData::with_bytes(png);
        let options: Retained<NSDictionary<_, AnyObject>> = NSDictionary::new();
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &options,
        );
        let requests = NSArray::from_retained_slice(&[request.clone().into_super().into_super()]);
        handler
            .performRequests_error(&requests)
            .map_err(|_| Error::Unsupported {
                op: Op::Ocr,
                reason: "Apple Vision could not read this screenshot",
            })?;
        let Some(results) = request.results() else {
            return Ok(Vec::new());
        };
        Ok(results
            .iter()
            .filter_map(|o| {
                let top = o.topCandidates(1).firstObject()?;
                let b = unsafe { o.boundingBox() };
                let (w, h) = (f64::from(screen.w), f64::from(screen.h));
                Some(TextBox {
                    text: top.string().to_string(),
                    bounds: Rect {
                        x: (b.origin.x * w) as f32,
                        y: ((1.0 - b.origin.y - b.size.height) * h) as f32,
                        w: (b.size.width * w) as f32,
                        h: (b.size.height * h) as f32,
                    },
                    confidence: top.confidence(),
                })
            })
            .collect())
    })
}

/// Recognizes text in `png`; only macOS has an engine.
#[cfg(not(target_os = "macos"))]
pub fn recognize(_png: &[u8], _screen: Size) -> Result<Vec<TextBox>, Error> {
    Err(Error::Unsupported {
        op: Op::Ocr,
        reason: "OCR uses Apple Vision, which only macOS has",
    })
}
