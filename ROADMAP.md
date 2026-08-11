# Yol Haritası — şehir, ve onun Gizmo'ya öğreteceği şeyler

> Bu bir plan belgesidir, kod değil. Amaç çift: NFSU2 yeniden yapımını ilerletmek **ve**
> bunu Gizmo motorunu bilerek büyüterek yapmak. Her milestone'un iki yüzü var.
>
> Satır numaraları bu belgenin yazıldığı andaki `HEAD`'e göredir; kayabilir, dosya ve
> tip adları kalıcıdır.

---

## 0. Bağlam — neden şimdi, neden şehir

Bugün oyun katmanı arabayı derinlemesine biliyor: `KIT##`/`STYLE##` parça uzayı, 123 renklik
boya paleti, `GlobalB`'den okunan dokuz noktalı tork eğrisi, dört vites kutusu, upgrade
kademeleri. Ama üzerinde sürdüğü şey `nfs_race.rs`'te `build_track(80, 55, 15, hill, 200)` —
**uydurma bir oval**, ve on iki checkpoint centerline'dan eşit aralıkla alınmış.

Eksik olan tek büyük parça şehir. `gizmo-nfs/README.md` `world` satırını "🔴 research-frontier"
diye işaretliyor; bu abartı. Crate STREAM'in ihtiyaç duyduğu her ilkeli **zaten** içeriyor —
chunk ağacı, JDLZ, HUFF, TPK betimleyicileri, DXT ve paletli çözücüler, `bStringHash`, ve
24 baytlık vertex düzeninin ta kendisi (`geometry/vertex.rs:115`). Eksik olan **montaj artı
bir TPK varyantı**, araştırma değil.

