// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture evaluation for DISPLACE (`BKE_texture_get_value_ex` with `use_color_management =
//! false`): `NONE` and `IMAGE` textures (`multitex` → `imagewrap` with its box filter and
//! extension modes, the default flat 2D mapping, brightness/contrast/saturation) and the
//! default colour ramp.
//!
//! Ported from blender/source/blender/render/intern/texture_image.cc (imagewrap, boxsample,
//! boxsampleclip, clip*_rctf, ibuf_get_color), blender/source/blender/render/intern/
//! texture_procedural.cc (multitex, multitex_nodes_intern, do_2d_mapping),
//! blender/source/blender/render/intern/texture_common.h (BRICONTRGB) and
//! blender/source/blender/blenkernel/intern/texture.cc (BKE_texture_get_value_ex).
//!
//! The image buffer follows Blender's `ImBuf`: rows bottom to top, a byte buffer (straight
//! RGBA, premultiplied when read) or a float buffer (premultiplied).

use frap_imbuf::{DecodedImage, PixelData};
use frap_scene::generated::plain::TextureFields;

/// `TexResult`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct TexResult {
    /// Intensity.
    pub tin: f32,
    /// RGBA.
    pub trgba: [f32; 4],
    /// The alpha is meaningful (`talpha`).
    pub talpha: bool,
}

/// `TEX_RGB`.
const TEX_RGB: i32 = 1;

/// An `ImBuf` (bottom row first).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ImBuf {
    /// Straight 8-bit RGBA.
    Byte {
        /// Width.
        x: usize,
        /// Height.
        y: usize,
        /// `4 · x · y` bytes.
        rect: Vec<u8>,
    },
    /// Premultiplied float RGBA (`channels` 1, 3 or 4).
    Float {
        /// Width.
        x: usize,
        /// Height.
        y: usize,
        /// Channels per pixel.
        channels: usize,
        /// Pixels.
        rect: Vec<f32>,
    },
}

impl ImBuf {
    fn size(&self) -> (usize, usize) {
        match self {
            ImBuf::Byte { x, y, .. } | ImBuf::Float { x, y, .. } => (*x, *y),
        }
    }

    /// The buffer Blender loads from a decoded file: 8-bit images become byte buffers (grey
    /// and grey-alpha expanded, opaque alpha added), 16-bit and float ones premultiplied float
    /// buffers in linear space (`srgb` converts the colour of 16-bit sRGB files).
    pub(crate) fn from_decoded(d: &DecodedImage, srgb: bool) -> Option<ImBuf> {
        let (w, h, ch) = (d.width as usize, d.height as usize, d.channels as usize);
        if w == 0 || h == 0 || ch == 0 {
            return None;
        }
        let at = |row: usize, col: usize, k: usize| -> usize {
            // Top row first in `DecodedImage`, bottom row first in an `ImBuf`.
            ((h - 1 - row) * w + col) * ch + k
        };
        match &d.pixels {
            PixelData::U8(p) => {
                let mut rect = vec![0u8; 4 * w * h];
                for row in 0..h {
                    for col in 0..w {
                        let px = |k: usize| p[at(row, col, k)];
                        let rgba = match ch {
                            1 => [px(0), px(0), px(0), 255],
                            2 => [px(0), px(0), px(0), px(1)],
                            3 => [px(0), px(1), px(2), 255],
                            _ => [px(0), px(1), px(2), px(3)],
                        };
                        let o = 4 * (row * w + col);
                        rect[o..o + 4].copy_from_slice(&rgba);
                    }
                }
                Some(ImBuf::Byte { x: w, y: h, rect })
            }
            PixelData::U16(p) => {
                let mut rect = vec![0f32; 4 * w * h];
                for row in 0..h {
                    for col in 0..w {
                        let px = |k: usize| f32::from(p[at(row, col, k)]) / 65535.0;
                        let mut rgba = match ch {
                            1 => [px(0), px(0), px(0), 1.0],
                            2 => [px(0), px(0), px(0), px(1)],
                            3 => [px(0), px(1), px(2), 1.0],
                            _ => [px(0), px(1), px(2), px(3)],
                        };
                        if srgb {
                            for c in &mut rgba[..3] {
                                *c = srgb_to_linear(*c);
                            }
                        }
                        for k in 0..3 {
                            rgba[k] *= rgba[3];
                        }
                        let o = 4 * (row * w + col);
                        rect[o..o + 4].copy_from_slice(&rgba);
                    }
                }
                Some(ImBuf::Float {
                    x: w,
                    y: h,
                    channels: 4,
                    rect,
                })
            }
            PixelData::F32(p) => {
                let mut rect = vec![0f32; 4 * w * h];
                for row in 0..h {
                    for col in 0..w {
                        let px = |k: usize| p[at(row, col, k)];
                        let rgba = match ch {
                            1 => [px(0), px(0), px(0), 1.0],
                            2 => [px(0), px(0), px(0), px(1)],
                            3 => [px(0), px(1), px(2), 1.0],
                            _ => [px(0), px(1), px(2), px(3)],
                        };
                        let o = 4 * (row * w + col);
                        rect[o..o + 4].copy_from_slice(&rgba);
                    }
                }
                Some(ImBuf::Float {
                    x: w,
                    y: h,
                    channels: 4,
                    rect,
                })
            }
            _ => None,
        }
    }

