// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Vectorized byte scanning used by the encoder and decoder.
//!
//! Two primitives are provided:
//!
//! * [`find_encode`]: the index of the first byte that must be percent-encoded
//!   for a given [`AsciiSet`] (any non-ASCII byte, or an ASCII byte in the set).
//! * [`find_percent`]: the index of the first `%` byte (a specialized `memchr`).
//!
//! `AsciiSet` stores its 128-bit membership bitmap "nibble-transposed": byte
//! `table[lo]` has bit `hi` set when the byte `hi << 4 | lo` is in the set.
//! That layout is directly usable as a 16-entry shuffle table (`pshufb` on x86,
//! `tbl` on AArch64), so classifying 16 input bytes takes two table lookups:
//!
//! ```text
//! row  = table[b & 0x0F]          // which high nibbles are in the set
//! bit  = POW2[b >> 4]             // 1 << hi for ASCII, 0 for non-ASCII
//! hit  = (row & bit) == bit       // non-ASCII: (row & 0) == 0 is always true
//! ```
//!
//! Every function here returns the same result as the obvious scalar loop; the
//! SIMD paths only run on inputs of at least 16 bytes.

// The decoding helpers are unused without `alloc`, and the vector helpers are
// unused on targets without a vector implementation.
#![cfg_attr(not(feature = "alloc"), allow(dead_code))]

use crate::AsciiSet;

/// Index of the first byte in `bytes` that `set` says must be percent-encoded,
/// or `bytes.len()` if there is none.
///
/// The short-input path is always inlined; the vector loop is not, so that
/// tiny inputs (such as encoding one `char` at a time) stay cheap.
#[inline(always)]
pub(crate) fn find_encode(bytes: &[u8], set: &AsciiSet) -> usize {
    // Short runs are common (for example between spaces in a query string),
    // and a few scalar checks are cheaper than calling into the vector loop.
    // Only runs longer than the prefix go to the vector loop.
    const PREFIX: usize = 8;
    let prefix = bytes.len().min(PREFIX);
    if let Some(i) = bytes[..prefix]
        .iter()
        .position(|&b| set.should_percent_encode(b))
    {
        return i;
    }
    let rest = &bytes[prefix..];
    if rest.len() >= 16 {
        if let Some(i) = arch::find_encode(rest, set) {
            return prefix + i;
        }
    }
    prefix + find_encode_scalar(rest, set)
}

#[inline]
fn find_encode_scalar(bytes: &[u8], set: &AsciiSet) -> usize {
    bytes
        .iter()
        .position(|&b| set.should_percent_encode(b))
        .unwrap_or(bytes.len())
}

/// Index of the first `%` in `bytes`, or `bytes.len()` if there is none.
#[inline(always)]
pub(crate) fn find_percent(bytes: &[u8]) -> usize {
    if bytes.len() >= 16 {
        if let Some(i) = arch::find_byte(bytes, b'%') {
            return i;
        }
    } else if bytes.len() >= 8 {
        return find_percent_swar(bytes);
    }
    find_percent_scalar(bytes)
}

/// SWAR (SIMD within a register) search for `%` in 8 to 15 bytes, using two
/// possibly overlapping 8-byte words. Used for inputs too short for the
/// vector path, such as most hostnames.
#[inline(always)]
fn find_percent_swar(bytes: &[u8]) -> usize {
    debug_assert!(bytes.len() >= 8 && bytes.len() < 16);
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    const PERCENT: u64 = LO * b'%' as u64;
    // Lanes that equal `%` become zero after the XOR. The classic
    // "has zero byte" test then sets the high bit of each zero lane, and of
    // no lane below the first zero lane (later lanes may have false
    // positives, which is fine because only the first match is used).
    #[inline(always)]
    fn first_percent(word: u64) -> Option<usize> {
        let x = word ^ PERCENT;
        let zeros = x.wrapping_sub(LO) & !x & HI;
        if zeros == 0 {
            None
        } else {
            Some((zeros.trailing_zeros() / 8) as usize)
        }
    }
    let len = bytes.len();
    let mut head = [0u8; 8];
    head.copy_from_slice(&bytes[..8]);
    if let Some(i) = first_percent(u64::from_le_bytes(head)) {
        return i;
    }
    let mut tail = [0u8; 8];
    tail.copy_from_slice(&bytes[len - 8..]);
    match first_percent(u64::from_le_bytes(tail)) {
        Some(i) => len - 8 + i,
        None => len,
    }
}

