use std::io::{Read as _, Write as _};

fn main() {
    // Get the first command line argument.
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("input file path not provided");
    let out = args.get(2).expect("output file path not provided");

    // Open the media source.
    let mut src = std::fs::File::open(path).expect("failed to open media");
    let mut buf = Vec::new();
    src.read_to_end(&mut buf).expect("failed reading file");

    let progress = |current: usize, total: usize| print!("\r{}", current * 100 / total);
    let result =
        transcoder::decode_and_normalize(buf.into(), progress).expect("failed transcoding");

    // Open and write output
    let mut dst = std::fs::File::create_new(out).expect("failed to open destination");
    dst.write_all(&result).expect("failed writing file");
}