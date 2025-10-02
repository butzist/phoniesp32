// based on https://github.com/HEnquist/rubato/blob/master/examples/process_f64.rs
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    calculate_cutoff,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResampleError {
    #[error("resampler construction failed")]
    Construction(#[from] rubato::ResamplerConstructionError),
    #[error("resampling failed")]
    Resample(#[from] rubato::ResampleError),
}

pub(crate) fn resample(
    input: &[f32],
    resample_ratio: f64,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<f32>, ResampleError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: calculate_cutoff(256, WindowFunction::BlackmanHarris2),
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f32>::new(resample_ratio, 1.1, params, 1024, 1)?;

    // Prepare
    let total_frames = input.len();
    let mut current_frames = 0;
    let mut outdata: Vec<f32> = Vec::new();
    let mut indata = [input];
    let mut input_frames_next = resampler.input_frames_next();
    let resampler_delay = resampler.output_delay();
    let mut outbuffer = vec![vec![0.0f32; resampler.output_frames_max()]; 1];

    // Process all full chunks
    while indata[0].len() >= input_frames_next {
        let (nbr_in, nbr_out) = resampler.process_into_buffer(&indata, &mut outbuffer, None)?;
        for chan in indata.iter_mut() {
            *chan = &chan[nbr_in..];
        }
        current_frames += nbr_in;
        outdata.extend(outbuffer[0][..nbr_out].iter());
        input_frames_next = resampler.input_frames_next();

        progress(current_frames, total_frames);
    }

    // Process a partial chunk with the last frames.
    if !indata[0].is_empty() {
        let (_nbr_in, nbr_out) =
            resampler.process_partial_into_buffer(Some(&indata), &mut outbuffer, None)?;
        outdata.extend(outbuffer[0][..nbr_out].iter());
    }
    let nbr_output_frames = (input.len() as f64 * resample_ratio) as usize;
    let end = (nbr_output_frames + resampler_delay).min(outdata.len() - 1);

    let resampled = &outdata[resampler_delay..end];
    Ok(resampled.to_vec())
}
