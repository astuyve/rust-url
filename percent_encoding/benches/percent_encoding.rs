//! Benchmarks for percent encoding and decoding.
//!
//! The short/long encode inputs are ported verbatim from ada-url's
//! `benchmarks/percent_encode.cpp`. The throughput inputs follow the style of
//! memchr's benchmarks: a long haystack with few / many needles.

#[macro_use]
extern crate bencher;

use bencher::{black_box, Bencher};
use percent_encoding::{
    percent_decode, percent_decode_str, percent_encode, utf8_percent_encode, AsciiSet, CONTROLS,
    NON_ALPHANUMERIC,
};
use std::borrow::Cow;

// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
// https://url.spec.whatwg.org/#query-percent-encode-set
const QUERY: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');
// https://url.spec.whatwg.org/#path-percent-encode-set
const PATH: &AsciiSet = &QUERY.add(b'?').add(b'^').add(b'`').add(b'{').add(b'}');
// https://url.spec.whatwg.org/#userinfo-percent-encode-set
const USERINFO: &AsciiSet = &PATH
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'|');

// ada-url benchmarks/percent_encode.cpp `examples[]`
const ADA_EXAMPLES: &[&str] = &[
    "\u{E1}|",
    "other:9818274x1!!",
    "ref=web-twc-ao-gbl-adsinfo&utm_source=twc&utm_",
    "connect_timeout=10&application_name=myapp",
];

// ada-url benchmarks/percent_encode.cpp `long_examples[]`
const ADA_LONG_EXAMPLES: &[&str] = &[
    "connect timeout=10 application name=myapp server=db host internal \
     database=production analytics read preference=secondary preferred \
     ssl=true retry writes=true w=majority max pool size=50",
    "ref=web twc ao gbl adsinfo utm source=twc utm medium=cpc \
     utm campaign=brand awareness q4 2024 utm content=banner 300x250 \
     utm term=weather forecast today gclid=Cj0KCQiA3Y ABhCnARIsAK",
];

const PROSE: &str = "Mr. Sherlock Holmes, who was usually very late in the mornings, save \
upon those not infrequent occasions when he was up all night, was seated at the breakfast \
table. I stood upon the hearth-rug and picked up the stick which our visitor had left behind \
him the night before. It was a fine, thick piece of wood, bulbous-headed, of the sort which \
is known as a \"Penang lawyer.\" ";

const UNICODE: &str = "Шерлок Холмс сидел за столом. シャーロック・ホームズは朝食をとっていた。 ";

fn repeat_to(s: &str, len: usize) -> String {
    let mut out = String::with_capacity(len + s.len());
    while out.len() < len {
        out.push_str(s);
    }
    out
}

fn bench_set(bench: &mut Bencher, inputs: &[&str], set: &'static AsciiSet) {
    bench.bytes = inputs.iter().map(|s| s.len() as u64).sum();
    bench.iter(|| {
        for s in inputs {
            black_box(utf8_percent_encode(black_box(s), set).to_string());
        }
    });
}

// ---- ada-url ported encode benchmarks ----
fn ada_fragment(b: &mut Bencher) {
    bench_set(b, ADA_EXAMPLES, FRAGMENT)
}
fn ada_query(b: &mut Bencher) {
    bench_set(b, ADA_EXAMPLES, QUERY)
}
fn ada_userinfo(b: &mut Bencher) {
    bench_set(b, ADA_EXAMPLES, USERINFO)
}
fn ada_c0_control(b: &mut Bencher) {
    bench_set(b, ADA_EXAMPLES, CONTROLS)
}
fn ada_long_fragment(b: &mut Bencher) {
    bench_set(b, ADA_LONG_EXAMPLES, FRAGMENT)
}
fn ada_long_query(b: &mut Bencher) {
    bench_set(b, ADA_LONG_EXAMPLES, QUERY)
}

// ---- encode throughput ----
fn encode_bench(b: &mut Bencher, input: &str, set: &'static AsciiSet) {
    b.bytes = input.len() as u64;
    b.iter(|| black_box(utf8_percent_encode(black_box(input), set).to_string()));
}
fn encode_cow_bench(b: &mut Bencher, input: &str, set: &'static AsciiSet) {
    b.bytes = input.len() as u64;
    b.iter(|| {
        let c: Cow<'_, str> = utf8_percent_encode(black_box(input), set).into();
        black_box(c);
    });
}
fn encode_extend_bench(b: &mut Bencher, input: &str, set: &'static AsciiSet) {
    // The way `url` uses this crate: `String::extend(utf8_percent_encode(..))`.
    b.bytes = input.len() as u64;
    let mut out = String::with_capacity(input.len() * 3);
    b.iter(|| {
        out.clear();
        out.extend(utf8_percent_encode(black_box(input), set));
        black_box(&out);
    });
}

