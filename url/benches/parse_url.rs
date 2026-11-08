#[macro_use]
extern crate bencher;

use bencher::{black_box, Bencher};

use url::Url;

fn short(bench: &mut Bencher) {
    let url = "https://example.com/bench";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn long(bench: &mut Bencher) {
    let url = "https://example.com/parkbench?tre=es&st=uff";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn fragment(bench: &mut Bencher) {
    let url = "https://example.com/parkbench?tre=es&st=uff#fragment";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn plain(bench: &mut Bencher) {
    let url = "https://example.com/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn port(bench: &mut Bencher) {
    let url = "https://example.com:8080";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn hyphen(bench: &mut Bencher) {
    let url = "https://hyphenated-example.com/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn leading_digit(bench: &mut Bencher) {
    let url = "https://1test.example/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn unicode_mixed(bench: &mut Bencher) {
    let url = "https://مثال.example/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn punycode_mixed(bench: &mut Bencher) {
    let url = "https://xn--mgbh0fb.example/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn unicode_ltr(bench: &mut Bencher) {
    let url = "https://නම.උදාහරණ/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn punycode_ltr(bench: &mut Bencher) {
    let url = "https://xn--r0co.xn--ozc8dl2c3bxd/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn unicode_rtl(bench: &mut Bencher) {
    let url = "https://الاسم.مثال/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn punycode_rtl(bench: &mut Bencher) {
    let url = "https://xn--mgba0b1dh.xn--mgbh0fb/";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn url_to_file_path(bench: &mut Bencher) {
    let url = if cfg!(windows) {
        "file:///C:/dir/next_dir/sub_sub_dir/testing/testing.json"
    } else {
        "file:///data/dir/next_dir/sub_sub_dir/testing/testing.json"
    };
    let url = url.parse::<Url>().unwrap();

    bench.iter(|| {
        black_box(url.to_file_path().unwrap());
    });
}

// Path-heavy inputs: these exercise path percent-encoding, which the short
// URLs above barely touch.

fn path_api(bench: &mut Bencher) {
    let url = "https://api.example.com/v1/organizations/1234/projects/abcd-ef01/deployments/latest";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn path_long(bench: &mut Bencher) {
    let url = "https://cdn.example.com/static/assets/js/vendor/node_modules/@scope/package-name/dist/esm/components/button/index.production.min.js?v=1.2.3";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn path_spaces(bench: &mut Bencher) {
    let url = "https://example.com/files/My Documents/Quarterly Report \"Final\" <draft>.pdf";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn path_unicode(bench: &mut Bencher) {
    let url = "https://ja.wikipedia.org/wiki/東京都/千代田区/丸の内";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn path_percent_encoded(bench: &mut Bencher) {
    let url =
        "https://example.com/search/caf%C3%A9/na%C3%AFve%20r%C3%A9sum%C3%A9/%E6%9D%B1%E4%BA%AC";

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(url).parse::<Url>().unwrap());
}

fn path_very_long(bench: &mut Bencher) {
    let mut url = String::from("https://example.com");
    for i in 0..64 {
        url.push_str(&format!("/segment-number-{}-with-some-text", i));
    }

    bench.bytes = url.len() as u64;
    bench.iter(|| black_box(&url).parse::<Url>().unwrap());
}

fn set_path(bench: &mut Bencher) {
    let mut url = "https://example.com/".parse::<Url>().unwrap();
    let path = "/v1/organizations/1234/projects/abcd-ef01/deployments/latest";

    bench.bytes = path.len() as u64;
    bench.iter(|| {
        url.set_path(black_box(path));
        black_box(&url);
    });
}

fn path_segments_extend(bench: &mut Bencher) {
    let mut url = "https://example.com/".parse::<Url>().unwrap();
    let segments = [
        "v1",
        "organizations",
        "1234",
        "projects",
        "my project name",
        "files",
    ];

    bench.iter(|| {
        url.set_path("");
        url.path_segments_mut()
            .unwrap()
            .extend(black_box(&segments));
        black_box(&url);
    });
}

fn query_pairs(bench: &mut Bencher) {
    let url = "https://example.com/?q=hello%20world&lang=en&utm_source=newsletter&utm_campaign=spring%20sale&ref=abc%2Fdef"
        .parse::<Url>()
        .unwrap();

    bench.iter(|| {
        for (k, v) in black_box(&url).query_pairs() {
            black_box((k, v));
        }
    });
}

benchmark_group!(
    benches,
    short,
    long,
    fragment,
    plain,
    port,
    hyphen,
    leading_digit,
    unicode_mixed,
    punycode_mixed,
    unicode_ltr,
    punycode_ltr,
    unicode_rtl,
    punycode_rtl,
    url_to_file_path,
    path_api,
    path_long,
    path_spaces,
    path_unicode,
    path_percent_encoded,
    path_very_long,
    set_path,
    path_segments_extend,
    query_pairs
);
benchmark_main!(benches);
