use std::hint::black_box;

use pesto::yenc::encode_scalar;

fn main() {
    // Warm-up + single-pass timing via `cargo bench` (criterion-free).
    // Use `cargo bench --bench yenc` to run; pipe through `grep ns/iter` for
    // a quick comparison table once SIMD paths are added.

    let sizes: &[(usize, &str)] = &[
        (512, "512B"),
        (4 * 1024, "4KB"),
        (128 * 1024, "128KB"),
        (750 * 1024, "750KB (1 article)"),
    ];

    for &(n, label) in sizes {
        let data: Vec<u8> = (0u8..).cycle().take(n).collect();
        let mut out = Vec::with_capacity(n + n / 16 + 128);

        let iters = (2_000_000 / n.max(1)).max(10);

        let t0 = std::time::Instant::now();
        for _ in 0..iters {
            out.clear();
            encode_scalar(black_box(&mut out), black_box(&data), 128);
        }
        let elapsed = t0.elapsed();

        let ns_per_iter = elapsed.as_nanos() / iters as u128;
        let throughput_mb = (n as f64 / (ns_per_iter as f64 / 1e9)) / 1_048_576.0;
        println!("encode_scalar  {label:>20}  {ns_per_iter:>8} ns/iter  {throughput_mb:>8.1} MB/s");
    }
}