    /// A generated blank byte image (`add_ibuf_for_tile`: the colour is stored as given,
    /// `rgba_float_to_uchar`).
    pub(crate) fn generated_byte(w: usize, h: usize, color: [f32; 4]) -> ImBuf {
        let c = color.map(unit_float_to_uchar_clamp);
        let mut rect = Vec::with_capacity(4 * w * h);
        for _ in 0..w * h {
            rect.extend_from_slice(&c);
        }
        ImBuf::Byte { x: w, y: h, rect }
    }

    /// A generated blank float image (the colour made linear, `srgb_to_linearrgb_v4`, unless
    /// the image is non-colour data).
    pub(crate) fn generated_float(w: usize, h: usize, color: [f32; 4], data: bool) -> ImBuf {
        let mut c = color;
        if !data {
            for v in &mut c[..3] {
                *v = srgb_to_linear(*v);
            }
        }
        let mut rect = Vec::with_capacity(4 * w * h);
        for _ in 0..w * h {
            rect.extend_from_slice(&c);
        }
        ImBuf::Float {
            x: w,
            y: h,
            channels: 4,
            rect,
        }
    }

    /// `ibuf_get_color`.
    fn get_color(&self, x: i32, y: i32) -> [f32; 4] {
        match self {
            ImBuf::Float {
                x: w,
                channels,
                rect,
                ..
            } => {
                let ofs = y as usize * w + x as usize;
                match channels {
                    4 => {
                        let p = &rect[4 * ofs..4 * ofs + 4];
                        [p[0], p[1], p[2], p[3]]
                    }
                    3 => {
                        let p = &rect[3 * ofs..3 * ofs + 3];
                        [p[0], p[1], p[2], 1.0]
                    }
                    _ => {
                        let v = rect[ofs];
                        [v; 4]
                    }
                }
            }
            ImBuf::Byte { x: w, rect, .. } => {
                let ofs = 4 * (y as usize * w + x as usize);
                let r = &rect[ofs..ofs + 4];
                let mut col = [
                    f32::from(r[0]) * (1.0 / 255.0),
                    f32::from(r[1]) * (1.0 / 255.0),
                    f32::from(r[2]) * (1.0 / 255.0),
                    f32::from(r[3]) * (1.0 / 255.0),
                ];
                // Bytes are straight; the render pipeline expects premultiplied.
                col[0] *= col[3];
                col[1] *= col[3];
                col[2] *= col[3];
                col
            }
        }
    }
}

/// `unit_float_to_uchar_clamp`.
fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `srgb_to_linearrgb`.
fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.04045 {
        if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) }
    } else {
        libm::powf((c + 0.055) * (1.0 / 1.055), 2.4)
    }
}

/// The image-texture settings `imagewrap` reads.
pub(crate) struct ImageTex<'a> {
    /// The texture's fields.
    pub fields: &'a TextureFields,
    /// The image's `alpha_mode` is not `'NONE'` (`IMA_ALPHA_IGNORE`).
    pub image_has_alpha: bool,
}

