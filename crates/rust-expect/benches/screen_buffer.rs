//! Screen buffer benchmarks.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

#[cfg(feature = "screen")]
mod screen_benches {
    use super::*;
    use rust_expect::screen::{ScreenBuffer, AnsiParser, ScreenQueryExt};

    /// Helper to write a string to the buffer character by character.
    fn write_str(buffer: &mut ScreenBuffer, s: &str) {
        for c in s.chars() {
            buffer.write_char(c);
        }
    }

    pub fn bench_screen_buffer_write(c: &mut Criterion) {
        let mut group = c.benchmark_group("screen_buffer_write");

        for size in [(80, 24), (120, 40), (200, 50)].iter() {
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("{}x{}", size.0, size.1)),
                size,
                |b, &(cols, rows)| {
                    b.iter(|| {
                        let mut buffer = ScreenBuffer::new(rows, cols);
                        for _ in 0..rows {
                            for _ in 0..cols {
                                buffer.write_char('X');
                            }
                        }
                        black_box(buffer)
                    })
                },
            );
        }

        group.finish();
    }

    pub fn bench_screen_buffer_scroll(c: &mut Criterion) {
        let mut buffer = ScreenBuffer::new(24, 80);
        // Fill the buffer
        for _ in 0..24 * 80 {
            buffer.write_char('A');
        }

        c.bench_function("screen_buffer_scroll", |b| {
            b.iter(|| {
                let mut buf = buffer.clone();
                for _ in 0..10 {
                    buf.scroll_up(1);
                }
                black_box(buf)
            })
        });
    }

    pub fn bench_screen_query(c: &mut Criterion) {
        let mut buffer = ScreenBuffer::new(24, 80);
        buffer.goto(0, 0);
        write_str(&mut buffer, "Hello, World! This is a test.");
        buffer.goto(5, 0);
        write_str(&mut buffer, "needle");
        buffer.goto(23, 0);
        write_str(&mut buffer, "Last line with some text");

        c.bench_function("screen_query_find", |b| {
            b.iter(|| buffer.query().find(black_box("needle")))
        });

        c.bench_function("screen_query_contains", |b| {
            b.iter(|| buffer.query().contains(black_box("Last line")))
        });

        c.bench_function("screen_query_text", |b| {
            b.iter(|| buffer.query().text())
        });
    }

    pub fn bench_ansi_parser(c: &mut Criterion) {
        let mut group = c.benchmark_group("ansi_parser");

        // Simple text
        let simple_text = "Hello, World!";
        group.bench_function("simple_text", |b| {
            b.iter(|| {
                let mut parser = AnsiParser::new();
                for byte in simple_text.bytes() {
                    black_box(parser.parse(byte));
                }
            })
        });

        // Text with colors
        let colored_text = "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m \x1b[34mBlue\x1b[0m";
        group.bench_function("colored_text", |b| {
            b.iter(|| {
                let mut parser = AnsiParser::new();
                for byte in colored_text.bytes() {
                    black_box(parser.parse(byte));
                }
            })
        });

        // Cursor movement
        let cursor_seq = "\x1b[10;20H\x1b[5A\x1b[3B\x1b[2C\x1b[4D";
        group.bench_function("cursor_movement", |b| {
            b.iter(|| {
                let mut parser = AnsiParser::new();
                for byte in cursor_seq.bytes() {
                    black_box(parser.parse(byte));
                }
            })
        });

        group.finish();
    }

    pub fn register_benches(c: &mut Criterion) {
        bench_screen_buffer_write(c);
        bench_screen_buffer_scroll(c);
        bench_screen_query(c);
        bench_ansi_parser(c);
    }
}

#[cfg(feature = "screen")]
criterion_group!(benches, screen_benches::register_benches);

#[cfg(not(feature = "screen"))]
fn dummy_bench(_c: &mut Criterion) {
    // No-op when screen feature is disabled
}

#[cfg(not(feature = "screen"))]
criterion_group!(benches, dummy_bench);

criterion_main!(benches);
