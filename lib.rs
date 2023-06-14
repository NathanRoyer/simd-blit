//! Alpha Compositing & SSAA, optionally using SIMD
//!
//! If you want this crate to use SIMD, activate the `simd` feature;
//! You will need a nightly toolchain for this to work, however.
//!
//! If the feature is disabled, a sequential implementation is also provided.

#![no_std]
#![cfg_attr(feature = "simd", feature(portable_simd))]

use rgb::RGBA8;

#[cfg_attr(feature = "simd", path = "simd.rs")]
#[cfg_attr(not(feature = "simd"), path = "sequential.rs")]
mod implementation;

#[doc(inline)]
pub use implementation::*;

/// Trait for 2D-sized & indexed pixel storage
pub trait PixelArray {
    fn get   (&self, index: usize) -> RGBA8;
    fn width (&self) -> usize;
    fn height(&self) -> usize;
    fn length(&self) -> usize;
    fn bytes_per_pixel() -> usize;
    fn has_alpha() -> bool;
}
