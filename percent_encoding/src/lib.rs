// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! URLs use special characters to indicate the parts of the request.
//! For example, a `?` question mark marks the end of a path and the start of a query string.
//! In order for that character to exist inside a path, it needs to be encoded differently.
//!
//! Percent encoding replaces reserved characters with the `%` escape character
//! followed by a byte value as two hexadecimal digits.
//! For example, an ASCII space is replaced with `%20`.
//!
//! When encoding, the set of characters that can (and should, for readability) be left alone
//! depends on the context.
//! The `?` question mark mentioned above is not a separator when used literally
//! inside of a query string, and therefore does not need to be encoded.
//! The [`AsciiSet`] parameter of [`percent_encode`] and [`utf8_percent_encode`]
//! lets callers configure this.
//!
//! This crate deliberately does not provide many different sets.
//! Users should consider in what context the encoded string will be used,
//! read relevant specifications, and define their own set.
//! This is done by using the `add` method of an existing set.
//!
//! # Examples
//!
//! ```
//! use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
//!
//! /// https://url.spec.whatwg.org/#fragment-percent-encode-set
//! const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
//!
//! assert_eq!(utf8_percent_encode("foo <bar>", FRAGMENT).to_string(), "foo%20%3Cbar%3E");
//! ```
#![no_std]

// Used for runtime CPU feature detection on x86.
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::{borrow::Cow, string::String, vec::Vec};
#[cfg(feature = "alloc")]
use core::ptr;
use core::{fmt, slice, str};

pub use self::ascii_set::{AsciiSet, CONTROLS, NON_ALPHANUMERIC};

mod ascii_set;
mod simd;

/// Return the percent-encoding of the given byte.
///
/// This is unconditional, unlike `percent_encode()` which has an `AsciiSet` parameter.
///
/// # Examples
///
/// ```
/// use percent_encoding::percent_encode_byte;
///
/// assert_eq!("foo bar".bytes().map(percent_encode_byte).collect::<String>(),
///            "%66%6F%6F%20%62%61%72");
/// ```
/// `%XY` for every byte, back to back, followed by one padding byte.
///
/// Entry `b` is at `3 * b`. The padding lets `encode_chunks` copy 4 bytes
/// from any entry (the fourth is overwritten or ignored).
static ENC_TABLE: &[u8; 769] = b"\
      %00%01%02%03%04%05%06%07%08%09%0A%0B%0C%0D%0E%0F\
      %10%11%12%13%14%15%16%17%18%19%1A%1B%1C%1D%1E%1F\
      %20%21%22%23%24%25%26%27%28%29%2A%2B%2C%2D%2E%2F\
      %30%31%32%33%34%35%36%37%38%39%3A%3B%3C%3D%3E%3F\
      %40%41%42%43%44%45%46%47%48%49%4A%4B%4C%4D%4E%4F\
      %50%51%52%53%54%55%56%57%58%59%5A%5B%5C%5D%5E%5F\
      %60%61%62%63%64%65%66%67%68%69%6A%6B%6C%6D%6E%6F\
      %70%71%72%73%74%75%76%77%78%79%7A%7B%7C%7D%7E%7F\
      %80%81%82%83%84%85%86%87%88%89%8A%8B%8C%8D%8E%8F\
      %90%91%92%93%94%95%96%97%98%99%9A%9B%9C%9D%9E%9F\
      %A0%A1%A2%A3%A4%A5%A6%A7%A8%A9%AA%AB%AC%AD%AE%AF\
      %B0%B1%B2%B3%B4%B5%B6%B7%B8%B9%BA%BB%BC%BD%BE%BF\
      %C0%C1%C2%C3%C4%C5%C6%C7%C8%C9%CA%CB%CC%CD%CE%CF\
      %D0%D1%D2%D3%D4%D5%D6%D7%D8%D9%DA%DB%DC%DD%DE%DF\
      %E0%E1%E2%E3%E4%E5%E6%E7%E8%E9%EA%EB%EC%ED%EE%EF\
      %F0%F1%F2%F3%F4%F5%F6%F7%F8%F9%FA%FB%FC%FD%FE%FF\
      \0";

#[inline]
pub fn percent_encode_byte(byte: u8) -> &'static str {
    let index = usize::from(byte) * 3;
    // SAFETY: ENC_TABLE is ascii-only, so any subset of it should be
    // ascii-only too, which is valid utf8.
    unsafe { str::from_utf8_unchecked(&ENC_TABLE[index..index + 3]) }
}

/// Percent-encode the given bytes with the given set.
///
/// Non-ASCII bytes and bytes in `ascii_set` are encoded.
///
/// The return type:
///
/// * Implements `Iterator<Item = &str>` and therefore has a `.collect::<String>()` method,
/// * Implements `Display` and therefore has a `.to_string()` method,
/// * Implements `Into<Cow<str>>` borrowing `input` when none of its bytes are encoded.
///
/// # Examples
///
/// ```
/// use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
///
/// assert_eq!(percent_encode(b"foo bar?", NON_ALPHANUMERIC).to_string(), "foo%20bar%3F");
/// ```
#[inline]
pub fn percent_encode<'a>(input: &'a [u8], ascii_set: &'static AsciiSet) -> PercentEncode<'a> {
    PercentEncode {
        bytes: input,
        ascii_set,
    }
}