Referans: [whoismept/OpenUG](https://github.com/whoismept/OpenUG) — C ile yazılmış, şehri ve
yarış rotalarını çözmüş rakip bir proje. `docs/FORMATS.md`'si spesifikasyon olarak okunur,
kodu kopyalanmaz. Hangi iddiasının ölçüldüğü, hangisinin tahmin, hangisinin yanlış olduğu
§8'de ayrıştırıldı.

### Şehrin ölçülmüş boyutu

Kendi kurulumumuza karşı sayıldı (crawl + resync yürüyüşüyle, §1.1):

| bölge | boyut | mesh | TPK |
|---|---:|---:|---:|
| `STREAML4RA` | 118.5 MB | 10.735 | 206 |
| `STREAML4RD` | 120.6 MB | 11.135 | 231 |
| `STREAML4RB` | 28.3 MB | 2.642 | 18 |
| `STREAML4RG` | 18.9 MB | 2.271 | 9 |
| `STREAML4RF` | 16.8 MB | 773 | 17 |
| `STREAML4RC` | 14.7 MB | 772 | 19 |
| `STREAML4RR` | 4.2 MB | 482 | 1 |
| `STREAML4RH` | 2.1 MB | 175 | 1 |
| **toplam** | **324 MB** | **28.985** | **502** |

Her bölgede mesh sayısı = vertex buffer sayısı = materyal sayısı, tam olarak. Yürüyüşün
doğru olduğunun kanıtı bu.

---

## 1. Ana fikir

Şehir tek bir özellik değil — **trençkot giymiş altı motor yeteneği**. Ve çarpıcı olan şu:
neredeyse hepsi Gizmo'da zaten var, sadece bağlanmamış, yanlış kapının arkasında, ya da bir
`if`'in yanlış tarafında:

| hazır duran | nerede | durum |
|---|---|---|
| `Frustum::test_aabb_masked` | `gizmo-math/src/frustum.rs:271` | hiyerarşik culling için yazılmış, **sıfır çağıran** |
| `GpuCullState::cull_pass` | `gizmo-renderer/src/gpu_cull.rs:201` | her native renderer'da inşa ediliyor, **hiç dispatch edilmiyor** |
| `PhysicsWorld::raycast_excluding` | `gizmo-physics-rigid/src/world/query.rs:87` | doc yorumu araç vakasını anlatıyor, araç ona **erişemiyor** |
| `State::apply_transitions` | `gizmo-core/src/state.rs:29` | **sıfır çağıran** → `State::set` kalıcı no-op |
| `Renderer::generate_mipmaps` | `gizmo-renderer/src/renderer/textures.rs:148` | çalışıyor, headless testli, **her asset yolu atlıyor** |
| `point_shadows_enabled` | `gizmo-renderer/src/renderer/mod.rs:170` | alan var, **render pass'i onu okumuyor** |

Plan bu yüzden "nesne sayısını yüzle çarpınca ilk ne sessizce bozulur" sırasına göre dizildi,
ve her düzeltme NFS'e özgü değil **genel** halinde yapılacak şekilde seçildi.

### 1.1 Ölçülmüş iki format gerçeği

**STREAM dosyaları düz chunk akışı değil.** Naif `off += 8 + size` yürüyüşü desenkron oluyor.
`STREAML4RH.BUN`'da dosyanın **%4.5'inde** duruyor: chunk'lar sektör sınırlarına hizalanmış ve
aralarda dolgu var (0xF10 → 0xF80, 0x1F60 → 0x3000). Adı boşuna "STREAM" değil.

Kurtarma: geçersiz `size` görünce kırılma, **8 bayt ilerle ve yeniden yakala**.
`STREAML4RH`: kıran yürüyüş 13 mesh, crawl+resync **175 mesh**.

**Container testi üst bit, üst yarım bayt değil.** `ID & 0x80000000` — `nfsu2_arac_plani.md:236`
bunu baştan doğru yazmış. OpenUG'un `FORMATS.md`'si "top nibble == 0x8" diyor ve bu yanlış:
`0xB3300000` (TPK kökü) o testten geçemez.

---

## 2. Gölge kararı

**Karar: şehir gölge alacak ve gölge düşürecek.** Baked-lit bir sahne için teknik olarak
"unlit yeterli" denebilirdi, ama araba ile şehrin ışık olarak birbirini görmemesi his olarak
kayıp. Kabul edildi. Aşağıdaki dört adım bunu ödenebilir kılıyor.

Bugünkü maliyet, `crates/gizmo/src/systems/render/passes/shadow.rs`'ten sayıldı:

```
aydınlatılmış bir batch = 23 draw call
  1  Z-prepass
  1  G-buffer
  8  CSM        → CASCADE_COUNT=4 pass × 2 draw (kamera + gölge instance bölgesi)
 12  point      → 6 yüz × 2 draw
  1  forward
unlit bir batch = 1 draw call          (shadow.rs:31 ve :77 `item.unlit` görünce atlıyor)
```

**Ama asıl mesele gölge değil, gölge pass'inde culling olmaması.** İki gerçek bunu çözüyor:

1. **`SHADOW_DISTANCE = 100.0`** (`gizmo-renderer/src/csm.rs:23`). Cascade'ler kameranın
   yalnızca 100 metresini kapsıyor. Şehrin tamamı hiçbir zaman gölge pass'ine girmez —
   girmemesi gerekir. Gölgeyi ödenebilir kılan tek gerçek bu.
2. Ama `record_shadow_passes` bugün **her** `draw_item`'ı, mesafeye bakmadan, dört cascade'in
   hepsine çiziyor. 29.000 meshlik bir şehirde bu tek başına ölümcül — gölgeler pahalı olduğu
   için değil, caster kümesi culling'siz olduğu için.

Dolayısıyla dört adım:

| adım | ne | kazanç | nerede |
|---|---|---|---|
| **G1** | 6 point-shadow yüzünü `renderer.point_shadows_enabled`'a bağla (alan var, pass okumuyor) | 23 → 11 draw | M2, ~5 satır |
| **G2** | `MaterialType::BakedLit` — vertex ARGB albedo × güneş × CSM gölge terimi, tek forward pass. Deferred PBR'a ve 10 point light'a girmez, ama CSM'e **düşürür** ve CSM'den **alır** | 11 → ~4-6 draw | M2 |
| **G3** | Cascade başına caster culling — hücre indeksini her cascade'in ortho frustum'una karşı test et. `cascade_vp: [Mat4; 4]` zaten `collect_draw_items`'a geçiyor (`batching.rs:181`); eksik olan hücre indeksi | caster kümesi O(görünür), 100 m ile sınırlı | M2/M5 |
| **G4** | Statik cascade önbelleği — şehir kıpırdamıyor; uzak cascade'ler yalnızca yeniden merkezlenince değişir. Kare başına sadece dinamik caster (arabalar) yeniden çizilir | uzak cascade'lerde büyük, yakında az | M7, isteğe bağlı |

`MaterialType` şu an 5 varyantlı kapalı bir enum (`gizmo-renderer/src/components/material.rs:5`:
`Pbr, Unlit, Skybox, Water, Grid`), yani G2 enum'a dokunuyor. `unlit.wgsl`'in ilk satırı
"No shadow group (unlit doesn't sample shadows)" diyor — `BakedLit` ondan ayrı bir shader.

**Gizmo'nun bundan kazandığı:** baked lighting + dinamik gölge, statik seviyeli **her** oyunun
istediği malzeme; ve cascade başına caster culling, gölge kullanan her sahnenin istediği şey.
İkisi de NFSU2 hakkında hiçbir şey bilmez.

---

## 3. Önce risk spike'ları

Sıra "geç öğrenirsen bir haftana mal olur" ölçütüne göre. Toplam ≈ 4 gün, ve çıktısı sadece
sayı değil, parser'ın temeli.

| risk | en ucuz deney | süre | neyi karara bağlar |
|---|---|---|---|
| **Bellek tavanı.** 13 GB makine. `Vertex` 92 B indexsiz (`gpu_types.rs:5`) + `Arc<Vec<Vec3>>` CPU gölgesi (`components/mesh.rs:16`) ≈ STREAM kaydının 11 katı. `STREAML4RA` 118 MB / 10.735 solid | `world::manifest()` (yalnız başlıklar, `chunk::walk` üzerinde `Visit::SkipChildren`) + `ug2 world --stats`, 8 bundle'da `/usr/bin/time -v` | 1.5 g | Indexed geometry + `StaticVertex` M2'ye mi çekilecek yoksa M5'te mi kalacak. **Projedeki en önemli tek sayı.** |
| **8192 instance tavanı sessizce kırpıyor.** `collect_draw_items` `&Renderer` alıyor (`batching.rs:177`) — `ensure_instance_capacity(&mut self, …)`'i **çağıramaz**. Şu an yalnız `gizmo-studio` çağırıyor | Headless küp süpürmesi, `nfs_shot.rs:188-249` readback yolunda. N = 500…20.000 | 0.5 g | İki satırlık imza düzeltmesi M2'de, **içerik ona bağlanmadan önce** |
| **Gölge maliyeti gerçek mi.** §2'nin 23 vs 1 sayımı | Aynı süpürmeyi `Pbr` / `Unlit` / (sonra) `BakedLit` ile üç kez koştur | 2 sa | G1-G3'ün yeterli olup olmadığı |
| **`compute_aabb` O(vertices), kare başına 24 kez.** `collider.rs:44-60` her vertex'i tek tek dönüştürüyor; çağrı yerleri `vehicle/dynamics.rs:310` (4 tekerlek × 4 substep) + `world/construction.rs` + `pipeline.rs` | Mevcut `scratchpad/aabbbench`'i gerçek üçgen sayılarıyla koştur | 1 sa | ~17 fps tabanını şehir terimleriyle ifade eder. Düzeltmesi ~20 satır ve rakiplerden önce şart |
| **`0x11` dolgu tuzağı sessiz ve başarı gibi görünüyor.** `geometry::name::read_matrix` mutlak `MATRIX_OFFSET=64` uyguluyor, dolguyu atlamıyor; `STREAML4RH` başlıklarının **%74'ü** 4-12 bayt dolgu taşıyor (histogram `{0:46, 4:36, 8:49, 12:44}`) | 0/4/8/12 dolgu genişliği için dört sentetik test, aynı hash/bbox/matris/isim | 2 sa | `read_matrix` yeniden kullanılabilir mi (kullanılamaz). Yanlış-ama-makul matris en kötü hata modu: proplar *bir yere* düşer |
| **Yinelenen geometri z-fighting sanılacak.** OpenUG 8 bundle'da solid meshlerin **%51.5'ini** (14.551/28.270) dz = 0.000'da birebir kopya olarak ölçmüş | `--stats`'a dedup anahtarı çakışma sayısını ekle: `(doku hash, 3 eksen bbox, üçgen, vertex)` | 3 sa | Çözüm yükleme-zamanı dedup mı (öyle) yoksa depth bias mı (değil) |
| **Aynalanmış şehir instance'ları.** `placement::should_place` her `det < 0` için koşulsuz `false` dönüyor — araba parçaları için doğru, meşru aynalanmış bir binayı orijine yığar | `--stats`'a det işareti histogramı | 20 dk | Dünya objelerinin yerleşim sezgiselliğine ihtiyacı var mı (yok: identity = baked, değilse uygula) |
| **Şehir ölçeğinde derinlik hassasiyeti.** `nfs_race.rs:259` near 0.1 / far 4000 kullanıyor — `nfs_shot.rs:177-182`'nin bir jant üzerinde bile zararlı olduğunu yazdığı yapılandırma. Dünya açıklığı −8.960 … +8.010 | Aynı yol/arazi örtüşmesinin 300 m'den iki çekimi, near=0.1 vs ≥1.0, diff | 0.5 g (M2'de) | Reversed-Z motor görevi mi olacak, yoksa kamera başına near/far türetmesi yeter mi |
| **Kırmızı golden bir tuzak.** `tests/golden_assets.rs:1090` `the_chunkless_pack_is_refused_by_name`, `CARS/PEUGOT/TEXTURES.BIN`'in `0x33320002` chunk'ı olmadığını iddia ediyor; bu stok kurulumda var | `:1095`'teki mevcut skip koluna `find_chunk(&roots, 0x33320002).is_some()` ekle | 30 dk | Hiçbir şey — ama kırmızı suite sana suite'i yok saymayı öğretir. Tek dünya golden'ı yazmadan önce |

---

## 4. Milestone'lar

**Sıralama kararı, açıkça:** yarış (M4) streaming'den (M5) **önce**. Gerekçe — `STREAML4RH`
2 MB / 175 solid, ve kapı/state-machine/AI işi streaming'den tamamen bağımsız; `NFS_REGION` +
`NFS_RADIUS` gibi atılacak bir filtre büyük bölgeleri M5'e kadar taşır. **M0'ın aritmetiği
bunu bozabilir:** bir bölge indexsiz belleğe sığmıyorsa, indexed geometry M2'ye çekilir ve
streaming yarıştan öne geçer.

---

### M0 — Baytları ve tavanları kanıtla · ~4 gün

**Hedef:** format, bellek ve draw-call tavanı hakkındaki her yük taşıyan bilinmeyenin ölçülmüş
bir sayısı olsun — tek bir şehir üçgeni GPU'ya ulaşmadan önce.

**Oyun/parser işi**
- `PryHUB/crates/gizmo-nfs/tests/golden_assets.rs:1095` düzelt; `NFSU2_ROOT=… cargo test -p gizmo-nfs` yeşil olsun.
- `PryHUB/crates/gizmo-nfs/src/world/{mod,format,header}.rs` — **yalnız `manifest()`**: başlıklar,
  hash'ler, bbox'lar, transformlar, sayaçlar, sıfır buffer. `chunk::walk` üzerinde
  `Visit::SkipChildren` ile (`ChunkNode::parse` L4RA'da ~102.000 düğüm materyalize ediyor).
  Bu, `Tpk::parse` vs `Tpk::directory` ayrımının aynısı — o da tam bu yüzden var.
- `header.rs` her offsetten **önce** `geometry::solid::skip_leading_filler` çağırmalı; hash `+0x10`,
  bbox `+0x20`/`+0x30`, matris `+0x40`, isim `payload[len-28..]`'den NUL'a kadar (sondan sabit
  mesafe — `geometry::name::part_name`'in en-uzun-yazdırılabilir-dizi sezgiselliği dünya
  başlıklarında float gürültüsü döndürüyor).
- `ug2 world <FILE|TRACKS-dir> --stats` — `probe.rs:20`'deki tabloyu örnek al.

**Motor işi:** kasıtlı olarak **yok**. İki atılacak spike binary'si, hiçbir motor kodu commit edilmez.

**Kanıt:** `--stats` 8 bundle'da 2 GB tepe RSS altında bitiyor. Süpürme CSV'sinin
`geometry_present` sütunu N=8192'de `false`'a dönüyor. Dört sayı kayıtlı: RAM, vertex sayısı,
draw-call tavanı, `compute_aabb` maliyeti.

---

### M1 — Şehir parse oluyor, dokular 8× büyük olmayı bırakıyor · 6-8 gün

**Hedef:** `gizmo_nfs::world` ve `::routes` bir STREAM bundle'ını ve bir ROUTES dosyasını
ölçülmüş golden testlerle çözüyor; Gizmo mipli, anizotropik filtrelenmiş dokuyu tek paylaşılan
sampler üzerinden yükleyebiliyor.

**Parser işi**
- `src/world/object.rs` — `geometry::vertex::layout_for` + `parse_packed` + `normals_from_triangles`,
  `index::parse_indices`, `material::material_ranges` + `ordered_hashes`, `solid::mesh_field`
  hepsi **değişmeden** yeniden kullanılır. `NfsMeshPart` **değil** `NfsWorldObject` üret
  (öteki araba-özel `role`/`lod` taşıyor ve dünyanın birincil anahtarı olan obje hash'i yok).
  Submesh word 8 için `shader: AssetHash(0)` bildir — ölçülen 272/272 dünya kaydında sabit
  `0xff` bayrağı, shader indeksi değil. Çözülmemiş kökleri (`0x80034100`, `0x80034130`,
  `0x80036000`, `0x80135000/100`, `0x34027`, `0x35020`) düşürmek yerine `undecoded_roots`
  olarak bildir.
- `src/world/tpk.rs` — **track TPK varyantı**. `0x33310004`'e anahtarlı, stride `0x7c`,
  piksel havuzu `0x33320002`'de kendi `0x11` dolgusundan sonra. `texture::Tpk::directory`
  `0x33310003` olmadan hata veriyor; o chunk `STREAML4RA`'nın 206 paketinin **hiçbirinde yok**.
  `texture::decode::palette_at`'i yeniden kullanma (palet-imgeden-sonra dayatıyor; L4RH'nin
  0. kaydında palet 0'da, imge 1024'te) — taze bir çakışmama kontrolü yaz, çözücülere devret.
  `unpack_palettised`, `unpack_bgra`, `level_size`, `named_format`, `PALETTE_BYTES`, `MAX_DIM`,
  `directory::texture_name`'i `pub(super)` → `pub(crate)` genişlet (yalnız görünürlük).

  **Doğrulanmış kayıt düzeni** (`STREAML4RH`, bu kurulumda ölçüldü):

  ```
  +0x0C  char[24]  isim          +0x38  u32       boyut
  +0x24  u32       BinKey        +0x3C  u32       palet boyutu (1024 = P8, 0 = DXT)
  +0x30  u32       offset        +0x44  u16,u16   genişlik, yükseklik
  ```

  | kayıt | isim | key | boyut | mip zinciri |
  |---|---|---|---|---|
  | 0 | `RDP_PARKING_NL_AA_KT` | `0x1DE30448` | 64×64 P8 | 4096+1024+256+64 = 5440 ✓ |
  | 1 | `TRN_GRASSC` | `0x5671B2B5` | 256×256 | 32768+8192+2048+512+128+32 = 43680 ✓ |
  | 2 | `OBJ_BLKPLAS` | `0xE90EB7A2` | 128×128 | 8192+2048+512+128+32 = 10912 ✓ |
  | 3 | `OBJ_PYLON` | `0xFB1E978B` | 64×**128** | 4096+1024+256+64 = 5440 ✓ |

  Dördünde de aritmetik tam oturuyor. `OBJ_PYLON` **kare değil** — OpenUG'un "dokular karedir,
  formatı boyuttan çıkar" varsayımı bu yüzden güvenli değil; başlık formatı zaten söylüyor.

- `src/routes/{mod,format,line,events,markers}.rs` — `0x00034148` 24 baytlık düğümler
  (`+12/+14/+16` link alanlarını **oku**), `0x0003414c` 272 baytlık etkinlik kayıtları,
  `0x00034146` 48 baytlık başlangıç işaretçileri (8 bayt dolgunun ardında). `+20`'yi `progress`
  diye adlandır, **metre değil**: ölçülen delta'lar XY adımının ≈0.787 katı.
- `ug2 track <TRACKS-dir> --region L4RA [--event 4001] [--csv]`.

**Motor işi** (küçük, parser'dan bağımsız)
- `asset/texture.rs`: `mip_level_count = w.max(h).ilog2()+1` ve **zaten çalışan**
  `Renderer::generate_mipmaps`'i çağır. `mipmap_filter: Linear`, `lod_max_clamp: f32::MAX`,
  `anisotropy_clamp: 1 → 16`.
- Doku başına `wgpu::Sampler`'ı tek paylaşılan `Arc<Sampler>`'a yükselt — hepsi aynı
  `SAMPLER_LINEAR_REPEAT`, ve `maxSamplerAllocationCount` bazı sürücülerde ~1024.
- `batching.rs:177`: `collect_draw_items` `&mut Renderer` alsın ve `ensure_instance_capacity`
  çağırsın — `gizmo-studio/src/render_pipeline/mod.rs:460`'ın yaptığı gibi.

**Kanıt:** evdeki üslupla **tam** sayı golden'ları (`golden_assets.rs:160` `(39, 28, 6)` iddia
ediyor, alt sınır değil): L4RH → 175 obje, 175/175 stride-24, 175/175
`bStringHash(isim) == başlık hash`, `(169 identity, 6 placed)`, dolgu histogramı
`[(0,46),(4,36),(8,49),(12,44)]`; TPK'sı → 4 kayıt, `names[0] == "RDP_PARKING_NL_AA_KT"`,
boyutlar `[(64,64),(256,256),(128,128),(64,128)]`. `world_parse_never_panics` /
`routes_parse_never_panics`. Motor: 256×256 yüklemenin `mip_level_count == 9` bildirdiğini
iddia eden headless test; 20.000 instance spawn edip hiçbirinin düşmediğini iddia eden test
(bugün başarısız).

---

### M2 — Bir bölge ekranda, gölgeleriyle · 10-14 gün

**Hedef:** `STREAML4RH` 60 fps'te, doğru dokulanmış, sisli, arka yüz culling'li render
oluyor — **ve gölge düşürüp gölge alıyor.**

**Oyun işi** — `nfsu2-gizmo/game/src/world/`, `car/`'ın **yanında**, asla içinden geçmeden
- Şehir meshlerini `car::build_car_visuals`'tan geçirme. `parts::group_of` araba kelime dağarcığıyla
  eşleşiyor: bina `ROOF`/`DOOR` → `Grp::Paint` (arabanın palet rengine boyanır),
  `WINDOW`/`GLASS` → 0.32 alfada yarı saydam, gerisi neredeyse siyah `Grp::Trim`.
  `car::shader::shader_group` burada işlevsiz — `0x00134013` L4RA'da **1.610 solid'e karşı
  25 kez** geçiyor. `skin::texture_for_name`'in 16 karakterlik ortak-önek eşlemesi
  `TRN_ROADA_CHOP_*`'u şehir genelinde alakasız meshlere bağlar.
- Genelleşen **tek** araba mekanizması: `0x00134012` slot hash'i ∩ bölge TPK anahtar kümesi, ve
  doku başına run'ları tek meshte birleştirme. `geom::build_mesh_items`, `geom::remap`
  (det = +1, yani winding korunur ve şehir nihayet arka yüz culling yapabilir), `scene::Textures`
  aynen kullanılır.
- **Hücre başına yeniden merkezleme.** `scene::spawn_body` `Transform::new(Vec3::ZERO)`'da
  spawn ediyor; dünya-uzayına pişmiş şehir vertex'leriyle bunu yaparsan her hücrenin
  `Mesh::bounds`'u tüm şehir olur ve culling hiçbir şeyi eleyemez. `build_mesh_items`'ı
  `off = hücre_centroid` ile çağır, `Transform::new(hücre_centroid)` ile spawn et.
- **Yükleme-zamanı dedup**, geometriyi kimse görmeden önce. Anahtar
  `(doku key, 3 eksen bbox, üçgen, vertex)` — farklı konumdaki instance prop hayatta kalsın.
  Sonra anahtarı hayatta kalanlar üzerinde tekrar koştur ve artık == 0 iddia et.
- (256 m hücre, doku) başına tek meshte birleştir. Doku zinciri: bölge TPK → paylaşılan
  `TRACKS/LOC4DYNTEX.BIN` (bu **mevcut** sıkıştırılmış `0x33310003` varyantı) → komşu bölge.
- **Atılacak:** `NFS_REGION` + `NFS_RADIUS` env filtresi. M5'te silinir.
- Yeni `nfs_city` binary'si, headless çalışabilir. near/far'ı görünür kümeden türet
  (`nfs_shot.rs:177-182` gibi), `nfs_race.rs:259`'un 0.1/4000'i **değil**.

**Motor işi**
- **G1 — point-shadow kapısı.** `crates/gizmo/src/systems/render/passes/shadow.rs:56`'daki
  6 yüz döngüsünü `renderer.point_shadows_enabled`'a bağla. Alan `renderer/mod.rs:170`'te
  ve `SceneUniforms`'ta (`gpu_types.rs:143`) **zaten var**; bugün yalnız shader'da örneklemeyi
  kapatıyor, 12 render pass'i yine de koşuyor ve kimsenin örneklemediği bir cubemap'e çiziyor.
- **G2 — `MaterialType::BakedLit`.** 6. varyant (`components/material.rs:5`). Shader:
  vertex ARGB albedo × güneş × CSM gölge terimi, tek forward pass; deferred PBR'a ve 10 point
  light'a girmez. Shadow pass'teki `if item.unlit` atlaması `BakedLit`'i **atlamaz** — şehir
  cascade'lere düşürür. `unlit.wgsl` gölge grubu taşımıyor, bu ayrı bir shader.
- **G3 — cascade başına caster culling.** `cascade_vp: [Mat4; 4]` zaten `collect_draw_items`'a
  geçiyor (`batching.rs:181`); eksik olan, hücre bbox'larını her cascade'in ortho frustum'una
  karşı test edip caster listesini hücre bazında kısmak. `SHADOW_DISTANCE = 100.0` sayesinde
  bu, caster kümesini kameranın 100 m'siyle sınırlar.
- **Opak unlit pipeline.** `pipelines.rs` `unlit`'i `cull_mode: None` + `ALPHA_BLENDING` ile
  kuruyor; `unlit_opaque` varyantı ekle (`Face::Back`, `blend: None`) — şehir genelinde
  ~2× daha az fill.
- **Veriye dayalı sis, her pass'te.** `compute_height_fog` şu an `deferred_lighting.wgsl`'de
  gömülü ve yalnız deferred pass'te. Sis rengi/yoğunluğu/yükseklik alanlarını `SceneUniforms`'a
  ekle (`gpu_types.rs:122-153`'te hiç yok), fonksiyonu `shaders/common.wgsl`'e taşı (naga_oil
  `gizmo::common` modülü var ve `unlit.wgsl` ondan zaten import ediyor), `BakedLit`, `unlit`,
  `shader`, saydam yol ve gökyüzünde uygula.

**Kanıt:** üç sabit kamera pozundan commit'lenmiş golden ekran görüntüleri (`nfs_shot.rs`
readback). Batch/draw/instance/CPU cull ms bildiren bir kare-istatistik satırı; draw sayısının
solid sayısı değil birleştirilmiş (hücre × doku) sayısı olduğunu ve kırpma olmadığını iddia
etsin. 175 obje için 1080p'de 8 ms altı. **Gölge kanıtı:** arabayı bir binanın gölgesine sür,
gövdenin karardığını gör; arabayı yola park et, gölgesinin asfalta düştüğünü gör. G1 öncesi/
sonrası draw sayısı 23n → 11n → ~5n olarak kaydedilsin.

---

### M3 — Üstünde sür · 6-9 gün

**Hedef:** araba gerçek bölge geometrisi üzerinde sürüyor — yollar, kaldırımlar, bariyerler,
çok katlı üstgeçitler — stabil 60 fps'te.

**Oyun işi**
- **Önce yinelemeyi topla.** `nfs_drive.rs` ve `nfs_race.rs` ~230 satır neredeyse aynı kodu
  paylaşıyor. Bu zaten canlı bir hataya mal oluyor: `nfs_race.rs:344` `RigidBody::new(1200.0, true)`
  sabitliyor, `nfs_drive.rs:246` ise `tune.mass_kg`'ı onurlandırıyor — ve `nfs_race.rs:390`
  arabanın kullanmadığı kütleyi yazdırıyor. Direksiyon kilidi iki yerde
  (`:404` fizik, `:436` görsel). `game/src/rig/`'e çıkar. Aşağıdaki her milestone bunun
  maliyetini ikiye katlar.
- Dünya hücresi başına bir `Collider::trimesh` (~128-256 m) — tek şehir meshi değil (şehir
  boyutlu AABB broadphase'de her dinamik gövdeyle eşleşir), solid başına da değil.
- **Bariyer sınıflandırması oyun politikasıdır.** OpenUG korkulukların ayrı mesh olmadığını,
  yol/arazi chunk'larına pişmiş neredeyse dikey üçgenler olduğunu ölçmüş (|Nz| < 0.30, sürülebilir
  asfaltta ≈0.98). Her hücrenin üçgenlerini sürülebilir/duvar diye ayır, ayrı `PhysicsMaterial`
  ile etiketle.
- Üstgeçit ayrımı oyunda kalır: arabanın mevcut Y'sini referans yükseklik olarak geç ve en
  yakın yüzeyi tercih et. NFSU2'nun şehri katlı; naif aşağı ışın yanlış katı seçer.
- Sonsuz zemin düzlemini sil (`nfs_race.rs:161-176`); bölge bbox tabanının altına düşünce en
  yakın rota düğümüne respawn.
- Hijyen: `state.wheel_spin`'i 2π'ye sar (bir saat sonra ~3.2e5 rad → ~1.8° f32 titremesi);
  `std::env::var("NFS_DIAG")`'ı kare başına yoldan çıkar (`:562`).

**Motor işi**
- **Trimesh AABB'sini önbelleğe al.** `TriMeshShape`'e bir `Aabb` sakla, BVH ile birlikte
  hesapla; `compute_aabb`'nin TriMesh kolu (`collider.rs:44-60`, bugün her vertex'i tek tek
  dönüştürüyor) 8 köşe döndürmeye iner. **~20 satır**, ve 17 fps ile 60 fps arasındaki fark.
- **`update_vehicle`'a sorgu tutamacı ver.** `vehicle/dynamics.rs:306-326` tekerlek başına,
  adım başına `all_colliders: &[(BodyHandle, Transform, Collider)]` üzerinde doğrusal tarama
  yapıyor, ve `gather_colliders` o Vec'i her çağrıda her collider'ı klonlayarak yeniden kuruyor.
  `PhysicsWorld::raycast_excluding` doğru şeyi zaten yapıyor ve kendi testinden başka hiçbir
  yerden çağrılmıyor. **Tek engel imza.**
- `Ray`'e `max_distance` ekle (yok) ve raycast yoluna collision-layer maskesi (maskeler yalnız
  narrowphase çiftlerinde var). Bunlar olmadan tekerlek ışını rakibin şasi kutusuna basar ve
  süspansiyon arabayı onun üstüne kaldırır. **M4 bunu görünür kılmadan önce ekle.**
- Statikleri her substep yeniden ekleme: `broadphase_step` `DynamicAabbTree`'yi `clear()`'layıp
  yeniden kuruyor, şişirilmiş-AABB tasarımının bütün varlık sebebini çöpe atıyor. Rewind
  halkasını da sınırla — ~3.000 statik şehir gövdesinde 600 kare ≈ 115 MB, hiç kıpırdamayan
  geometri için.
- **`NarrowPhase::shape_trimesh`'te iç kenar işleme.** Üçgen başına GJK en-yakın-öznitelik
  normali döndürüyor, yani iki düzlemsel yol dörtgeninin paylaştığı kenarı geçen bir kutu
  sahte, kenara dik bir temas alıyor. 4-en-derin kırpması bunu düşük hızda maskeliyor,
  200 km/s'te maskelemeyecek.
- `TriMeshShape` üzerinde submesh başına yüzey materyali — `surface_friction` lastiğe kadar
  akıyor ve okuyacak bir şeyi yok, çünkü collider başına tek materyal var.
- `BvhTree::build`'i thread dışına al — 400k üçgende çağıran thread'de 250 ms.

**Kanıt:** `grep -rl TriMesh crates/gizmo-physics-rigid crates/gizmo-physics-dynamics` bugün
yalnız `rigid_body.rs` dönüyor. Ekle: (a) statik trimesh üzerine kutu düşürüp durduğunu iddia
eden `PhysicsWorld` testi; (b) `VehicleController`'ı tesselatlanmış çok-dörtgenli şerit üzerinde
sürüp dikişlerde yanal impuls sıçraması olmadığını iddia eden test; (c) `compute_aabb`'nin
200k üçgende 20k'ya göre sabit olduğunu iddia eden criterion bench; (d) L4RH üzerinde 60 saniyelik
headless otosürüş, p99 kare süresi 16.6 ms altı. Görsel: aynı üstgeçidin altından ve üstünden
geç, tekerlek ışınının iki seferde de doğru katı seçtiğini doğrula. `README.md:41-43`'ü güncelle —
istediği box-vs-trimesh düzeltmesi `aa4a5d9`'da inmiş.

---

### M4 — Üstünde yarış · 10-14 gün

**Hedef:** Menü → Geri sayım → Yarış → Sonuç döngüsü, gerçek şehirde 4001 etkinliğiyle,
türetilmiş kapılar, türetilmiş bariyerler ve beş rakiple, determinist olarak tekrar oynanabilir.

**Oyun işi**
- **Yol ağını rota verisinden kur.** Gönderilen dosyalar **etkinlik başına** polyline, yol ağı
  değil: OpenUG düğüm 0'dan rota-içi kenarlar üzerinden BFS'in 15.257 düğümün **9'una** (%0.1)
  ulaştığını ölçmüş. Kurulum: ardışık düğümleri eşik altında bağla, uzamsal hash ile çakışan
  düğümleri kaynaştır, CSR'a düzleştir. Buradaki her sabit bir yargı çağrısı ve
  `game/src/world/`'de yaşar.
- **M1'in okuduğu `+12/+14/+16` link alanlarını yakınlık grafiğine karşı çapraz doğrula.**
  OpenUG'un `world_load_nav`'ı bu alanları **hiç okumuyor**, 120 m eşiğiyle kaynaştırıyor.
  Bu karşılaştırma deneyin kendisi, ve referansı gerçekten aşabileceğin en net yer.
- **Kapılar `0x3414c` etkinlik outline'ından**: her outline köşesini *bu etkinliğin* yarış
  hattının en yakın düğümüne snap et (OpenUG 2-23 m ölçmüş), polygon boyunca merkezî farkla
  gidiş yönüne dikleştir, yarı-genişlik ver. `0x34146 TrackPosMarkers` checkpoint listesi
  **değil** — OpenUG bu okumayı geri çekti; 4001'in 18 kaydı ~15 × 30 m'lik bir yamada, yani
  iki başlangıç gridi. Onları grid ve yön için kullan.
- **`nfs_race.rs:543-558`'i tamamen değiştir.** Bugün: 15 m'lik şeridin centerline'ı etrafında
  9 m'lik disk, yön testi yok — kenardan giden araba kapıyı ıskalayabilir, geri giden yeniden
  tetikler. Yerine: önceki XY'yi sakla, prev→cur'un kapı çizgisini kesip kesmediğini **ve**
  geçişin kapı yönüyle iç çarpımının pozitif olduğunu test et. ~15 satır, üç kusuru da bitirir.
- **Bariyerler eksiklikten.** `TRACKS/L4R*.BUN`, `GLOBAL/InGame*.bun` ve her ROUTES dosyası
  üzerinde chunk sayımı hiçbir bariyer chunk'ı bulmuyor (`0x0003410B` hiçbir dosyada yok) —
  bu **ölçülmüş bir olumsuz sonuç** ve türetmenin tembellik değil doğru yol olmasının sebebi.
- **Kinematik rakipler, simüle değil.** 240 Hz'de beş `VehicleController` oyuncunun 5 katına
  mal olur ve kimse farkı anlamaz. Waypoint imleci, hız sınırlı direksiyon, öndeki virajdan
  köşe-farkında tempo, hedef hıza küçük bir çarpan olarak lastik bandı, yüksekliği lerp'leyerek
  zemin takibi, **sarmalanmış ilerleme** ile tur sayımı. Saf takip çekirdeği
  `nfs_race.rs:479-502`'deki otosürüşte zaten var.

**Motor işi**
- **State machine'i bağla.** `State<S>`'in `set`, `apply_transitions` ve `in_state` koşulu var,
  prelude'dan dışa aktarılmış — ve `apply_transitions`'ın **workspace genelinde sıfır çağıranı**
  var, yani `State::set` kalıcı bir no-op. `Phase::PreUpdate`'te bir `state_transition_system`,
  `OnEnter(S)`/`OnExit(S)` tek-atışlık schedule'lar, ve `World::despawn_all_with::<C>()` üzerinde
  state kapsamlı despawn ekle. Scheduler koşul erişimlerini zaten doğru birleştiriyor.
  **Listedeki en ucuz yüksek-değerli düzeltme.**
- **Fixed/variable schedule ayrımı.** `gizmo-app/src/windowed/event.rs:429` **tüm** beş fazlı
  `Schedule`'ı — `Update` ve `Render` dahil — sabit fizik adımı başına, kare başına 8 kereye
  kadar koşuyor. Yarış döngüsündeki idempotent olmayan her şey yük altında çoklu tetikleniyor:
  tur sayaçları, geri sayım bipleri, kapı geçişleri, ses tetikleyicileri. `PhysicsTime::alpha()`
  interpolasyon yarısı için zaten var. Not: `Phase` kapalı 5 varyantlı bir enum.
- **Arc-length parametrizasyonlu `Path`/`Polyline`** (`gizmo-math`): `sample(s)`,
  `lookahead(s, d)`, `project(point) -> (s, lateral)`, kapalı/açık. Benzeri hiç yok.
- **`NavGraph::from_nodes_edges`** (`gizmo-ai`) — **yönlü, maskelenebilir** kenarlar + CSR + A*.
  Bugün navigasyon yalnızca statik collider AABB'lerini rasterleyerek *türetilebiliyor* ve
  `NavGrid.neighbors` yapı gereği simetrik, yani bariyer-eksiklikten koridor modeli hiç ifade
  edilemiyor.
- **Araç şeklinde `PathFollower`** — `steer_input`/`throttle_input`/`brake_input` üreten.
  `ai_navigation_system` `Velocity.linear` yazıp Y'yi sıfırlıyor ve `max_speed`'e kırpıyor;
  raycast `VehicleController` ile kavga eder. `NavAgent`'a dönüş yarıçapı/dingil mesafesi ekle.

**Kanıt:** headless determinist tekrar (motorda `state_hash`, çapraz-süreç oracle ve girdi
kayıt/oynatma zaten var): 4001 etkinliğinde 5 rakiple 3 tur, kayıtlı girdi akışından; tam tur
sayısı, tam kapı sırası, atlanmış kapı yok, silahlı kapıdan geri gitmenin puan vermediği, ve
commit'lenmiş bir son `state_hash` iddia edilsin. Birim testler: arc-length gidiş-dönüşü;
yönlü `NavGraph` ters kenarı reddediyor; maskelenmiş kenar rotayı ulaşılmaz yapıyor;
`apply_transitions` + `OnEnter` geçiş başına tam bir kez tetikleniyor; zorlanmış 8-substep
takılmasında bir `Update` sayacı sekiz değil bir artıyor.

---

### M5 — Tüm şehir, streamed · 12-18 gün

**Hedef:** Bayview'ın tamamında serbest dolaşım — her bölge, araba etrafında yüklenip
boşaltılan — sınırlı RSS, VRAM ve entity sayısıyla, bölge sınırında takılma olmadan.

**Oyun işi**
- `NFS_RADIUS` atılır. `manifest()` ile vertex çözmeden bölge başına obje indeksi kur, objeleri
  başlık bbox'larından motor hücrelerine ata, M2 dedup'ını hücre atamasından önce 8 bundle'ın
  tamamında koştur. Bölge birleştirme concatenation — bundle'lar tek dünya koordinat sistemini
  paylaşıyor ve komşu döşemeler değil, **yarış rotası başına örtüşen üst kümeler**.
- **İki ızgara, iki amaç, farklı boyut**: batch'leme için ~256 m, zemin/duvar sorguları için
  ~64 m. Tek ızgarayı paylaştırma.
- Pikseller GPU'ya çıkar çıkmaz bölge dosya buffer'ını serbest bırak (`STREAML4RD` 120 MB).

**Motor işi**
- **Önce render-cache sızıntısını düzelt — bu olmadan hiçbir şey çalışmaz.** `RenderCache.batches`
  (`batching.rs:13`, bir `thread_local!`) her batch'in instance vektörlerini temizliyor ve boş
  batch'leri draw listesinden eliyor ama **kayıtları hiç silmiyor**, yani çizilmiş her
  `Arc<wgpu::Buffer>`/`Arc<wgpu::BindGroup>` sonsuza dek sabitleniyor ve
  `AssetManager::garbage_collect` hiçbir şeyi geri alamıyor. `BatchData`'ya kare damgası +
  bayatlıkta `retain()` ekle. **Aynı commit'te**, `BatchKey.vbuf_id`/`mat_id`'yi (ham
  `Arc::as_ptr as usize`) kararlı id'lerle değiştir — buffer'lar gerçekten serbest kalınca
  ayırıcı aynı adresi yeni bir mesh'e verir ve bayat anahtar takma ad yapar.
- **Index buffer + kompakt statik vertex düzeni.** `Mesh`'e `ibuf` + `index_count` +
  `index_format`; beş kayıt noktasının hepsinde `draw_indexed`. Sahne yolunda bugün hiç
  `set_index_buffer` yok. `StaticVertex { pos: Float32x3, color: Unorm8x4, uv: Float32x2 }` =
  **24 bayt, STREAM kaydıyla bayt-özdeş**; `Unorm8x4` şu an atılan ARGB alfasını da geri
  kazandırır. **Kesinlikle eklemeli** — iki test 92 baytlık düzeni kilitliyor. Net ~11× daha az
  vertex belleği.
- **`Cell`/`Region` kavramı + renderable'lar üzerinde uzamsal indeks.** Gizmo'da bugün hiçbir
  hücre/chunk/bölge kavramı **yok** (`grep Bvh|Octree|quadtree|grid|stream|chunk|tile|sector`
  yalnız bir SoA `iter_chunks` ve rodio'nun `OutputStream`'ini buluyor). Önce hücreleri cull et,
  sonra yalnız hayatta kalanların entity'lerini gez — bu, `batching.rs:212-328`'in tek thread'li
  O(N-entity) taramasını O(N-görünür)'e çevirir. `Frustum::test_aabb_masked` tam bunun için
  yazılmış ve sıfır çağıranı var. `gizmo-physics-core/src/bvh.rs` bir **üçgen** BVH'sidir —
  incele, yeniden kullanma.
- (hücre, materyal) başına statik meshleri yüklemede tek indexed VBO+IBO'ya birleştir.
- **Async loader'ı genelleştir.** `async_assets.rs` kapalı 3 varyantlı bir `Job` enum'u
  (`Texture|Obj|Gltf`). Trait tabanlı `AssetLoader`/`AssetSource` + `Handle<T>` üzerinde
  `LoadState` + kare başına bütçe + boşaltmada iptal. Worker thread, `sync_channel(64)`,
  `drain_completed()` ve magenta placeholder hepsi var — engel enum.
- **BC doku yükleme.** `Features::TEXTURE_COMPRESSION_BC` iste ve `Bc1RgbaUnormSrgb`/
  `Bc2RgbaUnormSrgb` alan bir blok yükleme girişi ekle. NFSU2 DXT1/DXT3 gönderiyor; blokları
  olduğu gibi yüklemek 4-8× daha az VRAM ve loader'dan iş **çıkarır**.

**Kanıt:** soak testi — üç bölge sınırını geçen 5 dakikalık scripted rotada headless otosürüş,
5 saniyede bir örnekleyerek yerleşik hücre, entity sayısı, `RenderCache.batches.len()`, GPU
buffer sayısı ve RSS'in bir bantta kaldığını iddia et (`batches.len()` bugün yapı gereği
monoton artıyor). Yükle→çiz→boşalt→`garbage_collect` testi canlı buffer sayısının tabana
döndüğünü iddia etsin (bugün imkânsız). Cull benchmark'ı: 400 hücrede 20.000 entity, kare başına
cull süresi toplam entity'yle değil görünür hücreyle ölçeklensin. `/usr/bin/time -v` tepe RSS
6 GB altı.

