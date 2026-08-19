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
| commit | `58dc26232b50d12eb0554317922c5d59bf5a6ae5` — `main`, *"chore(media): logonun geniş kaynak sürümü depoya alındı"* |
| sabitlendi | 2026-08-20 (önceki: `09c948a9` aynı gün — erişilebilirlik düzeltmesi, aynı ağaç; `3433aefe`, 2026-08-14; `48ac99e`, 2026-08-12; `ba969c0`, 2026-08-11) |
| nerede yazılı | `game/Cargo.toml` → `rev = ...`; `Cargo.lock` aynı commit'i ayrıca kaydeder |

**2026-08-11 yükseltmesi.** `4d1a8cb..main` beş commit ve ikisi doğrudan bu dosyaya cevap:
`1dcab55` *"engine: close six gaps a game hit against a pinned build"* — 1, 3, 4, 5, 6 ve 7
numaralı maddeleri, bizim ölçtüğümüz sayıları alıntılayarak kapatıyor; `ba969c0` madde 2 için
uzamsal indeks getiriyor. Yükseltmenin oyun tarafına maliyeti iki satır oldu: `Vertex::color`
artık `[f32; 4]`.

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
| 3 | 2026-08-09 | `BakedLit` **çıplak bir çarpım zinciri**: `vertexRengi × instanceAlbedo × doku`, üstüne gölge terimi. Kazanç yok, emissive yok, ambient taban yok, sis yok — karanlık içeriği kaldıracak hiçbir kolu yok. Üstüne ACES toe'su (`post_process.wgsl:108`, `x→0`'da `aces(x) ≈ 0,214·x`) karanlıkta 4,67× kısıyor ve `exposure = 1,15` (`post_process.rs:151`) bunu telafi etmiyor. **Dikkat:** orta tonlarda boru hattı düz gamma modulate'e neredeyse eşit (128×128 → 62,8'e karşı 64,3), yani sorun tonemap'in yanlışlığı değil, **ayarlanabilir olmaması** | Bayview: pencerenin medyanı **1/255**, ortalama 8,5/255. Yol ekranda 2–14/255. Uçtan uca: vcol 128 × doku 128 → 63; 96×96 → 21; **64×64 → 2,6**; vcol 128 × doku 255 → 164 (parlayan pencereler) | yok | `gizmo-renderer` | **kapandı** (2026-08-12) `1dcab55` — `Material`'a ambient ve emissive eklendi; tonemap'e dokunulmadı (bizim orta-ton ölçümümüz sayesinde). **Kollar artık çevrildi:** `scene::city_lift` politikayı tek yerde tutuyor, üç binary de oradan okuyor, `NFS_AMBIENT`/`NFS_EMISSIVE` ile geçersiz kılınabiliyor. Varsayılan ambient `(0.10, 0.11, 0.14)`, süpürmeyle seçildi: sokak seviyesinde kare medyanı 54 → 68, p05 24 → 30, **p95 her ayarda 155'te sabit** — parlak uç dokuyla belirlendiği için kaldırma orada oransal olarak sıfır, yani neon ve yanan pencereler ayrışmasını koruyor. 0.24 gözle elendi (gece değil kapalı hava okuyor) |
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
