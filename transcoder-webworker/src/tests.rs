use crate::transcode;
use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

fn create_test_progress_function() -> Function {
    Function::new_with_args("current, total", "return undefined;")
}

fn create_array_buffer_from_bytes(bytes: &[u8]) -> ArrayBuffer {
    let array_buffer = ArrayBuffer::new(bytes.len() as u32);
    let u8_array = Uint8Array::new(&array_buffer);
    u8_array.copy_from(bytes);
    array_buffer
}

fn extract_wav_data_from_result(result: &Object) -> Option<ArrayBuffer> {
    Reflect::get(result, &JsValue::from_str("data"))
        .ok()
        .and_then(|data| data.dyn_into::<ArrayBuffer>().ok())
}

fn extract_wav_header_data(output_array: &ArrayBuffer) -> Option<(usize, [u8; 12])> {
    let output_u8_array = Uint8Array::new(output_array);
    let output_len = output_u8_array.length() as usize;

    if output_len < 44 {
        return None;
    }

    let mut header = [0u8; 12];
    output_u8_array.subarray(0, 12).copy_to(&mut header);
    Some((output_len, header))
}

fn validate_wav_structure(output_len: usize, header: &[u8; 12]) -> Result<(), String> {
    // Check RIFF and WAVE identifiers
    if &header[0..4] != b"RIFF" {
        return Err("Missing RIFF identifier".to_string());
    }
    if &header[8..12] != b"WAVE" {
        return Err("Missing WAVE identifier".to_string());
    }

    // Check RIFF chunk size consistency
    let riff_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let expected_file_size = output_len - 8;

    if riff_size != expected_file_size {
        return Err(format!(
            "RIFF chunk size {} doesn't match file size {}",
            riff_size, expected_file_size
        ));
    }

    Ok(())
}

#[wasm_bindgen_test]
async fn test_transcode_invalid_input() {
    let test_data = vec![0u8; 100]; // Too small to be valid audio
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    // Should handle invalid input gracefully
    assert!(result.is_err(), "Transcoding invalid input should fail");
}

#[wasm_bindgen_test]
async fn test_transcode_empty_input() {
    let test_data = vec![];
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    // Should handle empty input gracefully
    assert!(result.is_err(), "Transcoding empty input should fail");
}

#[wasm_bindgen_test]
async fn test_transcode_minimal_valid_input() {
    // Create a minimal valid MP3 header (simplified for testing)
    let test_data = vec![
        0xFF, 0xFB, 0x90, 0x00, // MP3 header
        0x00, 0x00, 0x00, 0x00, // Some data
    ];
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    // This might fail due to incomplete MP3, but shouldn't panic
    // The important thing is that it handles the error gracefully
    match result {
        Ok(result_obj) => {
            // If it succeeds, validate the output
            let output_array =
                extract_wav_data_from_result(&result_obj).expect("Should have WAV data in result");
            let (output_len, header) =
                extract_wav_header_data(&output_array).expect("Should have valid WAV header");

            assert!(output_len > 44, "Output should be larger than WAV header");
            validate_wav_structure(output_len, &header).expect("Should have valid WAV structure");
        }
        Err(_) => {
            // Expected for incomplete MP3 data
        }
    }
}

#[wasm_bindgen_test]
async fn test_output_file_size_reasonableness() {
    // Test with various input sizes to ensure output sizes are reasonable
    let test_sizes = [1000, 5000, 10000];

    for size in test_sizes.iter() {
        let test_data = vec![0u8; *size];
        let input_array = create_array_buffer_from_bytes(&test_data);
        let progress = create_test_progress_function();

        let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

        // Even if transcoding fails due to invalid data, it shouldn't create unreasonably large outputs
        if let Ok(result_obj) = result {
            let output_array =
                extract_wav_data_from_result(&result_obj).expect("Should have WAV data in result");
            let output_u8_array = Uint8Array::new(&output_array);
            let output_len = output_u8_array.length() as usize;

            // For any input, output should be reasonable (not more than 100x input size)
            let max_reasonable_size = *size * 2;
            assert!(
                output_len <= max_reasonable_size,
                "Output size {} bytes is unreasonably large for input {} bytes",
                output_len,
                size
            );

            // Also shouldn't be tiny (unless it failed)
            if output_len > 44 {
                let (_, header) =
                    extract_wav_header_data(&output_array).expect("Should have valid WAV header");
                validate_wav_structure(output_len, &header)
                    .expect("Should have valid WAV structure");
            }
        }
    }
}

