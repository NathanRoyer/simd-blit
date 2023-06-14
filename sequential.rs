use rgb::{RGBA, FromSlice};
use super::PixelArray;

/// A pack of 8 pixels (SIMD alignment)
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct EightPixels([u16; 32]);

impl EightPixels {
    /// Read up to 8 pixels from a byte slice (4 bytes per pixel)
    pub fn new(src: &[u8]) -> Self {
        let mut array = [0; 32];
        array[..src.len()].copy_from_slice(src);
        Self(array.map(|item| item as u16))
    }

    /// Write up to 8 pixels to a byte slice (4 bytes per pixel)
    pub fn write(&self, dst: &mut [u8]) {
        dst.copy_from_slice(&self.0.map(|item| item as u8)[..dst.len()]);
    }
}

/// Supported Alpha configurations
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
pub enum AlphaConfig {
    FirstByte,
    SecondByte,
    ThirdByte,
    FourthByte,
    /// The pixels will be directly copied, no blending
    None,
}

/// Perform alpha compositing on up to eight pixels
#[inline(always)]
pub fn blend8(
    src: EightPixels,
    dst: &mut [u8],
    alpha_config: AlphaConfig,
) {
    let result = if alpha_config != AlphaConfig::None {
        let dst_p = EightPixels::new(dst);

        let alpha_channel = match alpha_config {
            AlphaConfig::FirstByte  => 0,
            AlphaConfig::SecondByte => 1,
            AlphaConfig::ThirdByte  => 2,
            AlphaConfig::FourthByte => 3,
            _ => unreachable!(),
        };
        let u8_max = u8::MAX as u16;

        let mut result = [0; 32];
        for i in 0..32 {
            let p = i & !3;
            let src_a = src.0[p + alpha_channel];
            let dst_a = u8_max - src_a;
            result[i] = ((src.0[i] * src_a) + (dst_p.0[i] * dst_a)) / u8_max;
        }

        EightPixels(result)
    } else {
        src
    };

    result.write(dst);
}

/// An aligned structure storing `SSAA_SQ` (x, y) subpixel coordinates for up to eight pixels
pub struct SsaaCoords<const SSAA_SQ: usize> {
    src_o: [[usize; 8]; SSAA_SQ],
    src_x: [[usize; 8]; SSAA_SQ],
    src_y: [[usize; 8]; SSAA_SQ],
}

impl<const SSAA_SQ: usize> SsaaCoords<SSAA_SQ> {
    pub fn new() -> Self {
        const FULL_USIZE_MAX: [usize; 8] = [usize::MAX; 8];
        Self {
            src_o: [FULL_USIZE_MAX; SSAA_SQ],
            src_x: [FULL_USIZE_MAX; SSAA_SQ],
            src_y: [FULL_USIZE_MAX; SSAA_SQ],
        }
    }

    /// Insert coordinates (pixel < 8 && sub_pixel < SSAA_SQ)
    #[inline(always)]
    pub fn set(&mut self, pixel: usize, sub_pixel: usize, x: usize, y: usize) {
        assert!(pixel < 8);
        self.src_o[sub_pixel][pixel] = pixel;
        self.src_x[sub_pixel][pixel] = x;
        self.src_y[sub_pixel][pixel] = y;
    }
}

/// Performs SSAA on up to 8 pixels
#[inline(always)]
pub fn ssaa8<P: PixelArray, const SSAA_SQ: usize>(
    src_coords: SsaaCoords<SSAA_SQ>,
    src: &P,
) -> EightPixels {
    let src_w = src.width();
    let src_h = src.height();
    let src_l = src.length();

    // SUM SUBPIXELS

    let mut ssaa_px = [0; 8];
    let mut result = EightPixels::new(&[]);

    for i in 0..SSAA_SQ {
        for j in 0..8 {
            let src_o = src_coords.src_o[i][j];
            let src_x = src_coords.src_x[i][j];
            let src_y = src_coords.src_y[i][j];
            let src_i = src_y * src_w + src_x;

            let usable_x = src_x < src_w;
            let usable_y = src_y < src_h;
            let usable_l = src_i < src_l;
            let usable = usable_x & usable_y & usable_l;

            if usable {
                let rgba: RGBA<u16> = src.get(src_i).into();
                result.0.as_rgba_mut()[src_o] += rgba;
                ssaa_px[src_o] += 1;
            }
        }
    }

    // DIVIDE BY NUMBER OF SUBPIXELS

    for i in 0..8 {
        result.0.as_rgba_mut()[i] /= if true {
            // better perf but some weird line SouthEast
            SSAA_SQ as u16
        } else {
            match ssaa_px[i] {
                0 => 1,
                n => n,
            }
        };
    }

    result
}