---

### M6 — His ve ses · 6-9 gün

**Hedef:** çıkışta lastik yakabilir, viraja el freniyle girebilir ve gerçek bir RPM güdümlü
motor süpürmesi duyabilirsin.

**Oyun işi**
- El freni ayrı bağlanır, drift durumu tekerlek kayma açısından, drift süresi HUD'da, yanal
  kayma sinyalinden fren izi decal'ları, pipeline'ın zaten ürettiği `CollisionEvent`'ten çarpma sesi.
- `.gin` EA-XAS yükünü `Vec<i16>`'ya çöz (format işi → PryHUB, sentetik birim testler +
  `NFSU2_ROOT` golden'ı), `_DCL` yavaşlama eşlemesiyle. **Grain politikası** — döngü penceresi
  içinde `rpm / rpm_at(grain)` ile ilerleyen kesirli okuma imleci, ~12 ms'de çapraz geçen ikinci
  imleç, gaz yüküne göre hızlanma/yavaşlama karışımı — NFSU2 politikasıdır, `nfsu2-gizmo`'da kalır.
- `car/tune.rs`'in **her** araba için `..VehicleTuning::default()` miras almasını durdur:
  frenler (1500 N·m), aerodinamik (Cd 0.32 / Cl −0.8 / A 2.2 m²), anti-roll (3000). Ve
  `nfs_race.rs:360-371`'in aynı süspansiyon/Pacejka parametrelerini sabitlemesini durdur.
  Türetilebileni türet, uydurulanı dosyanın mevcut üslubuyla belgele — `tune.rs:215`'teki
  `STEERING_LOCK_RAD` notu izlenecek model.

