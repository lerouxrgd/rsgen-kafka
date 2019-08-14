use std::io::prelude::*;

use flate2::read::GzDecoder;

pub fn decompress(src: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut buffer = vec![];
    GzDecoder::new(src).read_to_end(&mut buffer)?;
    Ok(buffer)
}