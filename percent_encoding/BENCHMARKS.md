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
| `percent_encoding` benchmarks: worst case | 1.00x (`decode_tiny_noop`, 2 → 2 ns) | 0.67x (`decode_tiny_noop`, 2 → 3 ns) | 0.94x (`ada_userinfo`, 289 → 311 ns) | 1.00x (`decode_tiny_noop`, 3 → 3 ns) |
| `percent_encoding` benchmarks: geometric mean | 2.97x | 2.43x | 2.48x | 2.40x |
| `percent_encoding` benchmarks: best case | 53.2x | 35.0x | 42.1x | 34.0x |
| `url` benchmarks: geometric mean | 1.10x | 1.08x | 1.07x | 1.05x |
| `url` path/decoding benchmarks: geometric mean | 1.22x | 1.17x | 1.18x | 1.09x |

## `percent_encoding` benchmarks

### ada-url benchmarks (ported verbatim from `benchmarks/percent_encode.cpp`, `.to_string()`)

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `ada_fragment` | Short inputs, fragment set | 1.23x | 1.25x | 1.21x | 1.26x |
| `ada_query` | Short inputs, query set | 1.22x | 1.23x | 1.21x | 1.25x |
| `ada_userinfo` | Short inputs, userinfo set | 1.32x | 1.07x | 0.94x | 1.08x |
| `ada_c0_control` | Short inputs, C0 control set | 1.23x | 1.25x | 1.21x | 1.27x |
| `ada_long_fragment` | Long inputs, fragment set | 1.30x | 1.39x | 1.27x | 1.44x |
| `ada_long_query` | Long inputs, query set | 1.29x | 1.39x | 1.27x | 1.42x |