**Motor işi**
- `gizmo-physics-dynamics`: **el freni** (tamamen yok — ve bu oyunun bütün hissi o);
  **`tc_enabled` bayrağı** (`dynamics.rs:600-618`'deki hep-açık kırpma bir kontrolcü değil,
  bir kırpma, ve tekerlek kaydırmayı kelimenin tam anlamıyla imkânsız kılıyor);
  **diferansiyel** (`Drivetrain::drives` bir boolean ve tork eşit bölünüyor — açık/LSD yok,
  AWD dağılımı yok); fren dengesi `VehicleTuning`'e (sabitlenmiş 60/40 yerine); ayrı ön/arka
  anti-roll oranları. Ayrıca motorun kendi Ackermann test koşumunu düzelt — `dynamics.rs:748`
  `is_left: x > 0.0` kullanıyor, oyun ise doğru olarak `mount.x < 0.0` (`nfs_race.rs:359`),
  yani motorun testleri aynalanmış bir arabayı sınıyor.
- `gizmo-audio` (533 satır, tek dosya, rodio 0.17 üzerinde): **`load_pcm(...)`** —
  `load_sound_bytes` tam çözülebilir bir konteyner istiyor, yani kendi çözdüğün buffer hiç
  çalınamıyor. **Alt-buffer döngü bölgeleri ve seek** — yalnız tüm kaynak üzerinde
  `repeat_infinite()` var. **Örnek-hassas `crossfade_to(sink, ms)`** — kare başına `set_volume`
  ile ~60 Hz kazanç basamağı ve her grain sıçramasında duyulur bir fermuar sesi; gerçek
  çözüm bir DSP/`Source` kancası, ki bu aynı zamanda kabul edilmiş sahte alçak geçiren filtreyi
  (volume×0.4 + speed×0.85) de nihayet değiştirir. **Mixer bus'ları**, ses limitleme/öncelik,
  müzik için diskten streaming, seçilebilir mesafe modeli. Bu zaten `docs/ENGINE.md` M7.5.
  `set_pitch`'i koru — resample tarzı, bir RPM süpürmesi için doğru semantik.

**Kanıt:** çevrimdışı render testi — sentetik 800→7500→800 rpm rampası grain oynatıcısından
WAV'a; (a) hiçbir grain sınırında eşik üstü örnekten-örneğe süreksizlik olmadığını (bugün yapı
gereği başarısız), (b) baskın spektral tepenin komut edilen RPM'i takip ettiğini iddia et.
`tc_enabled` açık/kapalı A/B tekrarı, çekiş tekerleği ω'sının yalnız kapalıyken `v_long/r`'den
ayrıldığını iddia etsin. El freni testi: aynı direksiyon girdisinde arka yanal kuvvet düşsün,
yaw hızı artsın.

---

### M7 — Ölçeğe çık · 14-18 gün

**Hedef:** tüm şehir 20.000-30.000 objede ve yüzlerce yerel ışıklı bir gece merkezi, kare
süresi nesne sayısından bağımsız.

**Oyun işi:** bölge manifest'inden içe aktarma zamanında LOD zincirleri ve hücre başına HLOD
proxy'leri / bina impostor'ları üret. Işıklı tabelaları unlit + bloom yolundan geçir; gerçek
clustered ışığı farlar, stop lambaları ve sokak lambalarına sakla.

**Motor işi**
- **Zaten inşa edilmiş GPU cull'u dispatch et.** `gpu_cull.rs` tam bir compute-frustum-cull →
  indirect-args sistemi (`MeshBoundsRaw`, `DrawIndirectArgs`, `mesh_cull.wgsl`, `prepare()`,
  `cull_pass()`, `indirect_offset()`), her native renderer'da 8192 kapasiteyle kuruluyor ve hiç
  çağrılmıyor — `systems/render/mod.rs:426` "GPU cull pass removed since we use CPU instancing"
  diyor. Eksik: `DrawIndirectArgs` 16 baytlık non-indexed düzen, `base_vertex`'li 20 baytlık
  indexed varyanta ihtiyaç var; shader yalnız küre-vs-frustum. `gpu_physics/system.rs:699`
  bu kod tabanında `draw_indexed_indirect`'i zaten gösteriyor.
- **HZB occlusion culling.** Hiç yok — her `RenderPassDescriptor` `occlusion_query_set: None`
  geçiyor. **Mevcut** Z-prepass üzerine kur → derinlik piramidi → sınırları GPU cull'un aynı
  compute shader'ında test et. Neredeyse tüm tesisatı paylaşıyorlar, birlikte gelmelerinin sebebi
  bu. (NFSU2 kendi ön-hesaplanmış görünürlüğünü göndermiş — `TRACKS/PrecullerBooBooScript.hoo`
  hâlâ kurulumda.)
- **Gerçek LOD.** `batching.rs:259-276` LOD1'i yalnız `!mesh.lod_vbufs.is_empty()` iken seçiyor,
  ve o alan yalnız 20.000 vertex üstünde, tek seviyede, %50 meshopt sadeleştirmeyle, **düz
  non-indexed buffer'a yeniden açılarak** dolduruluyor — ki bu M5'in indexlemesini geri alır.
  Zincirleri vertex sayısından bağımsız doldur, indexed tut, mesafe cull'u ekle (yok),
  `dist > world_r * 15.0` sezgiselliğini ekran-uzayı hatasıyla değiştir, dithered çapraz geçiş ekle.
- **Clustered/tiled ışıklar.** `SceneUniforms.lights` `[LightData; 10]` ve tam bir point-shadow
  caster. On, bir gece şehri için demo sayısı. Froxel ızgarası → ışık indeksi storage buffer'ı →
  cluster başına döngü.
- **G4 — statik cascade önbelleği** (§2). Şehir kıpırdamıyor; uzak cascade'ler yalnız yeniden
  merkezlenince değişir. Kare başına yalnız dinamik caster yeniden çizilir.
- Şehrin açığa çıkardığı sabit maliyetleri kırp: G-buffer `world_position`'ı tam bir
  `Rgba16Float` MRT olarak saklıyor (~8 B/px) ve derinlik + zaten yüklenmiş `inv_view_proj`'ten
  yeniden kurulabilir; bloom dört **tam çözünürlük** pass koşuyor; CSM 4 × 3072² Depth32 =
  151 MB, 100 m gölge mesafesi için.

**Kanıt:** commit'lenmiş ölçekleme benchmark'ı — sabit kameradan 1.000 / 10.000 / 30.000 obje,
CPU kare süresinin süpürme boyunca düz olduğunu iddia etsin (bugün doğrusal, ~0.5-2 µs/entity).
Occlusion testi: kamera yoğun bir sahnede kapalı bir kutunun içinde, draw sayısı sıfıra yakın
çöksün. M2'nin golden ekran görüntüleri indirect yoldan yeniden render edilip tolerans içinde
diff'lensin — GPU güdümlü yol CPU yoluyla piksel-eşdeğer olmalı.

---

## 5. Gizmo'nun kazanacakları

NFSU2'dan tamamen bağımsız değerli olanlar:

