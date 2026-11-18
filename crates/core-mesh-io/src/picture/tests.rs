use std::io::Cursor;

use image::{GrayImage, ImageFormat, Luma};

use super::*;

fn png_of(side_px: u32, grey: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    GrayImage::from_pixel(side_px, side_px, Luma([grey]))
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("a grey picture encodes as PNG");
    bytes
}

#[test]
fn a_picture_past_the_cap_comes_back_at_the_cap_and_white_reads_one() {
    let picture = decode_picture("big.png", &png_of(2048, 255)).expect("a PNG decodes");

    assert_eq!(picture.size_px, [2048, 2048]);
    assert_eq!(
        (picture.heights.width(), picture.heights.height()),
        (MAX_PICTURE_PX as usize, MAX_PICTURE_PX as usize),
        "a 2048 px picture is read 1024 px across"
    );
    let first = picture.heights.samples()[0];
    assert!((first - 1.0).abs() < 1e-3, "white reads 1.0, got {first}");
}

#[test]
fn bytes_that_are_no_picture_are_undecodable() {
    let error = decode_picture("bad.png", b"not a picture").expect_err("these are no PNG");

    assert!(
        matches!(error, MeshIoError::UndecodableTexture { .. }),
        "{error:?}"
    );
}
