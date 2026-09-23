//! libwebp's VP8 encoder, fed the way the game feeds it (RE child, disassembly of the writer at
//! 0x1402159f0): planes DIRECTLY as YUV420 (the compress shader's BT.601 studio-swing YCbCr —
//! no RGB→YUV inside libwebp), `WebPConfigInit(preset DEFAULT, quality)` with nothing else
//! changed, quality 91 for the colour atlas / probe / frame-1 images and ≈ 25 for the three
//! grey directional images. libwebp 1.4.0 is what we can build offline; the game's is 1.6.0 —
//! the VP8 header comes out identical at these settings (cwebp 1.5.0 -q 91 reproduced the
//! editor's), the coefficients may differ in the last bits between versions.

#[cfg(have_libwebp)]
mod ffi {
    use std::os::raw::{c_float, c_int, c_void};

    #[repr(C)]
    pub struct WebPConfig {
        pub lossless: c_int,
        pub quality: c_float,
        pub method: c_int,
        pub image_hint: c_int,
        pub target_size: c_int,
        pub target_psnr: c_float,
        pub segments: c_int,
        pub sns_strength: c_int,
        pub filter_strength: c_int,
        pub filter_sharpness: c_int,
        pub filter_type: c_int,
        pub autofilter: c_int,
        pub alpha_compression: c_int,
        pub alpha_filtering: c_int,
        pub alpha_quality: c_int,
        pub pass: c_int,
        pub show_compressed: c_int,
        pub preprocessing: c_int,
        pub partitions: c_int,
        pub partition_limit: c_int,
        pub emulate_jpeg_size: c_int,
        pub thread_level: c_int,
        pub low_memory: c_int,
        pub near_lossless: c_int,
        pub exact: c_int,
        pub use_delta_palette: c_int,
        pub use_sharp_yuv: c_int,
        pub qmin: c_int,
        pub qmax: c_int,
    }

    #[repr(C)]
    pub struct WebPPicture {
        pub use_argb: c_int,
        pub colorspace: c_int,
        pub width: c_int,
        pub height: c_int,
        pub y: *mut u8,
        pub u: *mut u8,
        pub v: *mut u8,
        pub y_stride: c_int,
        pub uv_stride: c_int,
        pub a: *mut u8,
        pub a_stride: c_int,
        pub pad1: [u32; 2],
        pub argb: *mut u32,
        pub argb_stride: c_int,
        pub pad2: [u32; 3],
        pub writer: Option<unsafe extern "C" fn(*const u8, usize, *const WebPPicture) -> c_int>,
        pub custom_ptr: *mut c_void,
        pub extra_info_type: c_int,
        pub extra_info: *mut u8,
        pub stats: *mut c_void,
        pub error_code: c_int,
        pub progress_hook: *mut c_void,
        pub user_data: *mut c_void,
        pub pad3: [u32; 3],
        pub pad4: *mut u8,
        pub pad5: *mut u8,
        pub pad6: [u32; 8],
        pub memory_: *mut c_void,
        pub memory_argb_: *mut c_void,
        pub pad7: [*mut c_void; 2],
    }

    #[repr(C)]
    pub struct WebPMemoryWriter {
        pub mem: *mut u8,
        pub size: usize,
        pub max_size: usize,
        pub pad: [u32; 1],
    }

    pub const WEBP_ENCODER_ABI_VERSION: c_int = 0x020f;

    extern "C" {
        pub fn WebPGetEncoderVersion() -> c_int;
        pub fn WebPConfigInitInternal(config: *mut WebPConfig, preset: c_int, quality: c_float, version: c_int) -> c_int;
        pub fn WebPPictureInitInternal(pic: *mut WebPPicture, version: c_int) -> c_int;
        pub fn WebPEncode(config: *const WebPConfig, pic: *mut WebPPicture) -> c_int;
        pub fn WebPMemoryWriterInit(writer: *mut WebPMemoryWriter);
        pub fn WebPMemoryWrite(data: *const u8, size: usize, pic: *const WebPPicture) -> c_int;
        pub fn WebPMemoryWriterClear(writer: *mut WebPMemoryWriter);
    }
}

/// Is libwebp linked in?
pub fn available() -> bool {
    cfg!(have_libwebp)
}

