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

    let samples = transcoder::decode_and_normalize(vec.into(), progress)
        .await
        .map_err(|e| js_sys::Error::new(&e.to_string()))?;

    // TODO: encode as IMA ADPCM
    let output = unsafe {
        std::slice::from_raw_parts(
            samples.as_ptr() as *const u8,
            samples.len() * std::mem::size_of::<f32>(),
        )
    };

    let u8_array = Uint8Array::new_with_length(output.len() as u32);
    u8_array.copy_from(output);

    Ok(u8_array.buffer())
}
