# Motor notları — sabitlenmiş Gizmo sürümü

Oyun **donmuş bir motor sürümüne** karşı geliştiriliyor. `Gizmo` kendi yolunda ilerlemeye devam
ediyor; oyun onu takip etmiyor. Bu dosya iki şeyi tutar: **pinin ne olduğunu**, ve pinlenmiş
motorda **eksik ya da yanlış bulduğumuz her şeyi** — böylece pini yükselttiğimiz gün elimizde
tahmin değil bir liste olur.

## Pin

| | |
|---|---|
| paket | `gizmo-engine` (kütüphane adı `gizmo`), sürüm `0.10.0` |
| kaynak | `https://github.com/bdrtr/Gizmo` |
| commit | `3461c2d9` — `main`, *"volumetric: a light's air by quadrature over each stretch, not a random point — no grain"* (tam karma `game/Cargo.toml`'da) |
| sabitlendi | 2026-11-20 (önceki: `32d0bd93` aynı gün; `c86b0dcb` aynı gün; `e05b1063` aynı gün; `9ae1820e`, 2026-11-19; `001ad491` aynı gün; `37625d72` aynı gün; `c6e99031` aynı gün; `457a11d3` aynı gün; `7e4828e4` aynı gün; `209e7c1b`, 2026-11-18; `afd1a9d4` aynı gün; `b07d3b41` aynı gün; `d349bdde` aynı gün; `0e34a83c` aynı gün; `7a6f6b46`, 2026-11-17; `fd8825dd` aynı gün; `550a7dfd`, 2026-08-20; `58dc2623` aynı gün; `09c948a9` aynı gün — erişilebilirlik düzeltmesi, aynı ağaç; `3433aefe`, 2026-08-14; `48ac99e`, 2026-08-12; `ba969c0`, 2026-08-11) |
| nerede yazılı | `game/Cargo.toml` → `rev = ...`; `Cargo.lock` aynı commit'i ayrıca kaydeder |

**2026-11-20 — `32d0bd93` → `3461c2d9`, 1 commit (#35).** Lambaların halesi kumlu değil: hava her parçada açı
uzayında kuadratürle hesaplanıyor. Oyunda pus 5 adım. Fizik ve kilit dosyasının gizmo dışı kısmı değişmedi.

**2026-11-20 — `c86b0dcb` → `32d0bd93`, 1 commit (#35).** Hacimsel ışık yerel ışıklar için fiziksel ve kümeli;
oyunda pus açık (`scene::set_haze`). Fizik ve kilit dosyasının gizmo dışı kısmı değişmedi; tezgâh ve süpürme
koşulmadı.

**2026-11-20 — `e05b1063` → `c86b0dcb`, 1 commit (#34).** Yerel pozlama ve log-ortalamayla ölçen göz uyumu;
oyunda ikisi de açık (`scene::enable_local_exposure`, `scene::enable_night_eye`). Fizik ve kilit dosyasının gizmo dışı
kısmı değişmedi; tezgâh ve süpürme koşulmadı.

**2026-11-20 — `9ae1820e` → `e05b1063`, 1 commit (#33).** Motorun ölü `Renderer::exposure` alanı kaldırıldı;
oyun gecenin pozlamasını kameraya veriyor (`scene::set_exposure`). Fizik ve kilit dosyasının gizmo dışı kısmı
değişmedi; tezgâh ve süpürme koşulmadı.

**2026-11-19 — `001ad491` → `9ae1820e`, 1 commit: farlar (#32).** Motorun ışıkları `in_bake` taşıyor;
şehrin lambaları ve sahnenin dolgu ışığı `in_bake`, arabaların farları değil. Farlar `scene::enable_headlights`
ile açılıyor (`nfs_cruise`'da varsayılan, `NFS_HEADLIGHTS=off` eski hal). Fizik ve kilit dosyasının gizmo dışı
kısmı değişmedi; tezgâh ve süpürme koşulmadı.

**2026-11-19 — `37625d72` → `001ad491`, 1 commit: kümeleme (#31).** Şehrin 2.233 sokak lambası artık
motorun nokta ışıkları (`world::lights`, ayrıştırıcı `gizmo_nfs::world::lights`); motor her kare en iyi 256'sını
kümelere dağıtıyor ve o dağıtım CPU'da 0,6 ms'ti. Motorda düzeldi; yalnız `clustered.rs`, sonuç küme küme aynı.
Fizik ve kilit dosyasının gizmo dışı kısmı değişmedi; tezgâh ve süpürme koşulmadı.

**2026-11-19 — `7e4828e4` → `37625d72`, üç adımda (`457a11d3`, `c6e99031`, `37625d72`), 11 commit, hepsi
render.** Islak sokak: SSR ekran uzayında yürüyor ve sokağın dibine erişiyor, pürüzlü yüzeyde yansıma
yayılıyor, `Renderer::wetness` (yağmur, birikinti), yağmur haritası (köprü altı, sundurma, arabanın altı
kuru), ıslak yüzeyde güneş ve lamba parıltısı; yansımalar açıkken baked şehir G-buffer'dan değil forward
çiziliyor (ıslak kare 14,3 → 12,0 ms, AMD). Son adım #27'yi kapatıyor: TAA gece şehrinin ışıklarını
artık kırpıştırmıyor ve `nfs_cruise`'da varsayılan açık (`NFS_TAA=off` eski hal). Oyunda değişen:
`NFS_WET`/`NFS_PUDDLES` (`7356448`) ve TAA anahtarının yönü. Bu tablo `457a11d3` ve `c6e99031`
adımlarında güncellenmemişti; bu paragraf üçünü birden kapsıyor. Aralıkta fizik, ECS ve matematik
kasalarında değişiklik yok, kilit dosyasında yalnız gizmo kaynaklarının commit'i değişti — tezgâh ve
süpürme koşulmadı.

**2026-11-18, üçüncü yükseltme — `d349bdde` → `b07d3b41`: çizim yolunun CPU bedeli (#29).** Oyunda değişen:
şehrin üç kurucusu (`world::build`'de `build_region`, `build_lods`, `build_local`) ağlarını artık
`renderer.mesh_arena`'ya yerleştiriyor (`MeshArena::triangle_list`), `Mesh::from_vertices` yerine. Otoyolda
şehre bakış (6.989 çizim), araç durdurulmuş, kare süresi: AMD 780M 47,5 → **15,9 ms**, RTX 3050 35,0 →
**11,9 ms** (pin `afd1a9d4`). Ayrıntısı #29'da. Fizik kodu değişmedi; tezgâh/süpürme koşulmadı.

**2026-11-18, ikinci yükseltme — `0e34a83c` → `d349bdde`, 1 commit: mip seviyeleri (#30).** Motorun
ürettiği her mip seviyesi bir öncekinin sol üst çeyreğinin büyütülmüşüydü; tabelalar ve araba
ayrıntıları uzakta başka bir görüntüydü. Yalnız `mipmap.wgsl` ve bir test değişti — fizik ve kilit
dosyası aynı (kilitte yalnız gizmo kaynaklarının commit'i), o yüzden tezgâh ve süpürme yeniden
koşulmadı: bu commit `nfs_sim`/`nfs_bench`'in çalıştırdığı hiçbir kodda değil.

**2026-11-18 yükseltmesi — `7a6f6b46` → `0e34a83c`, 8 commit, hepsi fizik ve motorun kendi
kapıları.** Oyunda tek satır değişmedi: derleme temiz, `cargo test -p nfsu2` 147/147. Kilit dosyasında
yalnız `chrono` çıktı, `jiff` girdi (motorun günlük saati); glam/rayon sürümleri yerinde. Oyunu
ilgilendirenler:
- **Üçgen ağı zemin oldu** (`7c7b96b9`): şehrin çarpışma ağı (`Collider::trimesh`) her üçgeni kendi
  başına bir plaka gibi çözüyordu; içine gömülen gövde altından itilebiliyor, iki üçgenin ya da iki
  ağın birleşim yeri hayalet bir duvar gibi davranıyordu. Artık temas yüzün normaliyle, gövdenin
  bulunduğu taraftan; sırtlar (5°'den keskin kıvrımlar) kendi normalleriyle.
- Arazi (`Collider::heightfield`) için aynısı (`9ddec666`) — oyun kullanmıyor.
- CCD'li gövde dinlenirken sürtünme ve yuvarlanma direnci alıyor (`73352cfc`) — oyunun CCD'li
  gövdesi yok.
- Uyuyan ve statik gövdelerin adım bedeli (#26): ECS köprüsü yalnız değişeni okuyor (`7221e816`),
  alt adımlar oturmuşları atlıyor (`d4b4681f`), statik gövdeler ayrı bir geniş-faz ağacında
  (`0e34a83c`). Motorun ölçümünde 513 uyuyan kutu 0,26 → 0,11 ms/adım.
- Fizik adımı yalnız hareket ettirdiğini `Changed` işaretliyor (`4a6dfd61`, #29'un bir parçası).
- Çarpışma olayları kimlik sırasıyla (`0e34a83c`); ağacın iç şekline bağlı değil.

Tezgâhın sekiz koşusu (`nfs_bench`: varsayılan, `crash`, `wall`, `kerb`, `trip`, `power`, `mirror`,
`steer`) eski pinle **bayt bayt aynı**: araç modeli ve düz zemindeki temas değişmedi. Süpürme (aynı
kilit, rota başına 90 s), farkı commit commit ayırarak — `base` = `7a6f6b46`, `pre` = `9ddec666`
(üçgen ağından önceki her şey), `mesh` = `7c7b96b9`, `new` = `0e34a83c`:

```
  rota    base   pre  mesh   new
  4001     440   440   439   439
  4002      68    68    66    66
  4021     168   168   168   168
  4041     209   209   207   207
  4061     260   260   351   351
  4081     143   143   139   139
  4102     140   140   183   183
  4121     139   139   205   205
  TOPLAM  1567  1567  1758  1758
```

`pre` her rotada `base`'in aynısı: köprü, geniş-faz, CCD ve arazi değişiklikleri şehirde hiçbir şey
kaydırmadı. Farkın tamamı üçgen ağı commit'inden, ve `0e34a83c` üstüne hiçbir şey eklemedi. Alanın
öbür ölçüleri aynı yönde: takılan (`held`) 482 → 435, kavşak 1.726 → 1.945, en uzak 5.966 → 6.213;
`away` 64, `fallen` 0 iki tarafta da. 4061'de takılan 61 → 3. **Karar: gerileme yok.** +191 bu
süpürmenin gürültü tabanının sınırında (aşağı bak), yani tek başına iyileşme kanıtı sayılmaz; ama
tezgâh aynı, fark tek bir commit'e iniyor ve beklenen yönde (takılan azaldı).

**2026-11-17 yükseltmesi — `550a7dfd` → `fd8825dd`, 700 commit.** Bu dosyadaki kuyruğun çoğu bu
aralıkta kapandı; motor oturumu maddeleri tek tek bu dosyadan aldı. Oyunda gereken geçişler:
- `world.borrow::<T>()` → `world.query::<&T>()` (motor `eaacb22e`). Sorgular artık `Entity`
  alıyor; oyunun `u32` kimlikleri için `ecs::{ById, ByIdMut}` (`by_id`, `by_id_mut`) köprüsü var.
- Kamera yönü artık `Transform`'un rotasyonundan geliyor, `Camera::rotation_from(yaw, pitch)`
  (`22ac95c7`); `Camera::new` 4 argümanlı. `set_setup` `&mut Renderer` alıyor.
- **Direksiyonun işareti döndü** (`a9a1b1f2`, 2026-11-15): `steering_input` +1 artık **sağ**.
  Oyunun `Controls::steer`'ı +1 = sol; `CarRig::drive` işareti çeviriyor. Bu yapılmadan derleme
  temiz, testler yeşil, düz kalkış tezgâhı ve `nfs_cruise`'un düz otomatik sürüşü sorunsuzdu;
  yalnız süpürme gördü: pilot sola kırınca araba sağa gidiyor, alan ilk virajda duvara gömülüyordu
  (4001 tek araba 30 s: 19 → 3 işaret).
- **Pin taşırken `Cargo.lock` sabit tutulmalı.** Bir ara `befbc9f6`'nın bu şehirde sonucu değiştirdiğini
  sandım (4121: 91 → 156). Motor oturumu aynı kilit dosyasıyla (`--locked`) iki commit'i birebir
  aynı buldu: fark büyük olasılıkla `rev` değişirken kilidin başka glam/rayon sürümlerine
  çözülmesindendi. Pin taşınırken `Cargo.lock` commit'lenmeli ve karşılaştırma derlemeleri aynı
  kilitle yapılmalı.

Süpürme (8 rota × 90 s, araba başına geçilen işaret, `tools/sweep-table.py`), geçici çözümler
henüz yerinde:

```
rota     old  fd88
  4001     371   374
  4002      80    59
  4021     167   209
  4041     188   210
  4061     322   332
  4081     148   134
  4102     114   120
  4121      72   156
  TOPLAM  1462  1594
  old − fd88: alan -132 · rota rota [3, -21, 42, 22, 10, -14, 6, 84] · old 2/8 rotada önde, 0 berabere · en büyük tek rota +84 → sağlam
```

**Aynı gün ikinci adım — `fd8825dd` → `7a6f6b46`.** Motor araç fiziğini yine bit düzeyinde
değiştirdi: tekerlek ve gövde aynı lastik kuvvetini alıyor, yuvarlanma direnci temas kuvveti, kilitli
teker tam kayma tutuşuyla frenliyor. `AeroPackage::default()` artık kaldırma vermiyor (Cl −0,8 → 0).
Oyun `AeroPackage`'ı kurmuyor, NFSU2'nin ölçülmüş bastırmasını kendisi uyguluyor
(`CarRig::apply_downforce`), yani o güne dek iki bastırma birden alıyordu; artık yalnız kendi
kuvvetini alıyor. Tezgâh: 100 km/h'de yanal 11,85 → 11,16 m/s², son hız 209 → 213 km/h; ayna
birebir, duvar aynı. Süpürme, `base` = `fd8825dd` + eski geçici çözümler, `p7a6f` = yeni pin +
geçici çözümler kaldırılmış:

```
rota      old   base  p7a6f
  4001      371    374    440
  4002       80     59     68
  4021      167    209    168
  4041      188    210    209
  4061      322    332    260
  4081      148    134    143
  4102      114    120    140
  4121       72    156    139
  TOPLAM   1462   1594   1567
  old − base: alan -132 · rota rota [3, -21, 42, 22, 10, -14, 6, 84] · old 2/8 rotada önde, 0 berabere · en büyük tek rota +84 → sağlam
  old − p7a6f: alan -105 · rota rota [69, -12, 1, 21, -62, -5, 26, 67] · old 3/8 rotada önde, 0 berabere · en büyük tek rota +69 → sağlam
```

**Süpürmenin gürültü tabanı (2026-11-17).** Sonucu değiştirmemesi gereken bir şey — burada büyük
olasılıkla kilit dosyasının başka bağımlılık sürümlerine çözülmesi; kayan nokta ortamı iki iş
parçacığında aynı, `0x1fa7` — sekiz rotanın toplamını 1391 → 1594, tek rotayı 91 → 156 oynattı. Her
binary kendi içinde deterministik, ama sahne kaotik: son hanedeki bir fark birkaç saniyede başka bir
yarışa dönüşüyor. Yani bu süpürmede ±150–200'lük toplam farkı bir *sinyal değil*; kısa deneylerle
(tezgâh: ayna, duvar, kalkış) desteklenmeyen kararlar ona dayandırılmamalı.

**2026-08-11 yükseltmesi.** `4d1a8cb..main` beş commit ve ikisi doğrudan bu dosyaya cevap:
`1dcab55` *"engine: close six gaps a game hit against a pinned build"* — 1, 3, 4, 5, 6 ve 7
numaralı maddeleri, bizim ölçtüğümüz sayıları alıntılayarak kapatıyor; `ba969c0` madde 2 için
uzamsal indeks getiriyor. Yükseltmenin oyun tarafına maliyeti iki satır oldu: `Vertex::color`
artık `[f32; 4]`.

**2026-08-20'nin ikinci yükseltmesi — ve kaynağını yine biz yazdık.** `550a7df` *"renderer: an
alpha cutoff the baked-lit path can reach"*: madde 14. `Material::alpha_cutoff`, shader'a
`InstanceRaw`'ın zaten dolgu olarak ayırdığı `ambient.w` ile taşınıyor, yani stride ve o tamponu
indeksleyen hiçbir shader değişmiyor; `baked_lit.wgsl`'deki test `gbuffer.wgsl`'in aynısı ve sıfır
kapatıyor. Oyun tarafına maliyeti bir çağrı (`with_alpha_cutoff`) ve dokuları kesim/decal diye
ayıran bir sayım. Yükseltme doğrulandı: sekiz-rota süpürmesi **bayt-birebir aynı**.

**2026-08-12 yükseltmesi.** Tek commit, ve bu sefer kaynağı biz yazdık: `48ac99e` *"renderer: a
backdrop that stays where the level put it"* — madde 7'yi doğrularken çürüttüğümüz şeyi düzeltiyor.
`MaterialType::BackdropPlaced`, yeni shader ya da pipeline olmadan. Oyun tarafına maliyeti iki
çağrı: `with_backdrop` → `with_backdrop_placed`.

Motor üç iddiamızı da **düzeltti**, ve bunlar bizim hatalarımız:
- *"exposure dışarıdan ayarlanamıyor"* — **yanlış**, `Camera::exposure` zaten vardı;
  `post_process.rs`'teki 1,15 yalnız tamponun ilk içeriği, her kare üzerine yazılıyor.
- *"`gather_colliders` kendini ve trigger'ları da tarıyor"* — **yanlış**, ikisini de atlıyordu.
- *"son cascade sınırı"* okumamız — `cascade_splits.w` gerçekten `min(cam_far, SHADOW_DISTANCE)`.

Bu commit'te oyunun bugün dayandığı her şey var — doğrulandı, varsayılmadı:
`Collider::trimesh` ve `TriMeshShape::local_aabb` (önbelleklenmiş trimesh AABB,
`gizmo-physics-core/src/components/collider.rs:648`), `MaterialType::BakedLit`,
`VehicleTuning::torque_curve`. `trimesh-aabb` ve `shadow-gate` dallarının ikisi de `main`'e
girmiş durumda, yani ROADMAP'in "push edilmedi" notu artık geçerli değil.

**2026-08-20 yeniden pinleme — motor kaynağı DEĞİŞMEDİ.** Yeni `rev` yeni bir motor sürümü
değil: `git diff 3433aefe 09c948a9 -- crates/` **boş**, yani iki commit'in motor ağacı bayt-birebir
aynı. Değişen tek şey commit'in erişilebilir olması.

Sebep ölçüldü: motor deposunun tarihi 96 MB'lık bir medya temizliği için yeniden yazılmış, ve eski
hattaki commit'ler `main`'in atası olmaktan çıkmış. `3433aefe` **hiçbir daldan erişilemiyor** —
yalnız cargo'nun `rev=` için ürettiği yapay `refs/commit/<sha>` ref'inden. Pratik sonucu şuydu:
oyun bu makinede yalnız `~/.cargo/git/db` önbelleği sayesinde derleniyordu. Temiz bir kopyada,
CI'da ya da başka bir makinede cargo GitHub'dan erişilemeyen bir nesne istemek zorunda kalırdı, ve
GitHub onu bir gün toplar.

Yani pinin verdiği söz — *"oyunun derlemesi bizim değiştirmediğimiz bir sebeple bozulamaz"* — bir
commit tarafından değil bir önbellek tarafından tutuluyordu. Bu yeniden pinleme onu geri koyuyor:
`09c948a9` `main`'den erişilebilir, ve içerik aynı olduğu için oyun tarafında doğrulanacak bir
davranış değişikliği yok. Kontrol listesinin 5. adımı (gözle doğrulama) bu adımda atlanabilir; 4.
adım (derle ve test et) yine de koşuldu.

**Ders, defterin kendi diliyle:** bir git `rev` pini, o commit'e bir daldan erişilebildiği sürece
tekrarlanabilirlik verir. Erişilemeyen bir hash bir pin değil, bir önbellek bahsidir. Bir sonraki
pin değişiminde `git -C ../Gizmo branch --contains <rev>` çıktısının boş olmadığını doğrula.

**2026-08-20 yükseltmesi — 257 commit, oyuna maliyeti iki satır.**

`log pin..main` 964 commit gösteriyor ama gerçek delta **257**: `main..pin`'deki 707 konunun hepsi
`pin..main`'de birebir var, yani aynı işin yeniden yazılmış hâli. Ölçüm: `comm -12` = 707,
`comm -13` = 0.

**Derleme maliyeti: iki satır.** Tek kırık wgpu 30'un `get_mapped_range()` çağrısının artık
`Result` döndürmesi — `nfs_shot.rs` ve `nfs_city.rs`'teki kare yakalayıcılar. `.expect(...)`
eklendi; motorun kendi `capture.rs`'i de aynı deseni kullanıyor. Bunun dışında kaldırılmış ya da
yeniden adlandırılmış tek bir motor sembolü yok — oyunun dokunduğu yüzeyin tamamı yerinde.
Yığın sıçraması: wgpu 29→30, egui 0.34→0.36, MSRV 1.92→1.96 (bu makinede rustc 1.97.1).

**Sürüş kaydı mı? Ölçüldü — sürücü sağlam, temas kaydı.** Sekiz-rota süpürmesi, yükseltmeden önce
alınan baseline'a karşı (`BASELINE-SEKIZ-ROTA.md`):

| ölçü | pin | main | okuma |
|---|---|---|---|
| `away` | 64 | 64 | kavşak seçimi değişmedi |
| `junctions` | 1452 | 1445 | −%0,5 |
| distinct nodes | 1354 | 1352 | −%0,1 |
| `furthest` | 5350 | 5170 | −%3,4, **işaretler iki yönlü** (4001 +104, 4061 −182) |
| `fallen` | 2 | 3 | +1, rota 4041 |

Kontrol ölçümü yapıldı: aynı rota aynı binary'de iki kez koşuldu, sayılar **birebir aynı** çıktı
(`furthest=708`). Yani farklar gürültü değil.

Okuması: sürücü mantığı sağlam (`away` sabit, kavşak/düğüm yarım puan içinde), değişen şey
**yörüngeler**. Rota 4081 tek bir sayı bile oynamadan **birebir aynı** kaldı — duvara sürtmeyen,
temiz giden rota. Bu, aralıkta gelen statik/dinamik (stick-slip) temas sürtünmesi modelinin imzası:
temasa girmeyen rota etkilenmiyor, girenler 90 saniyede ayrışıyor. `fallen`'daki +1 aynı yerden.
Regresyon değil, ölçülmüş bir kayma — ve NFSU2 için doğru yönde olması muhtemel: gerçek araba
duvara sürtünce yapışıp kaymaz.

**Araç bit düzeyinde aynı.** `crates/gizmo-physics-dynamics/src/vehicle/` diff'inde yorum dışı tek
kod satırı yok (ölçüldü), ve canlı doğrulaması da örtüşüyor: `NFS_AUTODRIVE=1 NFS_DIAG=1 nfs_race`
→ 121 diag satırının 120'sinde dört teker yerde (tek istisna doğuş anındaki düşüş), 121/121'inde
tork yalnız arka akstan `[0, 0, 949, 949]`, kütle 1220 kg. Defterin 2026-08-12'de kaydettiği
sayıların aynısı.

Gözle: `nfs_race` kare 300'de doğru çiziyor — araba, gölge, pist çizgileri, HUD. wgpu 30 geçişinin
görünür bir kaybı yok.

### Neden pin

Motor bağımlılığı eskiden kardeş checkout'a **path** ile bağlıydı: `Gizmo/` hangi dalda ve hangi
commit'te duruyorsa oyun onu derliyordu. O ağaçta eşzamanlı olarak başkaları çalışıyor, dolayısıyla
oyunun derlemesi bizim değiştirmediğimiz bir sebeple bozulabiliyor — ya da daha kötüsü, sessizce
davranış değiştirebiliyordu. Pin bu bağı kesiyor: oyun tarafındaki bir regresyon artık oyun
tarafından gelir.

## Günlük kullanım

Normalde hiçbir şey değişmez — `cargo build --release -p nfsu2` her zamanki gibi. Cargo motoru bir
kez `~/.cargo/git/` altına indirir, sonrası çevrimdışı çalışır.

**Yerel motoru geçici olarak denemek** (yeni bir commit'i yoklamak, ya da `Gizmo`'da yaptığın bir
düzeltmeyi oyunda doğrulamak) için kök `Cargo.toml`'daki `[patch."https://github.com/bdrtr/Gizmo"]`
bloğunu yorumdan çıkar. Patch, `rev` pinini de ezer.

> Denemeden sonra **geri yoruma al.** Açık unutulan bir patch pini sessizce hükümsüz kılar ve
> motor yine ayağının altından kayar — pinin engellemek için var olduğu tam durum.
> `cargo tree -p gizmo-engine` hangi kaynağın geçerli olduğunu söyler: git kaynağı mı, yerel path mi.

Patch açıkken çıkan derleme hatasını **oyunun hatası sanma**: yerel ağaç yarım bir durumda olabilir.
2026-08-09'da tam olarak bu görüldü — `crates/gizmo/Cargo.toml` `gizmo-core/tracing-layer`'a
yönlendiriyordu, `gizmo-core` o özelliği henüz bildirmiyordu; o pencerede patch'i açmak oyunu
**motorun içini gösteren** bir hatayla düşürürdü. Pinlenmiş commit kendi içinde tutarlıdır, bu yüzden
bu yalnız kapak açıkken ısırır.

`Gizmo`'da **asla dal değiştirme.** O ağaçta başka oturumlar çalışıyor. Motorun bir commit'e sahip
olup olmadığını öğrenmek için checkout değil git yeter:
`git -C ../Gizmo log --oneline 48ac99e..main`.

## Pinin neyi dondurduğu — ve neyi dondurmadığı

Üç sınır, üçü de sonradan sürpriz olmasın diye yazılı.

**Motorun kaynağı donmuştur, motorun bağımlılıkları donmamıştır.** Git bağımlılığı kendi
`Cargo.lock`'unu getirmez; motorun caret aralıklarını bu repo çözer. Yani `cargo update` bir gün
donmuş motorun altındaki `wgpu`/`naga`'yı oynatabilir ve **hiçbir motor commit'inin açıklamadığı**
bir render farkı üretebilir. Bugün oyun `wgpu`/`naga` 29.0.4 çözüyor, Gizmo'nun kendi kilidi
29.0.3'te. Renderer'da açıklanamayan bir değişiklik görürsen ilk bakılacak yer motor değil,
`Cargo.lock` diff'idir.

**Pini yalnız `origin/main` tutuyor.** `48ac99e`'yi işaret eden bir etiket yok. `main` force-push
ile bu commit'in gerisine alınırsa GitHub nesneyi eninde sonunda toplar ve bağımlılık, deposu
`~/.cargo/git`'te sıcak olmayan her makinede çözülemez olur — bu makine fark etmez, kırılma önce
temiz bir checkout'ta ya da CI'da görünür. Kalıcı çözüm `Gizmo`'da `48ac99e`'ye bir etiket atıp
push etmek (ör. `nfsu2-pin-2026-08-11`); bu, motor deposuna yazmak demek olduğu için bilerek
yapılmadı.

**Kök `[patch]` donmuş motorun içine de uzanır.** Bir `[patch]` git bağımlılığının *geçişli*
bağımlılıklarını da yeniden yazar. Bugün zararsız — Gizmo `gizmo-nfs`'e bağlı değil — ama motor bir
gün ona bağlanırsa, pin duruyor olmasına rağmen parser sessizce yerel `../PryHUB`'dan gelir. Freeze'in
tek gerçek deliği bu.

**2026-08-14 yükseltmesi (`48ac99e..main`, 41 commit).** Bu sabah aynı soruya "taşımayalım" demiştim ve
gerekçe ölçülmüştü: aradaki commit'lerin hiçbiri oyunun davranışını değiştirmiyordu. Gün içinde
motorda yapılan render taraması bunu değiştirdi — **oyunun tam olarak kullandığı yolu düzelten on
iki kusur** girdi:

| ne düzeldi | oyuna neden dokunuyor |
|---|---|
| G-buffer albedo doğrusal 8-bit'ti, iki farklı karanlık malzeme birebir aynı çiziliyordu | Bayview kare medyanı 1-14/255 — kusurun tam ortası |
| dünya konumu mutlak koordinat olarak f16'da tutuluyordu (1 km'de 50 cm adım) | şehir ~1500 m'ye uzanıyor |
| yüksek yayıcılar güneş yükseldikçe kırpılıyordu (75°'de 65 m) | binalar |
| `baked_lit` gölge bias'ı NDC'de sabitti | **şehrin çizildiği malzeme yolu** |
| TAA yeniden izdüşümü, gölge texel snapping'i, öğle güneşinde NaN kaskad matrisi | her kare |

Doğrulama (kontrol listesi 4-5): `cargo build --release -p nfsu2` temiz, **63 test geçiyor**,
`cargo tree -p gizmo-engine` yeni rev'i gösteriyor, ve `NFS_AUTODRIVE=1 NFS_DIAG=1 nfs_race`
listenin beklediğini birebir veriyor — 240SX 1220 kg, dört teker yerde, tork yalnız arka aksta
`[0, 0, 949, 949]`.

**Sürüş tabanı bozulmadı, ve bu ayrıca doğrulandı:** sekiz rotalık süpürmenin iki rotası pin
öncesiyle aynı sayıyı verdi (4001 = 165, 4121 = 176). Fizik değişmedi — motorun determinizm karması
bu 41 commit boyunca `A462C9EB8A09D5CA`'da sabit kaldı — ama oyunun kendi ölçüm tabanı sabit bir
motora karşı anlamlı olduğu için sayıyla teyit edildi.

**Geri alınacak geçici çözüm çıkmadı:** kuyrukta "oyundaki geçici çözüm" sütunu dolu olan tek
kapanmış madde yoktu.

## Eksikler ve düzeltmeler

Geliştirirken pinlenmiş motorda ne bulduysak buraya. Kural: madde **motor-genel** dille yazılır.
NFSU2 kelime dağarcığı (chunk id, dosya adı, "Bayview") bir motor maddesine giremez — bkz.
ROADMAP §6. Bir eksiği oyun tarafında geçici çözümle atlattıysak, o çözüm de yazılır: pin
yükselince geri alınacak olan şey odur.

Durumlar: `açık` · `motorda düzeldi` (commit ile) · `pin yükselince doğrula` · `kapandı`

| # | tarih | eksik / hata | nerede çarptık | oyundaki geçici çözüm | motor tarafı | durum |
|---|---|---|---|---|---|---|
| 1 | 2026-08-04 | `update_vehicle` için broadphase destekli sorgu tutamacı yok: tekerlek başına tüm collider listesi doğrusal taranıyor, `gather_colliders` her çağrıda hepsini klonluyor. Şehirde adım başına ~14.000 collider kopyası | ROADMAP "Sıradaki adım" §3 — şehir + araba binary'sinin kare hızını bunun belirlemesi bekleniyor | yok (henüz ölçülmedi) | `gizmo-physics-dynamics` × `-rigid` | **kapandı** (2026-08-12) `1dcab55` — 4.098 collider'da 0,437 ms → 0,002 ms, adım başına klon 4.098 → 0. Davranış değişikliği: tekerlek artık yalnız rigid boru hattının simüle ettiği gövdelere basıyor, `Collider`-only zemin arabayı tutmuyor. **Doğrulandı, kapandı** (2026-08-12): `NFS_AUTODRIVE=1 NFS_DIAG=1 nfs_race` — 40 diag satırının 39'unda dört teker yerde, tek istisna doğuş anındaki düşüş; tork yalnız arka aksta (`[0, 0, 949, 949]`). Prosedürel zemin arabayı tutuyor |
| 2 | 2026-08-04 | Motorda hücre/bölge (`Cell`/`Region`) kavramı ve uzamsal indeks yok; `Frustum::test_aabb_masked` pinlenmiş commit'te hâlâ **sıfır çağıranlı** (yalnız kendi testleri çağırıyor) | 8.119 mesh her kare gönderiliyor, culling yok | culling oyun tarafında yok; `world/` kendi hücrelerini kuruyor ama render onları kullanmıyor | yeni `gizmo-world` ya da `gizmo-scene` + `gizmo-renderer` | **kapandı** (2026-08-12) — `ba969c0` ile `gizmo-renderer::visibility` (artımlı BVH, `VisibleSet`, `query_frusta`) geldi ve `test_aabb_masked` çağıranını buldu, ama motorun kendi `collect_draw_items`'ı hâlâ doğrusal tarıyor. Commit "oyun kendi render döngüsünü sürüyor" varsayıyor; bizimkiler `default_render_pass` kullanıyor, yani indeksi tüketmek **bizim işimiz** — **ve ölçtük, gerekmiyor: kapandı** (2026-08-12). Üç şey birden söylüyor: (a) maddenin "8.119 mesh her kare gönderiliyor, culling yok" tespiti eskimiş — `collect_draw_items` hem kamera hem cascade frustumlarına karşı `classify_visibility_world` çağırıyor ve `Visibility::Culled => continue` yapıyor; (b) motorun kendi kıyaslaması BVH'nin **8k renderable'ın altında kaybettiğini** yazıyor (4k'da doğrusal tarama 2,8× önde) ve çağırana "kendi sahneni ölç" diyor, bizde 5.672 mesh var; (c) ölçtük — Bayview'da serbest dolaşım **medyan 8,0 ms / en kötü 14,2 ms, 100–126 fps**. Alacak bir şey yok. Sahne 8k'nın üstüne çıkarsa (M5, birden çok bölge) madde yeniden açılır |
| 3 | 2026-08-09 | `BakedLit` **çıplak bir çarpım zinciri**: `vertexRengi × instanceAlbedo × doku`, üstüne gölge terimi. Kazanç yok, emissive yok, ambient taban yok, sis yok — karanlık içeriği kaldıracak hiçbir kolu yok. Üstüne ACES toe'su (`post_process.wgsl:108`, `x→0`'da `aces(x) ≈ 0,214·x`) karanlıkta 4,67× kısıyor ve `exposure = 1,15` (`post_process.rs:151`) bunu telafi etmiyor. **Dikkat:** orta tonlarda boru hattı düz gamma modulate'e neredeyse eşit (128×128 → 62,8'e karşı 64,3), yani sorun tonemap'in yanlışlığı değil, **ayarlanabilir olmaması** | Bayview: pencerenin medyanı **1/255**, ortalama 8,5/255. Yol ekranda 2–14/255. Uçtan uca: vcol 128 × doku 128 → 63; 96×96 → 21; **64×64 → 2,6**; vcol 128 × doku 255 → 164 (parlayan pencereler) | yok | `gizmo-renderer` | **kapandı** (2026-08-12) `1dcab55` — `Material`'a ambient ve emissive eklendi; tonemap'e dokunulmadı (bizim orta-ton ölçümümüz sayesinde). **Kollar artık çevrildi:** `scene::city_lift` politikayı tek yerde tutuyor, üç binary de oradan okuyor, `NFS_AMBIENT`/`NFS_EMISSIVE` ile geçersiz kılınabiliyor. Varsayılan ambient `(0.10, 0.11, 0.14)`, süpürmeyle seçildi: sokak seviyesinde kare medyanı 54 → 68, p05 24 → 30, **p95 her ayarda 155'te sabit** — parlak uç dokuyla belirlendiği için kaldırma orada oransal olarak sıfır, yani neon ve yanan pencereler ayrışmasını koruyor. 0.24 gözle elendi (gece değil kapalı hava okuyor) **2026-11-18:** şehir artık ACES yerine **PBR Neutral** ile ve köşe renkleri **sRGB'den çözülerek** çiziliyor (`scene::city_tonemap`, `world::build`; alfa doğrusal kalıyor). Kullanıcı "her yer çok parlıyor, gece gibi değil" dedi; aynı sokaktan beş kare (ACES/Neutral/eğrisiz × ham/çözülmüş) arasından gözle seçti. Kare medyanı 65 → 30, p95 186 → 76. Eskiye dönmek için `NFS_TONEMAP=aces NFS_RAW_VCOL=1`. Ambient yeni eğri altında yeniden süpürüldü ve yerinde kaldı (`CITY_AMBIENT` belgesi) |
| 4 | 2026-08-09 | `SHADOW_DISTANCE = 100` m ve son cascade'in ötesinde gölge fonksiyonu `1.0` (tam aydınlık) döndürüyor. Gölge terimi 0,55'te tabanlandığı için bu, 100 m'de **sert 1,82× parlaklık basamağı** demek — mesafeye göre yumuşayan bir geçiş yok | Bayview'da ufkun hemen altında ölçülen parlak şerit; takip kamerasından ~2,3° aşağı düşüyor | yok | `gizmo-renderer` | **kapandı** (2026-08-12) `1dcab55` — şekli taşıyan iki shader da düzeltildi. **Doğrulandı, kapandı** (2026-08-12): otoyol boyunca 1300 m'lik bir kare alındı, satır ortalamalarında en büyük sıçrama 9,3 — 1,82× basamak olsaydı ~66 olurdu; ortalama komşu fark 0,64. Yolda basamak yok |
| 5 | 2026-08-09 | `BakedLit`, "vertex rengi yok" ile "vertex rengi siyah"ı ayırt edemiyor: `baked_lit.wgsl:143-146`, `length(baked) < 0.0001` ise rengi **beyaza** çeviriyor. Gerekçe doğru (rengi set etmeyen importer modeli karartmasın) ama proxy yanlış — niteliğin varlığı vertex layout'undan bilinmeli, piksel değerinden tahmin edilmemeli. **Latent:** doğuş noktası çevresinde yalnız bir ışık-konisi decal'inin 12 vertex'i sıfır, yani bugün görünür bir bozulma yapmıyor | Kod okumasıyla bulundu, gözle değil — ufuktaki parlak şeridin sebebi bu **değil** (o 4 numara) | yok | `gizmo-renderer` | **kapandı** (2026-08-12) `1dcab55` — niteliğin varlığı artık vertex layout'undan okunuyor, piksel değerinden tahmin edilmiyor. Gözle doğrulanacak bir şey yoktu ve bu maddede bu bir kusur değil: kuyruk zaten latent olduğunu, doğuş çevresindeki bir ışık-konisi decal'inin 12 vertex'i dışında hiçbir yeri etkilemediğini yazıyordu. Kod okumasıyla bulundu, kod okumasıyla kapandı |
| 6 | 2026-08-09 | Vertex **alfası** sessizce düşürülüyor, şehrin fren-izi ve decal katmanları yolun üstüne opak geometri olarak çiziliyor | Bayview yol yüzeyi | yok | `gizmo-renderer` | **kapandı** (2026-08-12) `1dcab55` — `Vertex::color` `vec3` → `vec4`, blend state dahil uçtan uca. Oyun tarafı bağlandı (`world/build.rs`: dördüncü bayt artık taşınıyor). **Doğrulandı, kapandı** (2026-08-12): batık otoyol karesinde şerit çizgileri, sarı bordür taraması ve tekerlek izlerindeki aşınma asfaltla harmanlanıyor — alfası düşürülmüş bir decal katmanı sert kenarlı opak dörtgenler olarak görünürdü, görünmüyor |
| 7 | 2026-08-09 | **Boyalı backdrop çizecek bir yol yok.** Oyunun kendi gökyüzü/panorama geometrisini doğru çizmek için "önce çiz, kameraya kilitle, derinlik yazma" gerekiyor; motorda bu yok ve iki materyalden hiçbiri onu vermiyor. `Skybox` derinliği doğru yapıyor (`sky.wgsl` NDC z'yi uzak düzleme itiyor, hiçbir şeyi kapatamıyor) ama içinde **tek bir `textureSample` yok** — mesh'in dokusunu da vertex rengini de atıp `scene.sun_color`'dan prosedürel bir gradyan üretiyor. `Unlit` pikselleri doğru yapıyor (`vertex rengi × albedo × doku`) ama derinliği değil, paneller şehrin önüne geçiyor. `Material`'da `depth_write` kolu da yok; karar `pipelines.rs`'te veriliyor | Bayview: `Skybox` ile kare medyanı 30/255 ve NFSU2'nun 191 backdrop mesh'i ekrana hiç ulaşmıyor; `Unlit` ile medyan 14/255 ve iki soluk panel kamerayla dünya arasında | **geçici çözüm kaldırıldı** — `nfs_cruise` artık `with_backdrop` kullanıyor | `gizmo-renderer` | **kapandı** (2026-08-12, motor `48ac99e`) — önce doğrulandı ve çürüdü: `MaterialType::Backdrop` geldi ama **üç özelliği ayrılamaz biçimde birlikte** taşıyor: önce çiz, **kameraya kilitle**, derinlik yazma. Bize üçünden ikisi lazım. Kilit bir birim-küp skybox için doğru, bunun için yanlış: NFSU2'nun backdrop'u 12–18 km genişliğinde **dünya konumlu geometri**, kilitlenince üç kilometrelik bir panel merceğe yapışıyor. Ölçüm: kamerayı 1.000 m kaydır, panel ekranda **aynı konumda ve aynı boyutta** kalıyor; kare %52,4 değişiyor ve şehir arkasında soluyor. 153 mesh'in hepsi dokulu ve ekrana ulaşıyor — sorun ulaşmamaları değil, **önüne geçmeleri**. Motordan istenen: `draw_first` ve `depth_write` kollarının kameraya kilitten **bağımsız** olması (ya da `with_backdrop`'a kilitsiz bir varyant). Oyunda: `nfs_cruise` ve `nfs_city` artık `with_backdrop_placed` kullanıyor. **Kilit düzeldi ve ölçüldü**: kamerayı 1.000 m kaydır, sol üst kadranda aynı kalan piksel %45,4 → **%0,0** — panel dünyayla birlikte hareket ediyor, uzak silüetler kendi yerlerinde. Madde 9'a bak. Paneller soluk görünüyordu, o da düzeldi ve motor hatası değildi: `nfs_city`'nin doku yükleme döngüsü backdrop'u kapsamıyordu, oysa backdrop dokularının çoğu şehrin paketlerinde değil paylaşılan katmanda. **85/153 → 153/153**, backdrop-only kare medyanı **199 → 30**. Oyunda artık varsayılan açık |
| 8 | 2026-08-11 | **Mesafeye göre LOD seçimi motorda var ama oyunun geçtiği yoldan erişilmiyor.** `LodGroup` / `LodLevel` bileşenleri ve `LodGroup::select_mesh(distance)` `gizmo-renderer::components::misc` içinde duruyor (son sınırın ötesinde `None` dönüp cull de ediyor), fakat bunlara **yalnız `gizmo-studio`'nun kendi render boru hattı** bakıyor (`render_pipeline/mod.rs:232,294`). Motorun kendi `default_render_pass`'i (`gizmo/src/systems/render/mod.rs:97`) yalnız `Camera, Material, Mesh, MeshRenderer` ödünç alıyor — `LodGroup` diye bir şey görmüyor. Yani motorun hazır geçidini kullanan bir oyun için bu özellik yok hükmünde; madde 2'nin (`Frustum::test_aabb_masked` sıfır çağıranlı) aynı şekli: yetenek var, oyunun yürüdüğü yol oraya varmıyor | Bir bina için üç detay kademesi taşıyan veri elimizde; seçecek bir şey olmadığı için **üçü de** çiziliyor. 8.161 mesh'in 2.457 objesi zaten uzak kademe. Bkz. `ROADMAP.md` "Nerede kaldık (2026-08-11)" | yok — kademeler oyun tarafında ölçülüyor (`world::lod`) ama seçim yapılmıyor; `NFS_TIERS=finest` yalnız bir teşhis kolu | `gizmo` (`systems/render`) — bileşenler `gizmo-renderer`'da hazır | **motorda düzeldi** (2026-08-14, motor `eda05c3`) — `collect_draw_items` artık her iki sorgusunda da (bileşen ve asset-handle biçimi) `LodGroup`'a bakıyor; anlamı studio ile bilerek aynı: grup varlığın kendi `Mesh`'ini **ezer**, son bandın ötesi **cull** eder (en kabaya düşmez). Mesafe varlığın dünya konumuna ölçülüyor, çizilen mesh'in merkezine değil — merkez seçilecek mesh'e ait olduğu için o dairesel olurdu, ve fark metrelerle bandlar onlarca/yüzlerce metreyle ölçüldüğünden bandı değiştiremez. Politika `LodGroup::select_level` olarak ayrıldı ve testi yazıldı; öncesinde test edilemiyordu, çünkü `Mesh` `wgpu::Buffer` taşıyor ve GPU'suz kurulamıyor. **`motorda düzeldi, oyun tarafı bekliyor`** — ve bu iş yükseltmeyi BEKLEMİYOR: motor kodu pin 2026-08-14'te taşındığından beri elimizde, 2026-08-20 yükseltmesi buna bir şey eklemedi. `world/lod.rs`'in modül belgesi 2026-08-20'ye kadar hâlâ "renderer does not have yet" diyordu ve düzeltildi; kimsenin başlamamasının sebebi büyük olasılıkla oydu — doğrulama yalnız "çalışıyor mu" değil: oyunun bugün kaba kademeleri *attığı* yer (`world::lod::keep_finest`, 2.457 nesne) yerine `LodGroup` kurabileceği yer olur. Önceki not, hâlâ geçerli: motorun kendi mesh-içi auto-LOD'u var (`lod_vbufs`, `dist > world_r * 15`) ama bizim kademelerimiz ayrı **nesneler**, tek mesh'in LOD tamponları değil — o yüzden bize dokunmuyor, madde doğru. Ne var ki ölçüm önceliği düşürüyor: üç kademeyi birden çizmek 5.672 → 6.406 mesh ve **8,0 → 9,1 ms**. Kazanacağımız en fazla 1,1 ms. `keep_finest`'ın gerekçesi zaten hız değil kalite (kaba kademeler yakından bulanık kutu) |

| 9 | 2026-08-12 | **`Material`'ın backdrop kolu üç davranışı ayrılamaz paketliyor.** `with_backdrop` aynı anda "önce çiz", "kameraya kilitle" ve "derinlik yazma" veriyor. Kameraya kilit yalnız birim-küp skybox için doğru; dünya konumlu uzak geometri (boyalı panorama, uzak silüet panelleri) için yıkıcı — geometri kameranın önüne taşınıyor. Diğer ikisi kilitsiz istenebilmeli | 7 numarayı doğrularken | backdrop `NFS_BACKDROP=1` olmadan çizilmiyor | `gizmo-renderer` (`backdrop.rs`, `components/material.rs`) | **kapandı** (2026-08-12, motor `48ac99e`) — `MaterialType::BackdropPlaced` + `Material::with_backdrop_placed`. Yeni shader ya da yeni pipeline yok: shader zaten `local + camera_pos` hesaplıyor, yeni `backdrop::instance_model` yüklenen matristen kamerayı önceden çıkarıyor (`T(−c)·M`) ve ikisi tam olarak sadeleşiyor — geometri yazarın koyduğu yere düşüyor, aynı boru hattının kanıtlanmış durumuyla (önce çiz, derinlik yazma, uzak düzleme sabitle). `camera_locked_model` ile simetrik: biri "nereye düşüyor", öteki "ne gönderiyoruz", her tip için tam olarak biri birim. İkisi de saf fonksiyon, GPU'suz test edildi |

| 10 | 2026-08-13 | **`gizmo-ai`'nin navigasyon tarafı katmanlı bir şehre uymuyor; sürüş tarafı ise boid uzayında.** Erişim sorunu yok, madde bunu söylemek için var: `crates/gizmo/src/lib.rs:84` → `pub use gizmo_ai as ai`, feature'a bağlı değil ve **pinli commit'te mevcut** — yani bugün çağrılabilir. Ama (a) `navmesh` kendi belgesiyle *"en iyi ihtimalle 2.5D; ızgara XZ'de düz, her poligon tek bir Y taşır, üst üste binen katlar, rampalar ve eğim sınırı modellenmiyor"*, üstelik engelleri `collider.compute_aabb(...)` ile **statik gövdelerin AABB'sinden** rasterize ediyor (`navmesh.rs:301`); (b) `pathfinding`'in `NavGrid`'i aynı AABB kaynağını kullanıyor ve *"hareket y'yi hiç değiştirmez, farklı katmandaki iki hücre birbirine erişilemez"*; (c) `steering` (`seek`/`arrive`/`avoid_obstacles`/`separate`/`cohesion`/`alignment`) `desired_velocity − current_velocity` döndürüyor, çağıran onu hıza entegre ediyor — non-holonomic bir araba için doğrudan kullanılabilir değil; (d) crate'te **ışın, görüş hattı ya da segment sorgusu yok**, yani "şu iki nokta arasında bir şey duruyor mu" orada sorulamıyor | Bariyer filtresi süpürülürken sorulan "motorda AI zaten var, neden kendimiz yazıyoruz" sorusu (2026-08-13). Ölçülen çarpışma: şehrin collider'ları `world::collision_cells` ile **64 m'lik hücre başına tek trimesh**, yani her engelin AABB'si koca bir şehir hücresi — navmesh'in flood fill'ine yürünebilir hücre kalmaz. Bayview'ın katmanlı olduğu ayrıca ölçülü: düz XZ haritası köprü tabliyesinin kenarını altından geçen yoldan ayıramıyor, `Ground::edge_at` tam bu yüzden arabanın kendi yüksekliğinde soruyor | graf motordan değil **oyunun kendi rota dosyalarından** kuruluyor (`world::Network`, `Paths####.bin`) ve yarış hattının kaynağı da bu olmalı — üretilmiş bir navmesh NFSU2'nun parkurunu değil bizim ızgaramızı sürerdi; pilot (`rig::pilot`) elde yazıldı | `gizmo-ai` | **(a)/(b) kapandı — düzeltilmeyecek, ve düzeltilmemeli** (2026-08-14): katmanlı bir şehir için 2.5B navmesh ve AABB engelleri kullanılamaz, ama bunu motorda düzeltmek 3B navigasyon demek ve **bizim ihtiyacımız o değil**. Yarış hattının kaynağı NFSU2'nun kendi rota dosyaları olmalı; üretilmiş bir navmesh bizim ızgaramızı sürerdi, parkuru değil. Bu bir eksik değil, bir kapsam kararı — kuyrukta "yapılacak" gibi durması yanıltıcıydı. **(c) açık, bir fikir olarak**: pilotun elde yazdığı takip mesafesi `separate` ile, dolanma `avoid_obstacles` ile aynı işi yapıyor; Reynolds vektörünü gaz/fren/direksiyona çevirmek işin kendisi ve bir gün ölçülmeye değer. (d) — ışın/segment sorgusu yokluğu — oyun tarafında `world::collide::Walls` ile karşılandı, motordan istenmiyor |


| 11 | 2026-08-20 | **Kare başına ışık dizisi 10 ile sınırlı, ve ekran dışındaki ışık da yuva işgal ediyor.** Hangi onunun seçileceğine ECS sırası karar veriyor, yani kamera dönünce seçim zıplayabiliyor. Gece bir şehir için bu bir sokak aydınlatma bütçesi değil | Madde 3 süpürülürken: gecenin tamamı `BakedLit`'in ambient/emissive kollarıyla taşınıyor (`scene::city_lift`, CITY_AMBIENT `(0.10, 0.11, 0.14)`), çünkü ışıkla taşınamıyordu | gece ışıkla değil ambient/emissive ile kuruluyor — kaldırılacak bir hile değil, eksiğin etrafından dolaşan bir politika | `gizmo-renderer` | **motorda düzeldi** (main, `ba7c497c` tavan 32 + frustum kırpması, `0cd52034` kümelemeli kırpma + tavan 256, piksel başı iş küme başına 32 ile sınırlı) — **pin taşındı 2026-08-20, oyun tarafı bekliyor.** Doğrulama gece/aydınlatmalı bir rotayla yapılmalı; sekiz-rota süpürmesinde öyle bir rota bugün yok. CPU atama maliyeti motorun kendi makinesinde ölçülmüş (8 ışık 0,047 ms → 256 ışık 0,764 ms); kendi sahnemizde yeniden ölçülmeli |
| 12 | 2026-08-20 | **Motorda analog eksen/gamepad girdisi yok**; sürüş girdisi yalnız klavyeden dijital olarak üretilebiliyor | `rig::drive` direksiyonu ve gazı iki ayrı tuştan üretip yumuşatmayı elde yapıyor (`steer` −1..1 ama kaynağı dijital) | elde yumuşatma | `gizmo-core` (`input`) | **motorda düzeldi** (main, `0fe5b69d` gamepad + doğrulayan sanal cihaz, `450cb319` tek hareket ekseni) — **pin taşındı 2026-08-20, oyun tarafı bekliyor.** `Gamepad`, `GamepadAxis`, `GamepadButton` prelude'de; `gamepad` varsayılan feature. Bir yarış oyununda direksiyon ve gaz analog olmak ister, yani bu maddenin kazancı doğrudan sürüş hissinde |
| 13 | 2026-08-20 | **ECS'te yazarlanan bir collider örtüşme bildiremiyor:** `is_trigger` (ve `collision_layer`) her kare fizik dünyasına yeniden kurulurken düşüyordu, yani ECS kaynaklı hiçbir gövde `TriggerEvent` üretemiyordu | Doğrudan çarpılmadı — checkpoint mantığı baştan oyun tarafında yazıldı (`world::route::Checkpoints`), motorun tetikleyicisi hiç denenmedi. Madde, motorun o kapısının kapalı OLDUĞUNU kayda geçirmek için var | `world::route::Checkpoints` kendi geçiş mantığını yürütüyor | `gizmo-physics-rigid` | **motorda düzeldi** (main, `c566818c`) — **pin taşındı 2026-08-20.** Oyun tarafında iş AÇMIYOR: mevcut `Checkpoints` çalışıyor ve sekiz-rota süpürmesiyle yargılanabiliyor. Motora devretmek ayrı bir karar, ve süpürmenin zeminini değiştirir — devredilirse baseline yeniden alınmalı |

<!--
Yeni madde eklerken şablon — boş sütun bırakma, bilmiyorsan "ölçülmedi" yaz:
| n | YYYY-AA-GG | ne eksik/yanlış (motor-genel dille) | hangi iş sırasında çarptık | oyunda ne yaptık | crate | açık |
-->
| 14 | 2026-08-20 | **`BakedLit` yolunda alfa *testi* yok — yalnız alfa *karıştırma* var.** Kesim dokusu (bir dörtgene kesilmiş bitki örtüsü, çit, korkuluk) opak boru hattında `discard` ister; motor bunu `gbuffer.wgsl`'de **yapıyor** (`alpha_cutoff = material.uv_scale.z`, `if (alpha_cutoff > 0.0 && final_alpha < alpha_cutoff) { discard; }`) ama eşiği yalnız **glTF yükleyicisi** dolduruyor (`asset/loaders/material.rs:216`, `AlphaMode::Mask`) ve `Material` bileşeninde karşılığı olan bir alan yok. `with_baked_lit` ayrıca G-buffer'dan geçmiyor (kendi belgesi: *"Skips the G-buffer"*), yani o yolda eşik hiç okunmuyor. Çağırana kalan tek kol `with_transparent`, ve o **karıştırma**: eş düzlemli decal'ları kaybettiriyor | Statik, ışığı pişirilmiş bir şehir sahnesi. Kesim kartları opak çizilince havada duran koyu levhalar oluyor. Karıştırmaya çevirince kartlar düzeliyor ama zeminle eş düzlemde duran desen katmanları kayboluyor — sıralı geçiş onları orada tutmuyor. Eşikle ayrılamıyor: 1.500 dokunun 235'i saydamlık taşıyor ve saydam pikselin payı %0-100 arasında **düzgün** dağılmış (çeyrekler %48/%68/%82), yani "kesim" ile "decal" bu ölçüyle ayrılmıyor. 11 dokuda decal doğru ve kartlar levha; 235'te kartlar doğru ve desen kayıp | geçici çözüm **kaldırıldı** — motor kolu geldi, oyun onu kullanıyor | `gizmo-renderer` (`components/material.rs` + `baked_lit.wgsl`) | **motorda düzeldi — ve kaynağını biz yazdık** (yerel `Gizmo`, `550a7df` *"renderer: an alpha cutoff the baked-lit path can reach"*). `Material::alpha_cutoff` + `with_alpha_cutoff`; shader'a `InstanceRaw`'ın zaten dolgu olarak ayırdığı `ambient.w` ile taşınıyor, yani **stride ve o tamponu indeksleyen hiçbir shader değişmiyor**. Test `gbuffer.wgsl`'dekiyle aynı, sıfır kapatıyor. `InstanceRaw` on birinci yapıcı argümanı yerine ayrı bir `with_alpha_cutoff` adımı aldı, böylece kesimi olmayan altı çağıran elindeki çağrıyı korudu. 273 renderer testi geçiyor (dolgu yuvasını ve kırpmayı kilitleyen yeni bir tanesi dahil). **Oyun tarafı bağlandı** (`nfs_city`, `nfs_cruise`): kesim ile decal alfanın **payıyla değil şekliyle** ayrılıyor — kesim iki uçlu, decal ara opaklıkta; 156'ya 79. Decal'lar bilerek opak bırakıldı: üç işlemin yalnız o birinde desenleri görünüyor. Doğrulandı: gökyüzündeki levhalar ağaç oldu, kare orijinale göre **%0,9** değişti. **Pin taşındı ve kapak kapandı** (2026-08-20). Doğrulandı: `cargo tree -p gizmo-engine` git kaynağını gösteriyor (yerel path değil), ve sekiz-rota süpürmesi **bayt-birebir aynı** — motor taşınması sürüşü kaydırmadı. Kesim/decal sayıları (156/79) ve kare de aynı. **kapandı** |
| 15 | 2026-10-08 | **Bir gövdenin `PhysicsMaterial` bileşeni araç tekerleğine ulaşmıyor.** Tekerlek ışını yüzey sürtünmesini çarptığı **collider'ın kendi `material` alanından** okuyor (`vehicle/ground_query.rs:129,157`) ve her lastik kuvvetini `μ / ASPHALT.dynamic_friction` ile ölçekliyor (`dynamics.rs:605`). Aynı gövdeye eklenen `PhysicsMaterial` bileşeni bu yolda hiç okunmuyor; `Collider::trimesh`/`offset_box` varsayılan malzemeyle (dinamik sürtünme 0,5) geliyor. Yani "zemine asfalt bileşeni ekle" deyimi sessizce hiçbir şey yapmıyor, ve her lastik 0,5/0,65 = **%77** tutuyor | Oyuncunun "araba yavaş ve sürekli kayıyor" geri bildirimi. Yol tutuş tezgâhında (`nfs_bench`) tekerleklerin bildirdiği yüzey sürtünmesi asfalt bileşeni yerindeyken 0,5; yanal tutunma tavanı 0,62 g | `scene::road(collider)` collider'ın kendisine `with_material(ASPHALT)` veriyor; şehrin her hücresi ve her test düzlemi ondan geçiyor. Tavan 0,62 → 0,84 g. Bileşen satırları zararsız diye yerinde bırakıldı | `gizmo-physics-dynamics` (`vehicle/ground_query.rs`) × `gizmo-physics-core` (`Collider::material` / `PhysicsMaterial`) | **main'de kapandı** (`01a69fa3`: `Collider::material_with`, tekerlek gövdenin `PhysicsMaterial` bileşenini okuyor; 2026-11-17'de doğrulandı); pin yükselince alınacak. Önceki istek: — motordan istenen: ya tekerlek gövdenin `PhysicsMaterial` bileşenini okusun, ya da bileşen collider'ın `material`'ına senkronlansın; en azından iki kaynağın neden ayrı olduğu belgelensin. Düzelince `scene::road` silinebilir ama silinmesi gerekmiyor — açık yazılmış malzeme zararsız |
| 16 | 2026-10-25 | **Araç kurulumunun beş kolu sabit ya da tek:** (a) süspansiyonun geri dönüş sönümü bump'ın **sabit 2,5 katı** (`dynamics.rs`, `suspension_damping * 2.5`), ayrı verilemiyor; (b) fren dağılımı **sabit %60 ön / %40 arka**, `VehicleTuning`'de kolu yok; (c) `anti_roll_stiffness` **iki aksa tek değer**, ön ve arka viraj demiri ayrı verilemiyor; (d) kayan lastiğin tutuşu ayrı bir katsayı değil — kombine kaymada sabit 0,35 tabanı (`weighting_lorentzian`); (e) aero downforce yalnız `½ρ·Cl·A·v²` — hızla doğrusal artan bir bastırma kuvveti ifade edilemiyor | Arabanın kendi kaydındaki süspansiyon ve fren değerlerini motora taşırken. Kaynak verisi aks başına bump ve rebound'u ayrı (240SX ön 5,4 / 5,6 kN·s/m — yani rebound ≈ bump, motorun 2,5×'i değil), ön/arka viraj demirini ayrı (36 / 35 kN/m), tepe ve kayma tutuşunu ayrı (1,70 / 1,40) veriyor | `car::tune::AxleTune`: bump kaynaktan, rebound motorun 2,5×'ine kalıyor; viraj demiri iki aksın ortalaması; fren toplam torku korunacak şekilde `max_brake_torque` (dağılım motorun); kayma tutuşu kullanılmıyor; downforce oyun tarafında uygulanıyor (`CarRig::apply_downforce`, kontrolcü ile integrasyon arasında hıza `−F/m·dt`) | `gizmo-physics-dynamics` (`vehicle/mod.rs` `Wheel` / `VehicleTuning`, `vehicle/dynamics.rs`) | **main'de kapandı** (`1d60ec5e`, 2026-11-17'de doğrulandı: `suspension_rebound_damping`, `brake_bias_front/rear`, `anti_roll_front/rear`, `sliding_grip_floor`, `AeroPackage::downforce_speed_exponent`); pin yükselince alınacak. Önceki istek: — motordan istenen: `Wheel`'e ayrı `suspension_rebound_damping`, `VehicleTuning`'e `brake_bias_front`, aks başına anti-roll, isteğe bağlı bir kayma sürtünme katsayısı, ve `AeroPackage`'a hızın kuvvetini seçen bir üs (ya da doğrusal bir terim). Hepsi varsayılanı bugünkü davranış olan ek alanlar; mevcut çağıranları değiştirmez |
| 17 | 2026-11-02 | **Tahriksiz, yerdeki bir tekerlek düşük hızda kilitli tekerlek gibi davranıyor.** `update_vehicle`'ın adım sonundaki "durgun tekerleği durdur" kuralı (`dynamics.rs`, `|ω| < 0,05 && drive < 1 && brake < 1 → ω = 0`) yerdeki tekerleğe de uygulanıyor. Sert bir lastikte yuvarlanan tekerleğin ω'sı implisit güncellemeyle `≈ v_long / r`'ye oturuyor, yani `v < 0,05 · r` (0,4 m yarıçapta 2 cm/s) altında her adım sıfırlanıyor, ve sıfır ω'lu tekerlek kayma oranı `−v / 0,5` ile frenliyor — bu ölü bölgede tutabileceği kuvvet Pacejka tepesine kadar çıkıyor. Tahrikli aks bu kuvveti aşamazsa araç **kalkamıyor**: tork veriliyor, tahrikli tekerlek dengede, gövde 0 km/h'de duruyor | Ağır, tek akstan sürülen bir araç (2 900 kg, tekerlekte 2 509 N·m ≈ 6 kN itiş) 60 s tam gazda hiç kıpırdamadı; tahriksiz iki tekerleğin ω'su her adım tam 0,000, ölü bölgedeki tutuşları hesapla ≈ 14 kN. Aynı tork dört tekerleğe dağıtılınca (artık hiçbir tekerlek tahriksiz değil) araç kalkıyor. Hafif bir araç (1 220 kg, ≈ 9 kN itiş, tahriksiz aks ≈ 4 kN) ölü bölgeden çıkabildiği için görünmüyordu | yok — tetikleyen araba artık doğru (dört çeker) sürülüyor ve tahriksiz tekerleği yok; kural tek-akstan sürülen ağır araçlar için hâlâ geçerli. Hipotez kodla ve hesapla desteklendi, motor değiştirilerek doğrulanmadı | `gizmo-physics-dynamics` (`vehicle/dynamics.rs`, adım sonu) | **main'de kapandı** (`256aa377`: durgun teker sıfırlaması yalnız havadaki tekerde, yeni `Wheel::rolling_resistance`; 2026-11-17'de doğrulandı); pin yükselince alınacak. Önceki istek: — motordan istenen: durdurma kuralı yalnız havadaki tekerleğe (ya da gövde hızı da eşiğin altındayken) uygulansın; yerdeki tekerlekte ω'yu zaten lastik yönetiyor. Doğrulama: tek-akstan sürülen ağır bir aracın düz zeminde tam gazdan kalkması |
| 18 | 2026-11-02 | **Volan ataleti tahrikli tekerlek sayısı kadar sayılıyor.** `update_vehicle` tahrikli her tekerleğin etkin ataletine `flywheel_inertia · total_ratio²`'nin **tamamını** ekliyor (`dynamics.rs`, `effective_inertia`). Motor tek; doğru yansıma, toplamın tahrikli tekerleklere bölünmesi (`/ n`). Yani iki çekerde volan 2×, dört çekerde 4× sayılıyor. Kısa vitesteki etkisi büyük: oran² ≈ 180 ile tekerlek başına ≈ 45 kg·m², 0,32 m yarıçapta ≈ 450 kg eşdeğer kütle. Dört çeker iki çekere göre bu yüzden **≈ 900 kg daha ağır** hızlanıyor | Aktarma tipi kontrolcüye ilk kez verildiğinde (madde 17'nin işi): 1 500 kg'lık bir dört çeker, aynı tork ve vitesle, 15 s tam gazda iki çekerken 142 km/h'ye, dört çekerken 123 km/h'ye çıktı | `rig::spawn_car`: tahrikli tekerlek ikiden fazlaysa `flywheel_inertia *= 2 / n` — dört çeker iki çekerle aynı toplamı taşıyor. İki çekerdeki 2× sayım **bilerek bırakıldı**: sürüş hissi ve süpürme tabanı onunla ayarlandı | `gizmo-physics-dynamics` (`vehicle/dynamics.rs`) | **kapandı** (pin `fd8825dd`, 2026-11-17): oyundaki dört çeker `2 / n` ölçeği kaldırıldı. Tezgâh (`NFS_BENCH=launch`), 0-100 km/h: SKYLINE 6,9 → 7,5 s, IMPREZAWRX 6,1 → 6,8, LANCEREVO8 6,6 → 7,4. Yansıyan volan artık motorun ataletinin tamamı, yarısı değil; 240SX değişmedi (8,9 s). önceki durum: main'de kapandı (`299a8c74`: `dynamics.rs` volanı `driven_count`'a bölüyor, 2026-11-17'de doğrulandı); pin yükselince oyundaki `2 / n` ölçeği kaldırılacak. Önceki istek: — motordan istenen: yansıyan volan ataleti tahrikli tekerleklere bölünsün. Pin yükselince oyundaki `2 / n` ölçeği kaldırılmalı, ve iki çekerli arabalar volanın yarısıyla yeniden sürülüp süpürülmeli: bu düzeltme hepsini daha çevik yapar |
| 19 | 2026-11-03 | **Araç kontrolcüsü tekerlek kuvvetlerini sırayla uyguluyor, bu yüzden sağa ve sola dönüş birbirinin aynası değil.** `update_vehicle` her tekerlekte süspansiyon ve lastik kuvvetini `apply_force_at_point` ile gövdenin hızına **hemen** işliyor (`vel.linear += F/m·dt`, `vel.angular += I⁻¹·τ·dt`), sonraki tekerlek temas noktası hızını bu güncellenmiş hızdan hesaplıyor (Gauss-Seidel). Sonuç tekerlek sırasına bağlı, ve dört köşenin hiçbir sırası kendi aynası değil | Tezgâhın ayna testi: simetrik bir arabada (görsel merkez 0,0000, tekerlekler ±aynı) sola ve sağa aynı daire. Bir önden çekişlide yük farkı %7–12, yanal tutuş farkı %5'e kadar (100 km/h, yarım direksiyon: 10,45 / 9,89 m/s²). **Doğrulandı:** her aksın sol ve sağ tekerleğinin sırası yer değiştirilince sol ve sağ sonuçlar birebir yer değiştiriyor (10,45 / 9,89 → 9,89 / 10,45, yükler aynı sayılarla). Bazı arabalarda %3'ün altında kalıyor; büyüklük arabaya bağlı | **`rig::vehicle_step`** (2026-11-03): her adımda her aksın sol/sağ tekerleği kontrolcü çağrısının *etrafında* yer değiştiriyor ve hemen geri alınıyor — yan, 120 Hz'te işaret değiştiriyor, ikinci derece kalıyor; `wheels[i]` okuyan hiçbir şey sırayı ters görmüyor (eski itiraz buydu), motor tekerlekleri indeksle değil `axle_type`/`is_left` ile eşliyor. Tezgâhın ayna satırlarının hepsi "ayna", iki ondalığa kadar (PEUGOT, 240SX). `NFS_WHEELORDER=fixed` eski sırayı geri getiriyor | `gizmo-physics-dynamics` (`vehicle/dynamics.rs`, `apply_force_at_point` çağrıları) | **kapandı** (pin `fd8825dd`, 2026-11-17): `rig::vehicle_step`'teki teker sırası çevirmesi kaldırıldı. Ayna tezgâhı (`NFS_BENCH=mirror`) çevirmesiz de son haneye kadar ayna. Süpürme −78 (1594 → 1516), ama bu farkın fiziksel karşılığı yok: kuvvetlerin toplanma sırası değişince yörünge kaotik olarak sapıyor (aşağıdaki gürültü tabanına bak). önceki durum: main'de kapandı (`256aa377`: teker kuvvetleri adım sonunda bir kez uygulanıyor, simetrik araba sağ/sol bit düzeyinde ayna; 2026-11-17'de doğrulandı). Pin yükselince `rig::vehicle_step`'teki teker sırası çevirmesi kaldırılacak, süpürme tabanı yeniden alınacak (yörüngeler bit düzeyinde değişti). Önceki durum: oyun tarafında kapandı, motorda açık — motordan istenen değişmedi: tekerlek kuvvetleri adım içinde biriktirilip gövdeye bir kez uygulansın (Jacobi). **NFSU2 böyle yapıyor** (`SPEED2.EXE`, REA ile okundu): `RigidBody::AddForceAtPoint` `0x59bb00` kuvveti ve torku yalnız `+0x110`/`+0x120` biriktiricilerine ekliyor, integratör `0x59c8xx` onları adım başına bir kez harcayıp sıfırlıyor. Motor düzelince `vehicle_step`'in çevirmesi kaldırılır; doğrulama aynı: `NFS_BENCH=mirror nfs_bench` her satırda "ayna" |
| 20 | 2026-11-03 | **Çözücüde kinematik bir gövde sonsuz kütleli, ve bir çifte "bu ikisini sonlu kütleyle çöz" denemiyor.** Yeri kodla belirlenen (raydaki) bir arabaya çarpan sürülen araba bir duvara çarpmış gibi tepki alıyor | Raydaki rakipler. Tezgâh (`NFS_BENCH=crash`): 15 m/s ile duran bir rakibe çarpan oyuncu **−90 m/s**'ye fırlatılıyor ve 58 rad/s dönüyor; 25 m/s ile 10 m/s'lik rakibe çarpınca geri sekiyor (−2 m/s), momentumun %23'ü kalıyor | **`rig::bump`**: rakibin collider'ı `CollisionLayer` ile her şeye kapalı (maske 0), oyuncu–rakip teması oyun tarafında NFSU2'nin kendi kuralıyla çözülüyor (`SPEED2.EXE`: `0x5af0e0` iki gövdeli impuls, `e` 0,1, temas başına hafif arabaya 0,3 m/s sınırı, 1,85 t tepki kütlesi, dönmeye 0,4 pay) ve rayın durumuna (`Rail::bump`) yazılıyor. Momentum %93–97 korunuyor, sekme yok | `gizmo-physics-rigid` (çözücü, gövde türleri) | **main'de kapandı** (`fd8825dd`: `RigidBody::contact_mass`, hız değişimi `RigidBody::contact_response`'ta; 2026-11-17'de doğrulandı). Pin yükselince `rig::bump`'taki maske-0 + elle impuls çözümü bununla değişebilir. Önceki istek: — motordan istenen: hızı dışarıdan sürülen ama temasta sonlu kütle sayılan bir gövde türü (Bullet/PhysX'te "kinematic with mass" ya da kütle oranı ölçekleme), ya da çift başına ters kütle ölçeği. Gelirse `bump` yalnız NFSU2'nin sınırlarını uygulayan bir süzgece iner |
| 21 | 2026-11-03 | **Aktarma organının dört kolu yok:** (a) debriyaj hep kilitli, vites **anında** değişiyor — geçiş süresi ve geçiş sırasında tork kesilmesi ifade edilemiyor; (b) devir `max_rpm`'de yalnız **kırpılıyor**, tork kesilmiyor — kesici yok, son viteste devir sınırı yok; (c) `engine_torque()` hep `≥ 0` (`nm · |gaz|`) — gaz kesilince motor **frenlemiyor**; (d) sürüklenme katsayısı gaza bağlanamıyor. Ayrıca `AeroPackage::default()` **Cl −0,8** taşıyor: aero'yu ayarlamayan bir oyun farkında olmadan kareli bir downforce alıyor | `SPEED2.EXE`'nin motor ve aktarma kodu okunurken (`0x594300` net krank torku, `0x5A1400` debriyaj durum makinesi, `0x5AA760` kesici, `0x59E620` sürüklenme). Oyunda: stok bir vites değişimi 0,5 s (ilk üçte biri torksuz, kalanı %70 gaz), limitörün 25 devir altında yakıt kesiliyor, gaz kesilince `(1−gaz)·Tfren(rpm)` motor freni, sürüklenme `(2−gaz)·k` | **`rig::powertrain`**: motorun adımının etrafında — önce gaz geçiş evresine ve kesiciye göre kısılıyor, `aero.drag_coefficient` kaydın `k`'sından ve gazdan yazılıyor, aşağı/yukarı geçiş devirleri vitese göre yazılıyor; sonra motor freni tahrik tekerleklerinin `ω`'sından (etkin ataletleriyle) düşülüyor ve sürücünün gazı geri konuyor. `NFS_POWERTRAIN=off` kapatıyor; açılınca pilotun viraj gaz kesmesi 0,75 → 0,3 ayarlandı. Cl −0,8'e **dokunulmadı**: tutuş ve downforce onunla birlikte sürülerek ayarlandı | `gizmo-physics-dynamics` (`vehicle/mod.rs` `VehicleTuning`/`VehicleController`, `vehicle/dynamics.rs`) | **main'de kapandı** (`51fa4aef`: `VehicleTuning::shift_time`, `shift_torque_share`, `fuel_cut_rpm`, `engine_brake_curve`, `VehicleController::shift_to`, `extra_crank_torque`; 2026-11-17'de doğrulandı); pin yükselince alınacak. Önceki istek: — motordan istenen: `VehicleTuning`'e `shift_time` (ve geçiş sırasında tork payı), `fuel_cut_rpm`, motor freni eğrisi (`engine_brake_curve`, `(rpm, N·m)`), ve sürüklenmenin gaza bağlanabileceği bir kol ya da gövdeye kontrolcü içinden kuvvet ekleme kancası. Hepsinin varsayılanı bugünkü davranış |
| 22 | 2026-11-03 | **(a) `PhysicsMaterial::default()` esnekliği 0,3 ve `Max` ile birleşiyor** — `Max` her şeyi yendiği için malzemesi verilmemiş bir gövde, `Min` isteyen bir zemine (`ASPHALT` 0,05) bile 0,3 ile sekiyor; varsayılan, karşı tarafın seçimini sessizce eziyor. **(b) Bir gövdenin temas impulsuna dönme tepkisini ölçekleyen kol yok** — impulsun açısal payı hep 1 | Duvara çarpan arabanın sekmesi ve dönmesi (`NFS_BENCH=wall`): her açıda e ≈ 0,3 (eğik vuruşta 0,64), 30°'de 15 m/s ile değen araba 1,3 rad/s dönüyor. NFSU2: gövde esnekliği 0,1, sürtünme 0,3, çözücü küçüğünü alıyor, impulsun dönmeye giden payı 0,4 | `rig::body_material` (0,1 / 0,3, ikisi de `Min`) şasi collider'ında; `rig::physics_step` fizik adımında temasın açısal hıza kattığının 0,4'ünü bırakabiliyor (sönüm ayrılarak; pilot süpürmesini −105 gerilettiği ve zemin temasını ayıramadığı için `NFS_SPINSHARE=0.4` ile isteğe bağlı) | `gizmo-physics-core` (`physics_material.rs`), `gizmo-physics-rigid` (çözücü) | **kapandı** (pin `fd8825dd`, 2026-11-17): gövdeye motorun `RigidBody::contact_spin_share = 0,4`'ü veriliyor (`rig::contact_spin_share`, `NFS_CSS`); oyunun adım etrafındaki `NFS_SPINSHARE` kesimi kaldırıldı. Duvar tezgâhı: 30° 15 m/s'de sapma 1,53 → 0,64 rad/s, 60° 25 m/s'de 3,82 → 2,03. Oyunun eski kesimi zemin temasını da kesiyordu ve süpürmeyi −105 geriletmişti; motorunki −57, gürültünün içinde. önceki durum: main'de kapandı (`51fa4aef`: (a) varsayılan malzemenin `restitution_combine`'ı `Average`, `Min` isteyen taraf kazanıyor; (b) `RigidBody::contact_spin_share`, NFSU2 için 0,4; 2026-11-17'de doğrulandı). Pin yükselince `rig::physics_step`'in açısal hız düzeltmesi (`NFS_SPINSHARE`) bununla değişecek. Önceki istek: — motordan istenen: varsayılan malzemenin `restitution_combine`'ı `Average` (ya da `Max`'ın önceliği kaldırılsın); `RigidBody`'e temas impulsunun açısal payı (`contact_spin_share`, varsayılan 1) |
| 23 | 2026-11-03 | **Krank torkuna eğrinin dışında bir terim eklenemiyor.** `engine_torque()` `eğri(devir) · |gaz|` ve başka bir girişi yok; geçici bir ek tork (nitro, turbo basıncı, bir ceza çarpanı) ancak `throttle_input`'u 1'in üstüne çıkararak veriliyor. Bu çalışıyor çünkü gaz başka hiçbir yerde kırpılmıyor — ama bu, belgelenmiş bir sözleşme değil, bir rastlantı | Nitro (`rig::powertrain::Nitro`): oyunun ek torku `gaz · şekil(devir) · T` | Adımdan önce `throttle_input = gaz · (1 + ek / eğri(devir))`, adımdan sonra sürücünün gazı geri konuyor | `gizmo-physics-dynamics` (`VehicleController`): ek bir `extra_crank_torque` alanı ya da `engine_torque`'a bir kanca | **kapandı** (pin `fd8825dd`, 2026-11-17): nitro `extra_crank_torque = ek · gaz` yazıyor, gaz 1'in üstüne itilmiyor. Nitrolu kalkış aynı: 0-100 4,9 s, 60 s'de 221,5 km/h (6953 → 6975 rpm; gazı okuyan yollar artık gerçek gazı görüyor). önceki durum: main'de kapandı (`51fa4aef`: `VehicleController::extra_crank_torque`; 2026-11-17'de doğrulandı). Pin yükselince nitro `throttle_input`'u 1'in üstüne itmek yerine bunu kullanacak. |
| 24 | 2026-11-03 | **Sürtünme çemberine çekiş kuvveti ağırlıklı girmiyor, ve yanal tepe sertliğe dokunmadan kısılamıyor.** `pacejka_combined` çemberi `1,2 · max(d) · Fz` ile kuruyor ve `(Fx, Fy)`'yi eşit sayıyor. NFSU2 gaz verirken `Fx`'i çembere **1,5 katı** sayıyor (`0x59D600`), kesince çarpanı geri almıyor: sınırdaki tahrikli teker gazla yanal kuvvetini kaybediyor (power oversteer). Oyun tarafından ulaşılabilen tek kol `pacejka_lat.d`, ama o eğrinin tamamını, sıfır kaymadaki sertliği de ölçekliyor | Oyunun gazla arka kaydırmasını taşırken (`rig::powertrain::power_grip`) | Tahrikli tekerin `d`'si her adım `√(1 − (1,5r)²) / √(1,44 − r²)` ile kısıldı (`r = tork/r / d·Fz`): tezgâhta (`NFS_BENCH=power`) 240SX ve RX8 sabit virajda bile döndü, çünkü kısılan sertlik dengeyi bozuyor. **Varsayılan kapalı**, `NFS_POWER=1` deneme için | `gizmo-physics-dynamics` (`vehicle/mod.rs` `pacejka_combined`, `Wheel`) | **motorda var, oyunda varsayılan kapalı** (pin `fd8825dd`, 2026-11-17): `Wheel::traction_circle_weight` `rig::traction_circle_weight` ile veriliyor, varsayılan 1,0; `NFS_TCW=1.5` NFSU2'nin kuralı. Süpürme −128 (gürültü içinde) ve oyuncu için his değişikliği, yani sürülerek karar verilecek. `powertrain::power_grip` (`NFS_POWER`) artık gereksiz. önceki durum: main'de kapandı (`51fa4aef`: `Wheel::traction_circle_weight`, NFSU2 için 1,5; 2026-11-17'de doğrulandı). Pin yükselince oyunun `power_grip` geçici çözümü (`NFS_POWER`) kaldırılacak. Önceki istek: — motordan istenen: `Wheel`'e çembere girerken çekiş kuvvetinin ağırlığı (varsayılan 1,0) ya da tekerlek başına ayrı bir çember yarıçapı (`friction_circle_mu`), sertliği değiştirmeden |
| 25 | 2026-11-05 | **Son işlemde önceki kareyi tutan bir geçiş yok, bu yüzden oyunun hareket bulanıklığı yapılamıyor.** `PostProcess` bloom, vinyet, renk sapması, alan derinliği ve gren veriyor; önceki kareyi ya da özel bir tam ekran geçişi kullanmanın yolu yok. | Taşınmadı: oyunda bulanıklık yok. Hız hissi kamera ve sarsıntıdan geliyor. | `SPEED2.EXE` (`0x5CC8B0`, `0x5CC9A0`, `0x5CCA00`; ayar `g_MotionBlurEnable`, `0x870CE0`): kareyi çeyrek çözünürlüğe indirip 4 dokunuşlu bulanıklığı iki kez geçiriyor, sabit alfayla (`0xB0` ya da `0xA0`) üstüne biniyor. Bu üç fonksiyonda hız girdisi yok; alfa üreten bir ftol okunmadı. | `gizmo-renderer` (`components/post_process.rs`, `post_process.rs`) | **main'de kapandı** (`9e740d8b`: `PostProcess::motion_blur`, ve özel geçişler için `RenderStage` kancaları; 2026-11-17'de doğrulandı); pin yükselince alınacak. Önceki istek: — motordan istenen: özel tam ekran geçişi, ya da önceki kareyi girdi alan bir bulanıklık kolu. |
| 26 | 2026-11-17 | **Yüzlerce küçük dinamik cisim kareyi pahalılaştırıyor, uyutmak da çözmüyor.** Şehrin 513 kırılabilir eşyası (kutu çarpıştırıcı, 40–200 kg) hücre başına binlerce üçgenlik statik trimesh'lerin üstünde: kullanıcının ekranında medyan kare 20,8 → 29,1 ms. Hepsini `is_sleeping = true` başlatmak daha kötü: açılıştan itibaren 50 ms'lik tavan. | Kırılabilir eşyalar (`world::props`) | **Varsayılan kapalı**; eşyalar statik ve çarpışmasız çiziliyor, `NFS_PROPS=on` deneme için | `gizmo-physics-rigid` (`pipeline.rs` ada/uyku yolu, `physics_step_system` ECS eşleme) | **motor tarafı kapandı** (pin `0e34a83c`, 2026-11-18): istenen ikisi de geldi — çok sayıda uyuyan cisimde ECS↔fizik eşlemesi atlanıyor (`7221e816`: köprü yalnız değişeni okuyor), oturmuşların alt adım bedeli de (`d4b4681f`, ve statikler ayrı geniş-faz ağacında, `0e34a83c`); motorun ölçümünde 513 uyuyan kutu adım başına 0,26 → 0,11 ms. **Oyunda `NFS_PROPS` hâlâ varsayılan kapalı:** eşyalar yalnız `nfs_cruise`'da (pencereli) var ve kare süresi kullanıcının ekranında ölçülür; `NFS_PROPS=on` ile yeni pin ölçülünce açılabilir. 50 ms tavanın motor adımı olmadığı motor tarafında ölçüldü: bakılacak yer çizim ve eşyaların ilk konumu. önceki durum: main'de kapandı (`befbc9f6`: küçük paralel döngüler çağıran iş parçacığında, 513 uyuyan kutu adım başına 0,3 ms; 2026-11-17'de doğrulandı). Önceki istek: — motordan istenen: uyuyan ve statik çiftlerin adım başına bedelinin ölçülmesi; çok sayıda uyuyan cisimde ECS↔fizik eşlemesinin ve ada kurmanın atlanması |
| 27 | 2026-11-17 | **TAA, gömülü ışıklı gece şehrinde sürüş sırasında ışıkları yanıp söndürüyor.** Kullanıcının ekranında Bayview'da ilerledikçe ışıklar yanıp sönüyordu. Aynı veriyi aynı kurallarla ışıksız çizen Bevy eşinde (`nfsu2-bevy`) sorun yok. Ekranda ikiye bölme: bloom + TAA + SSAO kapalı → durdu; yalnız bloom → temiz; bloom + TAA (SSAO kapalı) → geri geldi. Kaynak TAA geçmişi: büyük olasılıkla küçük, parlak, yüksek kontrastlı öğeler (aydınlık pencereler, neon, tabela yüzleri) geçmiş reddi ile kabulü arasında gidip geliyor. Sebep motorda okunmadı. | Şehir sahnesi (`nfs_cruise`) | geçici çözüm **kaldırıldı** — TAA varsayılan açık, `NFS_TAA=off` eski hal (`NFS_SSAO=off` duruyor) | `gizmo-renderer` (`shaders/taa.wgsl`) | **motorda düzeldi, oyunda açıldı** (pin `37625d72`, 2026-11-19). Sebep: TAA geçmişi komşuluğun renk kutusuna doğrusal ışıkta kırpılıp karıştırılıyordu; titreşimin bir karede yakalayıp bir sonrakinde kaçırdığı piksel altı parlak bir ışık kutuyu duvar ile ışık arasında sallıyor, ışıksız kare geçmişi de aşağı çekiyordu. Artık her renk bir artı parlaklığına bölünerek kırpılıp karıştırılıyor (Karis 2014). Motor testi `taa_holds_small_bright_lights_steady_under_a_moving_camera` (koyu cephe, otuz kat parlak piksel altı pencereler, kayan kamera; parlak piksellerin kareden kareye değişimi, parlaklığına oranla): eski TAA 0,069 — TAA'sızdan (0,054) bile kötü — yeni 0,015. Oyunda kare kare ölçüm sonuçsuz kaldı (otopilot her koşuda başka yerden geçiyor, ardışık kareler zaten çok farklı); kabul gözle. Yanıp sönme yeniden görülürse `NFS_TAA=off` ile ikiye bölünüp madde yeniden açılır. |
| 28 | 2026-11-17 | **Malzeme başına "gölge alma" anahtarı yok.** `baked_lit.wgsl`, gömülü ışığı kameranın 100 m çevresinde güneşin gölgesiyle %45'e kadar karartıyor (`sun_share`). Bayview gece haritası, güneş yalnız araba için var; ama şehrin her direği ve tabelası gece sokağına kamerayla kayan bir gündüz gölgesi düşürüyordu. Malzeme gölge almamayı seçemiyor, yalnız mesh gölge düşürmemeyi seçebiliyor. | Şehir sahnesi | Şehir mesh'leri `ShadowCasting::Off` (`scene::city_shadows`, `NFS_CITY_SHADOWS=on` eski hali). Araba gölge düşürmeye devam ediyor. Yanıp sönmenin kaynağı bu değildi (bkz. #27). | `gizmo-renderer` (`shaders/baked_lit.wgsl`, `components/mesh.rs`) | **kapandı, oyunda değişiklik gerekmedi** (pin `fd8825dd`, 2026-11-17): şehrin gölge düşürmeyip gölge alması (`scene::city_shadows`) arabanın gölgesini yolda tutuyor; şehre `receives_shadows(false)` vermek o gölgeyi de silerdi. `MeshRenderer::receives_shadows` bir gün şehrin gölgesi arabaya düşsün istenirse kullanılacak. önceki durum: main'de kapandı (`00799bff`: `MeshRenderer::receives_shadows`, `baked_lit` dahil; 2026-11-17'de doğrulandı). Pin yükselince şehir mesh'leri `receives_shadows(false)` alıp gölge düşürmeye geri dönebilir (`scene::city_shadows`). Önceki istek: — motordan istenen: malzeme ya da mesh başına `receive_shadows`, ya da baked-lit için `sun_share` kolu |
| 29 | 2026-11-17 | **Çizim yolu her karede her varlığa CPU harcıyor; statik bir şehirde de.** Bayview ~14 600 birleşik mesh (256 m hücre × doku; hücre başına ~68 doku). Şehir merkezinde kare ~15–23 ms: fizik 1,6, update 2–4, kendi komut kaydımız (`default_render_pass`) 6–10, kalanı GPU bekleme ve sunum. `eu-stack` örneklemesi (`NFS_PTRACE=1`), ana iş parçacığı: `collect_draw_items` %31 (her varlıkta `LodGroup::pick` ve `TypeId` özetli `Query::get_inner`), wgpu-core komut kodlama %18 (çizim başına `set_bind_group`/`set_vertex_buffer`), `ensure_global_transforms` %8 (hiç kıpırdamayan varlıkların `update_local_matrix`'i), `sync_material_descriptions` %6 (her karede tüm malzemeler). Hepsi varlık ya da çizim sayısıyla büyüyor. Aynı şehri Bevy eşi (`nfsu2-bevy`, aynı birleştirme) ~130 fps çiziyor. | Şehir sahnesi (`nfs_cruise`) | Oyunun elindeki tek kol varlık sayısı. Kopyalar en ince sürümleriyle şehrin kovalarına katıldı (`NFS_LOD=on` eski LodGroup yolu): 15 120 → 14 614, yani %3. Hücre başına doku çeşitliliği birleştirmeyi sınırlıyor. | `gizmo` (`systems/render/mod.rs` `default_render_pass`, `render/batching.rs`), `gizmo-renderer` (`material_sync.rs`) | **kısmen kapandı** (`3ab4066f`, `e4d56ebe`: motorun kendi testinde `collect_draw_items` 11,7 → 2,8 ms, `sync_material_descriptions` 1,7 → 0,03 ms, `Query::get` FxHash; 2026-11-17'de doğrulandı). Pin `0e34a83c` (2026-11-18) ile statik bir fizik gövdesinin `Transform`'u artık her kare `Changed` işaretlenmiyor (`4a6dfd61`), yani render dünyası onu her kare yeniden kopyalamıyor — motorun ölçümünde 14 600 statik gövdede 3,72 → 0,027 ms; şehrin çizilen ağları fizik gövdesi olmadığından bu yalnız çarpışma hücrelerini kapsar. Çizim başına bağlama maliyeti (doku dizisi / bindless + multi-draw) hâlâ **açık**. Önceki istek: — motordan istenen: statik varlıklar için kirli bayrağı (dönüşüm ve malzeme senkronunu atlamak), karelere yayılan ön toplanmış çizim listesi, ve doku dizisi ya da bindless malzeme ile tek bind group altında çoklu çizim (hücre × doku yerine hücre başına tek çizim). **2026-11-18 ölçümü (pin `d349bdde`, `NFS_DIAG` artık çizimin parçalarını da yazıyor: `laps` satırı):** kullanıcının "120 km/s'de FPS 20'lere iniyor" şikâyetinin kaynağı hız değil, **kadrajdaki çizim sayısı** — aynı noktada 9 km/s'de 72 ms, 179 km/s'de başka yerde 21 ms. Araç durdurularak ölçüldü: 71 çizim 10 ms, 1.952 çizim 23 ms, 6.989 çizim (otoyoldan şehre bakış, 1,5 M üçgen) 47,5 ms. Ana iş parçacığı (`eu-stack`, 89 örnek): bizim kayıt (`collect_draw_items` + geçiş kaydı) %47, **wgpu'nun `CommandEncoder::finish`'i %45** (çizim başına doğrulama/izleme/HAL), `submit` %7, sunum %1 — yani "rest" GPU beklemesi değil, wgpu'nun çizim başına CPU işi; vsync açık/kapalı fark etmiyor (41,4 / 40,5 ms). Çizim başına toplam ~6 µs CPU. **Not:** COSMIC terminali dahili GPU'da açıyor (`DRI_PRIME`, `VK_LOADER_DRIVERS_SELECT=*radeon*`), o terminalden açılan oyun AMD 780M'de koşuyor; `env -u DRI_PRIME VK_LOADER_DRIVERS_SELECT='*nvidia*,*radeon*'` ile RTX 3050'de aynı nokta 35 ms (GPU geçişleri 19 → 11 ms, `view.batch` 10,6 → 4,1 ms — APU'nun ortak güç bütçesi), ama wgpu'nun payı aynı kalıyor. Kök: çizim sayısının kendisi — ortak ağ tamponları + aynı malzemeli çizimleri tek dolaylı çoklu çizimde toplamak (+ bindless doku), Bevy'nin yaptığı. **2026-11-18, motor tarafı (pin `b07d3b41`):** `ea271462` (geniş derinlik aralıklı kameranın frustum'u: 0,1/20.000 m'de uzak düzlem dejenere sayılıp `+Z` düzlemiyle değiştiriliyordu — sahnenin yarısı kesiliyordu; bizim 0,5 m'lik kameramız eşiğin hemen üstündeydi), `617fb75e` (kimsenin okumadığı hız tamponuna forward yüzeyler ikinci kez çizilmiyor), `bcbcd4fa` (opak dünya malzemeye göre sıralı), `bd62df65` (malzeme/ağ kontrolleri yalnız değişeni okuyor — kaydın %37'siydi), `d5771b73` (sıralama anahtarla), `cde818ca` (`MeshArena`: ortak köşe/indeks tamponları), `b07d3b41` (aynı durumu paylaşan ardışık çizimler tek dolaylı çoklu çizim). Otoyol noktası, AMD: 47,5 → 42,0 → 25,8 → 19,3 → **15,9 ms**; NV: 35,0 → 23,8 → 20,1 → 13,6 → **11,9 ms** (son adım `afd1a9d4`: örnekler doğrudan yerine dağıtılıyor, hız okunmuyorsa önceki modeller ne aranıyor ne yükleniyor). `426b7ef5` (aynı tamponları ve malzemeyi paylaşan ağlar tek toplu çizim, ağ aralığı örnek başına: şehirde 6.989 → 3.480 toplu çizim): NV'de beş eşli dönüşümlü koşu, medyan kare 11,4 → **10,8 ms**, toplama 3,4 → 2,8 ms; AMD aynı anda 26,8 → 26,0 ms — o sırada makine yüklüydü (Chrome'da video), AMD mutlak sayıları önceki satırlarla karşılaştırılamaz. Oyun tarafı aynı gün: şehir ağları arenaya **indeksli** konuyor (`MeshArena::indexed`): 8,9 M köşe / 928 MB → 4,3 M köşe + 8,9 M indeks (446 + 36 MB), yükleme süresi aynı; dört eşli koşu, AMD kare 20,8 → **16,1 ms** (GPU forward 8,9 → 5,0), NV 11,1 → **10,9 ms** (GPU forward 3,3 → 2,2). **Ölçüm notu:** tek koşu bu makinede ±%30 oynuyor (aynı ikili 17,0–22,5 ms); karşılaştırma ancak eşli dönüşümlü koşularla. Kalan en büyük kalem `collect_draw_items` (AMD 6,9 ms, NV 3,5 ms: görünür her statik nesnenin örnek verisi her kare yeniden kuruluyor ve yükleniyor — kök çözüm statik örnek verisini GPU'da kalıcı tutmak) ve AMD'de GPU forward (5,8 ms). Bindless doku (koşuları malzeme başına değil, dilim başına tek çizime indirmek) hâlâ açık ama artık küçük kalem. **2026-11-18, pin `17ff5d58`:** `04842a7f` (`GIZMO_MONITOR=<çıkış>` pencereyi o ekranda tam ekran açar, `GIZMO_VSYNC=0` ekranı beklemeden sunar — ölçüm koşuları artık yan ekranda: `GIZMO_MONITOR=DP-1`), `17ff5d58` (toplama her nesnenin görüşten bağımsız kısmını kareler arasında tutuyor, yalnız bileşeni değişeni yeniden hesaplıyor): otoyol noktası, 1080p, vsync kapalı, eşli koşular — AMD toplama 11,5 → 4,0 ms, kare ~26 → ~17–20 ms; NV toplama 3,5 → 2,4 ms, kare ~12 ms. Aynı nokta Bevy eşi: AMD 8,6–9,4 ms, NV 5,6–6,1 ms. **Kalan farkın kaynağı ölçüldü:** (1) Forward geçişi GPU'da parça gölgelendirmeye bağlı değil — `baked_lit.wgsl`'in yalnız doku döndüren hali, `discard`'sız ve gölgesiz halleri forward'u %10'dan az kısaltıyor; köşe 104 → 48 bayt deneyi forward'u AMD 6,2 → 5,1, NV 3,1 → 2,7 ms yapıyor ama kareyi kısaltmıyor. (2) **Kareyi tutan, malzeme başına bağlama grubu:** NV'de ana iş parçacığının %32'si wgpu'nun `CommandEncoder::finish`'i (`set_bind_group`/`merge_bind_group`/doku kullanım takibi). Deney (oyunda bütün dokular tek bağlama grubu, `NFS_ONETEX`, commit'lenmedi): çizim 3.872 → 45, AMD kare 17 → **10,6 ms**, NV 11,9 → **7,2 ms** — Bevy'nin bindless malzemeleriyle aynı sınıf. Kök çözüm motorda: dokuları paylaşılan doku dizilerine koymak (aynı boyut/biçimdekiler bir dizide, katman örnek başına) ya da bindless; böylece aynı ağ tamponlarını paylaşan şehir tek dolaylı çoklu çizim olur. **Kapandı, pin `209e7c1b` (2026-11-18):** `e5e8a3b8` motorun `TexturePool`'u — aynı boyuttaki dokular paylaşılan dizilerde, dizi başına tek bağlama grubu, katman örnek başına (`InstanceRaw::texture`). `nfs_cruise` şehir dokularını havuza koyuyor (`renderer.texture_pool.colour`, `Material::from_pool`; `NFS_POOL=0` eski yol). Otoyol noktası, eşli koşular: çizim 3.872 → 440, AMD kare ~18,2 → **~11,8 ms**, NV ~11,4 → **~5,9 ms** (Bevy eşi: 8,6–9,4 / 5,6–6,1 ms). AMD'de artık GPU sınırı: forward 6,4, son işlem ~2,4, G-buffer ~1,3 ms. **Pin `7e4828e4` (2026-11-19):** köşeler GPU'ya paketli gidiyor (`GpuVertex`, 104 → 60 bayt): forward AMD 7,0 → **4,5 ms**, NV 3,2 → 2,8 ms, şehrin köşe tamponları 446 → 257 MB (arena satırı artık `ArenaStats::vertex_bytes`). Kalan AMD farkı (~2–3 ms) Bevy eşinin çalıştırmadığı efektlerde: son işlem ~2, araba için G-buffer + ertelenmiş aydınlatma ~1,2, gölge 0,7, SSAO 0,4 ms. |
| 30 | 2026-11-18 | **Motorun ürettiği mip seviyeleri dokunun kendisi değil.** `mipmap.wgsl` yazdığı seviyenin pikselini okuduğu seviyenin boyutuna bölüyordu (iki kat büyük), koordinat 0,5'te duruyordu: her seviye bir öncekinin sol üst çeyreği, büyütülmüş; `n`. seviye görüntünün sol üst `1/2ⁿ`'i. Motorun mip zinciri ürettiği her doku (glTF, çözülmüş/yordamsal dokular, `create_texture`) ilk mip'in seçildiği uzaklıktan ötede yanlıştı | Kullanıcı: "tabelalar dibine girince doğru oluyor; sadece arabalarda değil, detay çizimlerinde". Başsız seri, aynı doğru üzerinde: "27 NORTH" tabelası 8 m'de doğru, 15 m'de kendi kalkanının hayaleti, 30 m'de yalnız kalkan, 60 m'den sonra gri köşe. Dokular dosyada doğruydu (256×128, saydamlık yok), aynı düzlemde başka nesne yoktu | yok | `gizmo-renderer` | **kapandı** (2026-11-18) `d349bdde` — her texel 2×2 bloğunun merkezinden örnekliyor; test 32×16 dokunun her seviyesini CPU'nun 2×2 ortalamasıyla texel texel karşılaştırıyor, eski gölgelendiricide 1. seviyenin ilk texel'inde düşüyor |
| 31 | 2026-11-19 | **Kümeli ışık ataması 256 ışıkta CPU'da 0,6 ms.** Şehrin 2.233 lambası nokta ışık olunca ışık dizisi doldu ve karenin hazırlığı (`view.setup`) 0,7 → 2,0 ms oldu; 0,1 ms'i ışık toplama, kalanı kümeleme. Her kümenin sınır kutusu her ışık ve her derinlik dilimi için baştan kuruluyordu (256 ışıkta ~40 000 kutu, ızgara 3.456). | Şehir sahnesi (`nfs_cruise`), lambalar açık | yok | `gizmo-renderer` (`clustered.rs`) | **kapandı** (pin `001ad491`, 2026-11-19): kutular kare başına bir kez kuruluyor; motorun tezgâhında 256 ışık 0,75 → 0,14 ms, oyunda `view.setup` lambalı 2,0 → 0,8 ms (lambasız 0,5). Atama küme küme aynı (motor testi eski kurulumu referans tutuyor). Kalan lamba maliyeti ıslak modda GPU'da: yolun lambaları yansıtması forward'a +1,0 ms (AMD 780M) — efektin kendisi. |
| 32 | 2026-11-19 | **Baked-lit yüzey dinamik ışığın doğrudan ışığını almıyordu.** `baked_lit.wgsl` yüzeyin ışığını yalnız bake'ten alıyordu; dinamik ışıklardan yalnız ıslak yüzeyin yansımasını ekliyordu. Şehir baked-lit olduğu için bir arabanın farları kuru yolu hiç aydınlatamazdı. | Şehir sahnesi, farlar | yok (farlar yoktu) | `gizmo-renderer` (`baked_lit.wgsl`, `deferred_lighting.wgsl`), `gizmo` (`systems/render/shared.rs`) | **kapandı** (pin `9ae1820e`, 2026-11-19): her ışık `in_bake` taşıyor — `true` olanın doğrudan ışığı bake'te sayılıyor (şehrin lambaları, sahnenin dolgu ışığı), `false` olan (varsayılan) baked yüzeyi her yüzey gibi aydınlatıyor. Oyunda: `world::lights` lambaları ve `scene::add_lights`'ın dolgusu `in_bake`; farlar `scene::spawn_headlights` (gövdeye çocuk iki spot, camların yeri `CarVisuals::headlights`, arabanın kendi `HEADLIGHT` parçalarından) ve kısa huzme deseni bir cookie (`scene::headlight_cookie`: ufkun üstünde parıltı, 17 m'deki yolu gören açıda tepe, altında `(tepe/açı)^2,5`). Desensiz koni tamponun dibindeki yolu beyaza çıkarıyordu (ters kare); tepeyi ufka koymak öndeki arabanın arkasını 3.000 kat aydınlatıyordu (dik yüzey `I/d²`, yol `I·h/d³`). Maliyet: 16 farın kuru şehirde GPU forward bedeli önce 1,25 ms; motorda koni dışı ışığı atlamak ve spotu konisini saran küreyle kümelemekle 0,5 ms; kare +0,1 ms (AMD 780M, eşli koşular). Öndeki arabanın alt tamponu yakında hâlâ parlak — fizik bu; sabit pozlamada belirgin. |
| 33 | 2026-11-20 | **`Renderer::exposure` hiçbir şey yapmıyordu.** Belgesi "ton eşlemeden önce uygulanan pozlama" diyordu, varsayılanı 1,15'ti; ama hiçbir çizim yolu onu okumuyordu — motorun tek pozlama ayarı kameranınki (`Camera::exposure`). Gece sahnesini karartmak için 0,75 ve 0,6 verildi, karenin medyanı ikisinde de kıpırdamadı. | Şehir sahnesi, gece pozlaması | yok | `gizmo-renderer` (`renderer/mod.rs`) | **kapandı** (pin `e05b1063`, 2026-11-20): alan kaldırıldı. Oyunda pozlama kameraya veriliyor (`scene::set_exposure`, `scene::city_exposure`, `NFS_EXPOSURE`); kullanıcının seçtiği 0,5 ile medyan 33 → 12. |
| 34 | 2026-11-20 | **Tek pozlama farların altındaki tamponu patlatıyordu; göz uyumu yanlış ölçüyordu.** Öndeki arabanın tamponu oyuncunun farlarıyla 5 m'den aydınlanınca yoldan onlarca kat parlak (dik yüzey `I/d²`, yol `I·h/d³`): gece sokağını karanlık tutan pozlama onu bej bir lekeye çeviriyordu. Motorun göz uyumu (otomatik pozlama) da karenin **doğrusal** ortalamasıyla ölçüyordu — birkaç parlak piksel bütün sokağı karartıyordu; Reinhard'ın 0,18 anahtarı log-ortalama için tanımlı. | Şehir sahnesi, farlar | yok | `gizmo-renderer` (`local_exposure.rs`, `luminance.rs`, `auto_exposure.rs`, `post_process.wgsl`) | **kapandı** (pin `c86b0dcb`, 2026-11-20): yerel pozlama (UE5'in bilateral ızgarası: her piksel kendi parlaklığındaki komşularının ortalamasına göre pozlanıyor, hale yok) ve log-ortalamayla ölçen göz uyumu. Oyunda: `scene::enable_local_exposure` (vurgu kontrastı 0,2, pivot 0,15; `NFS_LOCAL`), `scene::enable_night_eye` (anahtar 0,055 — başlangıçta göz ≈ 1 —, aralık 0,35–1,5; `NFS_EYE=off`). Tampon şekli ve stop lambalarının rengi geri geldi (karenin p99'u 223 → 157), şehrin vitrinleri yerinde, medyan aynı. |
| 35 | 2026-11-20 | **Hacimsel ışık şehrin lambalarıyla sise dönüyordu ve her adımda bütün ışıkları dönüyordu.** Motorun havası yerel ışıkları ters kare ve faz olmadan, yarıçap içinde `(1 − d/r)²` ile saçıyordu: 13 m'deki 1.300 şiddetli lamba havayı yarıçap boyunca neredeyse dibindeki kadar aydınlatıyordu; en ince saçılmada karenin medyanı 13 → 113, bütün kare beyaz sis. Ayrıca yürüyüş her adımda karedeki 256 ışığın hepsini dönüyordu. Oyun hacimsel ışığı kapalı tutuyordu. | Şehir sahnesi, gece pusu | hacimsel ışık kapalıydı | `gizmo-renderer` (`shaders/volumetric.wgsl`, `volumetric.rs`) | **kapandı** (pin `32d0bd93`, 2026-11-20): hava yüzey gibi aydınlanıyor (ters kare, pencere, faz), her adım parçasında ters kare tam integralleniyor, gerisi eşaçılı örnekleniyor (TAA ortalıyor); yürüyüş yalnız kendi kümesinin ışıklarını dönüyor. Oyunda `scene::set_haze`: kuru 0,0003, ıslak 0,0008 saçılma, 5 adım, 70 m (`NFS_HAZE=<saçılma>` / `off`); lambaların başında yumuşak hale, farların önünde huzme. İlk sürüm haleyi her pikselde tek rastgele noktadan topluyordu: TAA ancak kamera dururken ortalayabildi, sürüşte lambalar toz tanesi gibiydi (kullanıcı) — `3461c2d9` ile parça başına üç noktalı Gauss–Legendre, deterministik. Islakta GPU bedeli 1,9 ms (AMD 780M), kuruda kare CPU sınırında olduğundan görünmüyor. |

## Pini yükseltme kontrol listesi

**0. Baseline'ı al — yükseltmeden ÖNCE.** Sekiz-rota süpürmesini koştur ve sayıları diske yaz
(`BASELINE-SEKIZ-ROTA.md`). Motorun kendi determinizm hash'i bu iş için kullanılamaz: o kendi 200
kutuluk yıkım sahnesini kilitliyor, bu şehri değil. Yükseltmeden sonra alınan bir ölçümün
karşılaştıracak bir şeyi kalmaz. Süpürmeden önce `cargo build --release` çıktısını doğrula —
bayat binary eski sayıları sessizce tekrarlar ve "etki yok" diye okunur.

**0b. `rev`'in erişilebilirliğini doğrula.** `git -C ../Gizmo branch --contains <rev>` boş
dönmemeli. Erişilemeyen bir hash bir pin değil, bir önbellek bahsidir — 2026-08-20'de tam olarak
bu oldu, yukarıya bak.


1. **Ne değişmiş, gör:** `git -C ../Gizmo log --oneline 48ac99e..main`
2. **Kuyruğu tara:** yukarıdaki `açık` maddelerden hangileri o aralıkta kapanmış? Kapananları
   `pin yükselince doğrula` yap — henüz `kapandı` değil, doğrulanmadan kapanmaz.
3. **`rev`'i değiştir** (`game/Cargo.toml`) ve bu dosyanın Pin tablosunu güncelle. `cargo update`
   gerekmez; `rev` değişimi tek başına yeter.
4. **Derle ve test et:** `cargo build --release -p nfsu2` ve `cargo test -p nfsu2`.
   Motor API'si kırıldıysa burada derleme hatası olarak çıkar — sessiz davranış değişikliği değil.
5. **Gözle doğrula:** `nfs_drive`, `nfs_race`, `nfs_city`. Fizik ve gölge en çok kayan iki taraf.
   Araba için `NFS_AUTODRIVE=1 NFS_DIAG=1 nfs_race` (240SX: 1220 kg, dört teker yerde, tork yalnız
   arka aksta).
6. **Geçici çözümleri geri al:** kuyrukta "oyundaki geçici çözüm" dolu olan her kapanmış madde bir
   silme işi demektir. Silinmezse motor düzeldiği hâlde oyun eski yolda kalır.
7. Doğrulanan maddeleri `kapandı` yap, tarihini ve motor commit'ini yaz.

## İlgili

- `ROADMAP.md` §5 — motorun oyundan bağımsız olarak kazanacakları (uzun vadeli istek listesi;
  bu dosya ise **pinlenmiş sürümde şimdi canımızı yakan** şeylerin listesi).
- `ROADMAP.md` §6 — motor/oyun sınırı. Bir maddenin Gizmo'ya mı yoksa oyuna mı ait olduğunu
  tartışmadan önce oku.