fn encode_short_path_noop(b: &mut Bencher) {
    encode_extend_bench(b, "/api/v1/users/12345/profile", PATH)
}
fn encode_short_query(b: &mut Bencher) {
    encode_extend_bench(b, "q=hello world&lang=en&page=2", QUERY)
}
fn encode_per_char_extend(b: &mut Bencher) {
    // One call per character, as `url` did before servo/rust-url#1026, and as
    // other callers encoding char-by-char still do.
    let input = "/v1/organizations/1234/projects/my project/files/r\u{e9}sum\u{e9}.pdf";
    b.bytes = input.len() as u64;
    let mut out = String::with_capacity(input.len() * 3);
    let mut buf = [0u8; 4];
    b.iter(|| {
        out.clear();
        for c in black_box(input).chars() {
            out.extend(utf8_percent_encode(c.encode_utf8(&mut buf), PATH));
        }
        black_box(&out);
    });
}
fn encode_1k_prose_fragment(b: &mut Bencher) {
    encode_bench(b, &repeat_to(PROSE, 1024), FRAGMENT)
}
fn encode_1k_prose_cow_noop(b: &mut Bencher) {
    // Nothing to encode in CONTROLS: measures the pure scan + borrow.
    encode_cow_bench(b, &repeat_to(PROSE, 1024), CONTROLS)
}
fn encode_64k_prose_cow_noop(b: &mut Bencher) {
    encode_cow_bench(b, &repeat_to(PROSE, 64 * 1024), CONTROLS)
}
fn encode_64k_prose_extend_noop(b: &mut Bencher) {
    encode_extend_bench(b, &repeat_to(PROSE, 64 * 1024), CONTROLS)
}
fn encode_64k_prose_sparse(b: &mut Bencher) {
    // A '"' roughly every 400 bytes.
    let s = repeat_to(PROSE, 64 * 1024).replace(' ', "_");
    encode_bench(b, &s, FRAGMENT)
}
fn encode_64k_prose_query(b: &mut Bencher) {
    // A space every ~6 bytes.
    encode_bench(b, &repeat_to(PROSE, 64 * 1024), QUERY)
}
fn encode_64k_prose_sparse_extend(b: &mut Bencher) {
    let s = repeat_to(PROSE, 64 * 1024).replace(' ', "_");
    encode_extend_bench(b, &s, FRAGMENT)
}
fn encode_64k_prose_query_extend(b: &mut Bencher) {
    encode_extend_bench(b, &repeat_to(PROSE, 64 * 1024), QUERY)
}
fn encode_1k_prose_path_extend(b: &mut Bencher) {
    encode_extend_bench(b, &repeat_to(PROSE, 1024), PATH)
}
fn encode_1k_unicode_extend(b: &mut Bencher) {
    encode_extend_bench(b, &repeat_to(UNICODE, 1024), FRAGMENT)
}
fn encode_ada_long_query_extend(b: &mut Bencher) {
    b.bytes = ADA_LONG_EXAMPLES.iter().map(|s| s.len() as u64).sum();
    let mut out = String::with_capacity(1024);
    b.iter(|| {
        for s in ADA_LONG_EXAMPLES {
            out.clear();
            out.extend(utf8_percent_encode(black_box(s), QUERY));
            black_box(&out);
        }
    });
}
fn encode_1k_prose_non_alnum(b: &mut Bencher) {
    encode_bench(b, &repeat_to(PROSE, 1024), NON_ALPHANUMERIC)
}
fn encode_1k_unicode(b: &mut Bencher) {
    // Nearly every byte is encoded.
    encode_bench(b, &repeat_to(UNICODE, 1024), FRAGMENT)
}