/// `extend` of a texture.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Wrap {
    Extend,
    Clip,
    ClipCube,
    Repeat,
    Checker,
}

fn extend_of(f: &TextureFields) -> Wrap {
    match f.extension.as_str() {
        "EXTEND" => Wrap::Extend,
        "CLIP" => Wrap::Clip,
        "CLIP_CUBE" => Wrap::ClipCube,
        "CHECKER" => Wrap::Checker,
        _ => Wrap::Repeat,
    }
}

// Ported from blender/source/blender/blenkernel/intern/texture.cc (BKE_texture_get_value_ex)
// and blender/source/blender/render/intern/texture_procedural.cc (multitex_nodes_intern for
// an image without `MTex`: the flat 2D mapping; multitex for `NONE`).
/// `BKE_texture_get_value_ex(tex, co, …, use_color_management = false)` for a `NONE` texture
/// (`image == None` with `is_image == false`) or an `IMAGE` texture with its buffer (`None`:
/// no image or no pixels).
pub(crate) fn texture_value(
    f: &TextureFields,
    is_image: bool,
    image: Option<(&ImBuf, bool)>,
    co: [f32; 3],
) -> TexResult {
    let mut texres = TexResult::default();
    let mut retval = 0;
    if is_image {
        let mut texvec = co;
        do_2d_mapping_flat(f, &mut texvec);
        retval = imagewrap(
            &ImageTex {
                fields: f,
                image_has_alpha: image.is_some_and(|i| i.1),
            },
            image.map(|i| i.0),
            texvec,
            &mut texres,
        );
        if f.use_color_ramp {
            // The colour band `use_color_ramp` creates (`BKE_colorband_init(coba, true)`):
            // black, transparent at 0 to white, opaque at 1, linear.
            let t = texres.tin.clamp(0.0, 1.0);
            texres.talpha = true;
            texres.trgba = [t, t, t, t];
            retval |= TEX_RGB;
        }
    } else {
        // `tex->type == 0`: `multitex` returns before the colour band.
        texres.tin = 0.0;
    }
    if retval & TEX_RGB != 0 {
        // An RGB result has no valid intensity in this context; no perceptual conversion.
        texres.tin = (1.0 / 3.0) * (texres.trgba[0] + texres.trgba[1] + texres.trgba[2]);
    } else {
        texres.trgba[0] = texres.tin;
        texres.trgba[1] = texres.tin;
        texres.trgba[2] = texres.tin;
    }
    texres
}

// Ported from blender/source/blender/render/intern/texture_procedural.cc (do_2d_mapping,
// MTEX_FLAT).
fn do_2d_mapping_flat(f: &TextureFields, texvec: &mut [f32; 3]) {
    let mut fx = (texvec[0] + 1.0) / 2.0;
    let mut fy = (texvec[1] + 1.0) / 2.0;
    if extend_of(f) == Wrap::Repeat {
        if f.repeat_x > 1 {
            fx *= f.repeat_x as f32;
            let origf = fx;
            if fx > 1.0 {
                fx -= (fx as i32) as f32;
            } else if fx < 0.0 {
                fx += (1 - fx as i32) as f32;
            }
            if f.use_mirror_x {
                let orig = libm::floorf(origf) as i32;
                if orig & 1 != 0 {
                    fx = 1.0 - fx;
                }
            }
        }
        if f.repeat_y > 1 {
            fy *= f.repeat_y as f32;
            let origf = fy;
            if fy > 1.0 {
                fy -= (fy as i32) as f32;
            } else if fy < 0.0 {
                fy += (1 - fy as i32) as f32;
            }
            if f.use_mirror_y {
                let orig = libm::floorf(origf) as i32;
                if orig & 1 != 0 {
                    fy = 1.0 - fy;
                }
            }
        }
    }
    if f.crop_min_x != 0.0 || f.crop_max_x != 1.0 {
        let fac1 = f.crop_max_x - f.crop_min_x;
        fx = f.crop_min_x + fx * fac1;
    }
    if f.crop_min_y != 0.0 || f.crop_max_y != 1.0 {
        let fac1 = f.crop_max_y - f.crop_min_y;
        fy = f.crop_min_y + fy * fac1;
    }
    texvec[0] = fx;
    texvec[1] = fy;
}