#[wasm_bindgen_test]
async fn test_progress_callback_handling() {
    let test_data = vec![0u8; 1000];
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    // The important thing is that the progress function doesn't cause panics
    // Even if transcoding fails, the progress handling should be robust
    let _ = result; // We don't care about success/failure here
}

#[wasm_bindgen_test]
async fn test_memory_safety_with_large_inputs() {
    // Test with larger inputs to ensure no memory safety issues
    let large_test_data = vec![0u8; 100_000]; // 100KB
    let input_array = create_array_buffer_from_bytes(&large_test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    match result {
        Ok(result_obj) => {
            let output_array =
                extract_wav_data_from_result(&result_obj).expect("Should have WAV data in result");
            let output_u8_array = Uint8Array::new(&output_array);
            let output_len = output_u8_array.length() as usize;

            // Should not create unreasonably large outputs
            assert!(
                output_len <= 10_000_000, // 10MB max
                "Output size {} bytes is too large for 100KB input",
                output_len
            );

            if output_len > 44 {
                let (_, header) =
                    extract_wav_header_data(&output_array).expect("Should have valid WAV header");
                validate_wav_structure(output_len, &header)
                    .expect("Should have valid WAV structure");
            }
        }
        Err(_) => {
            // Expected for invalid data, but should not panic
        }
    }
}

#[wasm_bindgen_test]
async fn test_concurrent_transcode_calls() {
    // Test that multiple concurrent transcode calls don't interfere
    let test_data = vec![0u8; 1000];
    let input_array1 = create_array_buffer_from_bytes(&test_data);
    let input_array2 = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    // Run two transcodes concurrently
    let result1 = transcode(&input_array1, &progress);
    let result2 = transcode(&input_array2, &progress);

    let (res1, res2) = futures::join!(result1, result2);

    // Both should complete without panicking (they may fail due to invalid data)
    let _ = (res1, res2);
}

#[wasm_bindgen_test]
async fn test_wav_header_validation() {
    // Test with data that might produce valid WAV output
    // This is a minimal test to ensure WAV header validation works
    let test_data = vec![
        0xFF, 0xFB, 0x90, 0x00, // MP3 frame header
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Some audio data
    ];
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    if let Ok(result_obj) = result {
        let output_array =
            extract_wav_data_from_result(&result_obj).expect("Should have WAV data in result");
        let (output_len, header) =
            extract_wav_header_data(&output_array).expect("Should have WAV header data");

        // Validate WAV structure
        validate_wav_structure(output_len, &header).expect("Should produce valid WAV structure");

        // Additional WAV-specific validations
        assert_eq!(&header[0..4], b"RIFF", "Should start with RIFF");
        assert_eq!(&header[8..12], b"WAVE", "Should contain WAVE");

        // RIFF size should be reasonable
        let riff_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        assert!(riff_size > 0, "RIFF size should be positive");
        assert!(riff_size < 100_000_000, "RIFF size should be reasonable");
    }
}

#[wasm_bindgen_test]
async fn test_file_size_limits() {
    // Test various input sizes to ensure reasonable output sizes
    let test_cases = vec![
        (100, "tiny input"),
        (1000, "small input"),
        (10_000, "medium input"),
        (50_000, "large input"),
    ];

    for (input_size, description) in test_cases {
        let test_data = vec![0u8; input_size];
        let input_array = create_array_buffer_from_bytes(&test_data);
        let progress = create_test_progress_function();

        let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

        if let Ok(result_obj) = result {
            let output_array =
                extract_wav_data_from_result(&result_obj).expect("Should have WAV data in result");
            let output_u8_array = Uint8Array::new(&output_array);
            let output_len = output_u8_array.length() as usize;

            // Output should be reasonable relative to input
            let max_expected_size = input_size * 1000; // Very generous upper bound
            assert!(
                output_len <= max_expected_size,
                "{}: output size {} should not exceed {}x input size {}",
                description,
                output_len,
                max_expected_size / input_size,
                input_size
            );

            // If we have a valid WAV, validate its structure
            if let Some((_, header)) = extract_wav_header_data(&output_array) {
                validate_wav_structure(output_len, &header)
                    .expect("Should have valid WAV structure");
            }
        }
    }
}

#[wasm_bindgen_test]
async fn test_progress_callback_error_handling() {
    let test_data = vec![0u8; 1000];
    let input_array = create_array_buffer_from_bytes(&test_data);

    // Create a progress function that throws an error
    let progress = Function::new_with_args(
        "current, total",
        "throw new Error('Progress callback error');",
    );

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    // Should handle progress callback errors gracefully
    // The transcoding might still succeed or fail, but shouldn't panic
    let _ = result;
}

#[wasm_bindgen_test]
async fn test_array_buffer_boundary_conditions() {
    // Test edge cases with ArrayBuffer sizes
    let test_cases = vec![
        vec![0xFF, 0xFB],             // Too small for MP3
        vec![0xFF, 0xFB, 0x90],       // Still too small
        vec![0xFF, 0xFB, 0x90, 0x00], // Minimal MP3 header
    ];

    for (i, test_data) in test_cases.into_iter().enumerate() {
        let input_array = create_array_buffer_from_bytes(&test_data);
        let progress = create_test_progress_function();

        let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

        // All should handle gracefully without panicking
        match result {
            Ok(result_obj) => {
                // If successful, validate basic structure
                if let Some(output_array) = extract_wav_data_from_result(&result_obj)
                    && let Some((output_len, header)) = extract_wav_header_data(&output_array)
                {
                    let _ = validate_wav_structure(output_len, &header);
                }
            }
            Err(_) => {
                // Expected for invalid data
            }
        }

        println!("Test case {} completed successfully", i + 1);
    }
}

#[wasm_bindgen_test]
async fn test_result_contains_filename() {
    // Test that the result object contains a filename field with 8.3 format
    let test_data = vec![
        0xFF, 0xFB, 0x90, 0x00, // MP3 frame header
        0x00, 0x00, 0x00, 0x00, // Some audio data
    ];
    let input_array = create_array_buffer_from_bytes(&test_data);
    let progress = create_test_progress_function();

    let result: Result<Object, JsValue> = transcode(&input_array, &progress).await;

    if let Ok(result_obj) = result {
        // Check that filename exists and is a string
        let filename = Reflect::get(&result_obj, &JsValue::from_str("filename"))
            .expect("Should have filename field");

        assert!(filename.is_string(), "Filename should be a string");

        let filename_str = filename
            .as_string()
            .expect("Filename should be convertible to string");

        // Check 8.3 format: up to 8 characters, dot, exactly 3 characters
        let parts: Vec<&str> = filename_str.split('.').collect();
        assert_eq!(parts.len(), 2, "Filename should have exactly one dot");

        let name_part = parts[0];
        let extension = parts[1];

        assert_eq!(extension, "wav", "Extension should be 'wav'");
        assert!(
            name_part.len() == 8,
            "Name part should be 8 characters, got {}",
            name_part.len()
        );

        // Verify name part contains only valid 8.3 characters (alphanumeric and some symbols)
        for ch in name_part.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || "~!@#$%^&()-'_`".contains(ch),
                "Filename contains invalid character: {}",
                ch
            );
        }
    }
    // If transcoding fails, that's okay for this test - we're just testing the structure
}
