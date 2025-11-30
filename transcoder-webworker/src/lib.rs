use js_sys::{ArrayBuffer, Function, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn transcode(input: &ArrayBuffer, progress: &Function) -> Result<ArrayBuffer, JsValue> {
    let mut last_position: usize = 0;
    let progress = move |position: usize, total: usize| {
        if last_position == position {
            return;
        }

        last_position = position;
        progress
            .call2(
                &JsValue::NULL,
                &JsValue::from_f64(position as f64),
                &JsValue::from_f64(total as f64),
            )
            .ok();
    };

    // need to copy buffer to own it
    let u8_array = Uint8Array::new(input);
    let mut vec = vec![0u8; u8_array.length() as usize];
    u8_array.copy_to(vec.as_mut_slice());

    let wav_data = transcoder::decode_and_normalize(vec.into(), progress)
        .await
        .map_err(|e| js_sys::Error::new(&e.to_string()))?;

    // The result is already a complete WAV file as Box<[u8]>, so just copy it directly
    let u8_array = Uint8Array::new_with_length(wav_data.len() as u32);
    u8_array.copy_from(&wav_data);

    Ok(u8_array.buffer())
}

#[cfg(test)]
mod tests;