// Ported from blender/source/blender/render/intern/texture_image.cc (imagewrap).
fn imagewrap(t: &ImageTex<'_>, ibuf: Option<&ImBuf>, texvec: [f32; 3], texres: &mut TexResult) -> i32 {
    let f = t.fields;
    texres.tin = 0.0;
    texres.trgba = [0.0; 4];
    let retval = TEX_RGB;
    let Some(ibuf) = ibuf else {
        return retval;
    };
    let (ix, iy) = ibuf.size();
    let (ixf, iyf) = (ix as f32, iy as f32);
    let mut fx = texvec[0];
    let mut fy = texvec[1];
    // Set up the mapping.
    if f.use_flip_axis {
        std::mem::swap(&mut fx, &mut fy);
    }
    let extend = extend_of(f);
    if extend == Wrap::Checker {
        let xs = libm::floor(f64::from(fx)) as i32;
        let ys = libm::floor(f64::from(fy)) as i32;
        fx -= xs as f32;
        fy -= ys as f32;
        if !f.use_checker_odd && (xs + ys) & 1 == 0 {
            return retval;
        }
        if !f.use_checker_even && (xs + ys) & 1 != 0 {
            return retval;
        }
        // Scale around the centre, (0.5, 0.5).
        if f.checker_distance < 1.0 {
            fx = (fx - 0.5) / (1.0 - f.checker_distance) + 0.5;
            fy = (fy - 0.5) / (1.0 - f.checker_distance) + 0.5;
        }
    }
    let xi = libm::floorf(fx * ixf) as i32;
    let yi = libm::floorf(fy * iyf) as i32;
    let mut x = xi;
    let mut y = yi;
    let (w, h) = (ix as i32, iy as i32);
    match extend {
        Wrap::ClipCube => {
            if x < 0 || y < 0 || x >= w || y >= h || texvec[2] < -1.0 || texvec[2] > 1.0 {
                return retval;
            }
        }
        Wrap::Clip | Wrap::Checker => {
            if x < 0 || y < 0 || x >= w || y >= h {
                return retval;
            }
        }
        Wrap::Extend => {
            x = x.clamp(0, w - 1);
            y = y.clamp(0, h - 1);
        }
        Wrap::Repeat => {
            x %= w;
            if x < 0 {
                x += w;
            }
            y %= h;
            if y < 0 {
                y += h;
            }
        }
    }
    // Keep this before interpolation (#29761).
    if f.use_alpha && t.image_has_alpha && !f.use_calculate_alpha {
        texres.talpha = true;
    }
    if f.use_interpolation {
        let filterx = (0.5 * f.filter_size) / ixf;
        let filtery = (0.5 * f.filter_size) / iyf;
        // Wrapped back to the floating point values (#27782).
        fx -= (xi - x) as f32 / ixf;
        fy -= (yi - y) as f32 / iyf;
        boxsample(
            ibuf,
            fx - filterx,
            fy - filtery,
            fx + filterx,
            fy + filtery,
            texres,
            extend == Wrap::Repeat,
            extend == Wrap::Extend,
        );
    } else {
        texres.trgba = ibuf.get_color(x, y);
    }
    if texres.talpha {
        texres.tin = texres.trgba[3];
    } else if f.use_calculate_alpha {
        let m = texres.trgba[0].max(texres.trgba[1]).max(texres.trgba[2]);
        texres.trgba[3] = m;
        texres.tin = m;
    } else {
        texres.trgba[3] = 1.0;
        texres.tin = 1.0;
    }
    if f.invert_alpha {
        texres.trgba[3] = 1.0 - texres.trgba[3];
    }
    // De-premultiply (not for generated alpha, already straight).
    if texres.trgba[3] != 1.0 && texres.trgba[3] > 1e-4 && !f.use_calculate_alpha {
        let inv = 1.0 / texres.trgba[3];
        texres.trgba[0] *= inv;
        texres.trgba[1] *= inv;
        texres.trgba[2] *= inv;
    }
    bricont_rgb(f, texres);
    retval
}