#[inline]
fn find_percent_scalar(bytes: &[u8]) -> usize {
    bytes.iter().position(|&b| b == b'%').unwrap_or(bytes.len())
}

/// The positions of `%` bytes in a 16-byte window.
///
/// Returns `None` when no vector implementation is available.
#[inline(always)]
pub(crate) fn percent_mask16(window: &[u8; 16]) -> Option<Mask16> {
    arch::byte_mask16(window, b'%')
}

/// A set of lane positions in a 16-byte window.
///
/// Each lane is represented by one bit, at position `lane << LANE_SHIFT`.
#[derive(Clone, Copy)]
pub(crate) struct Mask16(u64);

#[allow(dead_code)]
impl Mask16 {
    #[inline(always)]
    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The number of lanes in the set.
    #[inline(always)]
    pub(crate) fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// The lowest lane in the set. Must not be called on an empty set.
    #[inline(always)]
    pub(crate) fn first(self) -> usize {
        (self.0.trailing_zeros() >> arch::LANE_SHIFT) as usize
    }

    /// Remove every lane below `lane` from the set.
    #[inline(always)]
    pub(crate) fn clear_below(self, lane: usize) -> Self {
        let shift = (lane as u32) << arch::LANE_SHIFT;
        Mask16(if shift < 64 {
            self.0 & (!0 << shift)
        } else {
            0
        })
    }

    /// Remove the lowest lane from the set.
    #[inline(always)]
    pub(crate) fn clear_first(self) -> Self {
        Mask16(self.0 & self.0.wrapping_sub(1))
    }
}

/// `POW2[hi]` is `1 << hi` for ASCII high nibbles, and 0 for non-ASCII ones so
/// that `(row & bit) == bit` holds for every non-ASCII byte.
#[allow(dead_code)]
const POW2: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0];

/// Expands to a loop returning the index of the first lane of `$bytes` for
/// which `$classify` yields a non-zero mask, or `$bytes.len()`.
///
/// Requires `$bytes.len() >= 16`. `$movemask(m)` must turn a 0x00/0xFF lane
/// mask into an integer with `$bits_per_lane` bits per lane, lowest lane first.
#[allow(unused_macros)]
macro_rules! scan {
    (
        $bytes:expr, $load:ident, $or:ident, $any:ident, $movemask:ident, $bits_per_lane:expr,
        |$v:ident| $classify:expr
    ) => {{
        let bytes: &[u8] = $bytes;
        let len = bytes.len();
        debug_assert!(len >= 16);
        let ptr = bytes.as_ptr();
        // Check the first 16 bytes on their own: short gaps between matches
        // are common, and should not pay for the unrolled loop below.
        let first = {
            let $v = $load(ptr);
            $movemask($classify)
        };
        let mut i = 16;
        let mut found = if first != 0 {
            (first.trailing_zeros() / $bits_per_lane) as usize
        } else {
            len
        };
        // 64 bytes per iteration while there is room, testing them together.
        'outer: while found == len && i + 64 <= len {
            let m0 = {
                let $v = $load(ptr.add(i));
                $classify
            };
            let m1 = {
                let $v = $load(ptr.add(i + 16));
                $classify
            };
            let m2 = {
                let $v = $load(ptr.add(i + 32));
                $classify
            };
            let m3 = {
                let $v = $load(ptr.add(i + 48));
                $classify
            };
            if $any($or($or(m0, m1), $or(m2, m3))) {
                let masks = [m0, m1, m2, m3];
                let mut k = 0;
                while k < 4 {
                    let bits = $movemask(masks[k]);
                    if bits != 0 {
                        found = i + 16 * k + (bits.trailing_zeros() / $bits_per_lane) as usize;
                        break 'outer;
                    }
                    k += 1;
                }
            }
            i += 64;
        }
        if found == len && i < len {
            loop {
                if i + 16 > len {
                    if i < len {
                        // Overlapping final load: the lanes before `i` were
                        // already checked and are known not to match.
                        i = len - 16;
                    } else {
                        break;
                    }
                }
                let bits = {
                    let $v = $load(ptr.add(i));
                    $movemask($classify)
                };
                if bits != 0 {
                    found = i + (bits.trailing_zeros() / $bits_per_lane) as usize;
                    break;
                }
                if i + 16 == len {
                    break;
                }
                i += 16;
            }
        }
        found
    }};
}

