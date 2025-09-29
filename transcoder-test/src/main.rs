use std::io::Read;

use rodio::buffer::SamplesBuffer;

const OUT_RATE: u32 = 44100;

fn main() {
    // Get the first command line argument.
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("file path not provided");

    // Open the media source.
    let mut src = std::fs::File::open(path).expect("failed to open media");
    let mut buf = Vec::new();
    src.read_to_end(&mut buf).expect("failed reading file");

    // --- Setup Rodio output ---
    let stream_handle =
        rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
    let sink = rodio::Sink::connect_new(stream_handle.mixer());

    let progress = |current: usize, total: usize| print!("\r{}", current * 100 / total);
    let processed_samples =
        transcoder::decode_and_normalize(buf.into(), progress).expect("failed transcoding");

    // --- Play via Rodio ---
    let source = SamplesBuffer::new(1, OUT_RATE, processed_samples);
    sink.append(source);

    sink.sleep_until_end();
}
