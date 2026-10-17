//! Explicit image paste and generated safe diagnostic copy only. Never inspect
//! clipboard text, HTML, file URLs or remote content.
//! ImageIO metadata bounds decoding before AppKit converts a TIFF to PNG.
use nexushub_core::codex::desktop::attachments::MAX_IMAGE_BYTES;
use objc2::{rc::autoreleasepool, runtime::AnyObject};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG,
    NSPasteboardTypeString, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSDictionary, NSString};
use std::ffi::c_void;

const CAPTURE_BYTES: usize = 64 * 1024 * 1024;
const PIXEL_BYTES: u64 = 128 * 1024 * 1024;
const INVALID: &str = "剪贴板图片格式无效或不受支持";
const TOO_LARGE: &str = "剪贴板图片超出安全大小限制";

pub(super) fn copy_diagnostics(
    diagnostics: &nexushub_core::services::codex_desktop::DesktopDiagnostics,
) -> Result<(), &'static str> {
    autoreleasepool(|_| write_diagnostics(&NSPasteboard::generalPasteboard(), diagnostics))
}

fn write_diagnostics(
    pasteboard: &NSPasteboard,
    diagnostics: &nexushub_core::services::codex_desktop::DesktopDiagnostics,
) -> Result<(), &'static str> {
    let text = serde_json::to_string_pretty(diagnostics).map_err(|_| "无法复制诊断，请重试")?;
    if text.len() > 64 * 1024 {
        return Err("诊断超出复制容量");
    }
    pasteboard.clearContents();
    // SAFETY: immutable AppKit text type; content is generated from the bounded
    // diagnostic DTO, never a caller-supplied string or native error payload.
    if unsafe { pasteboard.setString_forType(&NSString::from_str(&text), NSPasteboardTypeString) } {
        Ok(())
    } else {
        Err("无法复制诊断，请重试")
    }
}

pub(super) fn paste_png() -> Result<Vec<u8>, &'static str> {
    autoreleasepool(|_| read_png(&NSPasteboard::generalPasteboard()))
}

fn read_png(pasteboard: &NSPasteboard) -> Result<Vec<u8>, &'static str> {
    read_png_using(pasteboard, normalize)
}

fn read_png_using(
    pasteboard: &NSPasteboard,
    convert: impl FnOnce(&[u8], bool) -> Result<Vec<u8>, &'static str>,
) -> Result<Vec<u8>, &'static str> {
    let before = pasteboard.changeCount();
    // SAFETY: AppKit exports these immutable pasteboard type constants.
    let (data, png) = unsafe {
        if let Some(data) = pasteboard.dataForType(NSPasteboardTypePNG) {
            (data, true)
        } else {
            (
                pasteboard
                    .dataForType(NSPasteboardTypeTIFF)
                    .ok_or("剪贴板没有可粘贴的图片")?,
                false,
            )
        }
    };
    if data.len() > CAPTURE_BYTES {
        return Err(TOO_LARGE);
    }
    let bytes = data.to_vec();
    if before != pasteboard.changeCount() {
        return Err("剪贴板已变化，请重新粘贴");
    }
    let result = convert(&bytes, png)?;
    if before != pasteboard.changeCount() {
        return Err("剪贴板已变化，请重新粘贴");
    }
    Ok(result)
}

fn normalize(bytes: &[u8], png: bool) -> Result<Vec<u8>, &'static str> {
    if bytes.len() > CAPTURE_BYTES {
        return Err(TOO_LARGE);
    }
    if bytes.is_empty() {
        return Err(INVALID);
    }
    check_dimensions(bytes)?;
    if png {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("粘贴图片超过 20MiB");
        }
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(INVALID);
        }
        return Ok(bytes.to_vec());
    }
    let data = NSData::with_bytes(bytes);
    let bitmap = NSBitmapImageRep::imageRepWithData(&data).ok_or(INVALID)?;
    // SAFETY: the empty dictionary has the documented property-key/object type;
    // PNG is lossless and no scaling/quality properties are supplied.
    let png = unsafe {
        bitmap.representationUsingType_properties(
            NSBitmapImageFileType::PNG,
            &NSDictionary::<objc2_foundation::NSString, AnyObject>::new(),
        )
    }
    .ok_or(INVALID)?;
    if png.len() > MAX_IMAGE_BYTES {
        return Err("粘贴图片超过 20MiB");
    }
    Ok(png.to_vec())
}

