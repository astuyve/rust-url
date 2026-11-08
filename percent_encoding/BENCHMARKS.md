# percent-encoding SIMD benchmarks

Speedups of this branch over `main` (`13b385b`), measured with the benchmarks in
`percent_encoding/benches/percent_encoding.rs` and `url/benches/parse_url.rs`.
Values above 1.00x mean this branch is faster; **bold** is 1.5x or more.

## Machines

| Column | CPU | Platform |
|---|---|---|
| Apple M | Apple M-series | macOS, aarch64 (NEON) |
| Intel SPR | Intel Xeon 8488C (Sapphire Rapids) | AWS c7i.2xlarge, x86_64 (SSE2/SSSE3) |
| AMD Zen 4 | AMD EPYC 9R14 (Zen 4) | AWS c7a.2xlarge, x86_64 (SSE2/SSSE3) |
| Graviton4 | AWS Graviton4 (Neoverse V2) | AWS c8g.2xlarge, aarch64 (NEON) |

## Method

- Both sides run the *same* benchmark source; only the library under test differs.
  `main` is the unmodified `13b385b` checkout.
- Harness: `bencher` (as used elsewhere in this workspace), `cargo bench`, release profile, stable Rust,
  default target CPU (no `-C target-cpu=native`). On x86_64 the SSSE3 encoder is selected by
  runtime detection; the SSE2 decoder is part of the x86_64 baseline.
- `percent_encoding`: two alternating `main`/branch rounds; the table shows the mean of the two
  per-round speedups.
- `url`: three interleaved `main`/branch rounds; the table shows the ratio of the medians.
- Before measuring, the full test suite (including differential tests against reference
  implementations) passed on every machine, in both debug and release mode.

## Summary

| | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---|---|---|
| `percent_encoding` benchmarks: worst case | 1.20x | 1.08x | 1.04x | 1.03x |
| `percent_encoding` benchmarks: geometric mean | 3.10x | 2.55x | 2.71x | 2.55x |
| `percent_encoding` benchmarks: best case | 53.2x | 29.8x | 42.1x | 33.2x |
| `url` benchmarks: geometric mean | 1.10x | 1.10x | 1.09x | 1.03x |
| `url` path/decoding benchmarks: geometric mean | 1.23x | 1.21x | 1.19x | 1.06x |

## `percent_encoding` benchmarks

### ada-url benchmarks (ported verbatim from `benchmarks/percent_encode.cpp`, `.to_string()`)

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `ada_fragment` | Short inputs, fragment set | 1.24x | 1.23x | 1.22x | 1.23x |
| `ada_query` | Short inputs, query set | 1.23x | 1.24x | 1.22x | 1.25x |
| `ada_userinfo` | Short inputs, userinfo set | 1.32x | 1.08x | 1.04x | 1.03x |
| `ada_c0_control` | Short inputs, C0 control set | 1.24x | 1.24x | 1.22x | 1.23x |
| `ada_long_fragment` | Long inputs, fragment set | 1.26x | 1.31x | 1.20x | 1.38x |
| `ada_long_query` | Long inputs, query set | 1.25x | 1.31x | 1.19x | 1.39x |