// The NEON intrinsics were stabilized in Rust 1.59, above this crate's MSRV.
// That is fine: rustc 1.51 through 1.59 do not report `target_feature = "neon"`
// to `cfg` on any aarch64 target (checked for Linux, macOS, Android and
// Windows), so with those compilers this module is not compiled and the scalar
// fallback is used.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(clippy::incompatible_msrv)]
mod arch {
    use super::POW2;
    use crate::AsciiSet;
    use core::arch::aarch64::*;

    // Returning `None` means "use the scalar implementation".

    #[inline(never)]
    pub(super) fn find_encode(bytes: &[u8], set: &AsciiSet) -> Option<usize> {
        if bytes.len() < 16 {
            return None;
        }
        // SAFETY: NEON is statically enabled (cfg above), and `scan!` only
        // performs 16-byte loads that end within `bytes`.
        unsafe {
            let table = vld1q_u8(set.table().as_ptr());
            let pow2 = vld1q_u8(POW2.as_ptr());
            let low_nibble = vdupq_n_u8(0x0F);
            Some(scan!(bytes, vld1q_u8, vorrq_u8, any, movemask, 4, |v| {
                let row = vqtbl1q_u8(table, vandq_u8(v, low_nibble));
                let bit = vqtbl1q_u8(pow2, vshrq_n_u8(v, 4));
                vceqq_u8(vandq_u8(row, bit), bit)
            }))
        }
    }

    #[inline(never)]
    pub(super) fn find_byte(bytes: &[u8], needle: u8) -> Option<usize> {
        if bytes.len() < 16 {
            return None;
        }
        // SAFETY: as above.
        unsafe {
            let n = vdupq_n_u8(needle);
            Some(scan!(bytes, vld1q_u8, vorrq_u8, any, movemask, 4, |v| {
                vceqq_u8(v, n)
            }))
        }
    }

    /// `movemask` produces 4 bits per lane; we keep one of them.
    pub(super) const LANE_SHIFT: u32 = 2;

    #[inline(always)]
    pub(super) fn byte_mask16(window: &[u8; 16], needle: u8) -> Option<super::Mask16> {
        // SAFETY: NEON is statically enabled; the load is exactly `window`.
        unsafe {
            let bits = movemask(vceqq_u8(vld1q_u8(window.as_ptr()), vdupq_n_u8(needle)));
            Some(super::Mask16(bits & 0x1111_1111_1111_1111))
        }
    }

    #[inline(always)]
    unsafe fn any(m: uint8x16_t) -> bool {
        vmaxvq_u8(m) != 0
    }

    /// Compress a 0x00/0xFF lane mask into a u64 with 4 bits per lane.
    #[inline(always)]
    unsafe fn movemask(m: uint8x16_t) -> u64 {
        let nibbles = vshrn_n_u16(vreinterpretq_u16_u8(m), 4);
        vget_lane_u64(vreinterpret_u64_u8(nibbles), 0)
    }
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
))]
mod arch {
    use super::POW2;
    use crate::AsciiSet;
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    #[inline(never)]
    pub(super) fn find_encode(bytes: &[u8], set: &AsciiSet) -> Option<usize> {
        if bytes.len() < 16 || !has_ssse3() {
            return None;
        }
        // SAFETY: SSSE3 support was checked above, and the slice is >= 16.
        Some(unsafe { find_encode_ssse3(bytes, set.table()) })
    }

