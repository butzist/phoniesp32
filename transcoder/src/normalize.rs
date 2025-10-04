use ebur128::{EbuR128, Mode};

const CHUNK_SIZE: usize = 2048;

pub(crate) fn loudness_normalize(
    samples: &mut [f32],
    sample_rate: u32,
    channels: usize,
    target_lufs: f64,
    mut progress: impl FnMut(usize, usize),
) {
    let mut state = EbuR128::new(channels as u32, sample_rate, Mode::I).unwrap();
    state.add_frames_f32(samples).unwrap();

    if let Ok(loudness) = state.loudness_global() {
        let diff = target_lufs - loudness;
        let gain = 10f32.powf((diff as f32) / 20.0);

        let total_samples = samples.len();
        let mut chunk_start = 0;

        while chunk_start + CHUNK_SIZE <= total_samples {
            progress(chunk_start, total_samples);
            for s in &mut samples[chunk_start..chunk_start + CHUNK_SIZE] {
                *s *= gain;
            }
            chunk_start += CHUNK_SIZE;
        }

        for s in &mut samples[chunk_start..] {
            *s *= gain;
        }
        progress(total_samples, total_samples);
    }
}
