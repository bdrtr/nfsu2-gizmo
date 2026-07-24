//! The inspector: the selected chunk's parsed fields.
//!
//! Slice 1 shows what is true of *any* chunk — id, kind, offsets, size, the `0x11` filler shift,
//! a byte preview. That last one matters more than it looks: in this format a shifted header is
//! the difference between a correct read and garbage, so it is a first-class row rather than
//! something you infer from the hex.
//!
//! Per-chunk decoding (mesh counts, the solid's name and matrix, the descriptor table) lands next,
//! once the parser publishes its format map.

use crate::app::Strukt;
use crate::theme::{self, token};
use egui::RichText;

/// A row in the inspector.
struct Field {
    label: &'static str,
    /// Where the value lives in the file, when it is read rather than derived.
    offset: Option<usize>,
    value: String,
    note: Option<String>,
}

/// Draw the inspector for the current selection.
pub fn show(app: &Strukt, ui: &mut egui::Ui) {
    let t = app.lang.strings();
    let Some((doc, node)) = app.doc.as_ref().zip(app.selection.and_then(|o| app.doc.as_ref()?.node_at(o)))
    else {
        ui.add_space(theme::token::SPACE_3);
        ui.label(RichText::new(t.nothing_selected).color(theme::muted(50)));
        return;
    };

    let data = node.data(&doc.bytes);
    let header = format!("{:#010x}", node.header.id);
    ui.add_space(theme::token::SPACE_1);
    ui.label(RichText::new(chunk_label(node.header.id)).font(theme::font::heading(14.0)));
    ui.label(
        RichText::new(format!("{header} · {} B", node.header.size))
            .font(theme::font::mono(10.5))
            .color(theme::muted(50)),
    );
    ui.add_space(theme::token::SPACE_2);

    let filler = leading_filler_words(data);
    let mut fields = vec![
        Field {
            label: "chunk id",
            offset: Some(node.offset),
            value: format!("{header}  \"{}\"", gizmo_nfs::fourcc::FourCc(node.header.id)),
            note: None,
        },
        Field {
            label: "size",
            offset: Some(node.offset + 4),
            value: format!("{} B  ({:#x})", node.header.size, node.header.size),
            note: None,
        },
        Field {
            label: "kind",
            offset: None,
            value: format!("{:?}", node.kind()),
            note: None,
        },
        Field {
            label: "payload",
            offset: Some(node.data_offset),
            value: format!("{:#010x} … {:#010x}", node.data_offset, node.data_offset + data.len()),
            note: None,
        },
    ];
    if !node.children.is_empty() {
        fields.push(Field {
            label: "children",
            offset: None,
            value: node.children.len().to_string(),
            note: None,
        });
    }
    if filler > 0 {
        fields.push(Field {
            label: "0x11 filler",
            offset: Some(node.data_offset),
            value: format!("{filler} kelime / word ({} B)", filler * 4),
            note: Some(
                "hizalama dolgusu — alan offsetleri bu kadar kayar / alignment filler: fields shift by this".into(),
            ),
        });
    }
    if !data.is_empty() {
        let n = data.len().min(16);
        let hex: Vec<String> = data[..n].iter().map(|b| format!("{b:02X}")).collect();
        fields.push(Field { label: "preview", offset: Some(node.data_offset), value: hex.join(" "), note: None });
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for f in &fields {
            ui.add_space(theme::token::SPACE_1);
            ui.horizontal(|ui| {
                ui.label(RichText::new(f.label).size(11.5).color(theme::muted(60)));
                if let Some(off) = f.offset {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{off:#x}")).font(theme::font::mono(9.0)).color(theme::muted(40)),
                        );
                    });
                }
            });
            ui.label(RichText::new(&f.value).font(theme::font::mono(12.0)).color(token::TEXT));
            if let Some(note) = &f.note {
                ui.label(RichText::new(note).size(10.0).color(theme::muted(48)));
            }
            ui.separator();
        }
    });
}

/// The panel caption.
pub fn caption(app: &Strukt, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(app.lang.strings().p_inspector)
            .font(theme::font::heading(10.0))
            .color(theme::muted(60)),
    );
}

/// A readable name for a chunk id — the tree's label and the inspector's title.
///
/// Only the ids this project has actually locked down are named; anything else keeps its number,
/// which is more honest than inventing a label for a chunk nobody has decoded.
#[must_use]
pub fn chunk_label(id: u32) -> &'static str {
    match id {
        0x8013_4000 => "SolidList",
        0x8013_4001 => "SolidListHeader",
        0x0013_4002 => "ListInfo",
        0x8013_4010 => "SolidObject",
        0x0013_4011 => "ObjectHeader",
        0x0013_4012 => "MaterialList",
        0x0013_4013 => "ShaderList",
        0x8013_4100 => "MeshData",
        0x0013_4900 => "MeshHeader",
        0x0013_4B01 => "VertexBuffer",
        0x0013_4B02 => "MaterialRanges",
        0x0013_4B03 => "IndexBuffer",
        0xB330_0000 => "TexturePack",
        0xB331_0000 => "TPK InfoPart",
        0x3331_0001 => "TPK Header",
        0x3331_0002 => "TextureHashes",
        0x3331_0003 => "CompDescTable",
        0xB331_2000 => "TPK DataPart",
        _ => "chunk",
    }
}

/// How many leading `0x11111111` filler words a payload carries.
fn leading_filler_words(data: &[u8]) -> usize {
    let mut n = 0;
    while data.get(n * 4..n * 4 + 4) == Some(&[0x11; 4]) {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_ids_are_named_and_unknown_ones_are_not_invented() {
        assert_eq!(chunk_label(0x0013_4900), "MeshHeader");
        assert_eq!(chunk_label(0x0013_4B03), "IndexBuffer");
        assert_eq!(chunk_label(0xDEAD_BEEF), "chunk");
    }

    #[test]
    fn filler_is_counted_in_whole_words() {
        assert_eq!(leading_filler_words(&[0x11; 8]), 2);
        assert_eq!(leading_filler_words(&[0x11, 0x11, 0, 0]), 0, "a partial word is not a shift");
        assert_eq!(leading_filler_words(&[]), 0);
    }
}
