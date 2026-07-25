//! Turning parsed data back into files other tools read: OBJ + MTL, and PNG.
//!
//! Everything here is a pure function of already-parsed data — it returns text or bytes and never
//! touches the filesystem, so the caller decides where a file goes (and a GUI may put it on a
//! clipboard instead). What belongs here is the *format knowledge* an export needs: which material
//! a run resolves to, which frame the vertices are in, what a texture is called when the file
//! gives it no name. Keeping that in the library is what stops `ug2` and STRUKT from drifting into
//! two different answers for the same car.

/// Binary glTF. Behind the `png` feature because a `.glb` embeds its images, and an exporter that
/// silently dropped them would produce an untextured car rather than a smaller file.
#[cfg(feature = "png")]
pub mod gltf;
pub mod material;
pub mod obj;

#[cfg(feature = "png")]
pub use gltf::write_glb;
pub use material::MaterialPlan;
pub use obj::{write_mtl, write_obj, Material};

use crate::types::NfsTexture;

/// A texture's file name: its `DebugName` when the compiler left a usable one, else its hash.
///
/// The hash is appended either way — names in a TPK are truncated to a fixed field, so two
/// different textures can arrive under one name and only the hash tells them apart.
#[must_use]
pub fn png_name(t: &NfsTexture) -> String {
    let stem: String = t.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    if stem.is_empty() {
        format!("{:08X}.png", t.hash.0)
    } else {
        format!("{stem}_{:08X}.png", t.hash.0)
    }
}

/// Encode a decoded texture as PNG bytes (RGBA8, no filtering choices of our own).
///
/// # Errors
/// [`NfsError::BufferSizeMismatch`](crate::NfsError::BufferSizeMismatch) when the pixel buffer
/// does not match the descriptor's width × height, and [`NfsError::Io`](crate::NfsError::Io) if
/// the encoder itself fails.
#[cfg(feature = "png")]
pub fn png_bytes(t: &NfsTexture) -> crate::NfsResult<Vec<u8>> {
    let expected = t.width as usize * t.height as usize * 4;
    if t.rgba.len() != expected {
        return Err(crate::NfsError::BufferSizeMismatch {
            detail: "texture pixel buffer does not match its width × height",
        });
    }
    let mut out = Vec::new();
    let io = |e: png::EncodingError| crate::NfsError::Io(std::io::Error::other(e.to_string()));
    let mut enc = png::Encoder::new(&mut out, t.width, t.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(io)?;
    writer.write_image_data(&t.rgba).map_err(io)?;
    writer.finish().map_err(io)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AssetHash;

    fn tex(name: &str, w: u32, h: u32) -> NfsTexture {
        NfsTexture {
            name: name.to_string(),
            hash: AssetHash(0x1234_ABCD),
            width: w,
            height: h,
            rgba: vec![0x7F; (w * h * 4) as usize],
            ..NfsTexture::default()
        }
    }

    #[test]
    fn a_nameless_texture_is_named_after_its_hash() {
        assert_eq!(png_name(&tex("", 2, 2)), "1234ABCD.png");
        // The hash stays on the end even when there is a name: TPK names are truncated, so two
        // textures can share one.
        assert_eq!(png_name(&tex("240SX_BADGING", 2, 2)), "240SX_BADGING_1234ABCD.png");
    }

    #[test]
    fn a_name_with_punctuation_keeps_only_what_a_filename_may_hold() {
        assert_eq!(png_name(&tex("front/left.b", 2, 2)), "frontleftb_1234ABCD.png");
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_bytes_start_with_the_png_signature() {
        let bytes = png_bytes(&tex("t", 4, 2)).expect("4×2 RGBA must encode");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[cfg(feature = "png")]
    #[test]
    fn a_pixel_buffer_that_does_not_match_the_descriptor_is_refused() {
        let mut t = tex("t", 4, 2);
        t.rgba.truncate(7);
        assert!(png_bytes(&t).is_err());
    }
}