    #[inline(always)]
    fn has_ssse3() -> bool {
        #[cfg(target_feature = "ssse3")]
        return true;
        #[cfg(all(not(target_feature = "ssse3"), feature = "std"))]
        return std::is_x86_feature_detected!("ssse3");
        #[cfg(all(not(target_feature = "ssse3"), not(feature = "std")))]
        return false;
    }

    #[target_feature(enable = "ssse3")]
    unsafe fn find_encode_ssse3(bytes: &[u8], table: &[u8; 16]) -> usize {
        let table = _mm_loadu_si128(table.as_ptr() as *const __m128i);
        let pow2 = _mm_loadu_si128(POW2.as_ptr() as *const __m128i);
        let low_nibble = _mm_set1_epi8(0x0F);
        scan!(bytes, load, _mm_or_si128, any, movemask, 1, |v| {
            let row = _mm_shuffle_epi8(table, _mm_and_si128(v, low_nibble));
            let hi = _mm_and_si128(_mm_srli_epi16(v, 4), low_nibble);
            let bit = _mm_shuffle_epi8(pow2, hi);
            _mm_cmpeq_epi8(_mm_and_si128(row, bit), bit)
        })
    }

    #[inline(never)]
    pub(super) fn find_byte(bytes: &[u8], needle: u8) -> Option<usize> {
        if bytes.len() < 16 {
            return None;
        }
        // SAFETY: SSE2 is statically enabled (cfg above), and `scan!` only
        // performs 16-byte loads that end within `bytes`.
        unsafe {
            let n = _mm_set1_epi8(needle as i8);
            Some(scan!(bytes, load, _mm_or_si128, any, movemask, 1, |v| {
                _mm_cmpeq_epi8(v, n)
            }))
        }
    }

    pub(super) const LANE_SHIFT: u32 = 0;

    #[inline(always)]
    pub(super) fn byte_mask16(window: &[u8; 16], needle: u8) -> Option<super::Mask16> {
        // SAFETY: SSE2 is statically enabled; the load is exactly `window`.
        unsafe {
            let m = _mm_cmpeq_epi8(load(window.as_ptr()), _mm_set1_epi8(needle as i8));
            Some(super::Mask16(_mm_movemask_epi8(m) as u32 as u64))
        }
    }

    #[inline(always)]
    unsafe fn load(p: *const u8) -> __m128i {
        _mm_loadu_si128(p as *const __m128i)
    }

    #[inline(always)]
    unsafe fn any(m: __m128i) -> bool {
        _mm_movemask_epi8(m) != 0
    }

    #[inline(always)]
    unsafe fn movemask(m: __m128i) -> u32 {
        _mm_movemask_epi8(m) as u32
    }
}

#[cfg(not(any(
    all(target_arch = "aarch64", target_feature = "neon"),
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
)))]
mod arch {
    use crate::AsciiSet;

    #[inline(always)]
    pub(super) fn find_encode(_: &[u8], _: &AsciiSet) -> Option<usize> {
        None
    }

    #[inline(always)]
    pub(super) fn find_byte(_: &[u8], _: u8) -> Option<usize> {
        None
    }

    pub(super) const LANE_SHIFT: u32 = 0;