| özellik | crate | neden genel amaçlı bir motor için değerli |
|---|---|---|
| `TriMeshShape` üzerinde önbelleklenmiş AABB | `gizmo-physics-core` | ~20 satır, ve statik mesh collider'ları her oyun için broadphase'de bedava yapıyor. Beş raporun tamamındaki en büyük ölçülmüş performans düzeltmesi; `DynamicAabbTree`'nin şişirilmiş AABB'lerinin bütün varlık sebebini geri getiriyor |
| `MaterialType::BakedLit` — baked ışık + dinamik gölge | `gizmo-renderer` | Statik seviyeli **her** oyunun istediği malzeme. Deferred PBR'ın maliyetini ödemeden gölge alıp veren geometri |
| Cascade başına caster culling | `gizmo-renderer` | Gölge kullanan her sahne bunu ister. Bugün gölge pass'i mesafeye bakmadan her draw item'ı dört cascade'e çiziyor |
| `update_vehicle` (ve `update_character`) için broadphase destekli sorgu tutamacı | `gizmo-physics-dynamics` × `-rigid` | Her raycast süspansiyonlu araç, karakter kontrolcüsü (`character.rs:72`'de aynı doğrusal tarama) ve gameplay probu dünyanın anlık görüntüsünü değil bir sorgu tutamacı ister. `Ray::max_distance` ve raycast'te layer maskesini de açar |
| Trimesh temaslarında submesh başına yüzey materyali + iç kenar işleme | `gizmo-physics-core` | Karışık yüzeyli statik geometri ve dikişsiz temas normalleri, mesh collider'dan sürüş/kayma/yürüme yapan her oyunun istediği şey. Bullet'ın `btInternalEdgeUtility`'si aynı sebepten var |
| Index buffer + 24 B `StaticVertex` | `gizmo-renderer` | Indexed çizim, gerçek içerik yükleyen her motor için asgari şart; mevcut 92 baytın 44'ü hiçbir statik mesh'in kullanmadığı joint index/weight. `Unorm8x4` ayrıca atılan vertex alfasını geri kazandırır |
| Kararlı mesh/materyal id'leriyle render-cache tahliyesi | `crates/gizmo` | Renderer çizdiğini sabitlemeyi bırakmadan hiçbir motor **hiçbir şeyi** stream edemez. `Arc::as_ptr` anahtarı, buffer'lar serbest kaldığı anda canlanacak gizli bir takma ad hatası |
| `Cell`/`Region` kavramı + uzamsal indeks, önce-hücre culling | yeni `gizmo-world` ya da `gizmo-scene` | **Bugün Gizmo'da hiçbir hücre/chunk/bölge kavramı yok.** En büyük eksik motor yeteneği ve `docs/ENGINE.md` yol haritasında **olmayan** tek şehir engeli. LOD, HLOD, occlusion ve fizik chunk'lamayı hep bu açar |
| Her pass'te veriye dayalı sis | `gizmo-renderer` | `SceneUniforms`'ta sis alanı yok, `FogSettings` hiç yok. Satır başına en büyük görsel kazanç, ve gökyüzü/saydam/unlit geometriyi ışıklıyla aynı fikre getirir |
| Mipmap, anizotropi, paylaşılan sampler, BC blok yükleme | `gizmo-renderer` | `generate_mipmaps` zaten var, çalışıyor, testli — sadece her asset yolu onu atlıyor. Satır başına en yüksek görünür kalite; BC loader'dan iş **çıkarır**; paylaşılan sampler ~1000 doku civarındaki sert sürücü duvarını kaldırır |
| Arc-length parametrizasyonlu `Path`/`Polyline` | `gizmo-math` | Kamera rayları, devriye rotaları, konveyörler, kesit sahne track'leri, dolly çekimleri, mermi yayları — hepsi aynı tipi ister ve bugün hiçbiri kurulamaz |
| `NavGraph` — authored, yönlü, maskelenebilir, CSR | `gizmo-ai` | Elle yazılmış navigasyonu olan her oyun (ray shooter, RTS şeritleri, trafik, tek yönlü kapılar, dinamik kapatılan rotalar) yönlü maskelenebilir kenar ister. Yalnız-türetilmiş navigasyon güçlü bir kısıt |
| Araç şeklinde `PathFollower` | `gizmo-ai` × `-dynamics` | Holonomik olmayan yol takibi, süren/dönüş yarıçapıyla uçan/tekne süren her AI'nin ihtiyacı. Motor bugün yalnız bir noktaya doğru kaydırmayı biliyor |
| Canlı state machine: `apply_transitions` sürücüsü, `OnEnter`/`OnExit` | `gizmo-core` | Scheduler koşul erişimlerini zaten doğru birleştiriyor, yani yalnız sürücü eksik. Denetimdeki en ucuz yüksek-değerli düzeltme, ve her oyun buna muhtaç |
| Fixed/variable schedule ayrımı | `gizmo-app` | Gizmo üzerindeki her oyunun idempotent olmayan her gameplay sistemi için bir doğruluk hatası, yalnız bunun için değil |
| `LoadState` ve iptalli trait tabanlı async loader | `gizmo-core` + `-renderer` | `docs/ENGINE.md`'de zaten M7.7. Worker thread, iş kuyruğu, `drain_completed()` ve placeholder mekaniği hepsi var — engel yalnız kapalı enum |
| PCM kaynakları, döngü bölgeleri, seek, örnek-hassas crossfade, mixer bus'ları | `gizmo-audio` | Zaten M7.5. Ham PCM alımı + bir mixer, prosedürel/yeniden sentezlenmiş/üretilmiş her sesin ihtiyacı |
| GPU güdümlü culling + HZB occlusion + gerçek LOD zincirleri | `gizmo-renderer` | `gpu_cull.rs` ağaçta zaten GPU güdümlü bir renderer'ın ~%70'i, ve HZB'nin ihtiyaç duyduğu Z-prepass zaten kaydediliyor. Bu, yeni iş başlatmak değil, başlanmış işi bitirmek |

---

## 6. Motor/oyun sınırı

Üç katman, üçü için farklı kural.

**`Gizmo` NFSU2 hakkında hiçbir şey öğrenmez.** Chunk id yok, dosya adı yok, format sabiti yok,
kelime dağarcığı yok. Bir Gizmo commit'inde "NFSU2", "Bayview", "STREAM" ya da dört baytlık bir
magic geçiyorsa, değişiklik yanlış repodadır.

**`gizmo-nfs` yalnız bayt düzeyinde gerçekleri tutar, ve yalnız kuruluma karşı ölçülmüş
olanları.** Dosyanın ne dediğini ve neyi okuyamadığını bildirir; karar vermez. Somut olarak
şunların hiçbiri giremez: kapı yarı-genişliği, nav link/weld eşikleri, bariyer erişimi, terrain
z-bias'ı, dedup anahtarı, dosya boyutuna göre ana-bölge doku yedeği, "yatay açıklığı ≥ 3 m ise
prop katıdır" sezgiselliği. Ayrıca: **okumadığını iddia etmez** — submesh word 8 için bir shader
iddia etmek yerine `shader: AssetHash(0)` bildirir, ve 27 karakterlik kırpma için
`name_is_whole()` gönderir. Bu PryHUB'ın kendi kuralı: *"kullanıcının en çok güvendiği şey,
aletin 'bundan emin değilim' diyebilmesidir."*

**`nfsu2-gizmo/game/src/world/` her yargı çağrısının yeri**, yanında onu haklı çıkaran ölçümle.
OpenUG'un elle seçtiği her şey buraya ait: `NAV_LINK_MAX = 120`, `NAV_WELD = 5`, `GATE_HALF = 22`,
`BAR_HALF = 9`, `BAR_REACH = 40`, −5 cm terrain bias'ı, dedup anahtarı, ana-bölge yedeği,
korkuluk |Nz| < 0.30 eşiği, 64 m / 256 m ızgara boyutları.

**Oyun katmanının içinde iki sert "yeniden kullanma" kuralı**, çünkü araba yolu yeniden
kullanılabilir görünüyor ve değil:
- Şehir **`car/`'ın yanında kendi modülünü** alır, asla `car::build_car_visuals` üzerinden
  geçmez. `parts::group_of`, `car::shader::shader_group`, `skin::texture_for_name`,
  `skin::doorline_texture` ve `resolve_whole` hepsi araba-kelime-dağarcığı politikasıdır ve
  rastgele mavi boyalı çatıları olan neredeyse siyah bir şehir üretir.
- `placement::should_place` dünya objeleri için **danışılmaz**. O, araba solid'leri belirsiz
  olduğu için var; dünya verisi belirsiz değil — identity = pişmiş, değilse uygula.

**Sınır erozyonu projenin en büyük riski**, çünkü OpenUG'un yargı çağrıları format gerçeği gibi
okunuyor. Bir sabit dosyadan değil OpenUG'un kararından geldiyse `gizmo-nfs`'e geçmez, ve
kesinlikle Gizmo'ya geçmez.

---

## 7. Hafta bir

| gün | iş |
|---|---|
| **1 sabah** | `golden_assets.rs:1095`'i düzelt — mevcut skip koluna `find_chunk(&roots, 0x33320002).is_some()` ekle. Suite yeşil olmadan tek satır yeni kod inmesin; kırmızı taban sonraki her regresyonu gizler |
| **1 öğleden sonra** | Makineyi bir kez ölç: `/usr/bin/time -v cargo build --release -p gizmo-renderer` ve aynısı `nfsu2-gizmo` workspace'i için. Tepe RSS ve duvar süresini kaydet; `.cargo/config.toml`'daki `jobs = 4` hâlâ doğru mu karar ver |
| **2** | `gizmo-nfs/src/world/header.rs` — **önce header reader**, dört sentetik testle (0/4/8/12 bayt `0x11` dolgu, dördü de aynı hash/bbox/matris/isim). Makul-ama-yanlış cevabın mümkün olduğu tek yer burası; onu tüketen hiçbir şey yazılmadan test edilir |
| **3** | `ug2 world <FILE\|TRACKS-dir> --stats` — bölge başına: obje sayısı, toplam vertex, toplam üçgen, stride histogramı, dolgu histogramı, identity/placed ayrımı, **det işareti histogramı**, ve bundle içinde/arasında dedup anahtarı çakışma sayısı |
| **3 akşam** | **Sekiz bundle'ın hepsinde `/usr/bin/time -v` ile koştur**, 118 MB `STREAML4RA` ve 120 MB `STREAML4RD` dahil. Vertex toplamını **92 ve 24 ile çarp, ikisini de yaz.** Bu aritmetik indexed geometry'nin M2 problemi mi M5 problemi mi olduğunu karara bağlar ve projedeki en önemli tek ölçümdür |
| **4** | Ölçümleri evdeki üslupla golden olarak kilitle: `assert_eq!(objects.len(), 175)`, `assert_eq!((identity, placed), (169, 6))`, `assert_eq!(filler_hist, [(0,46),(4,36),(8,49),(12,44)])`. `world_manifest_never_panics` ekle. **Herhangi bir sayı keşif raporuyla çelişirse dur ve yeniden ölç** |
| **5 sabah** | Headless draw-call süpürmesi, `nfs_shot.rs:188-249` readback'i üzerinde: N = 500/2000/4000/8000/12000/20000, `collect_draw_items` içindeki CPU süresi ve toplam kare süresi loglanır, 8192'den sonra geometrinin kaybolduğu iddia edilir. Sonra aynısını `Pbr` ile — 1:23 oranı her M2 render kararının girdisi |
| **5 öğleden sonra** | Gizmo tarafı, ~2 saat, parser'dan tamamen bağımsız üç kazanç: 6 point-shadow pass'ini `renderer.point_shadows_enabled`'a bağla; doku başına sampler'ı tek paylaşılan `Arc<Sampler>`'a yükselt; `anisotropy_clamp: 16` ve mevcut `generate_mipmaps` çağrısı. `cargo clippy --workspace --all-features --all-targets -- -D warnings` ile doğrula. **PryHUB'da `cargo fmt` koşturma** |
| **hafta sonu** | `scratchpad/aabbbench`'i 3. günün gerçek üçgen sayılarıyla yeniden koştur. Elinde dört sert sayı var — RAM, vertex sayısı, draw-call tavanı, fizik AABB maliyeti — ve M1-M5 tahmine değil ölçüme karşı planlanabilir. `gizmo-nfs/README.md`'yi güncelle: `world` satırı artık kırmızı bir araştırma sınırı değil |

### Ölçülen değerler

**1. gün (2026-08-04).** `golden_assets.rs`'teki çevre-bağımlı test şekle göre tanıyacak biçimde
düzeltildi; `NFSU2_ROOT` set halinde `cargo test -p gizmo-nfs` **249 test, 0 hata**, clippy temiz.

Derleme makinesi, aynı iş yükünde (12 `gizmo-*` crate temizlenip `-p gizmo-engine` release):

| | duvar | CPU | eşzamanlı rustc RSS tepe | swap artışı |
|---|---:|---:|---:|---:|
| `jobs = 4` | 27.5 s | %369 | 2.9 GB | +443 MB |
| `jobs = 8` | 26.3 s | %485 | 2.6 GB | +1268 MB |

**Karar: `jobs = 4` kalıyor.** 8 iş duvar süresinden %4 kazandırıp swap baskısını üçe katlıyor.
Sebep RAM değil bağımlılık grafiği: 16 çekirdekte CPU %485'i geçemiyor, yani paralellik zaten
doymuş. Tek `rustc`'nin tepesi ölçümde 643-880 MB (config yorumundaki "1-2 GB" tahmininin altında),
ama sonuç değişmiyor. `Gizmo/CLAUDE.md` bu ayarlar için "Don't fix these settings" diyor ve ölçüm
onu doğruluyor — dosya değiştirilmedi.

Makinenin asıl kısıtı taban durumu: 13 GB RAM'in ~6-9 GB'si sürekli dolu ve **swap oturum boyunca
7.8 GB'den 9.2 GB'ye tırmandı**, hiç geri inmedi. Disk kısıt değil (887 GB boş; 28 GB target
dizini önemsiz). M0'ın `--stats` koşusu bu tabanın üstüne binecek — 2 GB tepe RSS hedefi buna
göre okunmalı.

**2. gün.** `gizmo-nfs::world::header` yazıldı: `0x00134011` okuyucusu, dört dolgu genişliği için
sentetik testler, ve gerçek kuruluma karşı bir golden. 255 test, clippy temiz.

Ölçüm planın kendi golden sayısını çürüttü. `(44 identity, 131 placed)` **hatalı okumanın
çıktısıymış** — dolgu atlanmadan okunduğunda çıkan rakam. Ayırt edici, matrisin formatın `1.0`'e
sabitlediği son elemanı:

| L4RH, 175 başlık | `m[15] == 1.0` | identity / placed | ilk öteleme |
|---|---:|---|---|
| dolgu atlanmış (**doğru**) | **175/175** | 169 / 6 | `(1026.5, -1703.1, 0)` |
| nominal offsetler | 46/175 | 44 / 131 | `(1.0, 0.0, 1026.5)` |

46, tam olarak dolgusuz başlıkların sayısı. İkisi de bir şehir için makul görünüyor, biri doğru —
tam olarak planın uyardığı "makul-ama-yanlış" hata modu, ve planın kendi sayısında oturuyordu.

**3. gün — projedeki en önemli ölçüm, ve cevabı iyi haber.** `world::manifest` + `ug2 world --stats`
yazıldı; 8 bundle'ın hepsinde koşuldu.

| bölge | obje | vertex | üçgen | disk @24B | genişlemiş @92B | identity | placed | neg det | kopya | kırpık isim |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| L4RA | 10.735 | 1.304.563 | 899.273 | 29.9 MB | 114.5 MB | 4.393 | 6.342 | 45 | 2.099 | 3.428 |
| L4RD | 11.135 | 1.367.985 | 941.066 | 31.3 MB | 120.0 MB | 4.406 | 6.729 | 49 | 2.652 | 3.488 |
| L4RB | 2.642 | 384.527 | 280.673 | 8.8 MB | 33.7 MB | 1.543 | 1.099 | 8 | 287 | 868 |
| L4RG | 2.271 | 245.114 | 183.538 | 5.6 MB | 21.5 MB | 1.480 | 791 | 0 | 120 | 537 |
| L4RC | 772 | 143.054 | 104.958 | 3.3 MB | 12.6 MB | 394 | 378 | 3 | 62 | 133 |
| L4RF | 773 | 135.284 | 93.526 | 3.1 MB | 11.9 MB | 49 | 724 | 1 | 140 | 41 |
| L4RR | 482 | 65.870 | 30.288 | 1.5 MB | 5.8 MB | 480 | 2 | 0 | 1 | 0 |
| L4RH | 175 | 20.610 | 10.832 | 0.5 MB | 1.8 MB | 169 | 6 | 0 | 3 | 0 |
| **toplam** | **28.985** | **3.667.007** | **2.544.154** | **83.9 MB** | **321.7 MB** | 12.914 | 16.071 | **106** | 5.364 | 8.495 |

**Karar: indexed geometry bir M5 optimizasyonu, M2 engeli değil.** Tüm şehrin vertex verisi
genişlemiş hâlde **321.7 MB** — 13 GB'lik bu makinede bile rahat sığıyor, tek bir bölge 120 MB.
CPU gölgesi (`Arc<Vec<Vec3>>`) üstüne ~44 MB koyuyor. Yani **streaming bellek için gerekli değil**;
gereken şey draw call ve culling. Risk tablosundaki "projedeki en önemli tek sayı" iyi çıktı ve
M4-önce-M5-sonra sıralaması olduğu gibi kalıyor.

Manifest'in maliyeti de sıfıra yakın: 8 bundle, 324 MB, **0.2 s ve 134 MB tepe RSS** — hem de
debug build'de. Sayaçlar 68 baytlık `0x00134900`'da, buffer'lara hiç dokunulmuyor.

Sürprizler: şehir **çoğunlukla yerleştirilmiş** (16.071 placed / 12.914 identity) — küçük arena
bölgeleri (L4RH 169/6) yanıltıcıymış. **106 negatif determinant** var, yani aynalanmış instance
riski gerçek: `placement::should_place` bunların hepsini reddedip orijine yığardı. İsimlerin
%29'u kırpılmış (8.495/28.985), o yüzden `name_is_whole()` şart. Kopya anahtarı 5.364 (%18.5) —
OpenUG'un ölçtüğü %51.5'ten düşük, ama anahtar farklı: benimki isim hash'ini içeriyor, onlarınki
doku slot'unu, yani bu daha katı bir anahtarın alt sınırı.

Doğrulanan diğer şeyler: payload boyutu her zaman `192 + dolgu`; `bStringHash(isim) == hash`
L4RH'de 175/175, L4RR'de 482/482; L4RC'de 639/772 ve fark **isim kırpılması** (raporun
"639 whole / 133 truncated"ıyla birebir), o yüzden `name_is_whole()` dünya tarafına da kondu.
Araba başlıkları ise **her zaman tam 192 bayt, sıfır dolgu** (609/610/569 solid'de) — yani
`geometry::read_matrix`'in mutlak `MATRIX_OFFSET`'i arabalar için doğru, araba yoluna dokunulmadı.

**4. gün ve sonrası — M1'in parser tarafı bitti.** Sırasıyla: `world::tpk` (şehrin kendi paket
varyantı), doku bağlaması, ve `world::object` (vertex/index tamponları).

| | ölçüm |
|---|---|
| doku paketi | **502 paket, 5.087 doku, 5.087'si çözülüyor** |
| format | **kayıtta yazıyor**, `+0x4A`: DXT1 4.464 · DXT3 532 · P8 91. `0x33310005 +0x14` ikinci kez söylüyor, çapraz tablo 5.087/5.087 diyagonal. Şehirde DXT5 yok |
| havuz tabanı | `0x33320002` payload'ının 120 baytlık dolgudan sonrası — en uzak uzantı havuzun sonuna **tam** oturuyor, 502/502 |
| paletler | imgelerden **önce**, havuzun başında. Araba yolunun `palette_at`'i bunu adıyla reddederdi (91 kaydın hepsi) |
| doku slotu | 70.439 slot, **69.150'si (%98,17) kendi bölgesinde** çözülüyor. Kalanı `LOC4DYNTEX.BIN` (araba varyantı!) ve `GLOBAL/`; 111 referans hiçbir yerde yok |
| geometri | 24 baytlık stride **doğrulanıyor, seçilmiyor** — `layout_for` önce 36'yı denerdi |

Şehirde **shader yok**: `0x00134013` 28.985 solid'in 3.440'ında var ve 70.548 run'ın 62.270'i
`0xFF` nöbetçisi taşıyor. Bu yüzden run'lar `shader: AssetHash(0)` ile dönüyor — dosyada olmayan
bir hash uydurulmuyor.

Goldenlar iki okuyucuyu birbirine karşı kontrol ediyor: aynı objeler, aynı sıra, aynı sayılar, her
paralel dizi aynı uzunlukta, her index kendi tamponunun içinde, her run böldüğü index tamponunun
içinde. Tahmin edilmiş bir stride ya da atlanmamış bir dolgu makul bir mesh üretir — bu anlaşmayı
ise bozar. İkinci bir golden pozisyonların sınırlı bir dünyaya düştüğünü ve Z'nin kısa eksen
olduğunu iddia ediyor: yanlış stride'da float'lar seve seve 1e38'e saçılır, bir şehir saçılmaz.

**Kalan:** rota dosyaları (`ROUTES*/Paths*.bin`) ve `L4R*.BUN`. Parser'ın geri kalanı hazır.

---

## 8. OpenUG `FORMATS.md` — ölçülmüş, tahmin, yanlış

| iddia | durum | ne yapmalı |
|---|---|---|
| Matris düzeni: taban satırları float 0-2/4-6/8-10, öteleme 12-14, `m[15]==1`; yol/arazi identity, proplar yerleştirilmiş | **Ölçülmüş** (2.817 obje) ve L4RH'de bağımsız doğrulandı — ama `m[15]==1` **dolgu atlandıktan sonra**: doğru okumada 175/175, nominal offsetlerde 46/175 | Güven. Bu satırın ilk hâli L4RH'yi 44/131 diye yazıyordu; o, hatalı okumanın çıktısıydı (§2. gün) |
| 8 bundle'da solid meshlerin %51.5'i kopya (14.551/28.270) | **Ölçülmüş** | Güven; sıfır-artık öz-kontrolüyle dedup uygula |
| Korkuluklar yol/arazi chunk'larına pişmiş \|Nz\| < 0.30 üçgenleri | **Ölçülmüş** | Güven; 0.30 eşiğinin kendisi bir yargı çağrısı → oyun katmanı |
| Hiçbir dosyada bariyer chunk'ı yok (`0x0003410B` yok) | **Ölçülmüş olumsuz sonuç** | Güven — bariyer türetmenin tembellik değil doğru yol olmasının sebebi |
| `0x34146 TrackPosMarkers` başlangıç gridleri, checkpoint değil | **Ölçülmüş geri çekme** | Orijinal okumaya değil geri çekmeye güven |
| Nav BFS düğüm 0'dan 15.257'nin 9'una ulaşıyor | **Ölçülmüş** | Güven — kaynaştırılmış grafiğin neden kurulması gerektiğinin sebebi |
| Yarış hattı `+12/+14/+16` alanları komşu indeksleri, `0xffff` = yok | **DOĞRULANDI (2026-08-11)** — `ug2 track`, kurulumun 105 düğüm tablosunda 54.192 indeks okuması yaptı ve **sıfırı** aralık dışına düştü. Kusur enjekte edildi (okuma iki bayt kaydırıldı): 15.931 aralık dışı, çıkış kodu 1 | Güven. `gizmo_nfs::world::routes` |
| ~~`+20` "metre cinsinden kümülatif mesafe" — ölçülebilir şekilde metre değil, delta'lar XY adımının ≈0.787 katı~~ | **BU SATIR YANLIŞTI (2026-08-11).** Doğrusu: `+20` koordinatlarla **aynı birimde**. 7.277 ardışık iç-koşu çiftinde delta/XY-adımı oranı **medyan 1,0005** (p05 0,884 · p95 1,054). 0,787 rakamı, çiftleri ardışık kayıtlardan değil **indeks alanlarından** örneklemekten geliyor — o örneklemede medyan 38,7 çıkıyor, yani 0,787 de o dağılımın bir dilimi. İnandırıcı kılan şey π/4 = 0,7854'e denk düşmesi: türetme gibi okunuyor, oysa örnekleme kazası | `progress` adı yine de doğru ad — ama "birimi bilinmiyor" değil, **dünya birimi**. Bkz. `world::routes` |
| Ana-bölge doku yedeği 8-60 MB arası dosya boyutuyla seçiliyor | **Tahmin sezgiselliği** | Üç kaynağı parser'dan aç; seçimi oyun yapsın |
| `NAV_LINK_MAX=120`, `NAV_WELD=5`, `GATE_HALF=22`, `BAR_HALF=9`, `BAR_REACH=40`, −5 cm bias | **Yargı çağrıları**, birkaçı OpenUG'un kendi yorumlarında öyle etiketli | Yalnız oyun katmanı, her biri onu haklı çıkaran ölçümle |
| Container testi "üst yarım bayt == 0x8" | **Yanlış** — doğrusu `ID & 0x80000000`; `0xB3300000` (TPK kökü) o testten geçemez | `nfsu2_arac_plani.md:236` baştan doğru yazmış; onu kullan |
| Track-TPK kayıt offsetleri: "bu varyantın atladığı `0x0C` önek" | **Dokümanı yanıltıcı, kodu doğru.** Tablo kayıt başına göre sunuluyor ama gerçekte **isim alanına** (`rec+0x0C`) göre. `n2_tpk_decode` isme çapalıyor (`d[i] >= 'A'`, sonra `i+0x18`) ve doğru sonucu alıyor; dokümanı harfiyen uygulayan ikinci bir kişi çöp okur | `P = rec + 0x24` kullan (= OpenUG'un `name+0x18`'i). Alan haritası §M1'de bu kuruluma karşı doğrulandı |
| Araba TPK'sı: "format saklanmıyor, çıkar: dokular karedir ve mip zinciri `DecodedSize`'a tamamlanır" | **Gerçek kısıt.** Her blob'un kuyruğunda gömülü bir `OldTextureInfo` var: `P = out_size − header_from_end + 0x88`, format baytı `P+38`. `header_from_end` slot kaydının 5. u32'si (OpenUG'un `i16 RefCount` dediği alan). Üç arabada ölçüldü: DXT3=79, DXT1=31 ve **RGBA8888=18** — sıkıştırılmamış BGRA, DXT1/DXT3 çıkarımının temsil edemediği bir format. Ayrıca `OBJ_PYLON` 64×**128**, kare değil | `named_format`/`level_size`'ı değiştirmeden kullan; DXT5 ve paletli etiketler de aynı baytta |
| Submesh kaydı: index buffer "sırayla tüketilir", offset alanı yok | **Eksik** — word 13 gerçek bir index offset'i ve ölçülen 94/94 çok-run'lı solid'de tam döşeniyor | `MAT_RANGE_OFFSET = 13*4` kullan; sırayla tüketme burada şansa çalışır, başka yerde kırılır |
| `ZCV_` varlıklarının hiçbir yerde yerleşim kaydı yok, üçlüler render edilebilir mesh taşımıyor | **Ölçülmüş geri çekme** | Yalnız inspector görünümü. Render özelliği olarak planlama |

