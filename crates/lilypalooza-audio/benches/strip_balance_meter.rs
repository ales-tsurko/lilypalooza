//! Manual strip balance/meter processing benchmark.

use std::{hint::black_box, time::Instant};

use lilypalooza_audio::mixer::{
    BalanceMeterBenchmarkBlock,
    BalanceMeterBenchmarkPath,
    benchmark_process_stereo_balance_meter,
};

const FRAMES: usize = 64;
const ITERS: usize = 200_000;

fn main() {
    if cfg!(debug_assertions) {
        return;
    }

    let left_in: Vec<f32> = (0..FRAMES)
        .map(|frame| ((frame as f32 * 0.17).sin() * 0.5) - 0.2)
        .collect();
    let right_in: Vec<f32> = (0..FRAMES)
        .map(|frame| ((frame as f32 * 0.11).cos() * 0.45) + 0.15)
        .collect();
    let mut left_out = vec![0.0; FRAMES];
    let mut right_out = vec![0.0; FRAMES];

    let scalar_started = Instant::now();
    for _ in 0..ITERS {
        black_box(benchmark_process_stereo_balance_meter(
            BalanceMeterBenchmarkPath::Scalar,
            benchmark_block(&left_in, &right_in, &mut left_out, &mut right_out),
        ));
    }
    let scalar_elapsed = scalar_started.elapsed();

    let simd_started = Instant::now();
    for _ in 0..ITERS {
        black_box(benchmark_process_stereo_balance_meter(
            BalanceMeterBenchmarkPath::Simd,
            benchmark_block(&left_in, &right_in, &mut left_out, &mut right_out),
        ));
    }
    let simd_elapsed = simd_started.elapsed();

    println!("strip perf over {ITERS} iters: scalar={scalar_elapsed:?} simd={simd_elapsed:?}");
}

fn benchmark_block<'a>(
    left_in: &'a [f32],
    right_in: &'a [f32],
    left_out: &'a mut [f32],
    right_out: &'a mut [f32],
) -> BalanceMeterBenchmarkBlock<'a> {
    BalanceMeterBenchmarkBlock {
        left_in: black_box(left_in),
        right_in: black_box(right_in),
        left_out: black_box(left_out),
        right_out: black_box(right_out),
        left_mul: black_box(0.82),
        right_mul: black_box(0.67),
        frames: black_box(FRAMES),
    }
}