### Encoding via `String::extend` (how `url` uses this crate)

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `encode_short_path_noop` | `/api/v1/users/12345/profile`, nothing to encode | **1.58x** | **1.54x** | **1.56x** | **1.73x** |
| `encode_short_query` | `q=hello world&lang=en&page=2` | 1.20x | 1.21x | 1.27x | 1.31x |
| `encode_per_char_extend` | One call per `char` (pre-servo/rust-url#1026 `url` pattern) | 1.35x | 1.17x | 1.25x | 1.14x |
| `encode_ada_long_query_extend` | ada-url long inputs, query set | **1.50x** | 1.42x | 1.45x | 1.25x |
| `encode_1k_prose_path_extend` | 1 KB English prose, path set (space every ~6 bytes) | **1.74x** | **1.58x** | **1.69x** | **1.54x** |
| `encode_1k_unicode_extend` | 1 KB Cyrillic/Japanese text (almost every byte encoded) | **4.34x** | **2.40x** | **4.08x** | 1.36x |
| `encode_64k_prose_extend_noop` | 64 KB prose, nothing to encode | **9.75x** | **8.98x** | **10.84x** | **11.89x** |
| `encode_64k_prose_sparse_extend` | 64 KB prose, an escape every ~400 bytes | **5.14x** | **5.60x** | **5.46x** | **6.09x** |
| `encode_64k_prose_query_extend` | 64 KB prose, query set (space every ~6 bytes) | **1.74x** | **1.54x** | **1.71x** | **1.59x** |

### Encoding via `Display` (`.to_string()`) and `Cow`

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `encode_1k_prose_cow_noop` | 1 KB prose to `Cow`, nothing to encode (borrowed) | **11.87x** | **10.97x** | **9.95x** | **11.34x** |
| `encode_64k_prose_cow_noop` | 64 KB prose to `Cow`, nothing to encode (borrowed) | **13.17x** | **11.89x** | **16.23x** | **12.63x** |
| `encode_1k_prose_fragment` | 1 KB prose, fragment set | **1.94x** | **1.57x** | **1.77x** | 1.42x |
| `encode_1k_prose_non_alnum` | 1 KB prose, `NON_ALPHANUMERIC` | **2.05x** | **1.73x** | **1.95x** | **1.61x** |
| `encode_64k_prose_sparse` | 64 KB prose, an escape every ~400 bytes | **3.38x** | **3.38x** | **3.16x** | **3.71x** |
| `encode_64k_prose_query` | 64 KB prose, query set | **2.05x** | **1.58x** | **1.71x** | **1.58x** |
| `encode_1k_unicode` | 1 KB Cyrillic/Japanese text | **3.83x** | **2.86x** | **3.73x** | **2.42x** |
| `encode_bytes_non_ascii` | 1 KB arbitrary bytes, `CONTROLS` | **3.05x** | **2.15x** | **2.48x** | **1.81x** |

### Decoding

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `decode_short_path_noop` | `decode_utf8` of a short path, nothing to decode | **2.45x** | **1.52x** | 1.26x | **1.96x** |
| `decode_short_query` | `decode_utf8` of `q=hello%20world&lang=en&page=2` | **2.27x** | **2.04x** | **1.95x** | **2.30x** |
| `decode_ada_long_query` | `decode_utf8` of an encoded ada-url long input | **3.22x** | **3.59x** | **2.71x** | **3.49x** |
| `decode_1k_noop` | 1 KB prose to `Cow`, nothing to decode (borrowed) | **43.00x** | **27.02x** | **34.44x** | **27.21x** |
| `decode_64k_noop` | 64 KB prose to `Cow`, nothing to decode (borrowed) | **53.19x** | **29.84x** | **42.11x** | **33.17x** |
| `decode_64k_sparse` | 64 KB to `Cow`, an escape every ~400 bytes | **13.05x** | **12.63x** | **13.65x** | **11.58x** |
| `decode_64k_spaces` | 64 KB `decode_utf8`, `%20` every ~6 bytes | **1.94x** | **1.64x** | **1.67x** | 1.30x |
| `decode_1k_unicode` | 1 KB encoded Cyrillic/Japanese, `decode_utf8` | **2.92x** | 1.33x | 1.45x | 1.47x |
| `decode_1k_unicode_cow` | 1 KB encoded Cyrillic/Japanese, `Cow<[u8]>` | **4.21x** | **1.52x** | **1.63x** | **1.76x** |
| `decode_1k_bytes_extend` | `Vec::extend(percent_decode(..))` (as `url` does) | **3.19x** | **2.32x** | **2.53x** | **2.39x** |
| `decode_1k_spaces_collect` | `percent_decode(..).collect::<Vec<u8>>()` | **2.37x** | **2.08x** | **2.20x** | **2.06x** |

## `url` benchmarks (`url/benches/parse_url.rs`)

The first 13 rows are the pre-existing parse benchmarks, whose short URLs do little percent
encoding; the remainder were added to exercise path encoding and decoding.

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `short` | `https://example.com/bench` | 1.01x | 1.05x | 1.03x | 1.01x |
| `long` | Short path + query | 1.05x | 1.07x | 1.01x | 1.05x |
| `fragment` | Short path + query + fragment | 1.04x | 1.06x | 1.05x | 1.02x |
| `plain` | `https://example.com/` | 0.96x | 0.99x | 0.97x | 0.97x |
| `port` | Host with port | 1.03x | 1.03x | 0.98x | 0.97x |
| `hyphen` | Hyphenated host | 1.02x | 1.04x | 1.09x | 1.02x |
| `leading_digit` | Host with leading digit | 0.99x | 1.04x | 1.00x | 1.06x |
| `unicode_mixed` | IDNA host (mixed) | 1.02x | 1.02x | 1.00x | 0.97x |
| `punycode_mixed` | Punycode host (mixed) | 1.00x | 1.00x | 1.02x | 1.02x |
| `unicode_ltr` | IDNA host (LTR) | 1.02x | 1.01x | 1.04x | 0.98x |
| `punycode_ltr` | Punycode host (LTR) | 1.02x | 1.04x | 1.04x | 1.04x |
| `unicode_rtl` | IDNA host (RTL) | 1.01x | 1.00x | 1.01x | 0.99x |
| `punycode_rtl` | Punycode host (RTL) | 1.02x | 1.02x | 1.01x | 1.03x |
| `path_api` | Typical REST API path (7 segments) | 1.08x | 1.12x | 1.05x | 0.98x |
| `path_long` | Long CDN asset path + query | 1.11x | 1.14x | 1.04x | 0.98x |
| `path_spaces` | Path with spaces, quotes, `<>` | 1.16x | 1.13x | 1.14x | 1.04x |
| `path_unicode` | Non-ASCII (Japanese) path | **1.53x** | 1.39x | 1.40x | 1.14x |
| `path_percent_encoded` | Already percent-encoded path | 1.08x | 1.08x | 1.06x | 1.01x |
| `path_very_long` | 64-segment path | 1.20x | **1.52x** | 1.29x | 1.10x |
| `set_path` | `Url::set_path` | 1.13x | 1.18x | 1.16x | 0.97x |
| `path_segments_extend` | `path_segments_mut().extend(..)` | 1.31x | 1.19x | 1.18x | 1.04x |
| `query_pairs` | Iterate `query_pairs()` (decoding) | 1.35x | 1.01x | 1.13x | 1.11x |
| `url_to_file_path` | `Url::to_file_path` (decoding) | 1.42x | 1.44x | **1.56x** | 1.21x |

### Results below 1.00x

Every `percent_encoding` benchmark is faster on every machine. At the `url` level, these
results are slower than `main`:

- `plain` on Apple M: 0.96x (137 → 142 ns)
- `plain` on Intel SPR: 0.99x (141 → 143 ns)
- `plain` on AMD Zen 4: 0.97x (146 → 150 ns)
- `plain` on Graviton4: 0.97x (169 → 175 ns)
- `port` on AMD Zen 4: 0.98x (185 → 189 ns)
- `port` on Graviton4: 0.97x (216 → 223 ns)
- `leading_digit` on Apple M: 0.99x (198 → 201 ns)
- `unicode_mixed` on Graviton4: 0.97x (435 → 447 ns)
- `punycode_mixed` on Intel SPR: 1.00x (430 → 432 ns)
- `unicode_ltr` on Graviton4: 0.98x (501 → 509 ns)
- `unicode_rtl` on Graviton4: 0.99x (519 → 525 ns)
- `path_api` on Graviton4: 0.98x (346 → 352 ns)
- `path_long` on Graviton4: 0.98x (498 → 506 ns)
- `set_path` on Graviton4: 0.97x (169 → 174 ns)

`plain` (`https://example.com/`) is 1–4% slower on all four machines, which points to a small
fixed per-call cost for one-byte inputs (the path `/`) rather than noise; it is a few
nanoseconds per parse and has not been investigated yet. The remaining items are within
about 3% and are not consistent across machines (for example `path_api` and `set_path` are
1.05–1.18x faster on the other three machines).

## Absolute timings (ns/iter, `main` → branch)

<details><summary><code>percent_encoding</code> benchmarks</summary>

| Benchmark | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---:|---:|---:|---:|
| `ada_fragment` | 164 → 132 | 145 → 117 | 146 → 121 | 155 → 126 |
| `ada_query` | 163 → 132 | 146 → 117 | 147 → 121 | 163 → 127 |
| `ada_userinfo` | 384 → 290 | 288 → 267 | 297 → 284 | 283 → 272 |
| `ada_c0_control` | 165 → 133 | 145 → 117 | 147 → 121 | 156 → 126 |
| `ada_long_fragment` | 883 → 699 | 849 → 643 | 821 → 690 | 854 → 618 |
| `ada_long_query` | 881 → 701 | 844 → 646 | 822 → 691 | 856 → 617 |
| `encode_short_path_noop` | 19 → 12 | 22 → 14 | 21 → 13 | 22 → 13 |
| `encode_short_query` | 30 → 25 | 32 → 26 | 31 → 24 | 29 → 23 |
| `encode_per_char_extend` | 410 → 302 | 451 → 381 | 466 → 379 | 367 → 367 |
| `encode_ada_long_query_extend` | 574 → 381 | 578 → 405 | 617 → 426 | 489 → 391 |
| `encode_1k_prose_path_extend` | 2,568 → 1,471 | 2,248 → 1,441 | 2,597 → 1,644 | 1,966 → 1,253 |
| `encode_1k_unicode_extend` | 5,539 → 1,275 | 4,123 → 1,726 | 5,916 → 1,485 | 3,172 → 2,327 |
| `encode_64k_prose_extend_noop` | 32,744 → 3,355 | 39,802 → 4,386 | 40,672 → 3,751 | 48,588 → 4,081 |
| `encode_64k_prose_sparse_extend` | 37,997 → 7,389 | 42,814 → 7,631 | 41,466 → 7,537 | 50,715 → 8,313 |
| `encode_64k_prose_query_extend` | 148,093 → 85,339 | 129,812 → 84,597 | 161,280 → 96,016 | 119,285 → 72,185 |
| `encode_1k_prose_cow_noop` | 558 → 47 | 664 → 61 | 522 → 52 | 669 → 59 |
| `encode_64k_prose_cow_noop` | 31,647 → 2,406 | 38,308 → 3,202 | 41,457 → 2,554 | 38,526 → 3,060 |
| `encode_1k_prose_fragment` | 2,914 → 1,506 | 2,610 → 1,656 | 3,015 → 1,744 | 2,172 → 1,529 |
| `encode_1k_prose_non_alnum` | 3,124 → 1,527 | 2,758 → 1,600 | 3,278 → 1,723 | 2,300 → 1,409 |
| `encode_64k_prose_sparse` | 38,566 → 11,350 | 44,346 → 13,111 | 44,463 → 13,956 | 44,630 → 12,044 |
| `encode_64k_prose_query` | 150,388 → 72,054 | 136,966 → 86,165 | 158,665 → 94,821 | 116,517 → 71,809 |
| `encode_1k_unicode` | 5,620 → 1,473 | 4,511 → 1,573 | 6,475 → 1,766 | 3,751 → 1,547 |
| `encode_bytes_non_ascii` | 4,252 → 1,412 | 3,350 → 1,562 | 4,222 → 1,723 | 2,676 → 1,480 |
| `decode_short_path_noop` | 27 → 11 | 30 → 20 | 27 → 22 | 26 → 13 |
| `decode_short_query` | 91 → 40 | 83 → 41 | 82 → 42 | 94 → 41 |
| `decode_ada_long_query` | 300 → 93 | 327 → 91 | 281 → 103 | 389 → 110 |
| `decode_1k_noop` | 731 → 17 | 505 → 19 | 620 → 18 | 612 → 23 |
| `decode_64k_noop` | 42,180 → 793 | 29,925 → 1,012 | 35,794 → 850 | 35,329 → 1,054 |
| `decode_64k_sparse` | 45,770 → 3,505 | 44,619 → 3,566 | 55,467 → 4,055 | 51,680 → 4,464 |
| `decode_64k_spaces` | 76,537 → 39,453 | 61,935 → 37,260 | 66,064 → 39,590 | 64,466 → 49,694 |
| `decode_1k_unicode` | 5,069 → 1,738 | 2,820 → 2,146 | 2,857 → 1,968 | 2,699 → 1,836 |
| `decode_1k_unicode_cow` | 4,355 → 1,034 | 2,196 → 1,458 | 2,318 → 1,425 | 1,973 → 1,119 |
| `decode_1k_bytes_extend` | 3,023 → 938 | 2,244 → 981 | 2,479 → 979 | 2,534 → 1,057 |
| `decode_1k_spaces_collect` | 3,195 → 1,336 | 2,865 → 1,403 | 2,872 → 1,312 | 2,788 → 1,350 |

</details>

<details><summary><code>url</code> benchmarks</summary>

| Benchmark | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---:|---:|---:|---:|
| `short` | 157 → 156 | 167 → 159 | 157 → 153 | 179 → 178 |
| `long` | 195 → 186 | 220 → 206 | 202 → 200 | 232 → 221 |
| `fragment` | 211 → 202 | 248 → 233 | 231 → 221 | 252 → 247 |
| `plain` | 137 → 142 | 141 → 143 | 146 → 150 | 169 → 175 |
| `port` | 193 → 188 | 199 → 194 | 185 → 189 | 216 → 223 |
| `hyphen` | 211 → 207 | 209 → 201 | 212 → 195 | 250 → 244 |
| `leading_digit` | 198 → 201 | 237 → 227 | 211 → 211 | 260 → 245 |
| `unicode_mixed` | 396 → 389 | 507 → 497 | 468 → 466 | 435 → 447 |
| `punycode_mixed` | 323 → 323 | 430 → 432 | 386 → 379 | 444 → 435 |
| `unicode_ltr` | 452 → 442 | 598 → 591 | 577 → 553 | 501 → 509 |
| `punycode_ltr` | 454 → 446 | 600 → 578 | 558 → 538 | 593 → 571 |
| `unicode_rtl` | 460 → 454 | 604 → 604 | 556 → 552 | 519 → 525 |
| `punycode_rtl` | 440 → 431 | 587 → 575 | 534 → 529 | 592 → 576 |
| `path_api` | 334 → 310 | 377 → 336 | 329 → 312 | 346 → 352 |
| `path_long` | 513 → 462 | 570 → 502 | 493 → 473 | 498 → 506 |
| `path_spaces` | 364 → 315 | 397 → 351 | 382 → 335 | 371 → 358 |
| `path_unicode` | 416 → 272 | 424 → 304 | 406 → 289 | 383 → 336 |
| `path_percent_encoded` | 300 → 279 | 340 → 314 | 305 → 288 | 321 → 317 |
| `path_very_long` | 4,671 → 3,904 | 5,172 → 3,401 | 4,978 → 3,846 | 4,828 → 4,390 |
| `set_path` | 189 → 167 | 213 → 181 | 189 → 163 | 169 → 174 |
| `path_segments_extend` | 234 → 178 | 228 → 192 | 215 → 182 | 191 → 184 |
| `query_pairs` | 375 → 278 | 330 → 326 | 371 → 328 | 318 → 287 |
| `url_to_file_path` | 190 → 134 | 199 → 138 | 178 → 114 | 166 → 137 |

</details>

## Binary size

Release build with LTO of a small program that calls every public API, `__TEXT` segment:
aarch64 248,918 → 251,874 bytes (+2.9 KB); x86_64 268,378 → 272,006 bytes (+3.5 KB).
`url`'s `parse_url` benchmark executable: `__TEXT` +761 bytes. Static tables: the existing
768-byte `%XX` table (now 769 bytes, padded), a new 256-byte hex-digit table and 16 bytes of
SIMD constants.
