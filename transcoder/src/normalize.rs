use ebur128::{EbuR128, Mode};

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

        for (i, s) in samples.iter_mut().enumerate() {
            *s *= gain;
            progress(i, total_samples);
        }
    }
}