// Narrow system-framework FFI avoids adding an image codec/runtime dependency.
type CF = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(allocator: CF, bytes: *const u8, length: isize) -> CF;
    fn CFRelease(value: CF);
    fn CFDictionaryGetValue(dictionary: CF, key: CF) -> CF;
    fn CFNumberGetValue(number: CF, kind: isize, value: *mut c_void) -> bool;
    fn CFGetTypeID(value: CF) -> usize;
    fn CFNumberGetTypeID() -> usize;
}
#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageSourceCreateWithData(data: CF, options: CF) -> CF;
    fn CGImageSourceGetCount(source: CF) -> usize;
    fn CGImageSourceCopyPropertiesAtIndex(source: CF, index: usize, options: CF) -> CF;
    static kCGImagePropertyPixelWidth: CF;
    static kCGImagePropertyPixelHeight: CF;
    static kCGImagePropertyDepth: CF;
}
struct OwnedCF(CF);
impl OwnedCF {
    fn new(value: CF) -> Result<Self, &'static str> {
        if value.is_null() {
            Err(INVALID)
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for OwnedCF {
    fn drop(&mut self) {
        // SAFETY: every instance owns one non-null Create/Copy CF reference.
        unsafe { CFRelease(self.0) }
    }
}
fn check_dimensions(bytes: &[u8]) -> Result<(), &'static str> {
    // SAFETY: buffers live for these synchronous calls, owned CF values are
    // released in reverse order, and numeric types are checked before reading.
    unsafe {
        let data = OwnedCF::new(CFDataCreate(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
        ))?;
        let source = OwnedCF::new(CGImageSourceCreateWithData(data.0, std::ptr::null()))?;
        // Clipboard images are a single bitmap; don't decode unbounded frames.
        if CGImageSourceGetCount(source.0) != 1 {
            return Err(INVALID);
        }
        let properties = OwnedCF::new(CGImageSourceCopyPropertiesAtIndex(
            source.0,
            0,
            std::ptr::null(),
        ))?;
        let number = |key| -> Result<u64, &'static str> {
            let value = CFDictionaryGetValue(properties.0, key);
            if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
                return Err(INVALID);
            }
            let mut number = 0i64;
            // kCFNumberSInt64Type = 4 in the system CoreFoundation header.
            if !CFNumberGetValue(value, 4, (&mut number as *mut i64).cast()) || number <= 0 {
                return Err(INVALID);
            }
            Ok(number as u64)
        };
        let width = number(kCGImagePropertyPixelWidth)?;
        let height = number(kCGImagePropertyPixelHeight)?;
        if number(kCGImagePropertyDepth)? > 16 {
            return Err(INVALID);
        }
        // Reserve sixteen bytes per pixel for decoded/conversion surfaces.
        if width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(16))
            .is_none_or(|bytes| bytes > PIXEL_BYTES)
        {
            return Err(TOO_LARGE);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::services::codex_desktop::{DesktopDiagnostic, DesktopDiagnostics};
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 96, 96, 96, 248, 15,
        0, 1, 4, 1, 0, 95, 229, 195, 75, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[test]
    fn png_is_preserved_and_tiff_is_converted_without_resizing() {
        autoreleasepool(|_| {
            assert_eq!(normalize(PNG, true).unwrap(), PNG);
            let bitmap = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(PNG)).unwrap();
            // SAFETY: documented property types and an empty property dictionary.
            let tiff = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::TIFF,
                    &NSDictionary::<objc2_foundation::NSString, AnyObject>::new(),
                )
            }
            .unwrap();
            let converted = normalize(&tiff.to_vec(), false).unwrap();
            assert!(converted.starts_with(b"\x89PNG\r\n\x1a\n"));
            let bitmap =
                NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&converted)).unwrap();
            assert_eq!((bitmap.pixelsWide(), bitmap.pixelsHigh()), (1, 1));
            assert!(normalize(b"not an image", true).is_err());
        });
    }

    #[test]
    fn private_pasteboard_changed_during_conversion_is_rejected() {
        autoreleasepool(|_| {
            // A unique test pasteboard never reads or changes the user's clipboard.
            let board = NSPasteboard::pasteboardWithUniqueName();
            board.clearContents();
            // SAFETY: immutable AppKit type constant, fixture NSData retained.
            assert!(unsafe {
                board.setData_forType(Some(&NSData::with_bytes(PNG)), NSPasteboardTypePNG)
            });
            assert_eq!(read_png(&board).unwrap(), PNG);
            let result = read_png_using(&board, |bytes, png| {
                board.clearContents();
                normalize(bytes, png)
            });
            assert_eq!(result.unwrap_err(), "剪贴板已变化，请重新粘贴");
            assert_eq!(read_png(&board).unwrap_err(), "剪贴板没有可粘贴的图片");
        });
    }

    fn diagnostics_with_hint(hint: String) -> DesktopDiagnostics {
        DesktopDiagnostics {
            records: vec![DesktopDiagnostic {
                timestamp: 1,
                source: "fixture-source".into(),
                method: "fixture-method".into(),
                version: Some(1),
                code: None,
                category: "fixture-category".into(),
                hint,
            }],
            desktop_build: Some("fixture-build".into()),
            bundled_cli_version: None,
        }
    }

    #[test]
    fn diagnostic_copy_writes_safe_json_to_a_private_pasteboard() {
        autoreleasepool(|_| {
            let board = NSPasteboard::pasteboardWithUniqueName();
            let diagnostics = diagnostics_with_hint("fixture diagnostic".into());
            let expected = serde_json::to_string_pretty(&diagnostics).unwrap();

            write_diagnostics(&board, &diagnostics).unwrap();

            // SAFETY: AppKit exports this immutable pasteboard type constant.
            assert_eq!(
                unsafe { board.stringForType(NSPasteboardTypeString) }
                    .expect("diagnostic string is present")
                    .to_string(),
                expected
            );
        });
    }

    #[test]
    fn oversized_diagnostic_copy_keeps_private_pasteboard_contents() {
        autoreleasepool(|_| {
            let board = NSPasteboard::pasteboardWithUniqueName();
            let existing = NSString::from_str("fixture-existing-content");
            // SAFETY: AppKit exports this immutable pasteboard type constant.
            assert!(unsafe { board.setString_forType(&existing, NSPasteboardTypeString) });
            let change_count = board.changeCount();
            let diagnostics = diagnostics_with_hint("x".repeat(64 * 1024));

            assert_eq!(
                write_diagnostics(&board, &diagnostics),
                Err("诊断超出复制容量")
            );
            assert_eq!(board.changeCount(), change_count);
            // SAFETY: AppKit exports this immutable pasteboard type constant.
            assert_eq!(
                unsafe { board.stringForType(NSPasteboardTypeString) }
                    .expect("existing content remains present")
                    .to_string(),
                "fixture-existing-content"
            );
        });
    }

    #[test]
    fn oversized_capture_is_rejected_before_image_decoding() {
        let bytes = vec![0; CAPTURE_BYTES + 1];
        assert_eq!(normalize(&bytes, true).unwrap_err(), TOO_LARGE);
    }
}
