use core_foundation::base::TCFType;
use core_foundation::url::CFURL;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use std::path::Path;
use std::sync::Arc;

/// Render all pages of a PDF to GPUI `RenderImage` objects.
/// Returns (images, page_count).
pub fn render_pdf(path: &Path, scale: f64) -> Option<(Vec<Arc<RenderImage>>, usize)> {
    let url = CFURL::from_path(path, false)?;
    let pdf = unsafe {
        let doc = CGPDFDocumentCreateWithURL(url.as_CFTypeRef());
        if doc.is_null() {
            return None;
        }
        doc
    };

    let page_count = unsafe { CGPDFDocumentGetNumberOfPages(pdf) };
    if page_count == 0 {
        unsafe { CGPDFDocumentRelease(pdf) };
        return None;
    }

    let mut images = Vec::with_capacity(page_count);

    for i in 1..=page_count {
        let page = unsafe { CGPDFDocumentGetPage(pdf, i) };
        if page.is_null() {
            continue;
        }

        let media_box = unsafe { CGPDFPageGetBoxRect(page, 0) };
        let w = (media_box.size.width * scale) as u32;
        let h = (media_box.size.height * scale) as u32;
        if w == 0 || h == 0 {
            continue;
        }

        let color_space = CGColorSpace::create_device_rgb();
        // kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little = BGRA premultiplied
        let bitmap_info: u32 = 1 | (2 << 12); // kCGImageAlphaPremultipliedLast (RGBA)
        let mut ctx = CGContext::create_bitmap_context(
            None,
            w as usize,
            h as usize,
            8,
            w as usize * 4,
            &color_space,
            bitmap_info,
        );

        // White background
        ctx.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);
        ctx.fill_rect(CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(w as f64, h as f64),
        ));

        // Scale and draw
        ctx.scale(scale, scale);

        // Get the CGContextRef raw pointer by reading the first field of the foreign type.
        // CGContext is a foreign_type wrapping a *mut sys::CGContext pointer.
        let ctx_ptr: *mut std::ffi::c_void =
            unsafe { *((&ctx as *const CGContext) as *const *mut std::ffi::c_void) };
        unsafe {
            CGContextDrawPDFPage(ctx_ptr, page);
        }

        // Read RGBA pixel data
        let data = ctx.data();
        let len = (w * h * 4) as usize;
        let mut rgba = data[..len].to_vec();

        // Un-premultiply alpha (CG outputs premultiplied)
        for pixel in rgba.chunks_exact_mut(4) {
            let a = pixel[3] as u16;
            if a > 0 && a < 255 {
                pixel[0] = ((pixel[0] as u16 * 255) / a).min(255) as u8;
                pixel[1] = ((pixel[1] as u16 * 255) / a).min(255) as u8;
                pixel[2] = ((pixel[2] as u16 * 255) / a).min(255) as u8;
            }
        }

        // Build GPUI RenderImage from the RGBA data
        if let Some(buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(w, h, rgba) {
            let frame = Frame::new(buf);
            images.push(Arc::new(RenderImage::new(vec![frame])));
        }
    }

    unsafe { CGPDFDocumentRelease(pdf) };
    Some((images, page_count))
}

// ── Core Graphics PDF FFI ───────────────────────────────────────────

type CGPDFDocumentRef = *const std::ffi::c_void;
type CGPDFPageRef = *const std::ffi::c_void;

extern "C" {
    fn CGPDFDocumentCreateWithURL(url: core_foundation::base::CFTypeRef) -> CGPDFDocumentRef;
    fn CGPDFDocumentGetNumberOfPages(document: CGPDFDocumentRef) -> usize;
    fn CGPDFDocumentGetPage(document: CGPDFDocumentRef, page_number: usize) -> CGPDFPageRef;
    fn CGPDFDocumentRelease(document: CGPDFDocumentRef);
    fn CGPDFPageGetBoxRect(page: CGPDFPageRef, box_type: i32) -> CGRect;
    fn CGContextDrawPDFPage(context: *mut std::ffi::c_void, page: CGPDFPageRef);
}