// ---- decode ----
fn decode_cow_bench(b: &mut Bencher, input: &str) {
    b.bytes = input.len() as u64;
    b.iter(|| {
        let c: Cow<'_, [u8]> = percent_decode(black_box(input.as_bytes())).into();
        black_box(c);
    });
}
fn decode_utf8_bench(b: &mut Bencher, input: &str) {
    b.bytes = input.len() as u64;
    b.iter(|| black_box(percent_decode_str(black_box(input)).decode_utf8().unwrap()));
}
fn decode_collect_bench(b: &mut Bencher, input: &str) {
    b.bytes = input.len() as u64;
    b.iter(|| black_box(percent_decode_str(black_box(input)).collect::<Vec<u8>>()));
}

fn decode_short_path_noop(b: &mut Bencher) {
    decode_utf8_bench(b, "/api/v1/users/12345/profile")
}
fn decode_short_query(b: &mut Bencher) {
    decode_utf8_bench(b, "q=hello%20world&lang=en&page=2")
}
fn decode_ada_long_query(b: &mut Bencher) {
    let s = utf8_percent_encode(ADA_LONG_EXAMPLES[0], QUERY).to_string();
    decode_utf8_bench(b, &s)
}
fn decode_1k_noop(b: &mut Bencher) {
    decode_cow_bench(b, &repeat_to(PROSE, 1024))
}
fn decode_64k_noop(b: &mut Bencher) {
    decode_cow_bench(b, &repeat_to(PROSE, 64 * 1024))
}
fn decode_64k_sparse(b: &mut Bencher) {
    let s = utf8_percent_encode(&repeat_to(PROSE, 64 * 1024), FRAGMENT)
        .to_string()
        .replace("%20", "_");
    decode_cow_bench(b, &s)
}
fn decode_64k_spaces(b: &mut Bencher) {
    let s = utf8_percent_encode(&repeat_to(PROSE, 64 * 1024), QUERY).to_string();
    decode_utf8_bench(b, &s)
}
fn decode_1k_unicode(b: &mut Bencher) {
    let s = utf8_percent_encode(&repeat_to(UNICODE, 1024), FRAGMENT).to_string();
    decode_utf8_bench(b, &s)
}
fn decode_1k_unicode_cow(b: &mut Bencher) {
    let s = utf8_percent_encode(&repeat_to(UNICODE, 1024), FRAGMENT).to_string();
    decode_cow_bench(b, &s)
}
fn decode_1k_spaces_collect(b: &mut Bencher) {
    let s = utf8_percent_encode(&repeat_to(PROSE, 1024), QUERY).to_string();
    decode_collect_bench(b, &s)
}
fn decode_1k_bytes_extend(b: &mut Bencher) {
    // The way `url` uses this crate in `path_to_file_url`-style code.
    let s = utf8_percent_encode(&repeat_to(PROSE, 1024), QUERY).to_string();
    b.bytes = s.len() as u64;
    let mut out = Vec::with_capacity(s.len());
    b.iter(|| {
        out.clear();
        out.extend(percent_decode(black_box(s.as_bytes())));
        black_box(&out);
    });
}

fn encode_bytes_non_ascii(b: &mut Bencher) {
    let data: Vec<u8> = (0..1024u32).map(|i| (i * 7) as u8).collect();
    b.bytes = data.len() as u64;
    b.iter(|| black_box(percent_encode(black_box(&data), CONTROLS).to_string()));
}

benchmark_group!(
    ada,
    ada_fragment,
    ada_query,
    ada_userinfo,
    ada_c0_control,
    ada_long_fragment,
    ada_long_query
);
benchmark_group!(
    encode,
    encode_short_path_noop,
    encode_short_query,
    encode_per_char_extend,
    encode_1k_prose_fragment,
    encode_1k_prose_cow_noop,
    encode_64k_prose_cow_noop,
    encode_64k_prose_extend_noop,
    encode_64k_prose_sparse,
    encode_64k_prose_query,
    encode_64k_prose_sparse_extend,
    encode_64k_prose_query_extend,
    encode_1k_prose_path_extend,
    encode_1k_unicode_extend,
    encode_ada_long_query_extend,
    encode_1k_prose_non_alnum,
    encode_1k_unicode,
    encode_bytes_non_ascii
);
benchmark_group!(
    decode,
    decode_short_path_noop,
    decode_short_query,
    decode_ada_long_query,
    decode_1k_noop,
    decode_64k_noop,
    decode_64k_sparse,
    decode_64k_spaces,
    decode_1k_unicode,
    decode_1k_unicode_cow,
    decode_1k_spaces_collect,
    decode_1k_bytes_extend
);
benchmark_main!(ada, encode, decode);
