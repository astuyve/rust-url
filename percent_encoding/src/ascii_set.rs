// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::{mem, ops};

/// Represents a set of characters or bytes in the ASCII range.
///
/// This is used in [`percent_encode`] and [`utf8_percent_encode`].
/// This is similar to [percent-encode sets](https://url.spec.whatwg.org/#percent-encoded-bytes).
///
/// Use the `add` method of an existing set to define a new set. For example:
///
/// [`percent_encode`]: crate::percent_encode
/// [`utf8_percent_encode`]: crate::utf8_percent_encode
///
/// ```
/// use percent_encoding::{AsciiSet, CONTROLS};
///
/// /// https://url.spec.whatwg.org/#fragment-percent-encode-set
/// const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct AsciiSet {
    /// The 128-bit membership bitmap, "nibble-transposed": bit `hi` of
    /// `table[lo]` is set when the byte `hi << 4 | lo` is in the set.
    ///
    /// This layout lets SIMD code use the bitmap directly as a 16-entry
    /// shuffle table (see `simd.rs`).
    table: [u8; 16],
}

/// `BIT[hi]` is the bit for high nibble `hi` within a table row.
///
/// Indexing it with the high nibble of a non-ASCII byte (8..=15) is out of
/// bounds, so `add`, `remove` and `contains` panic for non-ASCII bytes in every
/// build profile (and fail to compile in a `const` context), as they did before
/// the table layout changed. A plain `1 << (byte >> 4)` would instead overflow,
/// which only panics when overflow checks are enabled: in release builds the
/// shift amount wraps, and `add(0x80)` would silently add NUL.
const BIT: [u8; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

impl AsciiSet {
    /// An empty set.
    pub const EMPTY: Self = Self { table: [0; 16] };

    /// Called with UTF-8 bytes rather than code points.
    /// Not used for non-ASCII bytes.
    #[allow(dead_code)] // Used in tests and static assertions.
    pub(crate) const fn contains(&self, byte: u8) -> bool {
        self.table[(byte & 0x0F) as usize] & BIT[(byte >> 4) as usize] != 0
    }

    #[inline]
    pub(crate) fn should_percent_encode(&self, byte: u8) -> bool {
        // Bits 8..16 of the row are set so that every non-ASCII byte (high
        // nibble >= 8) is reported as needing encoding without a branch.
        ((u32::from(self.table[usize::from(byte & 0x0F)]) | 0xFF00) >> (byte >> 4)) & 1 != 0
    }

    #[inline]
    #[allow(dead_code)] // Unused on targets without a SIMD implementation.
    pub(crate) fn table(&self) -> &[u8; 16] {
        &self.table
    }

    pub const fn add(&self, byte: u8) -> Self {
        let mut table = self.table;
        table[(byte & 0x0F) as usize] |= BIT[(byte >> 4) as usize];
        Self { table }
    }

    pub const fn remove(&self, byte: u8) -> Self {
        let mut table = self.table;
        table[(byte & 0x0F) as usize] &= !BIT[(byte >> 4) as usize];
        Self { table }
    }

    /// Return the union of two sets.
    pub const fn union(&self, other: Self) -> Self {
        let mut table = self.table;
        let mut i = 0;
        while i < 16 {
            table[i] |= other.table[i];
            i += 1;
        }
        Self { table }
    }

    /// Return the negation of the set.
    pub const fn complement(&self) -> Self {
        let mut table = self.table;
        let mut i = 0;
        while i < 16 {
            table[i] = !table[i];
            i += 1;
        }
        Self { table }
    }
}

impl ops::Add for AsciiSet {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.union(other)
    }
}

impl ops::Not for AsciiSet {
    type Output = Self;

    fn not(self) -> Self {
        self.complement()
    }
}

/// The set of 0x00 to 0x1F (C0 controls), and 0x7F (DEL).
///
/// Note that this includes the newline and tab characters, but not the space 0x20.
///
/// <https://url.spec.whatwg.org/#c0-control-percent-encode-set>
pub const CONTROLS: &AsciiSet = &AsciiSet {
    // C0: 0x00 to 0x1F (high nibbles 0 and 1 for every low nibble),
    // plus DEL: 0x7F (high nibble 7, low nibble F).
    table: [
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b11,
        0b1000_0011,
    ],
};

