//! Explicit paste only. Never inspect text, HTML, file URLs or remote content.
//! ImageIO metadata bounds decoding before AppKit converts a TIFF to PNG.
use nexushub_core::codex::desktop::attachments::MAX_IMAGE_BYTES;
use objc2::{rc::autoreleasepool, runtime::AnyObject};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSDictionary};
use std::ffi::c_void;

const CAPTURE_BYTES: usize = 64 * 1024 * 1024;
const PIXEL_BYTES: u64 = 128 * 1024 * 1024;
const INVALID: &str = "剪贴板图片无效或超出安全大小限制";

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
        return Err(INVALID);
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
    if bytes.len() > CAPTURE_BYTES || bytes.is_empty() {
        return Err(INVALID);
    }
    check_dimensions(bytes)?;
    if png {
        if bytes.len() > MAX_IMAGE_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
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
        return Err(INVALID);
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
            return Err(INVALID);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn oversized_capture_is_rejected_before_image_decoding() {
        let bytes = vec![0; CAPTURE_BYTES + 1];
        assert_eq!(normalize(&bytes, true).unwrap_err(), INVALID);
    }
}
