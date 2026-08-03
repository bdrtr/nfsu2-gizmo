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
| **toplam** | **324 MB** | **28.985** | **492** |

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
  `nfs_race.rs:479-492`'deki otosürüşte zaten var.

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
| Yarış hattı `+12/+14/+16` alanları komşu indeksleri, `0xffff` = yok | **Tahmin, ve OpenUG'un kendi kodu doğrulamıyor** — `world_load_nav` bu alanları hiç okumuyor | **`ug2 track` ile doğrula**: oku ve sonuçlanan grafiği yakınlık grafiğiyle A/B'le. Referansı aşabileceğin en net yer |
| `+20` "metre cinsinden kümülatif mesafe" | **Tahmin, ve ölçülebilir şekilde metre değil** — delta'lar XY adımının ≈0.787 katı | `progress` diye adlandır. Kimsenin sabitlemediği bir birimi etiketleme |
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
