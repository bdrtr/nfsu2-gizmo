//! Property tests: the untrusted-input parsers must never panic, hang, or over-allocate
//! on arbitrary or adversarial byte streams — they either succeed or return an `NfsError`.

use gizmo_nfs::chunk::ChunkNode;
use gizmo_nfs::compression;
use gizmo_nfs::texture::Tpk;
use gizmo_nfs::viv::VivArchive;
use proptest::prelude::*;

proptest! {
    #[test]
    fn decompress_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = compression::decompress(&data);
    }

    #[test]
    fn chunk_parse_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = ChunkNode::parse(&data);
    }

    #[test]
    fn viv_parse_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = VivArchive::parse(&data);
    }

    #[test]
    fn tpk_parse_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = Tpk::parse(&data);
    }

    // Force a RefPack-looking signature + a plausible small output size so the decoder's
    // opcode loop and its bounds guards get exercised on garbage payloads.
    #[test]
    fn refpack_signed_never_panics(mut data in proptest::collection::vec(any::<u8>(), 5..4096)) {
        data[0] = 0x10;
        data[1] = 0xFB;
        let _ = compression::decompress(&data);
    }

    // Force a HUFF header onto garbage so the Huffman table build + bit reader are exercised.
    #[test]
    fn huff_signed_never_panics(mut data in proptest::collection::vec(any::<u8>(), 16..4096)) {
        data[0..4].copy_from_slice(b"HUFF");
        data[4] = 1; // version
        let _ = compression::decompress(&data);
    }
}
