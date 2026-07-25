//! The interface in Turkish and English.
//!
//! The strings are the design's own table, carried over verbatim so the two stay comparable when
//! a screen is checked against the prototype. One struct with one field per string keeps the
//! compiler honest: a missing translation is a build error, not a blank label.

/// Which language the interface is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    /// Türkçe — the project's own working language, and the design's default.
    #[default]
    Tr,
    /// English.
    En,
}

impl Lang {
    /// The other language, for a toggle button.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Tr => Self::En,
            Self::En => Self::Tr,
        }
    }

    /// The two-letter badge the language switch shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Tr => "TR",
            Self::En => "EN",
        }
    }

    /// The string table for this language.
    #[must_use]
    pub fn strings(self) -> &'static Strings {
        match self {
            Self::Tr => &TR,
            Self::En => &EN,
        }
    }
}

/// Every piece of interface text, in one language.
///
/// Strings for screens that are designed but not yet drawn are carried too — they come from the
/// design's own table, and translating them once is cheaper than hunting them down later.
#[allow(dead_code)]
pub struct Strings {
    pub brand_sub: &'static str,
    pub m_open: &'static str,
    pub m_export: &'static str,
    pub nav_workspace: &'static str,
    pub nav_validation: &'static str,
    pub nav_discovery: &'static str,
    pub nav_diff: &'static str,
    pub nav_dict: &'static str,
    pub p_tree: &'static str,
    pub p_inspector: &'static str,
    pub p_log: &'static str,
    pub tab_3d: &'static str,
    pub tab_hex: &'static str,
    pub tab_tex: &'static str,
    pub matrix: &'static str,
    /// Over the 3D viewport: the part's bounding box, and how to move the camera.
    pub bbox: &'static str,
    pub drag_hint: &'static str,
    /// Shown when the viewport is showing the showroom car rather than one part.
    pub stock_car: &'static str,
    /// The texture tab: the empty states, and the counts over the contact sheet.
    pub no_textures: &'static str,
    pub pick_texture: &'static str,
    pub textures_count: &'static str,
    pub textures_undecoded: &'static str,
    /// Export: the toolbar button's hover text, and the two log lines it can produce.
    pub export_hint: &'static str,
    pub exported: &'static str,
    pub export_failed: &'static str,
    /// The units an export's summary line counts in.
    pub ex_parts: &'static str,
    pub ex_materials: &'static str,
    /// The discovery screen: the schema controls, and the table's own vocabulary.
    pub d_header: &'static str,
    pub d_stride: &'static str,
    pub d_records: &'static str,
    pub d_left: &'static str,
    pub d_unclaimed: &'static str,
    pub d_candidates: &'static str,
    pub d_candidate_hint: &'static str,
    pub d_guess: &'static str,
    pub d_guess_hint: &'static str,
    pub d_offset: &'static str,
    pub d_cycle: &'static str,
    pub d_add: &'static str,
    pub d_remove: &'static str,
    /// The compare screen.
    pub df_left: &'static str,
    pub df_right: &'static str,
    pub df_pick: &'static str,
    pub df_identical: &'static str,
    pub df_changed: &'static str,
    pub df_resized: &'static str,
    pub df_only_left: &'static str,
    pub df_only_right: &'static str,
    pub df_same: &'static str,
    pub df_all: &'static str,
    pub df_first: &'static str,
    pub df_go: &'static str,
    pub st_chunks: &'static str,
    pub st_sel: &'static str,
    pub st_scale: &'static str,
    pub log_all: &'static str,
    pub log_warn: &'static str,
    pub log_err: &'static str,
    pub log_info: &'static str,
    pub w_open_source: &'static str,
    pub w_validates: &'static str,
    pub w_discovers: &'static str,
    pub w_open_file: &'static str,
    pub w_drop: &'static str,
    pub w_recent: &'static str,
    pub w_modes: &'static str,
    pub w_card_validation: &'static str,
    pub w_card_discovery: &'static str,
    pub w_card_diff: &'static str,
    pub w_card_dict: &'static str,
    /// Shown where a screen exists in the design but not yet in the app.
    pub soon: &'static str,
    /// Shown in the centre area before a file is open.
    pub no_file: &'static str,
    pub open_failed: &'static str,
    pub nothing_selected: &'static str,
    pub val_no_findings: &'static str,
    /// How many chunks a rule read, and how many findings it produced.
    pub val_examined: &'static str,
    pub val_findings: &'static str,
    /// Shown for a rule that had nothing to read — deliberately not phrased as a pass.
    pub val_unread: &'static str,
}

