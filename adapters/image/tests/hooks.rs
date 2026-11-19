//! Process-isolated image-reader hooks and reader-limit regressions.

mod common;

use std::io::Cursor;

use image::{ImageError, ImageReader, Limits};
use jbig2_image::{register_decoding_hooks, DecoderOptions};

#[test]
fn magic_extension_and_memory_loading_use_hooks_and_reader_limits() {
    let mut options = DecoderOptions::default();
    options.limits.max_stream_bytes = 4096;
    register_decoding_hooks(options).unwrap();
    let bytes = common::patterned(true, false);
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .unwrap();
    assert!(reader.format().is_none());
    assert_eq!(
        reader.decode().unwrap().into_luma8().as_raw(),
        &common::expected()
    );
    assert_eq!(
        image::load_from_memory(&bytes)
            .unwrap()
            .into_luma8()
            .as_raw(),
        &common::expected()
    );

    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .unwrap();
    let mut limits = Limits::default();
    limits.max_image_width = Some(8);
    reader.limits(limits);
    assert!(matches!(reader.decode(), Err(ImageError::Limits(_))));

    let large = common::standalone(
        true,
        Some(1),
        vec![
            common::page(0, 1, 257, 256, false),
            common::segment(1, 49, 1, vec![]),
            common::segment(2, 51, 0, vec![]),
        ],
    );
    let mut reader = ImageReader::new(Cursor::new(&large))
        .with_guessed_format()
        .unwrap();
    let mut limits = Limits::no_limits();
    // The remaining budget fits the packed bitmap, but not another L8 output.
    limits.max_alloc = Some(257 * 256 + large.len() as u64 + 32_768);
    reader.limits(limits);
    let decoded = reader.decode().unwrap().into_luma8();
    assert_eq!(decoded.dimensions(), (257, 256));
    assert!(decoded.as_raw().iter().all(|&sample| sample == 0));
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(27 + bytes.len() as u64 - 1);
    reader.limits(limits);
    assert!(matches!(reader.decode(), Err(ImageError::Limits(_))));

    let dir = std::env::temp_dir().join(format!("jbig2-image-hooks-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for extension in ["jb2", "jbig2"] {
        let path = dir.join(format!("test.{extension}"));
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            ImageReader::open(&path)
                .unwrap()
                .decode()
                .unwrap()
                .into_luma8()
                .as_raw(),
            &common::expected()
        );
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
    let mut broken_signature = bytes.clone();
    broken_signature[7] ^= 1;
    assert!(image::load_from_memory(&broken_signature).is_err());
    assert_eq!(
        register_decoding_hooks(DecoderOptions::default())
            .unwrap_err()
            .extension,
        "jb2"
    );
    let oversized = [bytes.as_slice(), &[0; 4096]].concat();
    assert!(matches!(
        image::load_from_memory(&oversized),
        Err(ImageError::Limits(_))
    ));
}