/// Percent-encode the UTF-8 encoding of the given string.
///
/// See [`percent_encode`] regarding the return type.
///
/// # Examples
///
/// ```
/// use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
///
/// assert_eq!(utf8_percent_encode("foo bar?", NON_ALPHANUMERIC).to_string(), "foo%20bar%3F");
/// ```
#[inline]
pub fn utf8_percent_encode<'a>(input: &'a str, ascii_set: &'static AsciiSet) -> PercentEncode<'a> {
    percent_encode(input.as_bytes(), ascii_set)
}

/// The return type of [`percent_encode`] and [`utf8_percent_encode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercentEncode<'a> {
    bytes: &'a [u8],
    ascii_set: &'static AsciiSet,
}

impl<'a> Iterator for PercentEncode<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let (&first_byte, remaining) = self.bytes.split_first()?;
        if self.ascii_set.should_percent_encode(first_byte) {
            self.bytes = remaining;
            Some(percent_encode_byte(first_byte))
        } else {
            // 1 for first_byte + the run of following bytes left unchanged.
            let (unchanged_slice, remaining) = self
                .bytes
                .split_at(1 + simd::find_encode(remaining, self.ascii_set));
            self.bytes = remaining;
            // SAFETY: bytes that are not percent-encoded are ASCII.
            Some(unsafe { str::from_utf8_unchecked(unchanged_slice) })
        }
    }

    // `String::extend` and other internal-iteration consumers go through
    // `fold`. Overriding it keeps the remaining input in a local rather than
    // writing it back to `self` after every item.
    //
    // It is deliberately not inlined: it is still specialized for each
    // caller's closure, but inlining it into callers such as `url`'s path
    // parser made those functions slower even on inputs that never reach it
    // (measured: +20-30 cycles per `Url::parse("https://example.com/")`).
    #[inline(never)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, &'a str) -> B,
    {
        let ascii_set = self.ascii_set;
        let mut bytes = self.bytes;
        let mut acc = init;
        while let Some((&first_byte, remaining)) = bytes.split_first() {
            if ascii_set.should_percent_encode(first_byte) {
                acc = f(acc, percent_encode_byte(first_byte));
                bytes = remaining;
            } else {
                let (unchanged_slice, remaining) =
                    bytes.split_at(1 + simd::find_encode(remaining, ascii_set));
                // SAFETY: bytes that are not percent-encoded are ASCII.
                acc = f(acc, unsafe { str::from_utf8_unchecked(unchanged_slice) });
                bytes = remaining;
            }
        }
        acc
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.bytes.is_empty() {
            (0, Some(0))
        } else {
            (1, Some(self.bytes.len()))
        }
    }
}

impl fmt::Display for PercentEncode<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        encode_chunks(self.bytes, self.ascii_set, |chunk| {
            formatter.write_str(chunk)
        })
    }
}

/// Percent-encode `bytes`, passing the output to `write` in chunks.
///
/// Long runs of unchanged bytes are found with a vectorized scan and passed
/// through as-is. Elsewhere, input is encoded in blocks into a stack buffer
/// without branching on each byte, so that densely-encoded input (such as
/// non-ASCII text, or a query string with many spaces) does not cost one
/// `write` call per encoded byte.
#[inline]
fn encode_chunks<E>(
    mut bytes: &[u8],
    ascii_set: &AsciiSet,
    mut write: impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
    /// Input bytes per block.
    const BLOCK: usize = 32;
    /// Unchanged runs at least this long are written directly.
    const DIRECT: usize = 16;
    // Each input byte produces at most 3 bytes, and the last 4-byte store
    // may write one byte past the end of the output.
    let mut buf = [0u8; BLOCK * 3 + 1];
    while !bytes.is_empty() {
        let unchanged = simd::find_encode(bytes, ascii_set);
        if unchanged >= DIRECT || unchanged == bytes.len() {
            // SAFETY: bytes that are not percent-encoded are ASCII.
            write(unsafe { str::from_utf8_unchecked(&bytes[..unchanged]) })?;
            bytes = &bytes[unchanged..];
            if bytes.is_empty() {
                break;
            }
        }
        let (block, rest) = bytes.split_at(bytes.len().min(BLOCK));
        let mut len = 0;
        for &byte in block {
            let encode = ascii_set.should_percent_encode(byte);
            let mut entry = [byte, 0, 0, 0];
            if encode {
                let index = usize::from(byte) * 3;
                entry.copy_from_slice(&ENC_TABLE[index..index + 4]);
            }
            buf[len..len + 4].copy_from_slice(&entry);
            len += 1 + 2 * usize::from(encode);
        }
        // SAFETY: the output is ASCII: unchanged bytes and `%XY` escapes.
        write(unsafe { str::from_utf8_unchecked(&buf[..len]) })?;
        bytes = rest;
    }
    Ok(())
}

#[cfg(feature = "alloc")]
impl<'a> From<PercentEncode<'a>> for Cow<'a, str> {
    fn from(iter: PercentEncode<'a>) -> Self {
        let bytes = iter.bytes;
        let unchanged = simd::find_encode(bytes, iter.ascii_set);
        if unchanged == bytes.len() {
            // SAFETY: bytes that are not percent-encoded are ASCII.
            return Cow::Borrowed(unsafe { str::from_utf8_unchecked(bytes) });
        }
        // At least one byte grows from 1 to 3; leave some room for more.
        let mut string = String::with_capacity(bytes.len() + 2 + bytes.len() / 4);
        let _ = encode_chunks(bytes, iter.ascii_set, |chunk| {
            string.push_str(chunk);
            Ok::<(), core::convert::Infallible>(())
        });
        Cow::Owned(string)
    }
}

