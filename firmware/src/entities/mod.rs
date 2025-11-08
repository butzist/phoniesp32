pub mod audio_file;
pub mod playlist;

fn basename(fname: &[u8], ext: &str) -> Option<heapless::String<8>> {
    let fname = str::from_utf8(fname).ok()?;
    let basename = fname.strip_suffix(ext)?;
    heapless::String::<8>::try_from(basename).ok()
}