macro_rules! static_assert {
    ($( $bool: expr, )+) => {
        fn _static_assert() {
            $(
                let _ = mem::transmute::<[u8; $bool as usize], u8>;
            )+
        }
    }
}

static_assert! {
    CONTROLS.contains(0x00),
    CONTROLS.contains(0x1F),
    !CONTROLS.contains(0x20),
    !CONTROLS.contains(0x7E),
    CONTROLS.contains(0x7F),
}

/// Everything that is not an ASCII letter or digit.
///
/// This is probably more eager than necessary in any context.
pub const NON_ALPHANUMERIC: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'-')
    .add(b'.')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'_')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'~');

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_op() {
        let left = AsciiSet::EMPTY.add(b'A');
        let right = AsciiSet::EMPTY.add(b'B');
        let expected = AsciiSet::EMPTY.add(b'A').add(b'B');
        assert_eq!(left + right, expected);
    }

    #[test]
    fn not_op() {
        let set = AsciiSet::EMPTY.add(b'A').add(b'B');
        let not_set = !set;
        assert!(!not_set.contains(b'A'));
        assert!(not_set.contains(b'C'));
    }

    // These must panic in every build profile, including release builds
    // where overflow checks are off (CI runs the tests in release mode too).
    #[test]
    #[should_panic]
    fn add_non_ascii_panics() {
        let _ = AsciiSet::EMPTY.add(0x80);
    }

    #[test]
    #[should_panic]
    fn add_non_ascii_high_panics() {
        let _ = AsciiSet::EMPTY.add(0xFF);
    }

    #[test]
    #[should_panic]
    fn remove_non_ascii_panics() {
        let _ = CONTROLS.remove(0x80);
    }

    #[test]
    #[should_panic]
    fn contains_non_ascii_panics() {
        let _ = CONTROLS.contains(0xC0);
    }

    #[test]
    #[should_panic]
    fn add_non_ascii_low_nibble_f_panics() {
        // Would alias to 0x0F (SI) with a wrapping shift.
        let _ = AsciiSet::EMPTY.add(0x8F);
    }

    #[test]
    fn contains_matches_reference_bitmap() {
        // Cross-check against a straightforward 128-bit bitmap.
        let custom = AsciiSet::EMPTY.add(b'A').add(0x7F).add(0).add(b'~');
        for set in [&custom, CONTROLS, NON_ALPHANUMERIC].iter() {
            let mut reference = 0u128;
            for b in 0..0x80u8 {
                if set.contains(b) {
                    reference |= 1 << b;
                }
            }
            for b in 0..=0xFFu8 {
                let expected = b >= 0x80 || reference & (1 << b) != 0;
                assert_eq!(set.should_percent_encode(b), expected, "{:#x}", b);
            }
        }
        for b in 0..0x80u8 {
            assert_eq!(CONTROLS.contains(b), b < 0x20 || b == 0x7F, "{:#x}", b);
            assert_eq!(
                NON_ALPHANUMERIC.contains(b),
                !(b as char).is_ascii_alphanumeric()
            );
            assert!(AsciiSet::EMPTY.add(b).contains(b));
            assert!(!AsciiSet::EMPTY.add(b).remove(b).contains(b));
            assert_eq!(
                AsciiSet::EMPTY.complement().remove(b).complement(),
                AsciiSet::EMPTY.add(b)
            );
        }
    }

    /// This test ensures that we can get the union of two sets as a constant value, which is
    /// useful for defining sets in a modular way.
    #[test]
    fn union() {
        const A: AsciiSet = AsciiSet::EMPTY.add(b'A');
        const B: AsciiSet = AsciiSet::EMPTY.add(b'B');
        const UNION: AsciiSet = A.union(B);
        const EXPECTED: AsciiSet = AsciiSet::EMPTY.add(b'A').add(b'B');
        assert_eq!(UNION, EXPECTED);
    }

    /// This test ensures that we can get the complement of a set as a constant value, which is
    /// useful for defining sets in a modular way.
    #[test]
    fn complement() {
        const BOTH: AsciiSet = AsciiSet::EMPTY.add(b'A').add(b'B');
        const COMPLEMENT: AsciiSet = BOTH.complement();
        assert!(!COMPLEMENT.contains(b'A'));
        assert!(!COMPLEMENT.contains(b'B'));
        assert!(COMPLEMENT.contains(b'C'));
    }
}