---

## 9. Açık sorular

1. ~~**Streaming yarıştan önce mi sonra mı.**~~ **KAPANDI (3. gün).** Tüm şehir genişlemiş hâlde
   321.7 MB, en büyük bölge 120 MB — bellek engel değil. Indexed geometry M5'te kalıyor, sıralama
   değişmiyor: M4 yarış, M5 streaming. Streaming'in gerekçesi bellek değil **draw call**.
2. **Yeni bir `gizmo-world` crate'i mi, `gizmo-scene` içinde modül mü.** Yeni crate mimari olarak
   temiz ve hücre kavramını hem renderer'dan hem fizikten adlandırılabilir kılar. Ama 13 GB
   makinede `jobs=4` ile 21. crate her workspace dokunuşunu yavaşlatır.
3. **Reversed-Z mi, kamera başına near/far türetmesi mi.** M2'nin A/B'si karara bağlar.
   Reversed-Z + F32 depth doğru motor cevabı ve ağaçtaki her pipeline'a dokunuyor.
4. **Kinematik rakipler mi, tam `VehicleController` mı.** Üç lens de kinematik öneriyor. Ama
   motorun araç modeli gerçekten güçlü (gerçek Pacejka birleşik kayma, asimetrik damping, bump
   stop, yer etkisi) ve tam simülasyon onu kanıtlayacak şey. Maliyet rakip başına oyuncunun ~5 katı.
5. **`.gin` granüler ses: gerçek bir milestone mı, yer tutucu mu.** M6 onu çözüyor. EA-XAS'ı
   çözmek ve grain-imleci crossfade'ini yeniden yazmak kendi başına bir proje.
6. **Menüler.** `gizmo-ui` taffy layout hesaplıyor ve **workspace'te onu tüketen hiçbir şey yok**
   — `Text` yok, font yok, imge yok, z-index yok. M4'ün menüsünü `nfs_race.rs:683`'ün zaten
   kullandığı egui overlay'i üzerine planla, **ya da** şimdi "menü göndermek" = "UI metin
   renderer'ı yazmak" olduğuna karar ver.
7. **Bitmiş görünen stub'lar: bitir mi, sil mi.** `gi.rs` 483 satır CPU-only SH probe, hiçbir
   şey referans etmiyor — baked-lit statik şehir + dinamik araba için probe-lit araçlar tam
   olarak onun işi. Navmesh funnel'ı reklam ediliyor ve yalnız kenar orta noktalarını itiyor.
   **Yalnız bir milestone'un kanıtı gerektirdiğinde bitir**, yoksa yol haritası içinde şehir
   olmayan bir motor temizlik projesine döner.
8. **`gizmo_nfs::world` / `::routes` için yayın yüzeyi** — kararlı public API mı, `undecoded_roots`
   listesi küçülene kadar feature arkasında mı? Yedi kök chunk id'si çözülmemiş durumda, yani bir
   bundle "tam okundu" değil ve tip bunu söylemeli.
9. **Türkçe/İngilizce UI dizeleri.** Her yeni HUD/menü dizesi ikisini de istiyor. Mekanizmayı
   (oyun katmanında küçük bir dize tablosu?) M4 bir düzine dize eklemeden **önce** kararlaştır.

---

**Dürüst toplam:** M0-M5 kabaca 8-11 hafta odaklı iş; M6-M7 üstüne 4-6 hafta daha. En büyük
gerçek belirsizlikler: (a) bir bölge indexsiz belleğe sığıyor mu, (b) gönderilen nav link
alanları OpenUG'un tahmin ettiği anlama geliyor mu, (c) fixed/variable schedule ayrımı `Phase`
kapalı bir enum olduğu için ne kadar dalgalanıyor. Bu plandaki diğer her şey birinin daha önce
aldığı bir ölçüme dayanıyor.

---

## Nerede kaldık (2026-08-11)

Güncel durum burası; 2026-08-09 ve 2026-08-04 bölümleri tarihsel kayıt.

### Karar verildi: `_1A/_1B/_1Z` "raf" değil LOD kademesi — ve elenmiyorlar

2026-08-09'da açık bırakılan tek karar buydu ve "bir bakışa bakıyor" diye bırakılmıştı. Bakıldı;
ama kararı bakış değil iki ölçüm verdi.

**Ne oldukları artık belli.** Harfler bir dizi değil: tüm şehirde **A (1.819), B (1.392),
Z (1.203)**, sonrası neredeyse yok (C 41, D 3, dört tekil). Üç seviye, alfabe değil — bu, "bir
binanın bölümleri" okumasını eler, çünkü bölümler A, B, C, D diye gider. Üstüne üç şey daha:

- Ad, harf dışında birebir tekrar ediyor: `XB_3TOWERAPARTLK_1A_00`, `_1B_00`, `_1Z_00` — aynı
  tasarım, aynı örnek numarası.