### Encoding via `String::extend` (how `url` uses this crate)

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `encode_short_path_noop` | `/api/v1/users/12345/profile`, nothing to encode | **1.50x** | **1.53x** | 1.46x | **1.54x** |
| `encode_short_query` | `q=hello world&lang=en&page=2` | 1.26x | 1.16x | 1.29x | 1.13x |
| `encode_per_char_extend` | One call per `char` (pre-servo/rust-url#1026 `url` pattern) | 1.43x | 1.13x | 1.27x | 1.14x |
| `encode_ada_long_query_extend` | ada-url long inputs, query set | **1.51x** | 1.44x | 1.46x | 1.20x |
| `encode_1k_prose_path_extend` | 1 KB English prose, path set (space every ~6 bytes) | **1.76x** | 1.46x | **1.58x** | 1.39x |
| `encode_1k_unicode_extend` | 1 KB Cyrillic/Japanese text (almost every byte encoded) | **4.34x** | **2.11x** | **3.79x** | 1.36x |
| `encode_64k_prose_extend_noop` | 64 KB prose, nothing to encode | **9.39x** | **9.17x** | **10.91x** | **9.90x** |
| `encode_64k_prose_sparse_extend` | 64 KB prose, an escape every ~400 bytes | **5.07x** | **5.62x** | **5.50x** | **5.14x** |
| `encode_64k_prose_query_extend` | 64 KB prose, query set (space every ~6 bytes) | **1.74x** | 1.50x | **1.70x** | 1.40x |

### Encoding via `Display` (`.to_string()`) and `Cow`

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `encode_1k_prose_cow_noop` | 1 KB prose to `Cow`, nothing to encode (borrowed) | **12.07x** | **11.09x** | **9.80x** | **11.46x** |
| `encode_64k_prose_cow_noop` | 64 KB prose to `Cow`, nothing to encode (borrowed) | **13.15x** | **11.91x** | **16.23x** | **12.79x** |
| `encode_1k_prose_fragment` | 1 KB prose, fragment set | **2.00x** | **1.64x** | **1.92x** | **1.72x** |
| `encode_1k_prose_non_alnum` | 1 KB prose, `NON_ALPHANUMERIC` | **2.15x** | **1.84x** | **2.12x** | **1.90x** |
| `encode_64k_prose_sparse` | 64 KB prose, an escape every ~400 bytes | **3.38x** | **3.50x** | **3.31x** | **4.41x** |
| `encode_64k_prose_query` | 64 KB prose, query set | **2.11x** | **1.70x** | **1.88x** | **1.81x** |
| `encode_1k_unicode` | 1 KB Cyrillic/Japanese text | **4.33x** | **3.08x** | **4.44x** | **2.86x** |
| `encode_bytes_non_ascii` | 1 KB arbitrary bytes, `CONTROLS` | **3.20x** | **2.36x** | **2.77x** | **2.12x** |

### Decoding

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `decode_tiny_noop` | `a` to `Cow`, nothing to decode | 1.00x | 0.67x | 1.00x | 1.00x |
| `decode_10_noop` | `newsletter` to `Cow`, nothing to decode (SWAR path) | **3.33x** | **3.00x** | **2.00x** | **1.50x** |
| `decode_host_noop` | `example.com` to `Cow`, as `url` does for every host (SWAR path) | **3.00x** | **3.33x** | **2.33x** | **1.50x** |
| `decode_9_escape_late` | `abc%2Fdef` to `Cow` | **1.72x** | 1.36x | 1.07x | **1.70x** |
| `decode_13_escape` | `hello%20world` to `Cow` | **3.16x** | **1.72x** | 1.31x | **2.35x** |
| `decode_query_pairs_mix` | The 10 keys/values of `url`'s `query_pairs` benchmark, to `Cow` | **2.12x** | 1.08x | 1.22x | **1.71x** |
| `decode_short_path_noop` | `decode_utf8` of a short path, nothing to decode | **2.60x** | **1.52x** | 1.21x | **2.00x** |
| `decode_short_query` | `decode_utf8` of `q=hello%20world&lang=en&page=2` | **2.28x** | **2.05x** | **1.92x** | **2.72x** |
| `decode_ada_long_query` | `decode_utf8` of an encoded ada-url long input | **3.19x** | **3.61x** | **2.66x** | **3.38x** |
| `decode_1k_noop` | 1 KB prose to `Cow`, nothing to decode (borrowed) | **41.76x** | **35.05x** | **34.44x** | **29.37x** |
| `decode_64k_noop` | 64 KB prose to `Cow`, nothing to decode (borrowed) | **53.18x** | **32.47x** | **42.12x** | **34.00x** |
| `decode_64k_sparse` | 64 KB to `Cow`, an escape every ~400 bytes | **13.00x** | **14.23x** | **13.72x** | **11.55x** |
| `decode_64k_spaces` | 64 KB `decode_utf8`, `%20` every ~6 bytes | **1.93x** | **1.75x** | **1.67x** | 1.29x |
| `decode_1k_unicode` | 1 KB encoded Cyrillic/Japanese, `decode_utf8` | **2.91x** | 1.33x | 1.46x | 1.46x |
| `decode_1k_unicode_cow` | 1 KB encoded Cyrillic/Japanese, `Cow<[u8]>` | **4.18x** | 1.49x | **1.63x** | **1.77x** |
| `decode_1k_bytes_extend` | `Vec::extend(percent_decode(..))` (as `url` does) | **3.13x** | **2.50x** | **2.54x** | **2.45x** |
| `decode_1k_spaces_collect` | `percent_decode(..).collect::<Vec<u8>>()` | **2.34x** | **2.08x** | **2.20x** | **1.89x** |

## `url` benchmarks (`url/benches/parse_url.rs`)

The first 13 rows are the pre-existing parse benchmarks, whose short URLs do little percent
encoding; the remainder were added to exercise path encoding and decoding.

| Benchmark | Input | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---|---:|---:|---:|---:|
| `short` | `https://example.com/bench` | 1.00x | 1.02x | 0.95x | 1.02x |
| `long` | Short path + query | 1.08x | 1.02x | 0.97x | 1.01x |
| `fragment` | Short path + query + fragment | 1.07x | 1.03x | 1.04x | 1.03x |
| `plain` | `https://example.com/` | 0.99x | 0.99x | 0.99x | 1.01x |
| `port` | Host with port | 1.04x | 1.00x | 0.93x | 1.03x |
| `hyphen` | Hyphenated host | 1.02x | 1.00x | 1.00x | 1.04x |
| `leading_digit` | Host with leading digit | 0.99x | 1.03x | 0.97x | 1.04x |
| `unicode_mixed` | IDNA host (mixed) | 1.02x | 1.01x | 1.00x | 1.00x |
| `punycode_mixed` | Punycode host (mixed) | 1.00x | 1.02x | 1.00x | 1.03x |
| `unicode_ltr` | IDNA host (LTR) | 1.01x | 1.02x | 1.04x | 1.00x |
| `punycode_ltr` | Punycode host (LTR) | 1.02x | 1.03x | 1.02x | 1.04x |
| `unicode_rtl` | IDNA host (RTL) | 1.02x | 1.01x | 0.99x | 1.00x |
| `punycode_rtl` | Punycode host (RTL) | 1.02x | 1.02x | 1.01x | 1.03x |
| `path_api` | Typical REST API path (7 segments) | 1.10x | 1.08x | 1.06x | 0.99x |
| `path_long` | Long CDN asset path + query | 1.12x | 1.07x | 1.04x | 0.99x |
| `path_spaces` | Path with spaces, quotes, `<>` | 1.15x | 1.11x | 1.15x | 1.05x |
| `path_unicode` | Non-ASCII (Japanese) path | **1.52x** | 1.37x | 1.39x | 1.17x |
| `path_percent_encoded` | Already percent-encoded path | 1.09x | 1.06x | 1.07x | 1.03x |
| `path_very_long` | 64-segment path | 1.19x | 1.21x | 1.08x | 1.15x |
| `set_path` | `Url::set_path` | 1.12x | 1.08x | 1.11x | 1.04x |
| `path_segments_extend` | `path_segments_mut().extend(..)` | 1.34x | 1.07x | 1.16x | 1.03x |
| `query_pairs` | Iterate `query_pairs()` (decoding) | 1.23x | 1.11x | 1.22x | 1.20x |
| `url_to_file_path` | `Url::to_file_path` (decoding) | 1.40x | **1.70x** | **1.63x** | 1.28x |

### Results below 1.00x

`percent_encoding` benchmarks below 1.00x: `ada_userinfo` on AMD Zen 4 (0.94x), `decode_tiny_noop` on Intel SPR (0.67x).
`url` benchmarks below 1.00x: `short` on AMD Zen 4 (0.95x), `long` on AMD Zen 4 (0.97x), `plain` on Apple M (0.99x), `plain` on Intel SPR (0.99x), `plain` on AMD Zen 4 (0.99x), `port` on Intel SPR (1.00x), `port` on AMD Zen 4 (0.93x), `leading_digit` on Apple M (0.99x), `leading_digit` on AMD Zen 4 (0.97x), `unicode_ltr` on Graviton4 (1.00x), `unicode_rtl` on AMD Zen 4 (0.99x), `path_api` on Graviton4 (0.99x), `path_long` on Graviton4 (0.99x).

These were investigated with hardware performance counters (`perf stat` on the AWS machines,
instructions and cycles via `/usr/bin/time -l` on the Mac), using small standalone programs
that repeat a single operation:

- **`url` parses.** For every URL measured (`plain`, `port`, `short`, `long`, `leading_digit`,
  `unicode_mixed`, `unicode_ltr`, `path_api`, `path_long`, `set_path`, `path_unicode`), this
  branch retires *fewer* instructions than `main` (2–16% fewer) on Intel, AMD and Graviton,
  and branch mispredictions are effectively zero for both. Cycle counts are within a few
  percent either way and move with code placement rather than with the code itself: on AMD,
  `port` is 0.93x in the benchmark binary above but 1.11x faster (750 → 676 cycles) in a
  standalone program; on Intel, `plain` is 0.99x above but 0.94x standalone. Where cycles are
  higher on Graviton, the difference is in front-end (instruction fetch) stalls, not in work done.
- **`ada_userinfo` on AMD** (0.94x above) is 1.18x faster standalone (1,229 → 1,040 cycles),
  with fewer instructions; its code is unchanged since a previous run that measured 1.02–1.05x.
- **`decode_tiny_noop`** (a one-byte input) really is about one cycle slower (≈20 more
  instructions of length dispatch before the scalar loop); the 2 → 3 ns shown on Intel is that
  cycle rounded by the 1 ns timer resolution.

An earlier version of this branch inlined `PercentEncode::fold` into callers. That made `url`'s
path parser slower on `https://example.com/` (+20–30 cycles per parse with fewer instructions)
even though the inlined code never ran for that URL; `fold` is now `#[inline(never)]`, which
keeps its `String::extend` speedups.

## Absolute timings (ns/iter, `main` → branch)

<details><summary><code>percent_encoding</code> benchmarks</summary>

| Benchmark | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---:|---:|---:|---:|
| `ada_fragment` | 160 → 131 | 155 → 125 | 147 → 122 | 161 → 127 |
| `ada_query` | 160 → 132 | 153 → 126 | 147 → 123 | 160 → 127 |
| `ada_userinfo` | 369 → 284 | 312 → 295 | 289 → 311 | 285 → 265 |
| `ada_c0_control` | 160 → 132 | 155 → 125 | 147 → 123 | 161 → 127 |
| `ada_long_fragment` | 892 → 688 | 913 → 667 | 843 → 667 | 900 → 626 |
| `ada_long_query` | 888 → 688 | 910 → 667 | 845 → 666 | 891 → 626 |
| `encode_short_path_noop` | 18 → 12 | 23 → 15 | 21 → 14 | 20 → 13 |
| `encode_short_query` | 29 → 23 | 33 → 29 | 31 → 24 | 26 → 23 |
| `encode_per_char_extend` | 393 → 275 | 474 → 420 | 481 → 380 | 434 → 348 |
| `encode_ada_long_query_extend` | 557 → 369 | 625 → 438 | 618 → 420 | 469 → 388 |
| `encode_1k_prose_path_extend` | 2,472 → 1,421 | 2,437 → 1,667 | 2,579 → 1,630 | 1,738 → 1,260 |
| `encode_1k_unicode_extend` | 5,386 → 1,239 | 4,463 → 2,160 | 5,288 → 1,475 | 3,164 → 2,327 |
| `encode_64k_prose_extend_noop` | 33,174 → 3,399 | 43,125 → 4,746 | 40,735 → 3,712 | 40,063 → 4,093 |
| `encode_64k_prose_sparse_extend` | 36,889 → 7,157 | 46,528 → 8,366 | 41,543 → 7,538 | 43,099 → 8,350 |
| `encode_64k_prose_query_extend` | 143,969 → 82,813 | 140,869 → 95,967 | 159,835 → 94,384 | 100,770 → 72,118 |
| `encode_1k_prose_cow_noop` | 541 → 45 | 721 → 65 | 501 → 52 | 668 → 58 |
| `encode_64k_prose_cow_noop` | 30,692 → 2,336 | 41,409 → 3,504 | 41,474 → 2,553 | 38,580 → 3,048 |
| `encode_1k_prose_fragment` | 2,962 → 1,490 | 2,787 → 1,744 | 3,394 → 1,650 | 2,435 → 1,414 |
| `encode_1k_prose_non_alnum` | 3,198 → 1,509 | 2,978 → 1,659 | 3,681 → 1,619 | 2,548 → 1,332 |
| `encode_64k_prose_sparse` | 37,618 → 11,150 | 47,791 → 13,835 | 43,901 → 13,276 | 52,661 → 11,940 |
| `encode_64k_prose_query` | 153,470 → 73,010 | 148,163 → 87,679 | 158,661 → 89,036 | 127,055 → 69,563 |
| `encode_1k_unicode` | 5,787 → 1,340 | 4,869 → 1,593 | 7,334 → 1,522 | 3,722 → 1,277 |
| `encode_bytes_non_ascii` | 4,336 → 1,361 | 3,622 → 1,573 | 4,167 → 1,515 | 2,736 → 1,293 |
| `decode_tiny_noop` | 2 → 2 | 2 → 3 | 3 → 3 | 3 → 3 |
| `decode_10_noop` | 8 → 2 | 9 → 3 | 6 → 3 | 6 → 4 |
| `decode_host_noop` | 9 → 3 | 10 → 3 | 7 → 3 | 6 → 4 |
| `decode_9_escape_late` | 37 → 22 | 34 → 25 | 29 → 27 | 34 → 20 |
| `decode_13_escape` | 69 → 22 | 43 → 25 | 34 → 26 | 47 → 20 |
| `decode_query_pairs_mix` | 189 → 89 | 176 → 160 | 264 → 217 | 156 → 92 |
| `decode_short_path_noop` | 26 → 10 | 32 → 21 | 26 → 21 | 24 → 12 |
| `decode_short_query` | 89 → 39 | 89 → 44 | 83 → 43 | 100 → 38 |
| `decode_ada_long_query` | 290 → 92 | 349 → 98 | 281 → 104 | 385 → 110 |
| `decode_1k_noop` | 710 → 17 | 718 → 21 | 620 → 18 | 680 → 22 |
| `decode_64k_noop` | 40,959 → 771 | 35,698 → 1,088 | 35,829 → 850 | 36,620 → 1,059 |
| `decode_64k_sparse` | 44,203 → 3,415 | 52,519 → 3,882 | 55,419 → 4,006 | 51,742 → 4,482 |
| `decode_64k_spaces` | 74,318 → 38,520 | 68,045 → 40,246 | 65,793 → 39,611 | 64,477 → 49,983 |
| `decode_1k_unicode` | 4,924 → 1,687 | 3,053 → 2,327 | 2,853 → 1,960 | 2,696 → 1,849 |
| `decode_1k_unicode_cow` | 4,228 → 1,012 | 2,348 → 1,591 | 2,319 → 1,425 | 1,977 → 1,119 |
| `decode_1k_bytes_extend` | 2,846 → 913 | 2,454 → 983 | 2,490 → 980 | 2,505 → 1,025 |
| `decode_1k_spaces_collect` | 3,064 → 1,299 | 3,109 → 1,505 | 2,875 → 1,307 | 2,730 → 1,445 |

</details>

<details><summary><code>url</code> benchmarks</summary>

| Benchmark | Apple M | Intel SPR | AMD Zen 4 | Graviton4 |
|---|---:|---:|---:|---:|
| `short` | 153 → 153 | 176 → 173 | 157 → 165 | 180 → 176 |
| `long` | 190 → 176 | 234 → 229 | 200 → 207 | 224 → 221 |
| `fragment` | 205 → 192 | 264 → 256 | 232 → 224 | 250 → 243 |
| `plain` | 133 → 135 | 150 → 152 | 146 → 148 | 169 → 167 |
| `port` | 187 → 180 | 211 → 212 | 180 → 194 | 217 → 211 |
| `hyphen` | 205 → 201 | 225 → 224 | 212 → 211 | 251 → 242 |
| `leading_digit` | 192 → 193 | 252 → 245 | 212 → 219 | 252 → 243 |
| `unicode_mixed` | 385 → 378 | 542 → 536 | 471 → 471 | 435 → 433 |
| `punycode_mixed` | 315 → 314 | 459 → 449 | 392 → 392 | 444 → 430 |
| `unicode_ltr` | 439 → 434 | 639 → 628 | 575 → 554 | 506 → 507 |
| `punycode_ltr` | 441 → 434 | 638 → 622 | 566 → 553 | 590 → 569 |
| `unicode_rtl` | 447 → 440 | 646 → 639 | 555 → 559 | 512 → 511 |
| `punycode_rtl` | 427 → 419 | 626 → 615 | 545 → 539 | 594 → 574 |
| `path_api` | 324 → 294 | 401 → 372 | 330 → 312 | 347 → 350 |
| `path_long` | 498 → 446 | 606 → 569 | 494 → 476 | 498 → 505 |
| `path_spaces` | 352 → 306 | 424 → 381 | 373 → 325 | 371 → 352 |
| `path_unicode` | 403 → 266 | 450 → 328 | 407 → 292 | 382 → 326 |
| `path_percent_encoded` | 291 → 267 | 363 → 342 | 305 → 284 | 332 → 323 |
| `path_very_long` | 4,537 → 3,812 | 5,501 → 4,559 | 4,742 → 4,389 | 4,826 → 4,189 |
| `set_path` | 182 → 163 | 226 → 210 | 187 → 169 | 169 → 163 |
| `path_segments_extend` | 229 → 171 | 241 → 225 | 215 → 186 | 191 → 185 |
| `query_pairs` | 365 → 296 | 355 → 319 | 378 → 309 | 317 → 265 |
| `url_to_file_path` | 184 → 131 | 217 → 128 | 179 → 110 | 158 → 123 |

</details>

## Binary size

Release build with LTO of a small program that calls every public API, `__TEXT` segment:
aarch64 248,918 → 252,138 bytes (+3.2 KB); x86_64 268,378 → 272,106 bytes (+3.6 KB).
`url`'s `parse_url` benchmark executable: `__TEXT` +1,081 bytes. Static tables: the existing
768-byte `%XX` table (now 769 bytes, padded), a new 256-byte hex-digit table and 16 bytes of
SIMD constants.