static TR: Strings = Strings {
    brand_sub: "nfsu2 varlık aracı",
    m_open: "Aç",
    m_export: "Dışa Aktar",
    nav_workspace: "Çalışma",
    nav_validation: "Doğrulama",
    nav_discovery: "Keşif",
    nav_diff: "Karşılaştır",
    nav_dict: "Sözlük",
    p_tree: "AĞAÇ",
    p_inspector: "DENETÇİ",
    p_log: "GÜNLÜK",
    tab_3d: "3D",
    tab_hex: "HEX",
    tab_tex: "DOKU",
    matrix: "Birim matris (4×4)",
    bbox: "sınır kutusu",
    drag_hint: "sürükle döndür · kaydır yakınlaş",
    stock_car: "stok araç",
    no_textures: "Bu dosyada doku yok — yanında TEXTURES.BIN de bulunamadı",
    pick_texture: "Izgaradan bir doku seç",
    textures_count: "doku",
    textures_undecoded: "çözülemedi",
    export_hint: "ekranda ne varsa onu strukt-export/ altına yazar",
    exported: "yazıldı",
    export_failed: "dışa aktarılamadı",
    ex_parts: "parça",
    ex_materials: "materyal",
    d_header: "başlık",
    d_stride: "stride",
    d_records: "kayıt",
    d_left: "artıyor",
    d_unclaimed: "sahipsiz",
    d_candidates: "tam bölen:",
    d_candidate_hint: "stride × kayıt — payload'ı kalansız bölenler",
    d_guess: "tahmin et",
    d_guess_hint: "her 4 baytlık şeridi örnekleyip tipini öner",
    d_offset: "offset",
    d_cycle: "tıkla: tipi değiştir",
    d_add: "sütun ekle",
    d_remove: "son sütunu sil",
    df_left: "sol",
    df_right: "sağ",
    df_pick: "Karşılaştırmak için ikinci dosyayı seç — yolu yaz ya da pencereye bırak",
    df_identical: "İki dosya birebir aynı: aynı chunk'lar, aynı boyutlar, aynı baytlar",
    df_changed: "değişti",
    df_resized: "boyut değişti",
    df_only_left: "yalnız solda",
    df_only_right: "yalnız sağda",
    df_same: "aynı",
    df_all: "aynı olanları da listele",
    df_first: "ilk fark",
    df_go: "tıkla: sol dosyada bu chunk'a git",
    st_chunks: "chunk",
    st_sel: "seçim",
    st_scale: "1u = 1m · Z↑",
    log_all: "Tümü",
    log_warn: "Uyarı",
    log_err: "Hata",
    log_info: "Bilgi",
    w_open_source: "açık kaynak",
    w_validates: "doğrular",
    w_discovers: "keşfettirir",
    w_open_file: "DOSYA AÇ",
    w_drop: "Dosyayı buraya bırak ya da tıkla",
    w_recent: "SON KULLANILANLAR",
    w_modes: "MODLAR",
    w_card_validation: "Dosyayı açar açmaz sağlık kontrolü: stride, bbox, normaller, indeksler.",
    w_card_discovery: "Bilinmeyen chunk'ı canlı olarak yeniden yorumla: stride ve alanları dene.",
    w_card_diff: "İki dosyayı yan yana koy, chunk farklarını gör.",
    w_card_dict: "Doku hash'lerine isim ver; isim ↔ hash tablosunu yönet.",
    soon: "sonraki dilimde",
    no_file: "Bir dosya aç",
    open_failed: "Dosya açılamadı",
    nothing_selected: "Ağaçtan bir chunk seç",
    val_no_findings: "Bulgu yok — dosya temiz görünüyor",
    val_examined: "chunk okundu",
    val_findings: "bulgu",
    val_unread: "bu dosyada okunacak bir şey yoktu — geçti demek değil",
};