/// libwebp's version as "M.m.p".
pub fn version() -> String {
    #[cfg(have_libwebp)]
    {
        let v = unsafe { ffi::WebPGetEncoderVersion() };
        return format!("{}.{}.{}", (v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
    }
    #[allow(unreachable_code)]
    "none".into()
}

/// RGB → BT.601 studio-swing YUV420 planes, libwebp's own constants (what the compress shader
/// writes into the planes the game hands to libwebp).
pub fn rgb_to_yuv420(rgb: &[u8], w: usize, h: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (cw, ch) = ((w + 1) / 2, (h + 1) / 2);
    let mut y = vec![0u8; w * h];
    let mut u = vec![0u8; cw * ch];
    let mut v = vec![0u8; cw * ch];
    let mut uacc = vec![0i32; cw * ch];
    let mut vacc = vec![0i32; cw * ch];
    let mut cnt = vec![0i32; cw * ch];
    for yy in 0..h {
        for xx in 0..w {
            let i = (yy * w + xx) * 3;
            let (r, g, b) = (rgb[i] as i32, rgb[i + 1] as i32, rgb[i + 2] as i32);
            // libwebp: VP8RGBToY = (16839 r + 33059 g + 6420 b + rounding) >> 16 + 16
            y[yy * w + xx] = (16 + ((16839 * r + 33059 * g + 6420 * b + 32768) >> 16)).clamp(0, 255) as u8;
            let ci = (yy / 2) * cw + xx / 2;
            uacc[ci] += -9719 * r - 19081 * g + 28800 * b;
            vacc[ci] += 28800 * r - 24116 * g - 4684 * b;
            cnt[ci] += 1;
        }
    }
    for i in 0..cw * ch {
        let n = cnt[i].max(1);
        u[i] = (128 + ((uacc[i] / n + 32768) >> 16)).clamp(0, 255) as u8;
        v[i] = (128 + ((vacc[i] / n + 32768) >> 16)).clamp(0, 255) as u8;
    }
    (y, u, v)
}

/// Encode an RGB image with libwebp: preset DEFAULT at `quality`, planes fed directly.
/// `None` when libwebp is not linked.
pub fn encode_rgb(rgb: &[u8], w: u32, h: u32, quality: f32) -> Option<Vec<u8>> {
    let (y, u, v) = rgb_to_yuv420(rgb, w as usize, h as usize);
    encode_yuv(&y, &u, &v, w, h, quality)
}

/// Encode Y/U/V planes (Y `w`×`h`, U/V `(w+1)/2`×`(h+1)/2`) with libwebp, preset DEFAULT.
#[cfg(have_libwebp)]
pub fn encode_yuv(y: &[u8], u: &[u8], v: &[u8], w: u32, h: u32, quality: f32) -> Option<Vec<u8>> {
    use ffi::*;
    unsafe {
        let mut config: WebPConfig = std::mem::zeroed();
        if WebPConfigInitInternal(&mut config, 0, quality, WEBP_ENCODER_ABI_VERSION) == 0 {
            return None;
        }
        let mut pic: WebPPicture = std::mem::zeroed();
        if WebPPictureInitInternal(&mut pic, WEBP_ENCODER_ABI_VERSION) == 0 {
            return None;
        }
        let (mut y, mut u, mut v) = (y.to_vec(), u.to_vec(), v.to_vec());
        pic.use_argb = 0;
        pic.colorspace = 0; // WEBP_YUV420
        pic.width = w as i32;
        pic.height = h as i32;
        pic.y = y.as_mut_ptr();
        pic.u = u.as_mut_ptr();
        pic.v = v.as_mut_ptr();
        pic.y_stride = w as i32;
        pic.uv_stride = ((w + 1) / 2) as i32;
        let mut wr: WebPMemoryWriter = std::mem::zeroed();
        WebPMemoryWriterInit(&mut wr);
        pic.writer = Some(WebPMemoryWrite);
        pic.custom_ptr = &mut wr as *mut _ as *mut std::os::raw::c_void;
        let ok = WebPEncode(&config, &mut pic);
        let out = if ok != 0 && !wr.mem.is_null() { Some(std::slice::from_raw_parts(wr.mem, wr.size).to_vec()) } else { None };
        WebPMemoryWriterClear(&mut wr);
        out
    }
}

#[cfg(not(have_libwebp))]
pub fn encode_yuv(_y: &[u8], _u: &[u8], _v: &[u8], _w: u32, _h: u32, _quality: f32) -> Option<Vec<u8>> {
    None
}
