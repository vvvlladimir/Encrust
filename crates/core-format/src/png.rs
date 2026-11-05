//! PNG encoding and decoding for the containers that hold their layers as images.

use core_raster::{LayerRuns, Run};
use core_thumbnail::Thumbnail;

use crate::preview::{preview_in_range, thumbnail};
use crate::{FormatError, panel_in_range};

/// Expands a layer into an eight-bit greyscale PNG, one layer of an archive container.
/// `Fast` compression: on a panel-sized mask deflate costs more than the rasterisation it follows.
pub fn encode_grey(layer: &LayerRuns) -> Result<Vec<u8>, FormatError> {
    let mask = layer.to_mask();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, mask.width(), mask.height());
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);

    let mut writer = encoder.write_header().map_err(failed("a layer image"))?;
    writer
        .write_image_data(mask.pixels())
        .map_err(failed("a layer image"))?;
    writer.finish().map_err(failed("a layer image"))?;
    Ok(bytes)
}

/// Encodes a preview as a colour PNG, which is what an archive container holds two of.
pub fn encode_colour(image: &Thumbnail) -> Result<Vec<u8>, FormatError> {
    let pixels: Vec<u8> = image.pixels().iter().flatten().copied().collect();
    encode_preview(image, png::ColorType::Rgb, &pixels)
}

/// Encodes a preview as an opaque colour PNG with an alpha channel, for a container whose
/// known files hold their previews so.
pub fn encode_colour_alpha(image: &Thumbnail) -> Result<Vec<u8>, FormatError> {
    let pixels: Vec<u8> = image
        .pixels()
        .iter()
        .flat_map(|&[r, g, b]| [r, g, b, u8::MAX])
        .collect();
    encode_preview(image, png::ColorType::Rgba, &pixels)
}

fn encode_preview(
    image: &Thumbnail,
    colour: png::ColorType,
    pixels: &[u8],
) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width_px(), image.height_px());
    encoder.set_color(colour);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().map_err(failed("a preview"))?;
    writer
        .write_image_data(pixels)
        .map_err(failed("a preview"))?;
    writer.finish().map_err(failed("a preview"))?;
    Ok(bytes)
}

/// A colour or grey PNG preview as plain RGB over black where it is transparent, any depth
/// brought to eight bits.
pub fn decode_colour(bytes: &[u8]) -> Result<Thumbnail, FormatError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| preview_decoding(&e))?;
    let (width_px, height_px) = (reader.info().width, reader.info().height);
    preview_in_range(width_px, height_px)?;

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| preview_decoding(&"the image is larger than memory"))?;
    let mut buffer = vec![0; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|e| preview_decoding(&e))?;
    let channels = frame.color_type.samples();
    let pixels = buffer[..frame.buffer_size()]
        .chunks_exact(channels)
        .map(|pixel| match channels {
            1 => [pixel[0]; 3],
            2 => [over_black(pixel[0], pixel[1]); 3],
            3 => [pixel[0], pixel[1], pixel[2]],
            _ => [0, 1, 2].map(|channel| over_black(pixel[channel], pixel[3])),
        })
        .collect();
    thumbnail(width_px, height_px, pixels)
}

fn over_black(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

fn preview_decoding(reason: &dyn std::fmt::Display) -> FormatError {
    FormatError::Encoding {
        what: "a preview",
        reason: reason.to_string(),
    }
}

/// The shape of one image, which for a container of separate PNGs is the only place its
/// panel is recorded.
pub fn png_shape(bytes: &[u8]) -> Result<(u32, u32), FormatError> {
    let reader = decoder(bytes)?;
    let info = reader.info();
    panel_in_range(info.width, info.height)?;
    Ok((info.width, info.height))
}

/// One layer's PNG as runs of eight-bit grey.
///
/// The image is greyscale already, so the pixels come back as they went in and nothing is
/// quantised; a container of another depth is refused rather than squeezed.
pub fn decode_grey(bytes: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut reader = decoder(bytes)?;
    panel_in_range(reader.info().width, reader.info().height)?;

    let (colour, depth) = (reader.info().color_type, reader.info().bit_depth);
    if (colour, depth) != (png::ColorType::Grayscale, png::BitDepth::Eight) {
        return Err(decoding(format!(
            "it is {colour:?} at {depth:?}, not eight-bit grey"
        )));
    }

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| decoding("the image is larger than memory".to_owned()))?;
    let mut pixels = vec![0; size];
    let frame = reader
        .next_frame(&mut pixels)
        .map_err(|source| decoding(source.to_string()))?;

    let mut runs: Vec<Run> = Vec::new();
    for value in &pixels[..frame.buffer_size()] {
        match runs.last_mut() {
            Some(run) if run.value == *value => run.length += 1,
            _ => runs.push(Run {
                value: *value,
                length: 1,
            }),
        }
    }
    Ok(runs)
}