- Vertex sayısı harfle düşüyor (1.423 ardışık çiftin 1.377'sinde).
- 28 obje doğrudan **`LOD_<tasarım>_1Z_LL`** adını taşıyor. Dosya bunu kendisi söylüyor.
- Bir ailenin üyelerinin kutu **boyutları birebir aynı**, vertex sayıları değil
  (`XB_LANDMARKTOWER_RB_00`: üçü de 80×304×80, v = 323 / 258 / 32). Aynı tasarımın iki ayrı
  yerleşimi *aynı mesh*'i kullanır; bunlar aynı silueti farklı yoğunlukta kuruyor.

**Ama elenemezler.** "Öyleyse yalnız en incesini çiz" fikri ölçümle reddedildi:

| ölçüm | sayı |
|---|---|
| aile / üye | 1.354 / 3.811 |
| en ince olmayan üye | 2.457 obje · 286.954 vertex |
| üst üste duran aile | **24** — kalan **1.330'u ayrı duruyor**, en genişi 651 m |
| tek eksene paralel dizilmiş aile | 1.007 |
| kırpık ada yaslanan aile | 166 |
| **yere oturan üye** | kaba kademeler **%67** · en ince kademe **%70** |
| **başka bir `XB_` binanın içinden geçen üye** | kaba kademeler **%22** · en ince kademe **%26** |

Belirleyici olan son iki satır. İkisi de kaba kademeleri ayırt edemiyor: ne yerle ilişkileri ne de
komşularıyla çakışmaları en ince kademeden farklı. (İkinci satırın taban oranının bu kadar yüksek
olması ayrı bir gerçek — şehrin binalarının dörtte biri zaten bir başkasının kutusuna 2 m'den fazla
giriyor; podyum/taban kompozisyonları böyle kurulmuş.) Kaba kopyalar bir kenara park edilmiş olsaydı yerle ilişkileri
bozulurdu; bozulmuyor — dosya onları Bayview'ın sokaklarına diğer her şeyle **aynı oranda**
oturtuyor. Yani `NFS_TIERS=finest` bir tekilleştirme değil, yerde duran 2.457 objeyi silme işi.
**Varsayılan değişmedi: hepsi çiziliyor.** `keep_finest` bir teşhis aracı, aday bir varsayılan
değil.

> 2026-08-09'un "514 raf · 1.444 üye" sayıları bununla karşılaştırılmamalı: o sayım hem yalnız
> tasarıma göre gruplayan bir anahtarla hem de "aynı Y/Z + X'te yayılım" filtresiyle yapılmıştı.

### Kod nereye taşındı

Kademe mantığı `nfs_cruise.rs` içinde gömülü 78 satırdı; artık testli bir modül:
**`game/src/world/lod.rs`**. Taşırken üç hata çıktı:

- **Aile anahtarı yanlıştı.** Eski kod yalnız *tasarıma* göre grupluyordu, dolayısıyla `_1A_00` ile
  `_1A_01` — aynı tasarımın iki ayrı yerleşimi — tek aileye düşüyordu. Anahtar artık tasarım +
  kuyruk.
- **Ad alanı 27 karakterde kırpılıyor** (746 ad tam sınırda, 164'ü çıplak `_` ile bitiyor), yani
  uzun adlı iki yerleşim *birebir aynı* adla geliyor. Kendini doğrulayan kural: bir anahtar aynı
  harften iki üye topluyorsa bir yerleşimi tanımlamıyordur — reddediliyor. Bu kural olmadan en
  geniş "aile" 2,3 km'ye yayılıyordu. Bugün 7 anahtar / 29 obje bu yüzden reddediliyor; kırpıklık
  ayrıca `name_is_whole()` ile *kanıtlanarak* sayılıyor (166 aile).
- **Eski `tier_of` sondaki alt çizgiyi zorunlu tutuyordu**, yani kuyruğu kırpılmış `ARC_BLDING_D_1A`
  gibi adları hiç görmüyordu.

### Yeni teşhis kolları (`nfs_city`, başsız tek kare)

- Her koşuda kademe raporu basılıyor.
- `NFS_TIERS=finest` — yalnız en ince üye · `NFS_TIERS=coarse` — **yalnız elenecek olanlar**.
  İkincisi "ne kaybedilir" sorusunu bir yokluğu aratmak yerine doğrudan gösteriyor.
- `NFS_TIERS_LIST=<n>` — yayılım histogramı, yer-ilişkisi oranları, en geniş n aile üyeleriyle
  (harf, vertex, `placed`/`identity`, dosya sırası, merkez, boyut).
- `NFS_ONLY=<altdizi>` — yalnız adı eşleşen objeler; `nfs_shot`'ın araba için yaptığının şehir
  karşılığı.

### Sıradaki adım — mesafeye göre seçim

Kademeler bir yük değil, **zaten elimizde duran LOD verisi**. Eksik olan onları seçecek şey — ve
orada beklenenden iyi bir haber çıktı: pinlenmiş motorda `LodGroup` / `LodLevel` bileşenleri ve
`LodGroup::select_mesh(distance)` **var** (son sınırın ötesinde `None` dönüp cull de ediyor). Ama
onlara yalnız `gizmo-studio`'nun kendi render boru hattı bakıyor; oyunun kullandığı
`default_render_pass` `LodGroup`'u hiç ödünç almıyor. Yani iş "motora LOD eklemek" değil, o geçidi
oyunun yürüdüğü yola bağlamak — `MOTOR-NOTLARI.md` madde 8, madde 2 ile aynı şekilde duruyor.

Her kare 8.161 mesh gidiyor ve bunun 2.457 objesi zaten uzak kademe.

### Rota dosyaları okunmaya başlandı (PryHUB)

08-04 tablosundaki "Kalan: **rota dosyaları** (`ROUTES*/Paths*.bin`)" satırının ilk parçası düştü.
`gizmo_nfs::world::routes` artık **düğüm tablosunu** okuyor — bir yarışın üstünde sürüldüğü graf:

| | |
|---|---|
| dosya | 113 (`ROUTES*` altında 8 dizin); 105'i düğüm tablosu taşıyor, 8'i taşımıyor |
| kayıt | `0x00034148`, **24 bayt**, 105 payload'ın hepsi tam katı · **18.064 düğüm** |
| düzen | `+0/+4` konum (f32, dünya çerçevesi, yükseklik yok) · `+8`, `+10` **adlandırılmadı** · `+12/+14/+16` düğüm indeksi, `0xFFFF` = yok · `+18` hep 0 · `+20` kümülatif mesafe |

Kanıt tahmin değil: **54.192 indeks okumasının sıfırı** ne sentinel ne de kendi dosyasının tablosuna
geçerli bir indeks olmayan bir değer taşıyor. Okuma iki bayt kaydırılınca 15.931'i dışarı düşüyor —
yani kontrol, başarısız olduğu görülmüş bir kontrol. `ug2 track "$NFSU2_ROOT/TRACKS"` bunu koşuyor ve
bozulduğunda sıfırdan farklı çıkıyor.

**`+8` adını hak etti: güzergâh indeksi.** Önce adlandırılmamıştı, çünkü "18.064 kaydın hepsinde
geçerli bir indeks olurdu" testini herhangi bir küçük tamsayı da geçer. Kanıtı dosyanın kendi
düzeni verdi: **105 tablonun 105'inde** her `+8` değeri dosya sırasında **tam olarak bir bitişik
koşu** kaplıyor (kurulum genelinde 2.923 koşuya karşı 2.923 farklı değer, sıfır istisna) ve
değerler boşluksuz `0..k-1`. Konumla yalnızca ilişkili bir alan, oyundaki her tabloyu tesadüfen
temiz bölmez.

Kazanç, kayıtta başka hiçbir yerde bulunmayan **sürüş sırası**:

| sıralama | komşular arası 100 m üstü sıçrama |
|---|---|
| dosya sırası | %12,5 |
| `+20`'ye (mesafeye) göre | %31,4 |
| **önce `+8`'e göre böl** | **%0,9** (medyan adım 29,2 m · p95 65,9) |

Yani tablo tek bir rota değil, bir **yol ağı**; her koşu sürülebilir bir polyline. Link'ler de
kendini açıklıyor: mevcut 19.409 link'in **%94,8'i başka bir güzergâhın** düğümünü gösteriyor —
bir güzergâh boyunca komşuluk zaten sırada olduğu için `+12/+14/+16` **kavşaklar**. Yalnız %29'unun
karşılıklı olması da bundan: kavşak yönlü.

`+10` hâlâ adsız.

Yeni kontroller `ug2 track`'te ve kırılıp denendi: güzergâhı `+8` yerine `+10`'dan okuyunca 13.352
değer 15.967 koşuya dağılıyor ve numaralandırmada delik açılıyor — çıkış kodu sıfırdan farklı.
Yan etki olarak `+20` oranının p95'i 1,054'ten **1,027**'ye düştü: çiftleri "adım < 100 m" diye
süzmek yerine güzergâh üyeliğine göre süzmek dağılımı kendiliğinden sıkılaştırdı, ki bu da bölmenin
doğruluğunun ayrı bir küçük argümanı.

**M4 için anlamı:** yarış hattı artık elimizde — 2.923 güzergâh, sürüş sırasıyla, kümülatif
mesafesiyle, ve kavşaklarıyla. Düğümlerin %100'ü sürülebilir yüzeyin üstünde (yukarıdaki kontrol).

### Yarış hattı şehrin üstüne kondu — `world::route`

Kayıtta olmayan tek şey **yükseklik**ti ve onu yalnız şehir bilir; bu yüzden bu iş parser'ın değil
oyunun. `game/src/world/route.rs` güzergâhları Gizmo çerçevesine taşıyıp yüksekliği çarpışma
geometrisinden örneklüyor. İki kural, ikisi de önce yanlış yapılarak bulundu:

1. **Yalnız yollara sor.** `Surface` üçgeni normaline göre sınıflar, dolayısıyla **düz bir çatı
   asfaltla aynı anlamda sürülebilir**. Tüm sürülebilir üçgenler üzerinde bir düğümün altındaki
   yığın 79 m derin (341 düğümün 179'unda 4+ yüzey) ve "en üsttekini al" `Paths4001`'in 40
   güzergâhından altısını **y 115–133'teki çatılara** koydu — yol 22–28'deyken. `road_ground`
   (adında `ROAD` geçen 13.985 objeden 1.928'i) bunu çözüyor: 341 düğümün **259'unda tek aday**
   kalıyor, kalanların açıklığı 15 m'ye iniyor — ki bu zaten üst geçit demek.
2. **Aday kaldığı yerde en az tırmanan diziyi seç.** Üst geçitte düğüm başına bir kural güverte ile
   altındaki yolu ayıramaz; güzergâh ayırabilir, çünkü sürekli. `follow` tüm güzergâh boyunca
   `|Δh|` toplamını enazlıyor — tohum yok, hiçbir düğüm tek başına karar vermiyor.

İkincisi yalnız **birincisi sayesinde** güvenli: tüm sürülebilir üçgenler üzerinde "en az tırmanış"
dejenere, çünkü şehrin altındaki düz raf hiç tırmanmadan her yerde kazanıyor — denendi, hattı yolun
altına koydu.

Sonuç (`Paths4001`, 40 güzergâh / 341 nokta): komşudan yükseklik alan **10** nokta, 13,3 km hat, en
kötü basamak 20,4 m (69 m yatay mesafede). Seçilen kotlar şehir merkezinde 24,7–32,0 — `Ground`'un
bağımsız olarak ölçtüğü 26,4 ile aynı yerde.

`nfs_city NFS_ROUTE=<Paths*.bin>` hattı şerit olarak çiziyor. Şeridi 3 m kaldırmak gerekti: kareyi
bir kilometreden yukarıdan alınca yarım metre derinlik tamponunun gürültüsünde kalıyor ve hat doğru
çizildiği hâlde yolun içinde kayboluyor.

Bariyerler hâlâ bunun üstünde duruyor: kurulumda bariyer chunk'ı yok (§8, ölçülmüş olumsuz sonuç),
yani koridoru rota verisinden türetmek gerekiyor — ve en güçlü aday `0x0003414A` çıktı, aşağıda.
Düğümler ve mesafe artık elimizde; kalan parçalar okunmamış dört yaprak — `0x0003414D` (her dosyada 36'nın tam katı), `0x0003414C` (16),
`0x00034149` ve `0x0003414A` (4'ten büyük hiçbir şeye bölünmüyor: değişken uzunlukta ya da başlıklı).

#### `0x0003414A` — yönlü poligon bölgeler, ve bariyerin en güçlü adayı

Rota dosyalarının en büyük yaprağı (Paths4001'de 69.628 bayt) sabit stride taşımıyor — 8..80
arası her stride tarandı, hiçbirinde sütun tutmuyor. Çünkü kayıtlar **değişken uzunlukta**:

| offset | ne |
|---|---|
| `+0` | `u32` (nokta sayısı **değil** — aşağıya bak) |
| `+4` | konum (x, y) |
| `+12` | **birim** yön vektörü (uzunluk 1,000) |
| `+20..+32` | sıfır |
| `+32` | poligonun AABB'si (minx, miny, maxx, maxy) |
| `+48..+68` | 16 bayt + bir `u32` |
| `+68` | poligonun köşeleri (x, y çiftleri) |

Kanıt kendini doğrulayan cinsten: kayıt imzasını (küçük `u32` + birim vektör + üç sıfır) tarayarak
bulunan **11.361 kaydın 11.170'inde** `+32`'deki kutu `+68`'deki listeyi **birebir** sarıyor (%98,3).
Yanlış hizalanmış bir okuma bu testi geçemez.

Poligonlar çoğunlukla dörtgen (medyan 4 köşe, en fazla 12), medyan alan **8.073 m²**. Ve şehre
sorulduğunda: merkezlerinin **198/200'ü (%99) sürülebilir yüzeyin üstünde**, kenarlarının %97'sinin
iki yanında da yol var. Yani yolu kaplayan, **yönü olan**, büyük bölgeler.

**Bu bariyer için bulunan en güçlü aday.** "Parkurun içinde misin" ve "doğru yöne mi gidiyorsun"
sorularının ikisini birden taşıyor.

**Kayıt sınırı bulundu ve okuyucu yazıldı** (`gizmo_nfs::world::routes::regions`). Sınır kaydın
kendi içinde: `+64` köşe sayısı, **`+66` kaydın kendi bayt uzunluğu**. `uzunluk = 68 + 8 × sayı`
kurulumun **42.698 kaydının hepsinde** tutuyor ve bu uzunlukla yürümek payload'ın sonuna
**112 chunk'ın 112'sinde** tam olarak oturuyor. Böylece imza taraması gerekmiyor: 11.361 değil
**42.698** kayıt okunuyor, ve iki değişmez daha bedava geliyor — beyan edilen kutu köşelerin
gerçek sınırına eşit (**0** istisna) ve yön birim vektör (**0** istisna). Dört bağımsız kontrol,
sıfır istisna. İki alanı iki bayt erken okumak ilk dosyada ilk kayıtta patlıyor.

`+0`'daki tür kodu (14 değer) ve `+48`'deki kimlik (445 değer, 12.830 kayıtta sıfır) ham taşınıyor.
Kimlik obje adı hash'i **değil** — 445'in hiçbiri şehrin 29.515 adıyla eşleşmiyor.

**Tür kodu bir boyut hiyerarşisi.** `nfs_city NFS_REGIONS=<Paths*.bin>` poligonları türe göre
renklendirerek çiziyor; `Paths4001`'in 671 poligonu:

| tür | adet | ortalama alan | merkezi yolun üstünde |
|---|---|---|---|
| 3 | 1 | 7.961.881 m² | %0 |
| 16 | 4 | 4.000.447 m² | %25 |
| 17 | 13 | 1.048.373 m² | %38 |
| **12** | **131** | **10.109 m²** | **%86** |
| **5** | **39** | **4.674 m²** | **%85** |
| 4 | 24 | 2.711 m² | %25 |
| **6** | **79** | **939 m²** | **%91** |
| 11 | 4 | 672 m² | %25 |
| 7 | 42 | 241 m² | %62 |
| 14 | 23 | 225 m² | %22 |
| 13 | 196 | 79 m² | %44 |
| 18 / 19 | 56 / 59 | 64 m² | %39 / %41 |

Okunacak şey şu: **yola oturanlar orta boy türler** — 5, 6, 12, yani 671'in 249'u, %85-91 oranıyla.
Koridor adayı bunlar. Devasa olanlar (3, 16, 17) alan tanımı; en küçükler (13, 18, 19 — 64-79 m²,
311 adet) yalnız %40 oranında yolda, yani yol işaretçisi değiller. İlk bakışta "adaları dolduruyor"
diye okunan şey buydu; sayı düzeltti.

**Yön vektörü ölçüldü: yolun yönü değil.** En yakın güzergâh düğümü 60 m içinde olan bölgelerde
yerel yol yönüyle karşılaştırıldı — medyanlar 48–91°, hiçbir tür hizalanmıyor.

Ama yön poligonun **kendi** şekline göre tam anlamlı, ve tür kodları keskin iki aileye ayrılıyor:

**Yerleştirilmiş işaretçiler — poligonu türetilmiş.** 13, 18, 19 türleri **8,00 × 8,00 m**,
14 türü **15,00 × 15,00 m** kare. Kurulum genelinde her birinin dört köşesi var, dördünde de dik
açı, ve merkezi kendi `origin`'inde: **11.801 / 3.416 / 3.599 / 1.403 kayıt, dörtgenler arasında
sıfır istisna**. Yön, kenara **10. *ve* 90. persentilde 0,0°** ile paralel — iki uçta da sıfır
yayılım olan bir dağılım elle yazılmış veri değildir: köşeler `origin` + `heading` + boyuttan
üretiliyor. Yani bu kayıtlar yönlü bir kare, noktalar gereksiz. 42.698 kaydın **19.000'i** bunlar.

**Alanlar — poligonu asıl olan.** 3, 4, 11, 12, 16, 17 türlerinde `origin` poligonun **dışında**
(%100), alanlar 731 m²'den tek bir 7.961.882 m²'lik örtüye kadar. 5, 6, 7 türleri arada: orijin
içeride, ve asfalta oturanlar bunlar (merkezlerinin %85–91'i sürülebilir yolda, işaretçilerde
bu oran ~%40).

Yön, işaretçi türlerinde karenin kendi açısı; alan türlerinde hâlâ açıklanmadı.

#### Ve bariyer bu değil — arama bitti

Belirleyici test: bir yarışın **güzergâh düğümleri** bu poligonların içine düşüyor mu? 18.064 düğüm:

| tür | düğümü kapsıyor |
|---|---|
| 16 | %86 |
| 17 | %75 |
| 3 | %49 |
| **5 + 6 + 12 (koridor adayı)** | **%33** |
| herhangi bir bölge | %88 |

Koridor adayı türler parkurun üçte birini kapsıyor, ve düğümlerin %12'si hiçbir bölgenin içinde
değil. **`0x0003414A` bariyer değil.** §8 zaten "hiçbir dosyada bariyer chunk'ı yok" diye ölçmüştü;
son aday da elendi, yani koridor **türetilecek** — ve türetmek için gereken her şey artık var.

### Koridor türetildi — `world::route::Corridor`

Bir rota dosyasının güzergâhları o yarışın sürülebilir ağıdır, dolayısıyla "parkur dışı" =
"her güzergâhtan uzak". `Corridor::locate(nokta)` bir sorguda iki soruyu birden cevaplıyor:
**en yakın güzergâha uzaklık** ve **dosyanın kendi kümülatif mesafesi** o noktada (segment boyunca
interpolasyonla). Yani "parkurda mıyım" ve "neredeyim" aynı aramadan çıkıyor.

Yükseklik uzaklığa **bilerek girmiyor**: köprüdeki araba köprünün güzergâhına aittir, kırk metre
altındaki yola değil. Ağ ulaşmıyorsa `None` dönüyor, büyük bir sayı değil — "1.400 m uzakta" demek,
birinin onu yarı-genişlikle karşılaştırmasına davetiye olurdu.

**Yarı-genişlik tahmin edilmedi, ölçüldü.** Her güzergâh noktasından enine yürüyüp yolun nerede
bittiğine bakıldı (aynı kotta kalma koşuluyla): yol yanlara **medyan 9–11 m** uzanıyor (Paths4001
11, Paths4021 11, Paths4061 9). p90'ın 60 m'ye dayanması kavşaklar ve meydanlar. 12 m yarı-genişlik
bu ölçümün üstünde duruyor.

#### `0x0003414D` — düzeni çıktı, **anlamı çıkmadı**

Bariyerin en olası adayı buydu ve değil. Parser'a girmedi çünkü bir alanı adlandırmak bir iddiadır
ve burada iddia edilecek şey yok — ama ölçülenler burada dursun ki kimse baştan başlamasın.

**Kayıt 36 bayt.** 8..64 arası her stride tarandı; yalnız 36'da lane'ler tutuyor (9 lane'in 4'ü
kayıtların %98'inden fazlasında makul koordinat; diğer hiçbir stride'da tek lane bile tutmuyor).

| offset | ne |
|---|---|
| `+0..+16` | iki 2B nokta — bir segment. Uzunluk p05 7,9 · medyan 24 · p95 68 · maks 119 m |
| `+17` | sayaç baytı, 1..4 — `+20`'den itibaren kaç `u32` yuvasının dolu olduğuna **birebir** eşit |
| `+20..+36` | o sayıda `u32` kimlik |

**Kimlikler 159 tane** ve kurulumda **başka hiçbir yerde geçmiyorlar**: `L4RA.BUN` ve
`STREAML4RA.BUN` içinde 4 bayt hizalı sıfır eşleşme. Değerler 33 bitişik koşu hâlinde ve koşu
başlangıçları 33'ün katlarıyla ayrılıyor — `bStringHash` çarpanı 33 olduğu için bu, son iki
karakteri değişen (numaralı) adların imzası. `[A-Z0-9_]` üzerinden 4 karaktere kadar hiçbir ad
bunlardan birine hash'lenmiyor, yani adlar daha uzun.

118.729 kayıt var ama **yalnız 2.406 ayrı segment** — aynı küme dosyalar arasında tekrarlanıyor.
Bir kimlik 113 rota dosyasının medyan 61'inde görünüyor.

**Bariyer değiller — bu artık ölçülmüş, ve kontrollü.** Segmentler şehrin çarpışma üçgenlerine
soruldu (`nfs_city` içinde `NFS_PROBE`). Ölçümün kendisine güvenilmesini sağlayan şey pozitif
kontrol: aynı koşuda rota **düğümleri** de soruldu ve **5.502'nin 5.501'i (%100) sürülebilir
yüzeyin üstünde** çıktı, yani çerçeve dönüşümü doğru. Sonuç:

| soru | sonuç |
|---|---|
| segmentin orta noktası sürülebilir mi | 2.406/2.406 — **%100** |
| 4 m sağında *ve* solunda sürülebilir yüzey var mı | 2.401 — **%100** (birinde 5, hiçbirinde 0) |
| iki ucunun 3 m ötesinde de sürülebilir yüzey var mı | 2.396 — **%100** |
| segmentin üstünde duvar üçgeni var mı | 107 — %4 |

Yani bu çizgiler yolun kenarında değil, **içinde**; yolu enlemesine de kesmiyorlar (uçlarının
ötesi de yol). Bariyer okuması da, "yolu kapatan kapı" okuması da bitti.

**Elenenler** (hepsi ölçümle, tekrar denenmesin diye):

- *Yarış hattı değil.* Düğümden en yakın segmente uzaklık medyan 15,6 m; yalnız %4,1'i 1 m altında.
- *Zincirlenmiş duvar değil.* Uçların yalnız %16'sı paylaşılıyor; bir dosyanın 1.647 segmentinin
  938'i tekil, en uzun zincir 11.
- *Uzamsal bölge kimliği değil.* Medyan kimliğin segmentleri 4.938 m'lik haritanın 1.253 m'sine
  yayılıyor.
- *Rota kimliği değil.* 2.406 segmentin **sıfırında** taşıdığı kimlik sayısı, göründüğü dosya
  sayısına eşit.
- *Obje hash'i değil.* 29.515 dünya obje adının hash'iyle 195.561 referansın sıfırı eşleşiyor.
- *Cadde/yol kimliği değil.* Bir kimliğin segmentleri düz bir hat oluşturmuyor: yalnız 28/159'u
  kendi uzunluğunun %15'inden ince, medyan en/boy oranı 0,2–0,7.
- *Uzun bir güzergâh zinciri değil.* 5 m toleransta bile 159 kimliğin yalnız 4'ü tek bir zincir
  kuruyor; bileşen/segment oranı medyan **0,50**, yani segmentler **ikişerli** birleşiyor. Tüm
  2.406 segment 2 m toleransta 497 bileşene ayrılıyor (bileşen başına ~4,8 segment).

**Bilinen olumlu tarif** (bir sonraki hipotez bunu açıklamak zorunda): sürülebilir yüzeyin
*içinde* duran, medyan 24 m'lik, ikişerli birleşip küçük öbekler kuran 2.406 çizgi; her biri 159
kişilik bir kimlik uzayından 1–4 kimlik taşıyor; kimlikler kurulumda başka hiçbir yerde geçmiyor.

### Açık kalan soru — parser tarafı

Bir kademe zinciri neden haritaya yayılmış duruyor? 192 baytlık solid başlığı bunu söylemiyor:
`_1A`/`_1B`/`_1Z` kayıtları yalnız hash'te, sayaçlarda ve **tek bir öteleme bileşeninde** ayrılıyor
(`XB_LANDMARKTOWER_RB_00`: 0x070 üçünde de 151,517 · 0x074 −673,474 / −381,031 / −129,160 · 0x078
üçünde de 0). Cevap büyük olasılıkla hiç ayrıştırılmamış görünürlük verisinde:
`TRACKS/PrecullerBooBooScript.hoo` ve `ROUTES*/Paths*.bin`. Bu bir PryHUB işi.

### Bu oturumda çürütülenler (tekrar araştırılmasın diye)

- "`_1A/_1B/_1Z` bir şablon rafı" — **hayır**, LOD kademesi. "Raf" okuması, doğru bir ölçümün
  (aynı Y/Z, X'te düzenli adım) yanlış yorumuydu.
- "Kaba kademeler bir kenara park edilmiş" — **yanlış**. Yerle ilişkileri en ince kademeyle aynı
  (%67 / %70).
- "Kademeler dosyada ayrı LOD bloklarında duruyor" — **hayır**, yan yanalar (`#8524`, `#8525`,
  `#8530`).
- "Konum farkı bizim yerleştirme hatamız (identity/yerel karışması)" — **hayır**, üçü de `placed`,
  matrisleri gerçek ve yalnız ötelemede ayrılıyor.
- "Kaba kademe düşük çözünürlüklü dokusundan tanınır" — **hayır**, bu bir kanıt değil: Bayview'ın
  gerçek arka plan blokları da fotoğraf kaplı kutular.
- "Motorda mesafeye göre mesh seçimi yok" — **yanlış**, var (`LodGroup::select_mesh`). Eksik olan
  onu `default_render_pass`'e bağlayan yol. Motor maddesi yazmadan önce pinlenmiş kaynağa bakmak
  bu maddeyi baştan yanlış yazmaktan kurtardı.
- "Rota düğümlerindeki `+20` metre değil, XY adımının ≈0,787 katı" — **yanlış** (§8 satırı
  düzeltildi). Aynı birim; medyan oran 1,0005. Ders örneklemede: çiftleri indeks alanlarından
  seçmek ardışık kayıtlardan seçmekle aynı şey değil, ve π/4'e denk düşen bir sayı türetilmiş
  görünüyor diye türetilmiş olmuyor.

---

## Nerede kaldık (2026-08-09) — tarihsel

Bu bölümün açık bıraktığı tek karar (LOD rafları) yukarıda kapandı; gerisi geçerli.

### Motor artık sabit

Oyun `gizmo-engine`'i **`4d1a8cb`** commit'ine pinli derliyor (`game/Cargo.toml`, `rev = `). Kardeş
`Gizmo` checkout'u serbestçe ilerliyor ve oyunun derlemesini etkilemiyor. Pinin kendisi, kaçış
kapağı, yükseltme kontrol listesi ve **motorda bulunan yedi eksik** `MOTOR-NOTLARI.md`'de.

Pin ilk saatinde işini yaptı: `Gizmo` o sırada temizken on değişmiş dosyaya çıktı.

### Parser: şehir artık bütün okunuyor

`world/object.rs` tam-24 stride dayatıyordu; şehrin **234 solid'i 36 baytlık standart kaydı**
kullanıyor ve hepsi sessizce boş dönüyordu — `XS_*`/`XO_*` levhalar, refüj direkleri, şevronlar.
`world_layout` artık iki stride'ı da **tam eşitlikle** kabul ediyor (`layout_for`'un "sığıyor mu"
kuralı değil). Dünyaya ulaşan kazanç: 65 obje, +1.070 çarpışma üçgeni, +42 çizim mesh'i.

Modül belgesi "her tampon istisnasız `vertices × 24`" diyordu; iddia L4RH/L4RR/L4RC ile
doğrulanmıştı — yani 36 baytlık solid içermeyen dört bundle'ın üçüyle, onu yanlışlayamayacak veriyle.

**Ve bunu bir daha kimsenin fark etmeden kaybetmemesi için ölçü kondu:** `ug2 verify <TRACKS>` her
solid'i çözüp kendi başlığının beyanıyla karşılaştırıyor, `golden_assets.rs` içinde
`the_city_decodes_to_what_it_declares` olarak da koşuyor (NFSU2_ROOT yoksa atlanıyor).

```
solids 28985 · declared 3667007 v / 2544154 t · decoded 3667007 v / 2544154 t
DROPPED 0 · SHORT 0 · runs 70548 declared, 70548 kept · RUNS UNTEXTURED 0
```

Kusuru geri koyup denendi: 234 DROPPED, çıkış kodu 1. Başarısız olduğu görülmemiş kontrol,
çalıştığı bilinen kontrol değildir.

### Oyun: sürülebilir hâle geldi

- **Şehir görünür oldu.** Vertex rengi sRGB eğrisinden geçiriliyordu; bunlar radyometrik örnek
  değil, 2004'ün sabit-fonksiyonlu boru hattının dokuya **görüntü uzayında** uyguladığı çarpan.
  Kare medyanı **1/255 → 29/255**.
- **Sonsuz düşme kapandı.** `CarRig::keep_in_world`, iki ölçütle: kesintisiz 2,5 sn hava, **ve** son
  *ayakta durulan* pozun 60 m altına inmek (sürtme sayacı sıfırlıyor, derinlik sıfırlanamıyor).
  "Ayakta durulan" = dört teker yerde, dik, 0,25 sn — tek karelik sürtme değil. Üç binary'de de var.
- **Doğuş noktası şehir merkezine taşındı.** Eski `(1354, −11, −2457)` **havaalanının içindeydi**.
  Yenisi `(710, 27, 888)`, tuning dükkanlarının önü.
- **Zemin yüksekliği artık soruluyor.** `world::Ground`, 64 m hücrelerde, sürülebilir üçgenlerden.
  Elle bilinen iki değeri doğruluyor: merkez 26,41 (elle 27,0), havaalanı −11,03 (elle −11).
- **Harita kenarı uyarısı.** `world::Bounds` (472 hücre) + iki saniye ileriye bakan uyarı.
- **Lastik yönü.** Görsel açı girdiden türetiliyordu ve işareti tersti; artık `Wheel::steering_angle`
  fizikten okunuyor — Ackermann de bedava geliyor.
- **`dedup` anahtarına matris eklendi.** Bugün sıfır maliyet (13.985 aynı), ama 16.071 `placed`
  objenin bbox'ı **yerel**, yani anahtar dünya konumu taşımıyordu.

### Sıradaki adım — LOD rafları kararı · **2026-08-11'de kapandı, yukarıya bak**

> Bu bölümdeki "raf" okuması **yanlış çıktı**. Ölçüm doğruydu, yorumu değil: bunlar bir şablon rafı
> değil, oyunun kendi LOD kademeleri, ve yere en ince kademeyle aynı oranda oturdukları için
> elenmiyorlar. Aşağıdaki sayılar da farklı bir anahtarla sayılmıştı — güncel olanlar yukarıda.

`_1A/_1B/_1Z` tek bir tasarımın kademeli detay ailesi (1.423 ardışık çiftin 1.377'sinde vertex
düşüyor) ve **X ekseninde raflanmış**: aynı Y, aynı Z, düzenli adım.

```
514 raf · 1.444 üye · 213.366 vertex   →  objelerin %10,3'ü, vertex'lerin %12,3'ü
```

Raf olduğuna karar veren detay eğrisi: `XB_3TOWERAPARTLK` üç kardeşi 40 m arayla **258 → 176 → 27**
vertex. Sokakta yan yana üç gerçek bina böyle kabalaşmaz.

`NFS_TIERS=finest` bunları eliyor (8.161 → 7.858 çizim, 1.232.698 → 1.172.671 çarpışma üçgeni) ama
**varsayılan değil**: bunlar dosyada gerçek konumlardaki gerçek objeler ve okuma yanlışsa bu şehirden
bina siler. **Karar bir bakışa bakıyor** — rafları atınca şehirden bir şey eksiliyor mu?

### Bilinen açıklar

- **Culling yok.** 8.161 mesh her kare. Motor tarafı (`MOTOR-NOTLARI.md` 2): hücre/bölge kavramı yok,
  `Frustum::test_aabb_masked` hâlâ çağıransız.
- **Backdrop oyunun kendi verisi değil.** `MaterialType::Skybox` dokuyu hiç örneklemiyor, prosedürel
  gradyan üretiyor; `Unlit` ise pikselleri doğru yapıp derinliği bozuyor (paneller şehrin önüne
  geçiyor). Doğrusu "önce çiz, kameraya kilitle, derinlik yazma" ve motorda o yol yok (madde 7).
- **Pozlama.** `BakedLit` çıplak çarpım zinciri, ACES toe'su karanlıkta 4,67× kısıyor, exposure
  dışarıdan ayarlanamıyor (madde 3).
- **Bariyerler.** Kurulumda bariyer chunk'ı yok; türetmek ROUTES parser'ına bağlı ve o yazılmadı.
- **`nfs_drive`/`nfs_race` görsel-çarpışma uyumu.** Zemin dilimi artık sonlu ve çizilenle aynı
  boyutta, ama ikisi ayrı sabitlerden geliyordu; `GROUND_SIZE` tek kaynak yapıldı.

### Bu oturumda çürütülenler (tekrar araştırılmasın diye)

- "Şehri karartan çift-gamma" — **yanlış**. sRGB üs yasası olduğu için `f(a)·f(b) = f(ab)`.
- "`MODULATE2X` konvansiyonu" — **yanlış**. Boru hattı orta tonlarda düz gamma modulate'e eşit.
- "Gökyüzü oyunun kendi verisi" — **yanlış**. `sky.wgsl`'de tek `textureSample` yok.
- "Çizilen dünyanın üçte biri harita dışında" — **yanlış**. `Bounds` ile ölçüldü: %0,2.
- "OpenUG'un 715 obje farkı bizden" — **hayır**. Sekiz hipotez elendi; 28.985'i dosyalar üç ayrı
  chunk üzerinden beyan ediyor.

## Nerede kaldık (2026-08-04) — tarihsel

Bu bölüm taze bir oturumun buradan devam edebilmesi için. Ayrıntı commit mesajlarında.

### Biten

| katman | durum |
|---|---|
| **Parser** (`PryHUB`, dal `world-manifest`) | Şehrin geometrisi, dokuları, doku bağlaması. 28.985 obje · 5.087 doku (hepsi çözülüyor) · 70.439 doku slotunun %98,17'si kendi bölgesinde. Kalan: **rota dosyaları** (`ROUTES*/Paths*.bin`) |
| **Oyun** (`nfsu2-gizmo`, dal `roadmap`) | `world/` katmanı: dedup → hücre → (hücre,doku) birleştirme → yeniden merkezleme. `nfs_city` (tek kare), `nfs_fly` (pencere), `collision_cells` (hücre başına üçgen çorbası). `rig/` katmanı: tek `spawn_car` + `Driver` + `ChaseCamera`; `nfs_drive`/`nfs_race` artık onun üstünde |
| **Motor** (`Gizmo`) | `shadow-gate` dalı: point-shadow geçidi + `walk_positions` + `MaterialType::BakedLit`. `trimesh-aabb` dalı: önbelleklenmiş trimesh AABB (**push edilmedi**, commit `main`'de de duruyor) — *2026-08-09 güncellemesi: ikisi de `main`'e girdi ve push edildi; oyun artık motoru takip etmiyor, `4d1a8cb`'ye **pinli**, bkz. `MOTOR-NOTLARI.md`* |

### Sıradaki adım — M3, arabayı Bayview'a koymak

1. ~~**Önce ortak araba kurulumunu çıkar.**~~ **Bitti.** `game/src/rig/`: `spawn_car` +
   `Placement`, `Driver` (giriş + sabit adım), `ChaseCamera`. İki binary 1.193 satırdan 506'ya
   indi. Yolda üç sapma düzeldi: `nfs_race` her arabayı `1200.0` kg ile yarıştırıyordu (kaydın
   kütlesini okuyup yazdırırken), kayıtsız araba için uydurma tork ikisinde iki farklı sayıydı
   (520/560), ve `nfs_drive`'ın **R**'si arabayı doğduğu yere değil sabit `y = 1.5`'e bırakıyordu.
   Doğrulama: 240SX ile `NFS_AUTODRIVE=1 NFS_DIAG=1 nfs_race` — 1220 kg, dört teker yerde, tork
   yalnız arka aksta.
2. **Sıradaki:** şehir + araba binary'si. `world::collision_cells` → hücre başına
   `Collider::trimesh` + `phys.add_body`, artı `rig::spawn_car` (üçüncü kopya yok artık).
   Arabayı `NFS_AT="1354,-11,-2457"` civarına, `Placement { clearance }`'ı yüksek tut: hücrenin
   o noktadaki yüzey yüksekliği bilinmiyor.
3. `update_vehicle`'a sorgu tutamacı (motor): şu an tekerlek başına tüm collider listesini doğrusal
   tarıyor ve `gather_colliders` her çağrıda hepsini klonluyor. Şehirde bu adım başına 14.000
   collider'ın klonu demek — 2. adımın kare hızını bu belirleyecek.

### Bilinen açıklar

- **Culling yok.** 8.119 mesh her kare gönderiliyor; `Frustum::test_aabb_masked` hâlâ sıfır çağıranlı
  ve Gizmo'da hücre/bölge kavramı yok. `nfs_fly` ile kare hızına bakmak bunun aciliyetini belirler.
- **Backdrop ve LOD çizilmiyor.** `is_backdrop` / `is_distant_lod` onları eliyor; ikisi de gerçek
  veri, sadece mesafeye göre seçen bir şey yok.
- **98 çözülmeyen run** (589'dan indi). Kalanların 111'i kurulumun hiçbir yerinde yok.

### Gezerken işine yarayacak

Yolların çoğu `y ≈ -11` civarında; `nfs_fly`'ın varsayılan başlangıcı `y = 40`'ta havada.
Yola inmek için `NFS_AT="1354,-11,-2457"`. Diagnostikler: `NFS_ID="x,y"` (o pikseli hangi mesh
kaplıyor), `NFS_FIND=<substr>` (isme göre kaç obje yüklü, ilk birkaçı nerede), `NFS_NEAR=<r>`
(bir noktanın ayak izini kapsayan objeler + doku slotlarının hangi katmanda çözüldüğü).