// Ported from blender/source/blender/render/intern/texture_common.h (BRICONTRGB).
fn bricont_rgb(f: &TextureFields, texres: &mut TexResult) {
    let fac = [f.factor_red, f.factor_green, f.factor_blue];
    for (k, &fk) in fac.iter().enumerate() {
        texres.trgba[k] = fk * ((texres.trgba[k] - 0.5) * f.contrast + f.intensity - 0.5);
    }
    let no_clamp = !f.use_clamp;
    if !no_clamp {
        for c in &mut texres.trgba[..3] {
            if *c < 0.0 {
                *c = 0.0;
            }
        }
    }
    if f.saturation != 1.0 {
        let [h, s, v] = rgb_to_hsv(texres.trgba[0], texres.trgba[1], texres.trgba[2]);
        let rgb = hsv_to_rgb(h, s * f.saturation, v);
        texres.trgba[..3].copy_from_slice(&rgb);
        if f.saturation > 1.0 && !no_clamp {
            for c in &mut texres.trgba[..3] {
                if *c < 0.0 {
                    *c = 0.0;
                }
            }
        }
    }
}

// Ported from blender/source/blender/blenlib/intern/math_color.cc (rgb_to_hsv).
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let (mut r, mut g, mut b) = (r, g, b);
    let mut k = 0.0f32;
    if g < b {
        std::mem::swap(&mut g, &mut b);
        k = -1.0;
    }
    let mut min_gb = b;
    if r < g {
        std::mem::swap(&mut r, &mut g);
        k = -2.0 / 6.0 - k;
        min_gb = g.min(b);
    }
    let chroma = r - min_gb;
    [
        (k + (g - b) / (6.0 * chroma + 1e-20)).abs(),
        chroma / (r + 1e-20),
        r,
    ]
}

// Ported from blender/source/blender/blenlib/intern/math_color.cc (hsv_to_rgb).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let nr = ((h * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let ng = (2.0 - (h * 6.0 - 2.0).abs()).clamp(0.0, 1.0);
    let nb = (2.0 - (h * 6.0 - 4.0).abs()).clamp(0.0, 1.0);
    [
        ((nr - 1.0) * s + 1.0) * v,
        ((ng - 1.0) * s + 1.0) * v,
        ((nb - 1.0) * s + 1.0) * v,
    ]
}

/// `rctf`.
#[derive(Copy, Clone, Debug, Default)]
struct Rctf {
    xmin: f32,
    xmax: f32,
    ymin: f32,
    ymax: f32,
}

// Ported from blender/source/blender/render/intern/texture_image.cc (clipx_rctf_swap).
fn clipx_rctf_swap(stack: &mut Vec<Rctf>, x1: f32, x2: f32) {
    let count = stack.len();
    for i in 0..count {
        let mut rf = stack[i];
        if rf.xmin < x1 {
            if rf.xmax < x1 {
                rf.xmin += x2 - x1;
                rf.xmax += x2 - x1;
            } else {
                rf.xmax = rf.xmax.min(x2);
                let newrct = Rctf {
                    xmax: x2,
                    xmin: rf.xmin + (x2 - x1),
                    ymin: rf.ymin,
                    ymax: rf.ymax,
                };
                if newrct.xmin != newrct.xmax {
                    stack.push(newrct);
                }
                rf.xmin = x1;
            }
        } else if rf.xmax > x2 {
            if rf.xmin > x2 {
                rf.xmin -= x2 - x1;
                rf.xmax -= x2 - x1;
            } else {
                rf.xmin = rf.xmin.max(x1);
                let newrct = Rctf {
                    xmin: x1,
                    xmax: rf.xmax - (x2 - x1),
                    ymin: rf.ymin,
                    ymax: rf.ymax,
                };
                if newrct.xmin != newrct.xmax {
                    stack.push(newrct);
                }
                rf.xmax = x2;
            }
        }
        stack[i] = rf;
    }
}

