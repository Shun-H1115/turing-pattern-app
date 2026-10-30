use std::{fs::OpenOptions, io::BufWriter, path::Path};

pub fn save_png(
    path: &Path,
    size: usize,
    pixels: &[u8],
    description: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(16..=512).contains(&size) || pixels.len() != size * size * 3 {
        return Err("Invalid image dimensions".into());
    }
    // Encode before creating the destination; never overwrite an existing image.
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, size as u32, size as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.add_itxt_chunk("Description".into(), description.into())?;
        encoder.write_header()?.write_image_data(pixels)?;
    }
    use std::io::Write;
    let mut file = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?);
    file.write_all(&encoded)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_roundtrip_and_no_overwrite() {
        let path = std::env::temp_dir().join(format!("turing-test-{}.png", std::process::id()));
        let pixels = vec![123; 16 * 16 * 3];
        save_png(&path, 16, &pixels, "test").unwrap();
        assert!(save_png(&path, 16, &pixels, "test").is_err());
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()))
                .read_info()
                .unwrap();
        let mut out = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut out).unwrap();
        assert_eq!((info.width, info.height), (16, 16));
        assert_eq!(&out[..info.buffer_size()], &pixels);
        std::fs::remove_file(path).unwrap();
    }
}
