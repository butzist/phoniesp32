use audio_file_utils::metadata::extract_metadata;
use std::io::{Read, Write};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("input file path not provided");
    let out = args.get(2).expect("output file path not provided");

    let mut src = std::fs::File::open(path).expect("failed to open media");
    let mut buf = Vec::new();
    src.read_to_end(&mut buf).expect("failed reading file");

    let mut last_progress = None;
    let progress = move |current: usize, total: usize| {
        let percent = current * 100 / total;
        if last_progress != Some(percent) {
            last_progress = Some(percent);
            print!("\r{}%", percent);
        }
    };
    let result = transcoder::decode_and_normalize(buf.into(), progress)
        .await
        .expect("failed transcoding");
    println!(); // newline after progress

    let mut dst = std::fs::File::create_new(out).expect("failed to open destination");
    dst.write_all(&result).expect("failed writing file");

    let metadata = extract_metadata(&*result)
        .await
        .expect("failed to extract metadata");
    println!("\nRIFF Metadata:");
    println!("Artist: {}", metadata.artist.as_str());
    println!("Title: {}", metadata.title.as_str());
    println!("Album: {}", metadata.album.as_str());
}

