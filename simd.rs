use super::PixelArray;

use core::simd::{
    SimdPartialOrd,
    SimdUint,
    simd_swizzle,
    usizex8,
    u8x4,
    u16x4,
    u8x32,
    u16x32,
};

/// A pack of 8 pixels (SIMD alignment)
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct EightPixels(u16x32);

impl EightPixels {
    /// Read up to 8 pixels from a byte slice (4 bytes per pixel)
    pub fn new(src: &[u8]) -> Self {
        let mut array = [0; 32];
        array[..src.len()].copy_from_slice(src);
        Self(u8x32::from_array(array).cast())
    }

    /// Write up to 8 pixels to a byte slice (4 bytes per pixel)
    pub fn write(&self, dst: &mut [u8]) {
        let u8simd: u8x32 = self.0.cast();
        dst.copy_from_slice(&u8simd.as_array()[..dst.len()]);
    }
}

const fn gen_swizzle(byte: usize) -> [usize; 32] {
    let mut result = [byte; 32];
    let mut i = 0;
    while i < 32 {
        result[i] += i & !3;
        i += 1;
    }
    result
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

        // map [r, g, b, a] to [a, a, a, a]
        let src_a = match alpha_config {
            AlphaConfig::FirstByte  => simd_swizzle!(src.0, gen_swizzle(0)),
            AlphaConfig::SecondByte => simd_swizzle!(src.0, gen_swizzle(1)),
            AlphaConfig::ThirdByte  => simd_swizzle!(src.0, gen_swizzle(2)),
            AlphaConfig::FourthByte => simd_swizzle!(src.0, gen_swizzle(3)),
            _ => unreachable!(),
        };

        let u8_max = u16x32::from_array([u8::MAX as _; 32]);
        let dst_a = u8_max - src_a;

        EightPixels(((src.0 * src_a) + (dst_p.0 * dst_a)) / u8_max)
    } else {
        src
    };

    result.write(dst);
}

/// An aligned structure storing `SSAA_SQ` (x, y) subpixel coordinates for up to eight pixels
pub struct SsaaCoords<const SSAA_SQ: usize> {
    src_o: [usizex8; SSAA_SQ],
    src_x: [usizex8; SSAA_SQ],
    src_y: [usizex8; SSAA_SQ],
}

impl<const SSAA_SQ: usize> SsaaCoords<SSAA_SQ> {
    pub fn new() -> Self {
        const FULL_USIZE_MAX: usizex8 = usizex8::from_array([usize::MAX; 8]);
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
    let src_w = usizex8::from_array([src.width(); 8]);
    let src_h = usizex8::from_array([src.height(); 8]);
    let src_l = usizex8::from_array([src.length(); 8]);

    // SUM SUBPIXELS

    let mut ssaa_px = [0; 8];
    let mut src_sum = [u16x4::from_array([0; 4]); 8];

    for i in 0..SSAA_SQ {
        let src_o = src_coords.src_o[i];
        let src_x = src_coords.src_x[i];
        let src_y = src_coords.src_y[i];
        let src_i = src_y * src_w + src_x;

        let usable_x = src_x.simd_lt(src_w);
        let usable_y = src_y.simd_lt(src_h);
        let usable_l = src_i.simd_lt(src_l);
        let usable = (usable_x & usable_y & usable_l).to_array();

        for j in 0..8 {
            if usable[j] {
                let rgba = src.get(src_i[j]).into();
                src_sum[src_o[j]] += u8x4::from_array(rgba).cast();
                ssaa_px[src_o[j]] += 1;
            }
        }
    }

    // DIVIDE BY NUMBER OF SUBPIXELS

    let mut result = u16x32::from_array([0; 32]);
    for i in 0..8 {
        let j = i * 4;
        let result = &mut result.as_mut_array()[j..][..4];
        let src_sum = src_sum[i].to_array();

        let src = if true {
            // better perf but some weird line SouthEast
            src_sum.map(|sum| sum / (SSAA_SQ as u16))
        } else {
            let num_px = match ssaa_px[i] {
                0 => 1,
                n => n,
            };
            src_sum.map(|sum| sum / num_px)
        };

        result.copy_from_slice(&src);
    }

    EightPixels(result)
}