/// Percent-decode the given string.
///
/// <https://url.spec.whatwg.org/#string-percent-decode>
///
/// See [`percent_decode`] regarding the return type.
#[inline]
pub fn percent_decode_str(input: &str) -> PercentDecode<'_> {
    percent_decode(input.as_bytes())
}

/// Percent-decode the given bytes.
///
/// <https://url.spec.whatwg.org/#percent-decode>
///
/// Any sequence of `%` followed by two hexadecimal digits is decoded.
/// The return type:
///
/// * Implements `Into<Cow<u8>>` borrowing `input` when it contains no percent-encoded sequence,
/// * Implements `Iterator<Item = u8>` and therefore has a `.collect::<Vec<u8>>()` method,
/// * Has `decode_utf8()` and `decode_utf8_lossy()` methods.
///
/// # Examples
///
/// ```
/// use percent_encoding::percent_decode;
///
/// assert_eq!(percent_decode(b"foo%20bar%3f").decode_utf8().unwrap(), "foo bar?");
/// ```
#[inline]
pub fn percent_decode(input: &[u8]) -> PercentDecode<'_> {
    PercentDecode {
        bytes: input.iter(),
    }
}

/// The return type of [`percent_decode`].
#[derive(Clone, Debug)]
pub struct PercentDecode<'a> {
    bytes: slice::Iter<'a, u8>,
}

/// The value of each ASCII hex digit, and 0xFF for every other byte.
static HEX_DIGIT: [u8; 256] = {
    let mut table = [0xFF; 256];
    let mut i = 0;
    while i < 10 {
        table[b'0' as usize + i] = i as u8;
        i += 1;
    }
    let mut i = 0;
    while i < 6 {
        table[b'a' as usize + i] = 10 + i as u8;
        table[b'A' as usize + i] = 10 + i as u8;
        i += 1;
    }
    table
};

/// Decode the two hex digits `h` and `l`, if they are valid.
#[inline]
fn decode_hex_pair(h: u8, l: u8) -> Option<u8> {
    let h = HEX_DIGIT[usize::from(h)];
    let l = HEX_DIGIT[usize::from(l)];
    if (h | l) & 0xF0 == 0 {
        Some(h << 4 | l)
    } else {
        None
    }
}

/// If `bytes` (which follows a `%`) starts with two hex digits, decode them.
#[cfg(feature = "alloc")]
#[inline]
fn decode_after_percent(bytes: &[u8]) -> Option<u8> {
    match *bytes {
        [h, l, ..] => decode_hex_pair(h, l),
        _ => None,
    }
}

/// `&bytes[i]` without a bounds check.
///
/// # Safety
/// `i < bytes.len()`.
#[cfg(feature = "alloc")]
#[inline(always)]
unsafe fn src_at(bytes: &[u8], i: usize) -> &u8 {
    debug_assert!(i < bytes.len());
    bytes.get_unchecked(i)
}

/// Percent-decode `bytes` (which may or may not contain `%`) onto `out`.
#[cfg(feature = "alloc")]
fn decode_into(bytes: &[u8], out: &mut Vec<u8>) {
    // Decoding never makes the output longer than the input.
    out.reserve(bytes.len());
    let len = bytes.len();
    let mut read = 0;
    // SAFETY:
    // * `dst` has room for `len` bytes (reserved above).
    // * Invariant: `written <= read` (each step consumes at least as many
    //   bytes as it produces), so a store of `n` bytes at `dst + written` is
    //   in bounds whenever `bytes[read..read + n]` is.
    // * Only `dst[..written]` is exposed, and every byte of it was written.
    unsafe {
        let dst = out.as_mut_ptr().add(out.len());
        let mut written = 0;
        // The window loop reads up to 16 bytes past the window (a 16-byte
        // copy starting inside it, and the two digits after a final `%`).
        while read + 32 <= len {
            let src = bytes.as_ptr().add(read);
            let mut mask = match simd::percent_mask16(&*(src as *const [u8; 16])) {
                Some(mask) => mask,
                None => break,
            };
            if mask.is_empty() {
                // A long run without escapes: switch to the unrolled scanner.
                let run = 16 + simd::find_percent(&bytes[read + 16..]);
                ptr::copy_nonoverlapping(src, dst.add(written), run);
                read += run;
                written += run;
                continue;
            }
            if mask.len() >= 4 {
                // Dense escapes (such as encoded non-ASCII text): a plain
                // byte loop beats walking the mask. Decode until past this
                // window; the last escape reads at most `read + 17`.
                // Branches (rather than a computed advance) let the CPU
                // speculate ahead instead of waiting on each decode.
                let end = read + 16;
                while read < end {
                    let byte = *bytes.get_unchecked(read);
                    if byte == b'%' {
                        if let Some(decoded) =
                            decode_hex_pair(*src_at(bytes, read + 1), *src_at(bytes, read + 2))
                        {
                            *dst.add(written) = decoded;
                            written += 1;
                            read += 3;
                            continue;
                        }
                    }
                    *dst.add(written) = byte;
                    written += 1;
                    read += 1;
                }
                continue;
            }
            // Walk the `%` positions in this window. `pos` is the offset of
            // the first byte not yet consumed.
            let mut pos = 0;
            while !mask.is_empty() {
                let percent = mask.first();
                mask = mask.clear_first();
                if percent < pos {
                    // Already consumed as a digit of an earlier escape. (This
                    // cannot happen, since `%` is not a hex digit, but it
                    // keeps the invariant obvious.)
                    continue;
                }
                // Copy 16 bytes, but only keep the ones up to the `%`.
                ptr::copy_nonoverlapping(src.add(pos), dst.add(written), 16);
                written += percent - pos;
                if let Some(decoded) = decode_hex_pair(*src.add(percent + 1), *src.add(percent + 2))
                {
                    *dst.add(written) = decoded;
                    pos = percent + 3;
                } else {
                    *dst.add(written) = b'%';
                    pos = percent + 1;
                }
                written += 1;
            }
            if pos < 16 {
                ptr::copy_nonoverlapping(src.add(pos), dst.add(written), 16);
                written += 16 - pos;
                pos = 16;
            }
            read += pos;
        }
        while read < len {
            let byte = bytes[read];
            if byte == b'%' {
                if let Some(decoded) = decode_after_percent(&bytes[read + 1..]) {
                    *dst.add(written) = decoded;
                    written += 1;
                    read += 3;
                    continue;
                }
            }
            *dst.add(written) = byte;
            written += 1;
            read += 1;
        }
        out.set_len(out.len() + written);
    }
}