// Ported from blender/source/blender/render/intern/texture_image.cc (clipy_rctf_swap).
fn clipy_rctf_swap(stack: &mut Vec<Rctf>, y1: f32, y2: f32) {
    let count = stack.len();
    for i in 0..count {
        let mut rf = stack[i];
        if rf.ymin < y1 {
            if rf.ymax < y1 {
                rf.ymin += y2 - y1;
                rf.ymax += y2 - y1;
            } else {
                rf.ymax = rf.ymax.min(y2);
                let newrct = Rctf {
                    ymax: y2,
                    ymin: rf.ymin + (y2 - y1),
                    xmin: rf.xmin,
                    xmax: rf.xmax,
                };
                if newrct.ymin != newrct.ymax {
                    stack.push(newrct);
                }
                rf.ymin = y1;
            }
        } else if rf.ymax > y2 {
            if rf.ymin > y2 {
                rf.ymin -= y2 - y1;
                rf.ymax -= y2 - y1;
            } else {
                rf.ymin = rf.ymin.max(y1);
                let newrct = Rctf {
                    ymin: y1,
                    ymax: rf.ymax - (y2 - y1),
                    xmin: rf.xmin,
                    xmax: rf.xmax,
                };
                if newrct.ymin != newrct.ymax {
                    stack.push(newrct);
                }
                rf.ymax = y2;
            }
        }
        stack[i] = rf;
    }
}

fn clipx_rctf(rf: &mut Rctf, x1: f32, x2: f32) -> f32 {
    let size = rf.xmax - rf.xmin;
    rf.xmin = rf.xmin.max(x1);
    rf.xmax = rf.xmax.min(x2);
    if rf.xmin > rf.xmax {
        rf.xmin = rf.xmax;
        return 0.0;
    }
    if size != 0.0 {
        return (rf.xmax - rf.xmin) / size;
    }
    1.0
}

fn clipy_rctf(rf: &mut Rctf, y1: f32, y2: f32) -> f32 {
    let size = rf.ymax - rf.ymin;
    rf.ymin = rf.ymin.max(y1);
    rf.ymax = rf.ymax.min(y2);
    if rf.ymin > rf.ymax {
        rf.ymin = rf.ymax;
        return 0.0;
    }
    if size != 0.0 {
        return (rf.ymax - rf.ymin) / size;
    }
    1.0
}

// Ported from blender/source/blender/render/intern/texture_image.cc (boxsampleclip).
fn boxsampleclip(ibuf: &ImBuf, rf: &Rctf, texres: &mut TexResult) {
    let (w, h) = ibuf.size();
    let mut startx = libm::floor(f64::from(rf.xmin)) as i32;
    let mut endx = libm::floor(f64::from(rf.xmax)) as i32;
    let mut starty = libm::floor(f64::from(rf.ymin)) as i32;
    let mut endy = libm::floor(f64::from(rf.ymax)) as i32;
    startx = startx.max(0);
    starty = starty.max(0);
    if endx >= w as i32 {
        endx = w as i32 - 1;
    }
    if endy >= h as i32 {
        endy = h as i32 - 1;
    }
    if starty == endy && startx == endx {
        texres.trgba = ibuf.get_color(startx, starty);
        return;
    }
    let mut div = 0.0f32;
    texres.trgba = [0.0; 4];
    for y in starty..=endy {
        let mut muly = 1.0f32;
        if starty != endy {
            if y == starty {
                muly = 1.0 - (rf.ymin - y as f32);
            }
            if y == endy {
                muly = rf.ymax - y as f32;
            }
        }
        if startx == endx {
            let mulx = muly;
            let col = ibuf.get_color(startx, y);
            for (t, c) in texres.trgba.iter_mut().zip(col) {
                *t += c * mulx;
            }
            div += mulx;
        } else {
            for x in startx..=endx {
                let mut mulx = muly;
                if x == startx {
                    mulx *= 1.0 - (rf.xmin - x as f32);
                }
                if x == endx {
                    mulx *= rf.xmax - x as f32;
                }
                let col = ibuf.get_color(x, y);
                if mulx == 1.0 {
                    for (t, c) in texres.trgba.iter_mut().zip(col) {
                        *t += c;
                    }
                    div += 1.0;
                } else {
                    for (t, c) in texres.trgba.iter_mut().zip(col) {
                        *t += c * mulx;
                    }
                    div += mulx;
                }
            }
        }
    }
    if div != 0.0 {
        let d = 1.0 / div;
        for c in &mut texres.trgba {
            *c *= d;
        }
    } else {
        texres.trgba = [0.0; 4];
    }
}