fn decoder(bytes: &[u8]) -> Result<png::Reader<std::io::Cursor<&[u8]>>, FormatError> {
    png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|source| decoding(source.to_string()))
}

fn decoding(reason: String) -> FormatError {
    FormatError::Encoding {
        what: "a layer image",
        reason,
    }
}

fn failed(what: &'static str) -> impl Fn(png::EncodingError) -> FormatError {
    move |source| FormatError::Encoding {
        what,
        reason: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_becomes_a_colour_png() {
        let png = encode_colour(&Thumbnail::filled(4, 2, [255, 0, 0])).expect("a preview encodes");
        assert_eq!(png[24], 8);
        assert_eq!(png[25], 2, "three channels, with no palette and no alpha");
    }

    #[test]
    fn a_colour_png_decodes_back_to_its_pixels() {
        let image = Thumbnail::from_pixels(2, 1, vec![[255, 0, 0], [0, 0, 255]])
            .expect("two pixels fill 2x1");
        let png = encode_colour(&image).expect("a preview encodes");
        assert_eq!(decode_colour(&png).expect("it was just encoded"), image);
    }

    #[test]
    fn an_opaque_alpha_preview_is_four_channels_and_reads_back_to_its_pixels() {
        let image = Thumbnail::from_pixels(2, 1, vec![[255, 0, 0], [10, 10, 11]])
            .expect("two pixels fill 2x1");
        let png = encode_colour_alpha(&image).expect("a preview encodes");
        assert_eq!(png[25], 6, "red, green, blue and alpha");
        assert_eq!(decode_colour(&png).expect("it was just encoded"), image);
    }

    #[test]
    fn a_grey_png_decodes_as_a_colour_preview() {
        let mut runs = LayerRuns::builder(2, 1);
        runs.push(1, 0);
        runs.push(1, 200);
        let png = encode_grey(&runs.finish()).expect("a layer of our own encodes");
        let expected = Thumbnail::from_pixels(2, 1, vec![[0; 3], [200; 3]]).expect("2x1");
        assert_eq!(decode_colour(&png).expect("a grey PNG decodes"), expected);
    }

    #[test]
    fn a_transparent_pixel_reads_as_black_and_a_half_transparent_one_as_half_its_colour() {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("a vec takes a header");
        writer
            .write_image_data(&[255, 255, 255, 0, 200, 100, 50, 128])
            .expect("a vec takes the pixels");
        writer.finish().expect("the image closes");

        let preview = decode_colour(&bytes).expect("an RGBA PNG decodes");
        assert_eq!(preview.pixels(), [[0, 0, 0], [100, 50, 25]]);
    }

    #[test]
    fn bytes_that_are_not_a_png_are_refused_as_a_preview() {
        let err = decode_colour(b"not a picture").unwrap_err();
        assert!(matches!(err, FormatError::Encoding { what, .. } if what == "a preview"));
    }

    #[test]
    fn a_layer_becomes_an_eight_bit_greyscale_png() {
        let mut runs = LayerRuns::builder(4, 2);
        runs.push(3, 255);
        let png = encode_grey(&runs.finish()).expect("a layer of our own encodes");

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The header chunk carries the shape: width, height, bit depth, colour type.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 4);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 2);
        assert_eq!(png[24], 8, "eight bits a pixel");
        assert_eq!(png[25], 0, "greyscale, with no palette and no alpha");
    }

    #[test]
    fn a_layer_image_of_another_depth_is_refused_rather_than_squeezed() {
        let colour = encode_colour(&Thumbnail::filled(2, 2, [1, 2, 3])).expect("a preview encodes");
        let err = decode_grey(&colour).unwrap_err();
        assert!(matches!(err, FormatError::Encoding { what, .. } if what == "a layer image"));
    }

    #[test]
    fn a_layer_decodes_back_to_the_runs_it_was_written_from() {
        let mut runs = LayerRuns::builder(4, 2);
        runs.push(3, 255);
        let layer = runs.finish();
        let png = encode_grey(&layer).expect("a layer encodes");

        assert_eq!(png_shape(&png).expect("the shape is in the header"), (4, 2));
        assert_eq!(decode_grey(&png).expect("it decodes"), layer.runs());
    }
}