impl Iterator for PercentDecode<'_> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        let (&byte, rest) = self.bytes.as_slice().split_first()?;
        if byte == b'%' {
            if let [h, l, ref after @ ..] = *rest {
                if let Some(decoded) = decode_hex_pair(h, l) {
                    self.bytes = after.iter();
                    return Some(decoded);
                }
            }
        }
        self.bytes = rest.iter();
        Some(byte)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let bytes = self.bytes.len();
        ((bytes + 2) / 3, Some(bytes))
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<PercentDecode<'a>> for Cow<'a, [u8]> {
    fn from(iter: PercentDecode<'a>) -> Self {
        match iter.if_any() {
            Some(vec) => Cow::Owned(vec),
            None => Cow::Borrowed(iter.bytes.as_slice()),
        }
    }
}

impl<'a> PercentDecode<'a> {
    /// If the percent-decoding is different from the input, return it as a new bytes vector.
    #[cfg(feature = "alloc")]
    fn if_any(&self) -> Option<Vec<u8>> {
        let bytes = self.bytes.as_slice();
        let mut start = 0;
        loop {
            start += simd::find_percent(&bytes[start..]);
            if start == bytes.len() {
                // Nothing to decode
                return None;
            }
            if let Some(decoded_byte) = decode_after_percent(&bytes[start + 1..]) {
                // Every escape shrinks the output by two bytes.
                let mut decoded = Vec::with_capacity(bytes.len() - 2);
                decoded.extend_from_slice(&bytes[..start]);
                decoded.push(decoded_byte);
                decode_into(&bytes[start + 3..], &mut decoded);
                return Some(decoded);
            }
            start += 1;
        }
    }

    /// Decode the result of percent-decoding as UTF-8.
    ///
    /// This is return `Err` when the percent-decoded bytes are not well-formed in UTF-8.
    #[cfg(feature = "alloc")]
    pub fn decode_utf8(self) -> Result<Cow<'a, str>, str::Utf8Error> {
        match self.into() {
            Cow::Borrowed(bytes) => match str::from_utf8(bytes) {
                Ok(s) => Ok(s.into()),
                Err(e) => Err(e),
            },
            Cow::Owned(bytes) => match String::from_utf8(bytes) {
                Ok(s) => Ok(s.into()),
                Err(e) => Err(e.utf8_error()),
            },
        }
    }

    /// Decode the result of percent-decoding as UTF-8, lossily.
    ///
    /// Invalid UTF-8 percent-encoded byte sequences will be replaced � U+FFFD,
    /// the replacement character.
    #[cfg(feature = "alloc")]
    pub fn decode_utf8_lossy(self) -> Cow<'a, str> {
        decode_utf8_lossy(self.into())
    }
}