// Ported from blender/source/blender/render/intern/texture_image.cc (boxsample).
#[allow(clippy::too_many_arguments)]
fn boxsample(
    ibuf: &ImBuf,
    minx: f32,
    miny: f32,
    maxx: f32,
    maxy: f32,
    texres: &mut TexResult,
    imaprepeat: bool,
    imapextend: bool,
) {
    let (w, h) = ibuf.size();
    let (wf, hf) = (w as f32, h as f32);
    let mut alphaclip = 1.0f32;
    let mut stack: Vec<Rctf> = Vec::with_capacity(8);
    stack.push(Rctf {
        xmin: minx * wf,
        xmax: maxx * wf,
        ymin: miny * hf,
        ymax: maxy * hf,
    });
    let mut texr = TexResult {
        talpha: texres.talpha,
        ..TexResult::default()
    };
    if imapextend {
        let rf = &mut stack[0];
        rf.xmin = rf.xmin.clamp(0.0, wf - 1.0);
        rf.xmax = rf.xmax.clamp(0.0, wf - 1.0);
    } else if imaprepeat {
        clipx_rctf_swap(&mut stack, 0.0, wf);
    } else {
        alphaclip = clipx_rctf(&mut stack[0], 0.0, wf);
        if alphaclip <= 0.0 {
            texres.trgba = [0.0; 4];
            return;
        }
    }
    if imapextend {
        let rf = &mut stack[0];
        rf.ymin = rf.ymin.clamp(0.0, hf - 1.0);
        rf.ymax = rf.ymax.clamp(0.0, hf - 1.0);
    } else if imaprepeat {
        clipy_rctf_swap(&mut stack, 0.0, hf);
    } else {
        alphaclip *= clipy_rctf(&mut stack[0], 0.0, hf);
        if alphaclip <= 0.0 {
            texres.trgba = [0.0; 4];
            return;
        }
    }
    if stack.len() > 1 {
        let mut tot = 0.0f32;
        texres.trgba = [0.0; 4];
        for rf in &stack {
            boxsampleclip(ibuf, rf, &mut texr);
            let opp = (rf.xmax - rf.xmin) * (rf.ymax - rf.ymin);
            tot += opp;
            texres.trgba[0] += opp * texr.trgba[0];
            texres.trgba[1] += opp * texr.trgba[1];
            texres.trgba[2] += opp * texr.trgba[2];
            if texres.talpha {
                texres.trgba[3] += opp * texr.trgba[3];
            }
        }
        if tot != 0.0 {
            texres.trgba[0] /= tot;
            texres.trgba[1] /= tot;
            texres.trgba[2] /= tot;
            if texres.talpha {
                texres.trgba[3] /= tot;
            }
        }
    } else {
        boxsampleclip(ibuf, &stack[0], texres);
    }
    if !texres.talpha {
        texres.trgba[3] = 1.0;
    }
    if alphaclip != 1.0 {
        // Premultiply it all.
        for c in &mut texres.trgba {
            *c *= alphaclip;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_texture_is_black() {
        let f = TextureFields::default();
        let r = texture_value(&f, false, None, [0.3, 0.2, 0.0]);
        assert_eq!(r.tin, 0.0);
        assert_eq!(r.trgba[..3], [0.0; 3]);
    }

    #[test]
    fn a_blank_byte_image_samples_its_colour() {
        let f = TextureFields::default();
        let ib = ImBuf::generated_byte(4, 4, [0.5, 0.25, 1.0, 1.0]);
        let r = texture_value(&f, true, Some((&ib, true)), [0.1, -0.2, 0.0]);
        // Bytes 128, 64, 255.
        let expect = (128.0 + 64.0 + 255.0) / 255.0 / 3.0;
        assert!((r.tin - expect).abs() < 1e-6, "{}", r.tin);
    }
}