    #[inline(always)]
    pub(super) fn byte_mask16(_: &[u8; 16], _: u8) -> Option<super::Mask16> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CONTROLS, NON_ALPHANUMERIC};
    use alloc::vec::Vec;

    fn sets() -> [&'static AsciiSet; 4] {
        const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
        [&AsciiSet::EMPTY, CONTROLS, FRAGMENT, NON_ALPHANUMERIC]
    }

    #[test]
    fn find_encode_matches_scalar_every_byte_every_position() {
        for set in sets().iter() {
            for len in 0..if cfg!(miri) { 20 } else { 80 } {
                for pos in 0..len {
                    for b in (0..=255u8).step_by(if cfg!(miri) { 37 } else { 1 }) {
                        let mut v: Vec<u8> = (0..len).map(|_| b'a').collect();
                        v[pos] = b;
                        assert_eq!(
                            find_encode(&v, set),
                            find_encode_scalar(&v, set),
                            "len={} pos={} byte={:#x}",
                            len,
                            pos,
                            b
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn find_encode_full_set_complement() {
        // Every ASCII byte is in the set: the first byte always matches.
        let all = AsciiSet::EMPTY.complement();
        let v: Vec<u8> = (0..128u8).collect();
        for start in 0..v.len() {
            assert_eq!(find_encode(&v[start..], &all), 0);
            assert_eq!(find_encode(&v[start..], &AsciiSet::EMPTY), v.len() - start);
        }
    }

    #[test]
    fn percent_mask16_matches_scalar() {
        for seed in 0..if cfg!(miri) { 200 } else { 5000u32 } {
            let mut w = [0u8; 16];
            for (k, b) in w.iter_mut().enumerate() {
                let x = seed.wrapping_mul(2654435761).wrapping_add(k as u32 * 40503);
                *b = if (x >> 7) % 5 == 0 {
                    b'%'
                } else {
                    (x >> 11) as u8
                };
            }
            if let Some(mut mask) = percent_mask16(&w) {
                let expected: Vec<usize> = (0..16).filter(|&k| w[k] == b'%').collect();
                let mut got = Vec::new();
                for lane in 0..=17 {
                    let rest: Vec<usize> =
                        expected.iter().copied().filter(|&k| k >= lane).collect();
                    let mut m = mask.clear_below(lane);
                    let mut got = Vec::new();
                    while !m.is_empty() {
                        got.push(m.first());
                        m = m.clear_below(m.first() + 1);
                    }
                    assert_eq!(got, rest, "{:?} {}", w, lane);
                }
                while !mask.is_empty() {
                    got.push(mask.first());
                    mask = mask.clear_first();
                }
                assert_eq!(got, expected, "{:?}", w);
            }
        }
    }

    #[test]
    fn find_percent_swar_exhaustive() {
        // Every byte value, at every position, for every length handled by
        // the SWAR path, both alone and followed by a `%` (borrows in the
        // zero-byte test can set bits above a real match, never below).
        for len in 8..16 {
            for pos in 0..len {
                for b in 0..=255u8 {
                    let mut v: Vec<u8> = (0..len).map(|_| b'a').collect();
                    v[pos] = b;
                    assert_eq!(find_percent(&v), find_percent_scalar(&v), "{:?}", v);
                    if pos + 1 < len {
                        v[pos + 1] = b'%';
                        assert_eq!(find_percent(&v), find_percent_scalar(&v), "{:?}", v);
                    }
                }
            }
        }
    }

    #[test]
    fn find_percent_matches_scalar() {
        for len in 0..100 {
            let base: Vec<u8> = (0..len).map(|i| (i * 37 % 251) as u8 | 0x40).collect();
            assert_eq!(find_percent(&base), find_percent_scalar(&base));
            for pos in 0..len {
                let mut v = base.clone();
                v[pos] = b'%';
                assert_eq!(find_percent(&v), pos);
                if pos + 1 < len {
                    v[pos + 1] = b'%';
                    assert_eq!(find_percent(&v), pos);
                }
            }
        }
    }

    /// Input lengths for the structured sweeps below.
    ///
    /// Every length up to 100, then every third length up to 300 (3 is
    /// coprime with 16 and 64, so the end of the input still falls at every
    /// alignment relative to the 16- and 64-byte steps of `scan!`).
    ///
    /// Under Miri, a short list that still reaches every part of `scan!` for
    /// both `find_percent` (input starts at 0) and `find_encode` (the vector
    /// part starts after the 8-byte prefix): first 16 only, exactly 16, the
    /// overlapping tail, one and several 64-byte iterations with and without
    /// 16-byte steps and an overlapping tail after them.
    fn sweep_lens() -> Vec<usize> {
        if cfg!(miri) {
            alloc::vec![0, 5, 8, 15, 16, 17, 24, 25, 39, 80, 81, 88, 89, 101, 109, 172]
        } else {
            (0..=100).chain((101..=300).step_by(3)).collect()
        }
    }

    /// Positions to place the first match at in an input of length `len`.
    ///
    /// Every position natively. Under Miri every seventh position (7 is
    /// coprime with 16 and 64, and the lengths vary, so all lane and sub-mask
    /// alignments are still reached), plus the last 4 positions so that the
    /// overlapping tail load is always exercised.
    fn sweep_positions(len: usize) -> Vec<usize> {
        if cfg!(miri) {
            (0..len).filter(|&p| p % 7 == 0 || p + 4 >= len).collect()
        } else {
            (0..len).collect()
        }
    }

    /// Where to put a second match, relative to the first one at `p`: none,
    /// in the same 16-byte lane group, in the next one, a whole 64-byte
    /// iteration later, and at the very end (in the overlapping tail).
    fn second_positions(p: usize, len: usize) -> Vec<Option<usize>> {
        let offsets: &[usize] = if cfg!(miri) {
            &[16]
        } else {
            &[1, 2, 15, 16, 17, 47, 64]
        };
        let mut out = alloc::vec![None];
        out.extend(
            offsets
                .iter()
                .map(|&d| p + d)
                .chain(core::iter::once(len - 1))
                .filter(|&q| q > p && q < len)
                .map(Some),
        );
        out
    }

    #[test]
    fn find_encode_two_matches_every_alignment() {
        const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
        const CUSTOM: &AsciiSet = &AsciiSet::EMPTY.add(0).add(b' ').add(b'%').add(0x7F);
        // ASCII bytes in each set; non-ASCII bytes are added below.
        let cases: [(&AsciiSet, &[u8]); 5] = [
            (&AsciiSet::EMPTY, &[]),
            (CONTROLS, &[0x00, 0x1F, 0x7F]),
            (FRAGMENT, &[b' ', b'`', 0x00]),
            (NON_ALPHANUMERIC, b"%~/"),
            (CUSTOM, &[0x00, 0x7F, b'%']),
        ];
        // Not in any of the sets above.
        const CLEAN: &[u8] = b"xyzXYZ0189qQ";
        let clean = |i: usize| CLEAN[i % CLEAN.len()];
        let lens = sweep_lens();
        let mut buf: Vec<u8> = (0..*lens.iter().max().unwrap()).map(clean).collect();
        for &(set, ascii) in cases.iter() {
            let mut specials = ascii.to_vec();
            specials.extend_from_slice(&[0x80, 0xC3, 0xFF]);
            for &b in specials.iter().chain(CLEAN) {
                assert_eq!(set.should_percent_encode(b), !CLEAN.contains(&b));
            }
            for &len in lens.iter() {
                let v = &mut buf[..len];
                assert_eq!(find_encode(v, set), len, "len={}", len);
                for p in sweep_positions(len) {
                    for (k, q) in second_positions(p, len).into_iter().enumerate() {
                        // Rotate through the special bytes so that each one
                        // lands at every alignment without multiplying the
                        // number of cases.
                        let b = specials[(p + k + len) % specials.len()];
                        let b2 = specials[(p + 2 * k + 1) % specials.len()];
                        v[p] = b;
                        if let Some(q) = q {
                            v[q] = b2;
                        }
                        assert_eq!(
                            find_encode(v, set),
                            p,
                            "len={} p={} byte={:#x} q={:?} byte2={:#x}",
                            len,
                            p,
                            b,
                            q,
                            b2
                        );
                        v[p] = clean(p);
                        if let Some(q) = q {
                            v[q] = clean(q);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn find_percent_two_matches_every_alignment() {
        // Not `%`, but each differs from it (0x25) in a single bit, plus one
        // unrelated byte so that the pattern does not repeat every 8 bytes.
        const NEAR: &[u8] = &[0x24, 0x27, 0x21, 0x2D, 0x35, 0x05, 0x65, 0xA5, b'a'];
        let near = |i: usize| NEAR[i % NEAR.len()];
        let lens = sweep_lens();
        let mut buf: Vec<u8> = (0..*lens.iter().max().unwrap()).map(near).collect();
        for &len in lens.iter() {
            let v = &mut buf[..len];
            assert_eq!(find_percent(v), len, "len={}", len);
            for p in sweep_positions(len) {
                for q in second_positions(p, len) {
                    v[p] = b'%';
                    if let Some(q) = q {
                        v[q] = b'%';
                    }
                    assert_eq!(find_percent(v), p, "len={} p={} q={:?}", len, p, q);
                    v[p] = near(p);
                    if let Some(q) = q {
                        v[q] = near(q);
                    }
                }
            }
        }
    }

    /// Checks `Mask16` through its in-crate API, for every subset of lanes
    /// (a subsample under Miri). This validates whichever lane layout the
    /// target uses: one bit per lane on x86 (`LANE_SHIFT == 0`), or one bit
    /// per 4-bit nibble on AArch64 (`LANE_SHIFT == 2`).
    #[test]
    fn mask16_every_lane_subset() {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        assert_eq!(arch::LANE_SHIFT, 2);
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "x86", target_feature = "sse2")
        ))]
        assert_eq!(arch::LANE_SHIFT, 0);
        // Not `%`, but each differs from it (0x25) in a single bit.
        const NEAR: [u8; 8] = [0x24, 0x27, 0x21, 0x2D, 0x35, 0x05, 0x65, 0xA5];
        for lanes in (0..=0xFFFFu32).step_by(if cfg!(miri) { 251 } else { 1 }) {
            let mut w = [0u8; 16];
            for (k, b) in w.iter_mut().enumerate() {
                *b = if (lanes >> k) & 1 != 0 {
                    b'%'
                } else {
                    NEAR[(k + lanes as usize) % NEAR.len()]
                };
            }
            let mask = match percent_mask16(&w) {
                Some(mask) => mask,
                None => {
                    // Only targets without a vector implementation.
                    assert!(!cfg!(any(
                        all(target_arch = "aarch64", target_feature = "neon"),
                        target_arch = "x86_64",
                        all(target_arch = "x86", target_feature = "sse2")
                    )));
                    return;
                }
            };
            // Exactly one bit per lane, at `lane << LANE_SHIFT`.
            let raw = (0..16)
                .filter(|&k| (lanes >> k) & 1 != 0)
                .fold(0u64, |acc, k| acc | 1 << (k << arch::LANE_SHIFT));
            assert_eq!(mask.0, raw, "{:?}", w);
            assert_eq!(mask.is_empty(), lanes == 0);
            assert_eq!(mask.len(), lanes.count_ones());
            // `first` + `clear_first` enumerate the lanes in order.
            let mut m = mask;
            let mut rest = lanes;
            while rest != 0 {
                assert!(!m.is_empty(), "{:?}", w);
                assert_eq!(m.len(), rest.count_ones(), "{:?}", w);
                assert_eq!(m.first(), rest.trailing_zeros() as usize, "{:?}", w);
                m = m.clear_first();
                rest &= rest - 1;
            }
            assert!(m.is_empty(), "{:?}", w);
            // `clear_below`, including lanes at and past the end of the window
            // (16 shifts by 64 bits with 4-bit lanes).
            for lane in 0..=20 {
                let keep = lanes & (!0u32 << lane);
                let m = mask.clear_below(lane);
                assert_eq!(m.is_empty(), keep == 0, "{:?} {}", w, lane);
                assert_eq!(m.len(), keep.count_ones(), "{:?} {}", w, lane);
                if keep != 0 {
                    assert_eq!(
                        m.first(),
                        keep.trailing_zeros() as usize,
                        "{:?} {}",
                        w,
                        lane
                    );
                }
            }
        }
    }
}