// std::ptr::addr_eq was stabilized in rust 1.76. Once we upgrade
// the MSRV we can remove this lint override.
#[cfg(feature = "alloc")]
#[allow(ambiguous_wide_pointer_comparisons)]
fn decode_utf8_lossy(input: Cow<'_, [u8]>) -> Cow<'_, str> {
    // Note: This function is duplicated in `form_urlencoded/src/query_encoding.rs`.
    match input {
        Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
        Cow::Owned(bytes) => {
            match String::from_utf8_lossy(&bytes) {
                Cow::Borrowed(utf8) => {
                    // If from_utf8_lossy returns a Cow::Borrowed, then we can
                    // be sure our original bytes were valid UTF-8. This is because
                    // if the bytes were invalid UTF-8 from_utf8_lossy would have
                    // to allocate a new owned string to back the Cow so it could
                    // replace invalid bytes with a placeholder.

                    // First we do a debug_assert to confirm our description above.
                    let raw_utf8: *const [u8] = utf8.as_bytes();
                    debug_assert!(core::ptr::eq(raw_utf8, &*bytes));

                    // Given we know the original input bytes are valid UTF-8,
                    // and we have ownership of those bytes, we re-use them and
                    // return a Cow::Owned here.
                    Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) })
                }
                Cow::Owned(s) => Cow::Owned(s),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// A small deterministic PRNG (xorshift64*), to avoid a dev-dependency.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn reference_encode(input: &[u8], set: &AsciiSet) -> String {
        let mut out = String::new();
        for &b in input {
            if b >= 0x80 || set.contains(b) {
                out.push_str(&alloc::format!("%{:02X}", b));
            } else {
                out.push(b as char);
            }
        }
        out
    }

    fn reference_decode(input: &[u8]) -> Vec<u8> {
        let hex = |b: u8| (b as char).to_digit(16);
        let mut out = Vec::new();
        let mut i = 0;
        while i < input.len() {
            if input[i] == b'%' && i + 2 < input.len() {
                if let (Some(h), Some(l)) = (hex(input[i + 1]), hex(input[i + 2])) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
            }
            out.push(input[i]);
            i += 1;
        }
        out
    }

    /// Random bytes drawn from `alphabet`, with a random density of "special"
    /// bytes so that both sparse and dense inputs are generated.
    fn random_input(rng: &mut Rng, len: usize, alphabet: &[u8], special: &[u8]) -> Vec<u8> {
        let density = rng.below(101);
        (0..len)
            .map(|_| {
                if rng.below(100) < density {
                    special[rng.below(special.len() as u64) as usize]
                } else {
                    alphabet[rng.below(alphabet.len() as u64) as usize]
                }
            })
            .collect()
    }

    #[test]
    fn differential_encode() {
        const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
        const CUSTOM: &AsciiSet = &AsciiSet::EMPTY.add(b'a').add(b'%').add(0x7F).add(0);
        const CUSTOM_COMPLEMENT: &AsciiSet = &CUSTOM.complement();
        let sets: [&'static AsciiSet; 6] = [
            &AsciiSet::EMPTY,
            CONTROLS,
            FRAGMENT,
            NON_ALPHANUMERIC,
            CUSTOM,
            CUSTOM_COMPLEMENT,
        ];
        let alphabet = b"abcxyzABCXYZ0189-._~/?=&";
        let special: Vec<u8> = (0..=255u8).collect();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for iteration in 0..if cfg!(miri) { 300 } else { 20_000 } {
            let len = match iteration % 4 {
                0 => rng.below(20) as usize,
                1 => rng.below(70) as usize,
                2 => rng.below(300) as usize,
                _ => 1000 + rng.below(100) as usize,
            };
            let input = random_input(&mut rng, len, alphabet, &special);
            let set = sets[rng.below(sets.len() as u64) as usize];
            let expected = reference_encode(&input, set);
            let encoded = percent_encode(&input, set);
            // `collect` goes through `fold`.
            assert_eq!(encoded.clone().collect::<String>(), expected, "{:?}", input);
            // External iteration through `next` (a `for` loop calls `next`,
            // never `fold`).
            let mut by_next = String::new();
            for chunk in encoded.clone() {
                by_next.push_str(chunk);
            }
            assert_eq!(by_next, expected, "{:?}", input);
            // Some `next` calls, then `fold` from the middle.
            let mut iter = encoded.clone();
            let mut mixed = String::new();
            for _ in 0..rng.below(4) {
                mixed.extend(iter.next());
            }
            mixed.extend(iter);
            assert_eq!(mixed, expected, "{:?}", input);
            assert_eq!(alloc::format!("{}", encoded), expected, "{:?}", input);
            let cow: Cow<'_, str> = encoded.into();
            assert_eq!(cow, expected, "{:?}", input);
            assert_eq!(
                matches!(cow, Cow::Borrowed(_)),
                expected.as_bytes() == &input[..]
            );
        }
    }

    #[test]
    fn differential_decode() {
        let alphabet = b"abcxyzABCXYZ0189-._~/?=&+";
        // `%` plus hex digits and near-misses, so that valid, truncated and
        // invalid escapes are all common.
        let special = b"%%%%%0123456789abcdefABCDEFgG:@/`\x00\xFF";
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        for iteration in 0..if cfg!(miri) { 300 } else { 20_000 } {
            let len = match iteration % 4 {
                0 => rng.below(20) as usize,
                1 => rng.below(70) as usize,
                2 => rng.below(300) as usize,
                _ => 1000 + rng.below(100) as usize,
            };
            let input = random_input(&mut rng, len, alphabet, special);
            let expected = reference_decode(&input);
            let decoded = super::percent_decode(&input);
            assert_eq!(
                decoded.clone().collect::<Vec<u8>>(),
                expected,
                "{:?}",
                input
            );
            let cow: Cow<'_, [u8]> = decoded.into();
            assert_eq!(&*cow, &expected[..], "{:?}", input);
            assert_eq!(matches!(cow, Cow::Borrowed(_)), expected == input);
        }
    }

    #[test]
    fn decode_edge_cases() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"%", b"%"),
            (b"%4", b"%4"),
            (b"%41", b"A"),
            (b"%%41", b"%A"),
            (b"%4%41", b"%4A"),
            (b"%G1%41", b"%G1A"),
            (b"0123456789abcd%4", b"0123456789abcd%4"),
            (b"0123456789abcde%41", b"0123456789abcdeA"),
            (b"0123456789abcdef%41", b"0123456789abcdefA"),
            (
                b"0123456789abcdefghijklmnopqrst%41",
                b"0123456789abcdefghijklmnopqrstA",
            ),
            (
                b"0123456789abcdefghijklmnopqrs%41x",
                b"0123456789abcdefghijklmnopqrsAx",
            ),
            (
                b"0123456789abcdefghijklmnopqrstuvwxyz%",
                b"0123456789abcdefghijklmnopqrstuvwxyz%",
            ),
        ];
        for &(input, expected) in cases {
            assert_eq!(&*Cow::from(super::percent_decode(input)), expected);
            assert_eq!(super::percent_decode(input).collect::<Vec<u8>>(), expected);
        }
    }

    // Boundary tests for the unsafe decoding and encoding paths.
    //
    // `decode_into` runs a 16-byte window loop while `read + 32 <= len`, and
    // a scalar tail after it. Within a window, the mask walk over-copies 16
    // bytes from `read + pos` (pos <= 15) and reads hex digits up to
    // `read + 17`; the dense path (4 or more `%` in the window) can leave
    // `read` at `start + 16`, `+ 17` or `+ 18`. The tests below put escapes
    // at exactly those offsets, and at the window-loop / tail boundary.

    /// Check every decoding entry point against `reference_decode`.
    fn check_decode(input: &[u8]) {
        let expected = reference_decode(input);
        let cow = Cow::from(super::percent_decode(input));
        assert_eq!(&*cow, &expected[..], "Cow {:?}", input);
        assert_eq!(matches!(cow, Cow::Borrowed(_)), expected == input);
        assert_eq!(
            super::percent_decode(input).collect::<Vec<u8>>(),
            expected,
            "collect {:?}",
            input
        );
        assert_eq!(
            super::percent_decode(input).decode_utf8_lossy(),
            String::from_utf8_lossy(&expected),
            "lossy {:?}",
            input
        );
        if let Some(vec) = super::percent_decode(input).if_any() {
            assert_eq!(vec, expected, "if_any {:?}", input);
            // `if_any` allocates exactly `len - 2` (one escape shrinks the
            // output by 2), and `decode_into`'s `reserve` asks for exactly
            // the remaining room, so it never reallocates. With an exact
            // allocation, any write past the end is caught by Miri.
            assert_eq!(vec.capacity(), input.len() - 2, "{:?}", input);
        }
        // `decode_into` directly, so that its first window starts at
        // `input[0]` (through `if_any` it starts after the first escape).
        let mut out = Vec::new();
        decode_into(input, &mut out);
        assert_eq!(out, expected, "decode_into {:?}", input);
        let mut out = b"xy".to_vec();
        decode_into(input, &mut out);
        assert_eq!(&out[..2], b"xy");
        assert_eq!(&out[2..], &expected[..], "decode_into {:?}", input);
    }

    /// `check_decode` on `input`, and on `input` after a leading escape so
    /// that `if_any` hands exactly `input` to `decode_into`.
    fn check_decode_both(input: &[u8]) {
        check_decode(input);
        check_decode(&[&b"%41"[..], input].concat());
    }

    /// What follows a `%` in the boundary tests: two hex digits, a hex digit
    /// then a non-hex digit, another `%`, a non-hex digit, and lowercase.
    const AFTER_PERCENT: [&[u8]; 5] = [b"41", b"4z", b"%", b"z", b"fF"];

    /// `full`, or the smaller `miri` when running under Miri (which is
    /// several thousand times slower).
    fn pick<'a, T>(full: &'a [T], miri: &'a [T]) -> &'a [T] {
        if cfg!(miri) {
            miri
        } else {
            full
        }
    }

    /// The `AFTER_PERCENT` variants to test.
    fn afters() -> &'static [&'static [u8]] {
        pick(&AFTER_PERCENT, &AFTER_PERCENT[..3])
    }

    /// `len` filler bytes with `%` at `pos` followed by `after` (truncated
    /// at `len`, so the escape can run off the end of the input).
    fn with_percent(len: usize, pos: usize, after: &[u8]) -> Vec<u8> {
        let mut v = alloc::vec![b'z'; len];
        v[pos] = b'%';
        for (dst, &src) in v[pos + 1..].iter_mut().zip(after) {
            *dst = src;
        }
        v
    }

    #[test]
    fn decode_single_percent_every_position() {
        // Every length around the window-loop threshold (32) and a few
        // windows beyond, with one `%` at every position (including the last
        // and second-to-last bytes).
        let lengths: Vec<usize> = if cfg!(miri) {
            alloc::vec![0, 2, 31, 32, 33, 48]
        } else {
            (0..=80).collect()
        };
        for &len in &lengths {
            check_decode_both(&alloc::vec![b'z'; len]);
            for pos in 0..len {
                if cfg!(miri) && pos > 2 && pos + 3 < len && pos % 16 < 12 {
                    continue;
                }
                for after in afters() {
                    check_decode_both(&with_percent(len, pos, after));
                }
            }
        }
    }

    #[test]
    fn decode_percent_at_window_end_lanes() {
        // `%` at lanes 12..=15 of the first or second window, so that its
        // digits are at or past the window end (lane 15's digits are at
        // `read + 16` and `read + 17`), with 0..=3 other escapes in the same
        // window: 1..=3 `%` take the mask walk, 4 take the dense path.
        // Lengths put the window loop's last iteration and the scalar tail
        // on either side of the escape.
        let lens = pick(
            &[31, 32, 33, 34, 35, 47, 48, 49, 50, 51, 63, 64, 65, 66],
            &[32, 34, 49],
        );
        for &len in lens {
            for &lane in &[12, 13, 14, 15] {
                for &window in pick(&[0, 16], &[0, 16][..1 + len / 48]) {
                    let target = window + lane;
                    if target >= len {
                        continue;
                    }
                    for extra in 0..=3 {
                        for after in afters() {
                            let mut v = with_percent(len, target, after);
                            if window == 16 {
                                // One escape in the first window: it ends
                                // with `pos == 16`, so the second window
                                // starts exactly at 16.
                                v[..3].copy_from_slice(b"%41");
                            }
                            for &e in [0, 3, 6][..extra].iter() {
                                v[window + e..window + e + 3].copy_from_slice(b"%5a");
                            }
                            check_decode_both(&v);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn decode_dense_threshold() {
        // Exactly 3 (mask walk) or 4 and 5 (dense loop) `%` in the first
        // window, the last one at lanes 9..=15. A valid escape at lane 13,
        // 14 or 15 leaves the dense loop at `start + 16`, `+ 17` or `+ 18`;
        // the next window then has `%` at its lanes 12..=15, or the input
        // ends there.
        let (next_lanes, extra_lens): (&[usize], &[usize]) = if cfg!(miri) {
            (&[15], &[0, 17])
        } else {
            (&[11, 12, 13, 14, 15, 16], &[0, 1, 2, 3, 16, 17])
        };
        for count in 3..=5 {
            for &last in pick(&[12, 13, 14, 15], &[13, 15]) {
                for &valid in &[true, false] {
                    for &next_lane in next_lanes {
                        for &extra_len in extra_lens {
                            let mut v = Vec::new();
                            // `count` escapes ending at lane `last`: 3 apart.
                            let first = last - 3 * (count - 1);
                            v.resize(first, b'y');
                            for i in 0..count {
                                let last_one = i + 1 == count;
                                v.extend_from_slice(if valid || !last_one {
                                    b"%c3"
                                } else {
                                    b"%g3"
                                });
                            }
                            let resume = v.len().max(16);
                            v.resize(resume + next_lane, b'y');
                            v.extend_from_slice(b"%A9");
                            v.resize(v.len() + extra_len, b'y');
                            check_decode_both(&v);
                            // The same, cut short around the length (32) at
                            // which the first window runs at all.
                            for cut in 30..=if cfg!(miri) { 31 } else { 36 } {
                                if cut < v.len() {
                                    check_decode_both(&v[..cut]);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Four `%` that are all literal (none is a valid escape) still take
        // the dense path.
        for len in 16..=if cfg!(miri) { 16 } else { 40 } {
            for start in 0..=12 {
                let mut v = alloc::vec![b'q'; len];
                for b in v[start..].iter_mut().take(4) {
                    *b = b'%';
                }
                check_decode_both(&v);
            }
        }
    }

    #[test]
    fn decode_escape_after_clean_run() {
        // A clean run longer than one window goes to `find_percent`, whose
        // vector loop handles 64 bytes at a time after the first 16. Put the
        // escape at the end of each of those blocks, then let the window
        // loop resume right at it.
        let runs: &[usize] = if cfg!(miri) {
            &[16, 63, 64, 80]
        } else {
            &[
                15, 16, 17, 31, 32, 33, 47, 48, 49, 63, 64, 65, 79, 80, 81, 95, 96, 97, 127, 128,
                129, 143, 144, 145, 191, 192, 193,
            ]
        };
        for &run in runs {
            for &suffix in pick(&(0..=40).collect::<Vec<_>>(), &[0, 1, 2, 16, 17]) {
                for after in afters() {
                    let mut v = alloc::vec![b'z'; run];
                    v.push(b'%');
                    v.extend_from_slice(after);
                    v.resize(v.len() + suffix, b'z');
                    check_decode_both(&v);
                }
            }
        }
    }

    #[test]
    fn decode_back_to_back_escapes() {
        // Runs of escapes crossing every window boundary at every phase.
        for unit in [&b"%41"[..], b"%%41", b"%4%41", b"%e2%82%ac"].iter() {
            for prefix in 0..=3 {
                for &count in pick(&(0..=24).collect::<Vec<_>>(), &[0, 1, 5, 10, 11]) {
                    for suffix in 0..=if cfg!(miri) { 1 } else { 3 } {
                        let mut v = alloc::vec![b'x'; prefix];
                        for _ in 0..count {
                            v.extend_from_slice(unit);
                        }
                        v.resize(v.len() + suffix, b'x');
                        check_decode_both(&v);
                    }
                }
            }
        }
    }

    /// Check every encoding entry point against `reference_encode`.
    fn check_encode(input: &[u8], set: &'static AsciiSet) {
        let expected = reference_encode(input, set);
        let encoded = percent_encode(input, set);
        assert_eq!(alloc::format!("{}", encoded), expected, "{:?}", input);
        assert_eq!(encoded.clone().collect::<String>(), expected, "{:?}", input);
        let mut by_next = String::new();
        for chunk in encoded.clone() {
            by_next.push_str(chunk);
        }
        assert_eq!(by_next, expected, "{:?}", input);
        let cow = Cow::from(encoded);
        assert_eq!(cow, expected, "{:?}", input);
        assert_eq!(
            matches!(cow, Cow::Borrowed(_)),
            expected.as_bytes() == input
        );
    }

    #[test]
    fn encode_block_boundaries() {
        // `encode_chunks` encodes 32-byte blocks into a 97-byte buffer with
        // 4-byte stores; the last store of a block where the first 31 bytes
        // were encoded starts at 93. 0xFF reads the last table entry plus
        // the padding byte (765..769).
        let sets: [&'static AsciiSet; 3] = [NON_ALPHANUMERIC, &AsciiSet::EMPTY, CONTROLS];
        let lengths = pick(
            &[0, 1, 15, 16, 17, 31, 32, 33, 47, 48, 63, 64, 65, 95, 96, 97],
            &[0, 1, 16, 31, 32, 33, 65],
        );
        for &set in pick(&sets, &sets[..2]) {
            for &len in lengths {
                // Everything encoded, ending in 0xFF.
                check_encode(&alloc::vec![0xFF; len], set);
                let high: Vec<u8> = (0..len).map(|i| 0xFF - (i % 128) as u8).collect();
                check_encode(&high, set);
                let low: Vec<u8> = (0..len).map(|i| (i % 128) as u8).collect();
                check_encode(&low, set);
                // Everything encoded but one byte, at each position.
                for p in 0..if cfg!(miri) && len != 32 { 0 } else { len } {
                    let mut v = alloc::vec![0xFF; len];
                    v[p] = b'a';
                    check_encode(&v, set);
                }
                // A clean prefix just below and at the direct-write
                // threshold (16), so blocks start at different offsets.
                for &clean in &[15, 16, 17] {
                    let mut v = alloc::vec![b'a'; clean];
                    v.resize(clean + len, 0xFF);
                    check_encode(&v, set);
                }
            }
            // Every combination of encoded / unencoded for the last 5 bytes
            // of a block whose first 27 bytes are encoded (the largest
            // offsets of the 4-byte stores), with 0 or 1 following bytes.
            for bits in 0..32u32 {
                for &trail in pick(&[0, 1, 33], &[0, 1]) {
                    let mut v = alloc::vec![0xFF; 27];
                    v.extend((0..5).map(|i| if bits >> i & 1 != 0 { 0xFF } else { b'b' }));
                    v.resize(v.len() + trail, b'c');
                    check_encode(&v, set);
                }
            }
            // Every byte value, in both orders.
            let all: Vec<u8> = (0..=255).collect();
            check_encode(&all, set);
            let rev: Vec<u8> = (0..=255).rev().collect();
            check_encode(&rev, set);
        }
    }

    #[test]
    fn percent_encode_byte() {
        for i in 0..=0xFF {
            let encoded = super::percent_encode_byte(i);
            assert_eq!(encoded, alloc::format!("%{:02X}", i));
        }
    }

    #[test]
    fn percent_encode_accepts_ascii_set_ref() {
        let encoded = percent_encode(b"foo bar?", &AsciiSet::EMPTY);
        assert_eq!(encoded.collect::<String>(), "foo bar?");
    }

    #[test]
    fn percent_encode_collect() {
        let encoded = percent_encode(b"foo bar?", NON_ALPHANUMERIC);
        assert_eq!(encoded.collect::<String>(), String::from("foo%20bar%3F"));

        let encoded = percent_encode(b"\x00\x01\x02\x03", CONTROLS);
        assert_eq!(encoded.collect::<String>(), String::from("%00%01%02%03"));
    }

    #[test]
    fn percent_encode_display() {
        let encoded = percent_encode(b"foo bar?", NON_ALPHANUMERIC);
        assert_eq!(alloc::format!("{}", encoded), "foo%20bar%3F");
    }

    #[test]
    fn percent_encode_cow() {
        let encoded = percent_encode(b"foo bar?", NON_ALPHANUMERIC);
        assert_eq!(Cow::from(encoded), "foo%20bar%3F");
    }

    #[test]
    fn utf8_percent_encode_accepts_ascii_set_ref() {
        let encoded = super::utf8_percent_encode("foo bar?", &AsciiSet::EMPTY);
        assert_eq!(encoded.collect::<String>(), "foo bar?");
    }

    #[test]
    fn utf8_percent_encode() {
        assert_eq!(
            super::utf8_percent_encode("foo bar?", NON_ALPHANUMERIC),
            percent_encode(b"foo bar?", NON_ALPHANUMERIC)
        );
    }

    #[test]
    fn percent_decode() {
        assert_eq!(
            super::percent_decode(b"foo%20bar%3f")
                .decode_utf8()
                .unwrap(),
            "foo bar?"
        );
    }

    #[test]
    fn percent_decode_str() {
        assert_eq!(
            super::percent_decode_str("foo%20bar%3f")
                .decode_utf8()
                .unwrap(),
            "foo bar?"
        );
    }

    #[test]
    fn percent_decode_collect() {
        let decoded = super::percent_decode(b"foo%20bar%3f");
        assert_eq!(decoded.collect::<Vec<u8>>(), b"foo bar?");
    }

    #[test]
    fn percent_decode_cow() {
        let decoded = super::percent_decode(b"foo%20bar%3f");
        assert_eq!(Cow::from(decoded), Cow::Owned::<[u8]>(b"foo bar?".to_vec()));

        let decoded = super::percent_decode(b"foo bar?");
        assert_eq!(Cow::from(decoded), Cow::Borrowed(b"foo bar?"));
    }

    #[test]
    fn percent_decode_invalid_utf8() {
        // Invalid UTF-8 sequence
        let decoded = super::percent_decode(b"%00%9F%92%96")
            .decode_utf8()
            .unwrap_err();
        assert_eq!(decoded.valid_up_to(), 1);
        assert_eq!(decoded.error_len(), Some(1));
    }

    #[test]
    fn percent_decode_utf8_lossy() {
        assert_eq!(
            super::percent_decode(b"%F0%9F%92%96").decode_utf8_lossy(),
            "💖"
        );
    }

    #[test]
    fn percent_decode_utf8_lossy_invalid_utf8() {
        assert_eq!(
            super::percent_decode(b"%00%9F%92%96").decode_utf8_lossy(),
            "\u{0}���"
        );
    }
}
