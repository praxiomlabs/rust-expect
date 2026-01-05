//! Pattern matching benchmarks.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rust_expect::expect::{Pattern, PatternSet, RingBuffer};

fn bench_literal_pattern(c: &mut Criterion) {
    let pattern = Pattern::literal("needle");
    let haystack = "This is a long string that contains the word needle somewhere in the middle";

    c.bench_function("literal_pattern_match", |b| {
        b.iter(|| pattern.matches(black_box(haystack)));
    });
}

fn bench_regex_pattern(c: &mut Criterion) {
    let pattern = Pattern::regex(r"\d{3}-\d{4}").unwrap();
    let haystack = "Call us at 555-1234 for more information";

    c.bench_function("regex_pattern_match", |b| {
        b.iter(|| pattern.matches(black_box(haystack)));
    });
}

fn bench_pattern_set(c: &mut Criterion) {
    let mut set = PatternSet::new();
    set.add(Pattern::literal("$"));
    set.add(Pattern::literal("#"));
    set.add(Pattern::literal(">"));
    set.add(Pattern::literal("%"));

    let haystack = "user@host:~$ ";

    c.bench_function("pattern_set_match", |b| {
        b.iter(|| set.find_match(black_box(haystack)));
    });
}

fn bench_pattern_set_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_set_size");

    for size in &[2, 5, 10, 20] {
        let mut set = PatternSet::new();
        for i in 0..*size {
            set.add(Pattern::literal(format!("pattern{i}")));
        }
        // Add the matching pattern at the end
        set.add(Pattern::literal("$"));

        let haystack = "user@host:~$ ";

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| set.find_match(black_box(haystack)));
        });
    }

    group.finish();
}

fn bench_ring_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer");

    // Benchmark append operations
    group.bench_function("append_1k", |b| {
        b.iter(|| {
            let mut buffer = RingBuffer::new(1024);
            for i in 0..1000u32 {
                buffer.append(&[(i % 256) as u8]);
            }
            black_box(buffer)
        });
    });

    // Benchmark search operations
    let mut buffer = RingBuffer::new(4096);
    for i in 0..4000u32 {
        buffer.append(&[(i % 256) as u8]);
    }
    buffer.append(b"needle");

    group.bench_function("search", |b| {
        b.iter(|| buffer.find(black_box(b"needle")));
    });

    group.finish();
}

fn bench_complex_regex(c: &mut Criterion) {
    // Benchmark a more complex regex pattern
    let pattern = Pattern::regex(r"ERROR:\s+\[([A-Z]+)\]\s+(.+)$").unwrap();
    let haystack = "2024-01-01 12:00:00 ERROR: [AUTH] Failed to authenticate user john_doe";

    c.bench_function("complex_regex_match", |b| {
        b.iter(|| pattern.matches(black_box(haystack)));
    });
}

criterion_group!(
    benches,
    bench_literal_pattern,
    bench_regex_pattern,
    bench_pattern_set,
    bench_pattern_set_sizes,
    bench_ring_buffer,
    bench_complex_regex,
);
criterion_main!(benches);
