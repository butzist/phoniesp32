// based on https://github.com/HEnquist/rubato/blob/master/examples/process_f64.rs
use audioadapter_buffers::direct::InterleavedSlice;
use log::debug;
use rubato::{Fft, FixedSync, Resampler};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResampleError {
    #[error("resampler construction failed")]
    Construction(#[from] rubato::ResamplerConstructionError),
    #[error("resampling failed")]
    Resample(#[from] rubato::ResampleError),
    #[error("invalid audio buffer size")]
    SizeError(#[from] audioadapter_buffers::SizeError),
}

pub(crate) fn resample(
    input: &[f32],
    sample_rate: usize,
    target_rate: usize,
    progress: impl FnMut(usize, usize),
) -> Result<Vec<f32>, ResampleError> {
    let mut resampler = Fft::<f32>::new(target_rate, sample_rate, 1024, 2, 1, FixedSync::Both)?;

    let input_adapter = InterleavedSlice::new(input, 1, input.len())?;
    let outbuffer_len = resampler.process_all_needed_output_len(input.len());
    let mut outbuffer = vec![0.0f32; resampler.process_all_needed_output_len(input.len())];
    let mut output_adapter = InterleavedSlice::new_mut(&mut outbuffer, 1, outbuffer_len)?;

    let (_, outsize) = process_all_into_buffer(
        &mut resampler,
        &input_adapter,
        &mut output_adapter,
        input.len(),
        None,
        progress,
    )?;

    let resampled = &outbuffer[..outsize];
    Ok(resampled.to_vec())
}

// adapted from Resampler::process_all_into_buffer
fn process_all_into_buffer<'a, T: rubato::Sample, R: rubato::Resampler<T>>(
    resampler: &mut R,
    buffer_in: &dyn audioadapter::Adapter<'a, T>,
    buffer_out: &mut dyn audioadapter::AdapterMut<'a, T>,
    input_len: usize,
    active_channels_mask: Option<&[bool]>,
    mut progress: impl FnMut(usize, usize),
) -> Result<(usize, usize), ResampleError> {
    let expected_output_len = (resampler.resample_ratio() * input_len as f64).ceil() as usize;

    let mut indexing = rubato::Indexing {
        input_offset: 0,
        output_offset: 0,
        active_channels_mask: active_channels_mask.map(|m| m.to_vec()),
        partial_len: None,
    };

    let mut frames_left = input_len;
    let mut output_len = 0;
    let mut frames_to_trim = resampler.output_delay();
    debug!(
        "resamping {} input frames to {} output frames, delay to trim off {} frames",
        input_len, expected_output_len, frames_to_trim
    );

    let next_nbr_input_frames = resampler.input_frames_next();
    while frames_left > next_nbr_input_frames {
        debug!("process, {} input frames left", frames_left);
        let (nbr_in, nbr_out) =
            resampler.process_into_buffer(buffer_in, buffer_out, Some(&indexing))?;
        frames_left -= nbr_in;
        output_len += nbr_out;
        indexing.input_offset += nbr_in;
        indexing.output_offset += nbr_out;
        if frames_to_trim > 0 && output_len > frames_to_trim {
            debug!(
                "output, {} is longer than delay to trim, {}, trimming..",
                output_len, frames_to_trim
            );
            // move useful output data to start of output buffer
            buffer_out.copy_frames_within(frames_to_trim, 0, frames_to_trim);
            // update counters
            output_len -= frames_to_trim;
            indexing.output_offset -= frames_to_trim;
            frames_to_trim = 0;
        }
        progress(indexing.input_offset, input_len);
    }
    if frames_left > 0 {
        debug!("process the last partial chunk, len {}", frames_left);
        indexing.partial_len = Some(frames_left);
        let (_nbr_in, nbr_out) =
            resampler.process_into_buffer(buffer_in, buffer_out, Some(&indexing))?;
        output_len += nbr_out;
        indexing.output_offset += nbr_out;
    }
    indexing.partial_len = Some(0);
    while output_len < expected_output_len {
        debug!(
            "output is still too short, {} < {}, pump zeros..",
            output_len, expected_output_len
        );
        let (_nbr_in, nbr_out) =
            resampler.process_into_buffer(buffer_in, buffer_out, Some(&indexing))?;
        output_len += nbr_out;
        indexing.output_offset += nbr_out;
    }
    Ok((input_len, expected_output_len))
}