static EN: Strings = Strings {
    brand_sub: "nfsu2 asset toolkit",
    m_open: "Open",
    m_export: "Export",
    nav_workspace: "Workspace",
    nav_validation: "Validation",
    nav_discovery: "Discovery",
    nav_diff: "Compare",
    nav_dict: "Dictionary",
    p_tree: "TREE",
    p_inspector: "INSPECTOR",
    p_log: "LOG",
    tab_3d: "3D",
    tab_hex: "HEX",
    tab_tex: "TEXTURE",
    matrix: "Unit matrix (4×4)",
    bbox: "bbox",
    drag_hint: "drag to orbit · scroll to zoom",
    stock_car: "showroom car",
    no_textures: "No textures in this file — and no TEXTURES.BIN beside it",
    pick_texture: "Pick a texture from the grid",
    textures_count: "textures",
    textures_undecoded: "could not be decoded",
    export_hint: "writes whatever is on screen under strukt-export/",
    exported: "written",
    export_failed: "export failed",
    ex_parts: "parts",
    ex_materials: "materials",
    d_header: "header",
    d_stride: "stride",
    d_records: "records",
    d_left: "left over",
    d_unclaimed: "unclaimed",
    d_candidates: "divides exactly:",
    d_candidate_hint: "stride × records — the strides that leave no remainder",
    d_guess: "guess",
    d_guess_hint: "sample every 4-byte lane and propose its type",
    d_offset: "offset",
    d_cycle: "click to change the type",
    d_add: "add a column",
    d_remove: "drop the last column",
    df_left: "left",
    df_right: "right",
    df_pick: "Pick a second file to compare — type a path, or drop one on the window",
    df_identical: "The two files are identical: same chunks, same sizes, same bytes",
    df_changed: "changed",
    df_resized: "resized",
    df_only_left: "only left",
    df_only_right: "only right",
    df_same: "same",
    df_all: "list identical chunks too",
    df_first: "first difference",
    df_go: "click to go to this chunk in the left file",
    st_chunks: "chunks",
    st_sel: "sel",
    st_scale: "1u = 1m · Z↑",
    log_all: "All",
    log_warn: "Warn",
    log_err: "Error",
    log_info: "Info",
    w_open_source: "open source",
    w_validates: "validates",
    w_discovers: "discovers",
    w_open_file: "OPEN FILE",
    w_drop: "Drop a file here, or click",
    w_recent: "RECENT",
    w_modes: "MODES",
    w_card_validation: "A health check the moment a file opens: stride, bbox, normals, indices.",
    w_card_discovery: "Reinterpret an unknown chunk live: try a stride and a set of fields.",
    w_card_diff: "Put two files side by side and see the chunk-level differences.",
    w_card_dict: "Give texture hashes names; manage the name ↔ hash table.",
    soon: "in a later slice",
    no_file: "Open a file",
    open_failed: "Could not open the file",
    nothing_selected: "Select a chunk in the tree",
    val_no_findings: "No findings — the file looks clean",
    val_examined: "chunks read",
    val_findings: "findings",
    val_unread: "nothing here for this rule to read — which is not a pass",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_are_populated_and_distinct() {
        // A copy-paste slip would leave an English string in the Turkish table; the panel captions
        // are the ones most likely to be forgotten.
        assert_eq!(Lang::Tr.strings().p_tree, "AĞAÇ");
        assert_eq!(Lang::En.strings().p_tree, "TREE");
        assert_ne!(Lang::Tr.strings().nav_validation, Lang::En.strings().nav_validation);
    }

    #[test]
    fn the_toggle_returns_to_where_it_started() {
        assert_eq!(Lang::Tr.other(), Lang::En);
        assert_eq!(Lang::Tr.other().other(), Lang::Tr);
    }
}
