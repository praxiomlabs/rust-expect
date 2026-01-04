//! Comparative benchmarks between rust-expect and expectrl.
//!
//! This benchmark suite compares the performance of rust-expect against
//! expectrl for common terminal automation operations.
//!
//! Run with: `cargo bench --bench comparative`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ============================================================================
// Pattern Matching Benchmarks
// ============================================================================

/// Benchmark literal pattern matching performance.
fn bench_literal_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("literal_pattern");

    let haystacks = [
        ("short", "$ "),
        ("medium", "user@host:~/projects/rust-expect$ "),
        (
            "long",
            "2024-01-01 12:00:00 [INFO] user@host:~/projects/rust-expect$ command output here",
        ),
    ];

    for (name, haystack) in haystacks {
        // rust-expect
        group.bench_with_input(BenchmarkId::new("rust_expect", name), haystack, |b, h| {
            let pattern = rust_expect::Pattern::literal("$");
            b.iter(|| pattern.matches(black_box(h)));
        });

        // expectrl (uses Contains which is similar)
        group.bench_with_input(BenchmarkId::new("expectrl", name), haystack, |b, h| {
            use expectrl::Needle;
            let needle: &str = "$";
            b.iter(|| needle.check(black_box(h.as_bytes()), false));
        });
    }

    group.finish();
}

/// Benchmark regex pattern matching performance.
fn bench_regex_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_pattern");

    let patterns_and_texts = [
        ("simple", r"\$\s*$", "user@host:~$ "),
        ("digits", r"\d{3}-\d{4}", "Call 555-1234"),
        (
            "complex",
            r"ERROR:\s+\[([A-Z]+)\]\s+(.+)$",
            "ERROR: [AUTH] Failed login",
        ),
    ];

    for (name, pattern, text) in patterns_and_texts {
        // rust-expect
        group.bench_with_input(BenchmarkId::new("rust_expect", name), &text, |b, t| {
            let p = rust_expect::Pattern::regex(pattern).unwrap();
            b.iter(|| p.matches(black_box(*t)));
        });

        // expectrl (Regex is a tuple struct)
        group.bench_with_input(BenchmarkId::new("expectrl", name), &text, |b, t| {
            use expectrl::Needle;
            let re = expectrl::Regex(pattern);
            b.iter(|| re.check(black_box(t.as_bytes()), false));
        });
    }

    group.finish();
}

// ============================================================================
// Buffer Operations Benchmarks
// ============================================================================

/// Benchmark buffer append and search operations.
fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_ops");

    // Benchmark appending data to a buffer
    let sizes = [1024, 4096, 16384];

    for size in sizes {
        // rust-expect RingBuffer append
        group.bench_with_input(
            BenchmarkId::new("rust_expect_append", size),
            &size,
            |b, &sz| {
                let data: Vec<u8> = (0..64).map(|i| (i % 256) as u8).collect();
                b.iter(|| {
                    let mut buffer = rust_expect::expect::RingBuffer::new(sz);
                    for _ in 0..(sz / 64) {
                        buffer.append(black_box(&data));
                    }
                    buffer
                });
            },
        );

        // Standard Vec (baseline comparison)
        group.bench_with_input(BenchmarkId::new("vec_baseline", size), &size, |b, &sz| {
            let data: Vec<u8> = (0..64).map(|i| (i % 256) as u8).collect();
            b.iter(|| {
                let mut buffer = Vec::with_capacity(sz);
                for _ in 0..(sz / 64) {
                    buffer.extend_from_slice(black_box(&data));
                }
                buffer
            });
        });
    }

    // Benchmark searching in buffer
    group.bench_function("rust_expect_search", |b| {
        let mut buffer = rust_expect::expect::RingBuffer::new(4096);
        let data: Vec<u8> = (0..3000).map(|i| (i % 256) as u8).collect();
        buffer.append(&data);
        buffer.append(b"needle_here");
        b.iter(|| buffer.find(black_box(b"needle_here")));
    });

    group.finish();
}

// ============================================================================
// Pattern Set Benchmarks
// ============================================================================

/// Benchmark matching against multiple patterns.
fn bench_multi_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_pattern");

    let pattern_counts = [2, 5, 10, 20];
    let haystack = "user@host:~/project$ command output here";

    for count in pattern_counts {
        // rust-expect PatternSet
        group.bench_with_input(
            BenchmarkId::new("rust_expect_set", count),
            &count,
            |b, &n| {
                let mut set = rust_expect::expect::PatternSet::new();
                for i in 0..n {
                    set.add(rust_expect::Pattern::literal(format!("nomatch{i}")));
                }
                set.add(rust_expect::Pattern::literal("$")); // Last pattern matches
                b.iter(|| set.find_match(black_box(haystack)));
            },
        );

        // expectrl multiple checks (manual iteration)
        group.bench_with_input(BenchmarkId::new("expectrl_iter", count), &count, |b, &n| {
            use expectrl::Needle;
            let patterns: Vec<String> = (0..n).map(|i| format!("nomatch{i}")).collect();
            let patterns_ref: Vec<&str> = patterns.iter().map(std::string::String::as_str).collect();
            b.iter(|| {
                for p in &patterns_ref {
                    if let Ok(matches) = p.check(black_box(haystack.as_bytes()), false) {
                        if !matches.is_empty() {
                            return Some(*p);
                        }
                    }
                }
                // Check the matching pattern
                if let Ok(matches) = "$".check(black_box(haystack.as_bytes()), false) {
                    if !matches.is_empty() {
                        return Some("$");
                    }
                }
                None
            });
        });
    }

    group.finish();
}

// ============================================================================
// Memory Usage Comparison (measured via allocations)
// ============================================================================

/// Benchmark allocation patterns for typical operations.
fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocations");

    // Benchmark creating and using patterns
    group.bench_function("rust_expect_pattern_create", |b| {
        b.iter(|| {
            let p1 = rust_expect::Pattern::literal("$");
            let p2 = rust_expect::Pattern::regex(r"\w+@\w+").unwrap();
            black_box((p1, p2))
        });
    });

    group.bench_function("expectrl_regex_create", |b| {
        b.iter(|| {
            let re = expectrl::Regex(r"\w+@\w+");
            black_box(re)
        });
    });

    group.finish();
}

// ============================================================================
// Summary and Notes
// ============================================================================
//
// Expected results:
// - rust-expect should show comparable or better performance for:
//   - Literal string matching (optimized Contains implementation)
//   - Pattern set matching (early-exit optimization)
//   - Ring buffer operations (specialized data structure)
//
// - Areas where expectrl may be faster:
//   - Simple regex operations (depends on underlying regex crate usage)
//
// - Key differentiators:
//   - rust-expect has async-first design (not benchmarked here)
//   - rust-expect has streaming capabilities
//   - rust-expect has PII redaction built-in
//
// ============================================================================

criterion_group!(
    benches,
    bench_literal_matching,
    bench_regex_matching,
    bench_buffer_operations,
    bench_multi_pattern,
    bench_allocation_patterns,
);
criterion_main!(benches);
