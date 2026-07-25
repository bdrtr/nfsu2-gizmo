//! Property tests: the untrusted-input parsers must never panic, hang, or over-allocate
//! on arbitrary or adversarial byte streams — they either succeed or return an `NfsError`.
//!
//! [`gizmo_nfs::discover`] is here for the same reason with a second input: the *schema* is typed
//! by a person, so a stride of zero, a header past the end of the file and a record index of
//! `usize::MAX` are all things it will be asked to read.

use gizmo_nfs::chunk::ChunkNode;
use gizmo_nfs::compression;
use gizmo_nfs::discover::{self, Kind, Schema};
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

    // The schema is user input: any stride (including 0), any header, any column list, any row.
    #[test]
    fn discover_never_panics(
        data in proptest::collection::vec(any::<u8>(), 0..4096),
        header in 0usize..8192,
        stride in 0usize..512,
        kinds in proptest::collection::vec(0usize..10, 0..24),
        index in 0usize..100_000,
    ) {
        let columns: Vec<Kind> = kinds.iter().map(|i| Kind::all()[*i]).collect();
        let schema = Schema { header, stride, columns };
        let shape = discover::shape(data.len(), &schema);
        let _ = discover::row(&data, &schema, index);
        let _ = discover::row(&data, &schema, usize::MAX);
        let _ = discover::row_offset(&schema, usize::MAX);
        let _ = discover::guess_columns(&data, header, stride);
        let _ = discover::stride_candidates(data.len());
        let _ = discover::stride_for(data.len(), shape.records);
        // A row's cell count is the column count, whatever the bytes did: a table that loses a
        // column silently shifts every value in the row.
        prop_assert_eq!(discover::row(&data, &schema, index).len(), schema.columns.len());
    }
}
