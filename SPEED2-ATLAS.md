# `SPEED2.EXE` atlası — oyunun tamamı, adresleriyle

Hedef: NFSU2 v1.2 `SPEED2.EXE` (sha256 `f9dd86c0…`, korumasız; küçük harfli `speed2.exe` v1.0 +
kopya koruması, kullanılmıyor). Sembol yok, RTTI yok (yalnız beş `std` istisna sınıfı). Bu belge
exe'nin **haritası**: hangi adres aralığı hangi alt sistem, hangi chunk'ı kim yüklüyor, hangi ekran,
hangi kamera, oyun döngüsü nasıl dönüyor — ve her alt sistem için ya okunduğu yer (ROADMAP bölümü,
PryHUB modülü) ya da "okunmadı".

Derin okumalar burada tekrarlanmıyor; fizik, lastik, direksiyon, motor, nitro, stil puanları, kamera
koltuğu, rakip becerisi ve lastik bandı `ROADMAP.md`'nin "`SPEED2.EXE`'den" bölümlerinde.

**Nasıl üretildi.** `tools/speed2-atlas.py` (objdump + exe'nin kendi baytları, ~4 s):
her dizgeyi onu adresiyle kullanan fonksiyona bağlar, chunk yükleyici kayıtlarını, vtable'ları ve
FEng ekran tablosunu çıkarır. Derin noktalar REA/Ghidra 12.1.4 decompile'ı ile okundu (15 161
fonksiyon, 7 158 tanımlı dizge). Kanıt düzeyi: **okundu** = decompile/assembly'de görüldü;
**çıkarım** = ad, konum ya da veri şekli söylüyor, kod yürütülerek doğrulanmadı.

```
tools/speed2-atlas.py "$EXE" regions 64      # 64 KiB'lık dilimler ve sözlükleri
tools/speed2-atlas.py "$EXE" chunks          # chunk id → yükleyici / kullanan fonksiyonlar
tools/speed2-atlas.py "$EXE" string 'Mover$' # dizge → fonksiyonlar
tools/speed2-atlas.py "$EXE" funcs 'drift'   # fonksiyon → dokunduğu dizgeler
tools/speed2-atlas.py "$EXE" vtables         # vtable'lar, kurucularının dizgeleriyle
```

---

## 1. Bölümler ve büyük resim

| Bölüm | VA | Boyut |
|---|---|---|
| `.text` | `0x401000–0x782AF1` | 3,6 MB |
| `.rdata` | `0x783000–0x7E71DB` | 401 KB (vtable'lar, float sabitleri, `0x19930520` FuncInfo'ları) |
| `.data` | `0x7E8000–0x8BB5F8` | 848 KB, 212 KB'ı dosyada (FEng tablosu, statik nesneler) |
| `.rsrc` | `0x8BC000` | simge, sürüm |

`.text` içinde oyun kodu ile statik bağlanmış kütüphaneler ayrık bloklar halinde:

| Aralık | Ne | Kanıt |
|---|---|---|
| `0x401000–0x430000` | **AI ve trafik**: AIGoal/AIAction/FuturePoint/Pathfinding havuzları (`0x4035D0`), TrafficMover (`0x4087B0`), StreamingTrafficCarManager (`0x412720`), RandomEncounterManager (`0x412E40`), yeşil ok/işaretler | dizgeler |
| `0x430000–0x440000` | Animasyon: AnimCtrl, CAnimPart/Skeleton/NFSAnimBank (`0x4313B0…`), WorldAnim (`0x4385B0`), kapı/kaput animasyonları | dizgeler |
| `0x440000–0x456000` | **Kamera**: mover sınıfları, CameraAI, `.moo` kamera dosyaları (`0x44A7A0`: `RouteCameras.moo`, `GenericCameras.moo`, `SceneCameras.moo`), ana bellek havuzu (`0x440360` "Main Pool") | §5 |
| `0x456000–0x488000` | **Ses**: araç sesi (`SND ShiftDisengage`, `SND: Nitrous`, yol gürültüsü `0x45FB00`), mikser havuzları (`0x4678C0`), hava/yağmur/sıçrama | dizgeler |
| `0x488000–0x496000` | Acid parçacıkları (`0x4886E0`), eModel/ePoly/eStreamingPack havuzları, **katı model ve TPK yükleyicileri** (§3) | chunk kaydı |
| `0x496000–0x570000` | **Ön yüz (FEng)**: 157 ekranın sınıfları, garaj, özelleştirme, Dyno, harita, kariyer, online menüler | §4 |
| `0x570000–0x582000` | **Oyun akışı ve çekirdek**: dosya kuyruğu (`0x5736D0`), kaynak dosyaları (`0x570F80`), tekrar (ReplayManager `0x57AD80`), giriş (`0x579550`), WinMain `0x580E00`, ana döngü `0x5811C0` | §2 |
| `0x582000–0x590000` | Dil/metin, kalıcı veriler, oyun ayarları | dizgeler |
| `0x590000–0x5B6000` | **Fizik**: RigidBody/Suspension/Wheel/Engine/DriveTrain/Aerodynamics/TwoWheelAckermanSteering havuzları (`0x598F10`), çarpışma (`0x592B00`, `0x593DB0`, `0x5A0200`), PhysicsMover (`0x5A47B0`), MellowMover (`0x5A2900`), BeltMover (`0x59E930`), kırılabilir nesneler (`0x5B0E30`, `0x5B4220`) | ROADMAP fizik bölümleri |
| `0x5B6000–0x5D3000` | **Render (eView/eSolid/eTexture)**: doku kovaları, ışık malzemeleri (`0x5BB700`), D3D9 sarmalayıcı, `g_textureHeaderPool`/`g_vertexBufferHeaderPool` (`0x5D21F0`), ekran efektleri (`seeulator`, `filtertexture`, gökyüzü morph) | dizgeler |
| `0x5D3000–0x5EB000` | **Dünya/pist**: TrackStreamer (`0x88EB00` nesnesi), topoloji ağacı (`0x5D8350`), TrackPath (`0x5DECB0`), olaylar (`0x5D7AF0`), yol ızgarası, sis/çalı sallanması (`FernShaker 0x5E5670`) | §3 |
| `0x5EB000–0x606000` | **Yarış/oyun dünyası**: oyuncu, kamera yöneticisi (`0x5F1FD0`), sürükleme/drafting, drift, yarış başlangıcı, gece/gündüz, duraklat, yükleme ekranı, tik/zaman (`0x601A60`) | §2, §5 |
| `0x606000–0x640000` | **Araba**: CarTypeInfo (`0x610110` GetCarTypeInfo, `0x610130` FindByHash), araba yükleme havuzları (`0x61C700`), parça modelleri (`0x60C680`), çıkartma/doorline maskeleri, kabin/neon/ICE | memory `speed2-exe-anchors` |
| `0x640000–0x6E0000` | **Kütüphane**: D3DX HLSL/efekt derleyicisi (hata metinleri "shader", "register", "sampler") | dizgeler |
| `0x6E0000–0x710000` | **Kütüphane**: EAGL animasyon (`EAGLAnim Memory Pool`), EA RealCore 6.23 dosya sistemi (`big_vfs.cpp`, `prefetch.cpp`), RealGraph şekil | kaynak yolları |
| `0x710000–0x75B000` | **Kütüphane**: EA online (GameSpy/EA Messenger, lobi `LobbyCore.cpp`), HTTP | dizgeler |
| `0x75B000–0x783000` | **MSVC CRT** (giriş `0x75BCC7`) + **statik başlatıcılar** (`0x77xxxx`, 839 fonksiyon: chunk kayıtları, havuz adları, istisna funclet'leri) | §3 |

Oyuna özgü kod kabaca `0x401000–0x640000` (≈2,5 MB); gerisi kütüphane ve CRT.

---

## 2. Oyun döngüsü ve akış (okundu)

- **WinMain `0x580E00`**: tek örnek kontrolü (`"speed2.exe"`), başlatma `0x57ED10`, sonra akış
  durumunu ayarlar (`"LoadingFrontEnd"` → `0x57FF10`, ya da demo `0x53FEB0`/oto test `0x4FEF60`),
  ana döngü `0x5811C0`, kapanış `0x5CF960`.
- **Akış durumu** bir fonksiyon göstericisi + argüman + ad üçlüsü: `0x865484` (fn), `0x865488`
  (arg), `0x86548C` (ad dizgesi). Ana döngü göstericiyi sıfırlayıp çağırır; çağrı yeni bir durum
  yazarsa aynı karede onu da çalıştırır. Görülen adlar: `LoadingFrontEnd`, `LoadingRegion`
  (`0x580EE0`), `ServiceNISLoad` (`0x577F00`), `EndGameFlowLoadingFrontEnd` (`0x580090`),
  `Delay wait for Frontennd` (`0x578350`). Oyun durumu `0x8654A4` (`6` = yarışta).
- **Ana döngü `0x5811C0`** (kare başına, sıra okundu; adların bir kısmı çıkarım): akış geri
  çağrıları (iki tur) → `0x4C1210` (FEng bölgesi) → yarış varsa `0x600FA0`/`0x601020(3,0)` →
  `0x57EAD0(geçen ms)` → yarışta: `0x601A60` ile dt, `0x5809C0` (giriş: 0x1C8 baytlık oyuncu
  kayıtları `0x8655A0`, olay kuyruğu `0x865980`), `0x5F00F0`/`0x5F0130`, `0x609CE0(dt)` (dünya
  güncellemesi) → `0x60AEE0`, `0x5663B0`, `0x600900`, `0x44EFE0` (kamera) → `0x5CE850`/`0x5CE8A0`/
  `0x5D2850` (render bölgesi) → `0x57A050`/`0x57A0E0` (tekrar bölgesi) → `0x5CEC80`.
- **Saat**: oyun tik'i **4000 Hz** (`0x86518C`, tik × `0x784268` = 0,00025 s). `0x601A60` simülasyon
  dt'sini gerçek zamana bağlar: sapma ≤0,25 s ve >1 ms ise dt `[0,85; 1,2] × dt` aralığında
  kıstırılarak yakalanır, büyükse saat atlar. 1/13 s'den (`0x784504`) uzun kareler "yavaş" sayılır,
  art arda 10 tanesi `0x4EC570`'i çağırır (ne yaptığı okunmadı). Kare sınırlayıcı en fazla 0,03 s bekler
  (`0x5EA360`, `0x788150`).

---

## 3. Chunk yükleyicileri — dosya biçimlerinin exe tarafı

`bChunkLoader(id, load, unload)` kurucusu `0x43C0B0`; nesneler 64 kovalı bir hash tablosuna
(`0x828050`, sayaçlar `0x827FE8`) asılır. Statik başlatıcılarda 24 kayıt var; diğer chunk'lar
kendi üst chunk'larının yükleyicisi içinde `switch` ile açılıyor. PryHUB sütunu `gizmo-nfs`'in o
kimliği tanıyıp tanımadığını söylüyor.

| Chunk | Yükleyici | Ne (kanıt) | PryHUB |
|---|---|---|---|
| `0x80134000` | `0x494060` | katı model listesi (eSolid) — içi `0x134002–0x13401B`, `0x80134100` (`0x493BB0`, `0x493F20`, `0x48D910`) | var |
| `0xB3300000` / `0xB3320000` / `0xB0300100` | `0x494EA0` | doku paketi (TPK), animasyonlu doku, sıkıştırılmış TPK | ilk ikisi var, `0xB0300100` yok |
| `0x80135000` / `0x80135100` / `0x135200` | `0x48E070` | ışık malzemeleri (`0x48DF90`; `0x135001`, `0x135101`) | `0x135200` var |
| `0x80034100`, `0x34107` | `0x5DF8A0` | pist akış bölümleri (alt `0x34101–0x34106`, `0x5DF810`) | **yok** |
| `0x34108` | `0x5DB2E0` | pist akış yardımcı tablosu | **yok** |
| `0x34110–0x34113` | `0x5DEF30` | TrackStreamer (`0x88EB00`) akış dizini — §3.1 | var (`world::streaming`) |
| `0x3412F`, `0x80034130` | `0x5E81C0` | dünya bölgesi/ızgara (`0x5E78A0`) | **yok** |
| `0x34146` | `0x5D8470` | işaretçiler (topoloji ağacı `0x5D8350` yanında) | var (`MARKER_CHUNK`) |
| `0x80034147` | `0x5DA490` | rota/yol ağı üst chunk'ı — içi `0x34148–0x3414D` (`0x5D9A90`) | var |
| `0x34201` | `0x5D3EB0` | 0x128 baytlık kayıtlar, tek tablo `0x883DA0` (SkidSet/Clan havuzlarının yanında; çıkarım) | **yok** |
| `0x34026` | `0x5B0E30` | **kırılabilir nesne tanımları**, 0x100 bayt; tür 19 sınıftan (`0x7FC808`: NONE, GARBAGE_BAG, SHOPPING_CART, METAL_FENCE, TRAFFIC_CONE, WATER_DRUM, OIL_DRUM, PARKING_METER, ROAD_SIGN_METAL/WOOD, MARKET_STALL, SANDWICH_BOARD, SAW_HORSE, METAL_SMALL/BIG, WOODBOX, TRASHCAN, GLASS, CHAIR) — okundu | **yok** |
| `0x34027` | `0x5B0E30` | kırılabilir nesne **yerleşimleri**, 0x70 bayt (matris + 4 alan), pist bölümüne asılı (`0x5DB680`) — okundu | **yok** |
| `0x39100–0x39102` | `0x579340` | oyun akışı nesnesi `0x865564`'e (`0x579260`) | **yok** |
| `0x34600–0x3460A`, `0x80034602` | `0x61B2E0`, `0x636AC0` | araba kaydı/parçalar (`0x34607–0x34609` yalnız exe'de) | çoğu var |
| `0x80034A10`, `0x34A07`, `0x80034A30`/`0x34A31–33` | `0x500250`, `0x599430` | GLOBALB tabloları (stil olayları, performans çubukları) | `0x34A07` var |
| `0x34800` | `0x5EFE30` | online ile ilgili tablo (dizge "Online") | **yok** |
| `0x8003B200–0x8003B202` | `0x44A6C0`, `0x452D50` | `.moo` kamera animasyonları | **yok** |
| `0x8003B600` | `0x60B7E0` | araba yayıcı konumları (CarEmitterPosition `0x60C6A0` yanında; çıkarım) | **yok** |
| `0x80035000`/`0x80035010`/`0x35020`/`0x35021` | `0x491C50` | eModel/akış paketi yanında (çıkarım) | **yok** |
| `0x37030`, `0x37060`, `0x37100` | `0x437790`, `0x431840`, `0x438140` | animasyon bankaları / WorldAnim | **yok** |
| `0x39000` | `0x4FFAA0` | dil dosyası | var |
| `0x3A000` | `0x40A390` | AI/trafik bölgesinde (çıkarım: yol/şerit verisi) | **yok** |
| `0x3B700` | `0x489ED0` | Acid parçacık efektleri | **yok** |
| `0x36001` | `0x5DBE80` | pist bölümü yanında | **yok** |
| `0x34021`, `0x39200` | `0x5A1700` | fizik/çarpışma yanında | **yok** |

Tam ve güncel liste: `tools/speed2-atlas.py "$EXE" chunks` (yalnız ID'yi kullanan fonksiyonlar dahil).

### 3.0 Kırılabilir nesneler ve trafik tabloları: dosya tarafı (ölçüldü, 2026-11-16)

- **`0x34026`, `L4R?.BUN`** — kırılabilir nesne tanımları. Payload 16 bayta hizalı (8 bayt dolgu), sonra
  0x100 baytlık kayıtlar (L4RA'da 79). `+0x08` `char[24]` model adı (`ZCS_GardenGate`, `ZCS_Barrel_A`,
  `CS_STREETSIGN_B` …); `+0x20` kütle (0,15 …), `+0x24..+0x2C` üç 1,0; `+0x30..+0x5C` eylemsizlik köşegeni gibi
  küçük değerler; **`+0x80` = `bStringHash(ad)` — 79/79**; `+0xB0` vec3 (kütle merkezi?); tür alanı
  `bStringHash(TÜR_ADI)` (19 adın tablosu `0x7FC808`; 79'un 66'sında adlandı, kalan 13 `NONE` dışı bilinmeyen).
- **`0x34027`, `STREAML4R?.BUN` bölümlerinde** — yerleşimler: 0x10 baytlık başlık (`+0x0C` sayı), sonra
  0x70 baytlık kayıtlar. `+0x00` tanımın hash'i (514/514 eşleşti, L4RA), `+0x10..+0x2C` AABB (min/max xyz),
  `+0x30` 2B yön, `+0x60` dünya konumu xyz,1. L4RA'da 514 yerleşim: 227 `ZCS_Barrel_A`, 157 `CS_STREETSIGN_B`,
  62 `ZCS_GarbageCanD`, 26 `ZCS_OrangeBumper` …
- **Model bağlantısı açık:** ad/hash yalnız `L4R?.BUN`'da iki yerde geçiyor (tanım ve `0x39200`, 89 kayıt);
  şehir bundle'ında katı model başlığı olarak bulunmadı. Geometrinin nerede olduğu okunmadı.
- **`0x34124` kurulumda yok** (çapraz trafik çıkışları, §8): exe'nin yolu var, veri gelmiyor.
- **`0x34123`** yalnız `ROUTESL4RA`'nın 122 dosyasında, hepsi aynı: 38 kavşak × 276 bayt. Dosyadan gelen
  anlamlı kısım `+0x04` iki `u16` şerit bloğu no'su (`Lane::id`) ve `+0x0A..+0x0D` iki küçük sayı;
  kaydın geri kalanı çalışma anı yuvaları (−1, 0, −1,0).
- **`0x34122`** her rota dosyasında: 4 baytlık başlık + 32 baytlık kayıtlar, yönlü çiftler halinde
  (`5→55`, `55→5`) ve yanında iki blok no'su (`0x66`/`0x65`): şerit blokları arası bağlantı gibi (çıkarım).

### 3.0.1 Şehrin örneklenmiş sahnesi: `0x34102`/`0x34103` (okundu ve ölçüldü, 2026-11-16)

Exe'nin bölüm yükleyicisi (`0x5DF8A0`, `0x80034100`'ün çocukları): `0x34101` bölüm başlığı (16 bayta hizalı,
`+0x0C` bölüm no), `0x34102` model listesi (`boy/0x44`), `0x34103` örnekler (16'ya hizala, `boy>>6`),
`0x34104` 0x24'lük kayıtlar, `0x34105` (u16,u16) bölüm bağları, `0x34106` 0x80'lik kayıtlar. Yüklemede
modelin üç hash'i katılara çözülüyor (`0x5DFA90..0x5DFB37`; 0, `0xBE43EDBB`, `0x72923C81` atlanıyor).

- **Model (0x44):** ad `char[32]`, `+0x20` üç model hash'i (LOD 1A/1B/1Z), `+0x30` çalışma anı göstericileri.
- **Örnek (0x40):** `+0x00/+0x0C` dünya AABB; **`+0x18` u16 model indeksi** (48 270/48 270 geçerli);
  `+0x1A` bayraklar (`+0x1B & 2` aynalama, `0x5DB4B0`); `+0x20` konum; **`+0x2C` 3×3 int16 dönüş+ölçek,
  satır düzeninde, birim 1/8192**. Modelin gerçek köşeleri bu matrisle dönüştürülünce 43 877 örneğin
  43 798'inde örneğin kendi AABB'sine %5 içinde oturuyor (uymayanlar sınırı sonsuz `ANM_*`).

**Sonuç — şehrin büyük kısmı hiç doğru yerde çizilmiyordu.** `STREAML4RA`'da kimlik matrisli olmayan
her katı (6 342) bir örnek modeli; kendi başlık matrisleri **yazarlık konumu** (ör. `(-8093,-527,-978)`,
şehrin 1 km altında). Doğru yerleşimleri 48 270 örnekte (43 792 tekilleştirme sonrası): binalar, 3 828
`XO_TRACKBARRIER`, 1 899 `XW_GUARDRAIL`, lambalar, trafik ışıkları, ağaçlar, çitler. `lod.rs`'in ölçtüğü
"kaba katmanlar 651 m'ye kadar uzakta, bir eksene paralel" ızgarası da bu yazarlık yerleşimiydi.
PryHUB: `gizmo_nfs::world::scenery`; oyun: `world::instance` / `Bundles::scenery` (`NFS_SCENERY=off` eski hâl).

**Gruplar (`L4R?.BUN`):** `0x34108` = 130 sahne grubu (`u32,u32, char[32], hash, u32, u32 sayı, u16[sayı]`,
4'e dolgu); üyeler `0x34107` (8 bayt: bölüm no, örnek indeksi, bayrak) ile örneklere çözülüyor. `0x5F54A0`
yarışta `PLAYER_BARRIERS_%d` + `BARRIERS_%d` (pist no `0x88F2C0`), kariyerde `BARRIERS_CAREER%d` açıyor.
Bariyer örneklerinin bayrağı `+0x1A = 0x210` (başka hiçbir örnekte yok). `0x34106` grup değil: örnek başına
128 baytlık (32×32 bit) siluet gibi. Ölçümler ve pilot bedeli: ROADMAP "Şehrin yarısı yanlış yerdeydi".

### 3.0.2 Oyunun dünya çarpışması: `0x80034130` (okundu ve ölçüldü, 2026-11-16)

Yükleyici `0x5E81C0` → `0x5D8140`: `0x34131` başlık (16'ya hizalı), **`0x34134` 32 baytlık çokgenler**
(`boy>>5`; RA'da 153 625), `0x34135` u16, `0x34136`/`0x34137` u32 listeleri. Çokgen `0x5D7F10`'da açılıyor,
köşeler `0x5D2CC0`'da:

| Ofset | Tür | Anlam |
|---|---|---|
| `+0x00` | u8 | **anahtar**: 0 sabit, ≠0 açılıp kapanan (12 049; %70'i yarış bariyeri içinde, medyan yükseklik 6,8 m = bariyer modeli) |
| `+0x01` | u8 | kutu köşe indeksleri (2'şer bit) |
| `+0x02` | u8 | gövdeye kopyalanıyor (`+0xB`) |
| `+0x03` | u8 | `0x10` dörtgen (yoksa üçgen), `0x04` sonsuz duvar (z = ±2048) |
| `+0x04` | i16 | **taban yükseklik, m** |
| `+0x08`/`+0x10` | i16×4 | X/Y, ×1/8 m |
| `+0x18` | i16×4 | **tabandan yükseklik, ×1/256 m** |

Doğrulama: 18 437 düz çokgenin 18 349'u gömülü zeminle 0,5 m içinde, medyan fark 2 mm. Sabit dik çokgen
19 093 + ~21 700 sonsuz sınır duvarı. Eşya kapsamı (dik çokgenle kesişen örnek): korkuluk %100, duvar
korkuluğu %99, çitler %85–99, sokak lambaları %92–99, binalar; kedi gözü %0, trafik ışığı %1, ağaç/çalı
%14–58 (yan duvara değme). Kırılabilir eşyalar ve araçlar ayrı dışbükey gövdelerle (`L4R?.BUN` `0x80034020`:
`0x39200` ad, `0x39201` köşeler, `0x39202` indisler; 89 gövde). Gövde kurucu `0x5AA0B0`, çağıran `0x5AE930`.
PryHUB: `gizmo_nfs::world::collision`; oyun: `world::game_collision` / `Bundles::collision`
(`NFS_GAME_COLLISION=off` eski kural: çizilen her şey katı).

`+0x00` anahtarı **paket içi** bir kimlik: bariyer kutularındaki 12 049 anahtarlı çokgende değer → grup
kümesi eşlemesi yalnız %25 tutarlı (aynı değer farklı paketlerde farklı `BARRIERS_*` gruplarına gidiyor).
Eşleme büyük olasılıkla paketin `0x34135..0x34137` listelerinde; okunmadı. Oyunda bariyerin çarpışması
o yarışta açık bariyer modelinden.

### 3.1 Şehrin akış dizini: `0x34110–0x34113` (okundu, `L4RA.BUN` ile doğrulandı)

Yükleyici `0x5DA7A0` dört chunk'ı TrackStreamer nesnesine (`0x88EB00`) asıyor. `L4RA.BUN`'da:

- **`0x34110` — bölüm dizini**, 0x50 baytlık kayıt, L4RA'da 393 tane:

  | Ofset | Tür | Anlam | Kanıt |
  |---|---|---|---|
  | `+0x00` | `char[8]` | bölüm adı: `X0`, `F25`, `R99` | veri |
  | `+0x08` | `u16` | bölüm numarası = harf sırası × 100 + sayı (F25 → 625, R99 → 1899, X0 → 2400); exe bu alanla arıyor | `0x5DA7A0` + veri |
  | `+0x10` | `u32` | 1 (ilk kayıtta 0) | veri |
  | `+0x14` | `u32` | **`STREAML4RA.BUN` içindeki bayt ofseti** | uçların en büyüğü 117 528 224 < dosya 118 489 088 |
  | `+0x18`, `+0x1C` | `u32` ×2 | bölümün boyutu (iki kez; sıkıştırılmış/açılmış olabilir, burada hep eşit) | veri |
  | `+0x20` | `u32` | 10000·k + bölüm numarası (k = 1, 2: kademe?) | veri |
  | `+0x24..+0x2C` | `f32` ×3 | merkez x, y ve **yarıçap** (m, dünya Z-üstü) | veri |
  | `+0x30` | `u32` | hash | veri |

  Yani her bölüm kendi sınır dairesiyle diskte nerede durduğunu söylüyor: mesafeye göre akış
  (ROADMAP M5) için gereken dizin bu. **Sekiz bölgenin hepsinde doğrulandı** (PryHUB altın testi
  `every_streaming_section_is_its_own_chunk_stream`): ad→numara kuralı kök `--` dışında her kayıtta
  tutuyor, dilimler örtüşmüyor, **her dilim kendi başına bir chunk akışı** (`8 + boyut` yürüyüşü tam
  dilimin sonuna iniyor). `X`, `Y`, `Z` harfli bölümlerin yarıçapı her bölgede 0 — yer kaplamayan
  ortak bölümler (ilk `Y` bölgenin doku paketinin çoğunu taşıyor); diğer hepsinin dairesi var. İkinci
  boyut alanı, dilimi doku paketiyle başlayan bölümlerde ilkinin %0,5–2'si (pikseller karta gidince
  bellekte kalan kısım olabilir; çıkarım). PryHUB: `gizmo_nfs::world::streaming` (`sections`, `near`).
- **`0x34111`** — 8 bayt, L4RA'da sıfır (bir gösterici yeri).
- **`0x34112`** — 16 baytlık `(x1, y1, x2, y2)` çizgi parçaları; L4RA'da 6 tane, uçları birbirine
  ekli (iki polyline). Sayı bu kadar azken sürüş bariyeri olamaz: büyük olasılıkla akış kümelerinin
  geçiş çizgileri (çıkarım). Exe bunları 16'ya bölünmüş sayıyla tutuyor (`in_ECX[5]`).
- **`0x34113`** — değişken uzunluklu kayıtlar: 0x14 baytlık başlık (`+0x13` = sayı), ardından
  `sayı × {u16 bölüm no, gösterici}`; yükleyici bölüm numaralarını `0x34110` kayıtlarına bağlıyor.
  Bölüm başına "bununla birlikte yüklenecekler" listesi gibi (çıkarım). L4RA'da boş.

---

## 4. Ön yüz: FEng ekran fabrikası (okundu)

`.data` `0x7F7DC8`'den başlayan tablo `{ const char *paket.fng; Screen *(*kurucu)(…); … }` 157 ekranı
kurucusuna bağlıyor. Seçmeler (tamamı: tabloyu `0x7F7DC8–0x7F915C` arası 0x1C adımla oku):

| Paket | Kurucu | | Paket | Kurucu |
|---|---|---|---|---|
| `UI_Main.fng` | `0x4ED6B0` | | `UI_PerformanceDyno_MAIN.fng` | `0x55D7F0` |
| `GarageMain.fng` | `0x4EB130` | | `UI_PerformanceTuning_Sliders.fng` | `0x55DB10` |
| `UI_CareerWorldMap.fng` | `0x4F9D30` | | `UI_PerformanceDyno_Chart.fng` | `0x55AF20` |
| `UI_InGame_WorldMap.fng` | `0x4F9C10` | | `UI_Paint.fng` | `0x56C1B0` |
| `UI_QRModeSelect.fng` | `0x4EF280` | | `UI_Rims_Browser.fng` | `0x56B350` |
| `UI_QRTrackSelect.fng` | `0x4EF9B0` | | `UI_DecalMain.fng` | `0x554AC0` |
| `UI_QRCarSelect.fng` | `0x4FC190` | | `UI_ICEMAIN.fng` | `0x560550` |
| `UI_Pause.fng` | `0x4F4790` | | `UI_PostRaceResults.fng` | `0x4FBB60` |
| `Loading_Tips.fng` | `0x4D9480` | | `UI_SMS_Mailbox.fng` | `0x4E2050` |

Ekran sınıflarının ortak tabanı vtable `0x790C2C` (15 yuva; `Highlight`/`UnHighlight`), özelleştirme
listelerinin tabanı `0x791948…` ailesi (19 yuva). FEng paketleri (`*.fng`) PryHUB'da çözülmüyor.

---

## 5. Kamera (okundu — önceki bir notu düzeltir)

Kamera yöneticisinin `SetMode(mode)`'u **`0x5F1FD0`**. Hangi modun hangi mover'ı kurduğu, istisna
eşleme tablosundan (FuncInfo `0x7E50A8`, unwind map `0x7E5028`, 16 durum) mover adlarıyla okundu:

| Mod | Sınıf | Kurucu | Boyut |
|---|---|---|---|
| 1 | `CubicCameraMover` — **oyuncunun kovalama/tampon kamerası** | `0x451660` | 0x60 |
| 2 | `DebugWorldCameraMover` | `0x455B10` | 0x48 |
| 3 | `TrackRouteEditorCameraMover` | `0x44DBF0` | 0x2C |
| 0xF, 0x10 | `TrackCarCameraMover` (`0x44BEA0`, 0x470) — pist boyu geçiş; olmazsa `CubicCameraMover` | | |
| 0x11 | `AutoPilotCameraMover` | `0x44ED50` | 0x2C |
| 0x12 | `IceCameraMover` (garaj/ICE) | `0x453C00` | 0xD0 |
| 0x13 | `CopViewCameraMover` | `0x4519E0` | 0x40 |
| 0x14 | `ShowcaseCameraMover` | `0x452520` | 0xA0 |
| 0x15 | `DynoCarCameraMover` | `0x452600` | 0xC0 |
| diğer | `OrbitCarCameraMover` | `0x4531A0` | 0xD0 |

**Yeni:** oyuncunun bakış ayarı (`0x83AA74[oyuncu×10]`) POV kimliğine doğrudan değil, bir
eşlemeyle gidiyor (`0x5EB400` → `0x502500`): ayar 0→POV 2, 1→4, 2→1, 3→0, 4→5 (bilinmeyen→0).
Yani menüdeki sıra tampon/kaput içeriden başlayıp yakın, uzak, yüksek dış kameraya gidiyor; POV,
arabanın kaydındaki tablodan (`+0x720`, `0x441140`; PryHUB `CameraView`) aranıyor, yoksa POV 0.
POV 2 ve 3 her kurulumda yumuşatma durumunu sıfırlıyor (`0x44DE90` sonu: sert bağlı). Hıza bağlı
0xB0 baytlık anahtar kare enterpolasyonu `0x446DC0`'da (kurucu `0x451660` hız `0x787154` ile çağırıyor).

> **Ad düzeltmesi.** ROADMAP "Kovalama kamerası: koltuk arabanın kaydında" bölümü okunan kodu
> `TrackCarCameraMover`'a bağlıyor. Okunan kural fonksiyonları (`0x44DE90`, `0x445C20`, `0x453570`)
> doğru, ama sınıf `CubicCameraMover`: `0x44DE90`'ı onun kurucusu `0x451660` çağırıyor, ve
> `TrackCarCameraMover` yalnız 0xF/0x10 modlarında kuruluyor. Sayılar değişmiyor.

---

## 6. Alt sistemlerin durumu

| Alt sistem | Çapa | Durum |
|---|---|---|
| Süspansiyon, lastik, sürtünme çemberi | `0x5A8790`, `0x5A8280`, `0x59D600` | okundu — ROADMAP |
| Motor, aktarma, vites, nitro, drafting | `0x5B4DC0`, `0x5EBD80` | okundu — ROADMAP (stil puanı → nitro `0x6016A0` okunmadı) |
| Direksiyon (klavye/analog), karşı direksiyon | `0x41DFC0`, `0x405DC0` | okundu — ROADMAP |
| Stil puanları, drift olayı | `0x56C740`, `0x56ED40` | okundu — ROADMAP, PryHUB `globalb::style` |
| Dyno ayar aralıkları | `0x5ACA50` | okundu — PryHUB `Chassis` |
| Rakip becerisi, lastik bandı | GLOBALB `0x34B00`, `0x408200` | okundu — ROADMAP |
| Hareket sınıfları (PhysMov, Melov8r) | `0x5A47B0`, `0x5A2900` | okundu — ROADMAP |
| Kamera modları ve POV | `0x5F1FD0`, `0x451660` | okundu — §5 |
| Oyun döngüsü, saat | `0x5811C0`, `0x601A60` | okundu — §2 |
| Kırılabilir nesneler | `0x5B0E30` | biçim okundu — §3, PryHUB'da yok |
| AI sürüş (yol takibi) | `0x41C5E0`, `0x415F80`, `0x421030`, `0x41D8D0` | okundu — §7 |
| Trafik | `0x42F910`, `0x408820`, `0x412950` | okundu — §8 |
| Yarış kuralları (sıra, tur, drag, outrun) | `0x5F7C30`, `0x606CB0`, `0x6076D0` | okundu — §9 (vites penceresi, drag şerit, reset açık) |
| Ses | `0x456000–0x488000` | okunmadı |
| Render hattı, gölge, gökyüzü | `0x5B6000–0x5D3000` | okunmadı |
| Pist akışı (TrackStreamer) | `0x5DA7A0`, `0x5DF810` | okunmadı; chunk'ları PryHUB'da yok |
| Kariyer, para, sponsor, SMS | ön yüz `0x4E0000–0x570000` | okunmadı |
| Kayıt dosyası / profil | `0x582000–0x590000` | okunmadı |
| Online | `0x710000–0x75B000` | kapsam dışı |

---

## 7. AI sürüş: rakipler yolu nasıl takip ediyor (2026-11-16)

Kapsam: rakip yarışçının nişan noktası, direksiyonu, gaz/freni, hız hedefi, kaçınma ve takılma.
Lastik bandı ve beceri dağılımı ZATEN okunmuştu (ROADMAP "Rakiplerin lastik bandı", "Rakiplerin becerisi") — burada yok.

Kanıt düzeyi: **[okundu]** = decompile/assembly satırı doğrudan gösteriyor; **[çıkarım]** = yapı okundu, ad/anlam tahmin;
**[ölçüldü]** = gerçek dosyada sayıldı.

Araçlar: REA `batch_decompile` (Ghidra) çağrı hedefleri için; yalnız vtable'dan çağrılan fonksiyonlar Ghidra'da
**fonksiyon değil** (artımlı bağlama thunk'ları, `0x4204D0 → 0x41C5E0` gibi) — onlar `objdump` dökümünden okundu.
Eğri/tablo sayıları exe'den python ile okundu (`ai/curve.py`: eğri = `{n, xmin, xmax, ölçek=(n−1)/(xmax−xmin), veri*}`,
okuyucusu `0x5764D0`, doğrusal ara değer).

---

### 0. Katmanlar (kim kimi çağırıyor)

| katman | adres | ne |
|---|---|---|
| Kontrolcü `AICarController` | vtable `0x7852B0`, Update `0x42ABD0` | hedef (goal) ve eylem (action) seçer, sürücüye hedef verir |
| `RaceCarController` | vtable `0x785370`, Update `0x42C0C0` → `0x42ABD0` | yarışa özel: yarış başı bekleme, yoldan uzaklaşınca sıfırlama |
| Hedef (AIGoal) | vtable'lar `0x784768` "Win road race", `0x7847D0` "Win drag race", `0x7848A0`/`0x784900` trafik | slot 13 = her karede hız/hedef üretir: yarış `0x41C5E0`, trafik `0x41C380` |
| Eylem (AIAction) | `JustDrive 0x7855D8`, `Pass 0x7856B4`, `Block 0x785704`, `GetUnstuck 0x785784`, `AccidentRecovery 0x7857E8`, `AvoidAvoidable 0x785858`, `StopDriving 0x78591C` | 9 yuvalı: [2]=yürüt, [5]=ad, [6]=başlamalı mı |
| Sürücü (`PsychoDriver` `0x784EB0`, `RealDriver` `0x784F40`, `DragDriver` `0x785118`) | `SetDrive` = vtable +0x38 = `0x4054A0`; adım `0x421030` | hedef hız + nişan noktasından direksiyon/gaz/fren |
| DriveTarget | +0x10'dan 16 B'lık noktalar, sayaç bayt +0 | eylemin çizdiği yol (kaçınma, geri çıkma için) |
| FuturePoints | vtable `0x784AA0`, kayıt reset `0x40AD20` | önümüzdeki şerit noktaları + dosyadaki şerit hız baytları |

`SetDrive(hız, a2, a3, bayrak)` sürücüye `+0x230/+0x234/+0x238/+0x23C` yazar **[okundu, `0x4054A0`]**. Kontrolcü durdururken
`SetDrive(0,0,0,1)`, yarış başında `SetDrive(100.0,0,0,1)` çağırıyor (`0x42C1A1`) **[okundu]**.

Düşünme seyrekleştirilmiş: kontrolcü hedef/eylem seçimini her karede değil, araba sırası `kare % 8` (drag'de `% 4`) tuttuğunda
ve son düşünmeden ≥0,2 s (`0x784554`) geçtiyse yapıyor (`0x42ABD0`) **[okundu; "kare sayacı" `0x7FB724` çıkarım]**.

---

### 1. Nişan noktası — ileri bakış

`FUN_00415F80` (sürücü adımının ilk işi) **[okundu]**:

```
mover = araba+0x28 (1 PhysMov, 3 MeloMvr, 4 "tür4", diğer)   ← ROADMAP'teki hareket sınıfı numarası
PhysMov | tür4 | (MeloMvr ve araba+0x24==4):
    L = eğri_0x7E9D94(ileri_hız +0x42C) × eğri_beceri(beceri baytı)
    eğri_0x7E9D94: x 0..160, 10 nokta: 22,9 22,9 24,0 27,9 33,0 39,0 45,8 49,4 53,1 55,9  (m)
    eğri_beceri  : PhysMov 0x7E9DF8 [1,3 → 1,1], diğer 0x7E9DE4 [0,6 → 1,1], x = beceri 0..3
aksi hâlde (kip m: MeloMvr→0, diğer/trafik→3):
    L = max(ileri_hız × T[m], Dmin[m])     T 0x7839B0 = 0,55 0,4 0,35 0,55 0,9 0,4   Dmin 0x7839CC = 15 15 15 5 9 15
```

- Beceri baytı `0x5B5330` (araba+0x10+0x10, `+4`). Okunmuş beceri değerleri 3–5 olduğundan eğri ucunda: **L × 1,1** [çıkarım].
- Eğrinin x ekseni ham `+0x42C`. Notlarda bu alan m/s; 0..160 aralığı mph'yi de düşündürüyor — **birim doğrulanmadı**.
  m/s okunursa: 0 km/h'de 25 m, 180 km/h'de ~30 m, 360 km/h'de ~44 m (×1,1 dahil).

Nokta seçimi **[okundu]**:
- DriveTarget **boşsa**: hız ≥5 m/s → araba konumu + **hız yönünde** L (`0x403BA0`, L en az 5 m); <5 m/s → **gövde yönünde** L (`0x403AF0`).
- DriveTarget **doluysa** `0x415630`: yakındaki parçadan (`0x404EF0`) başlayıp yol boyunca L kadar yürür ve ara değer alır. Her köşede
  birikmiş mesafeye `(iki parça toplamı − kestirme) × k` **eklenir**: `k = 1` (`0x7837AC`), bir sayaç <5 s ise `k = 7`
  (`0x7837B0`/`0x7837B4`). Yani **virajlı yolda nişan yaklaşır** — köşe kesmeyi frenleyen düzeltme. Sayacın neyi saydığı okunmadı.
- Nişan sürücüde `+0x224/+0x228`.

### 2. Direksiyon

### Hata terimleri — `0x4054D0` [okundu]
- `e1` = gövde yönü (`+0x40C`, u16 açı) − nişana açı (`0x43E6D0` = atan2). ±π'ye sarılır.
- `e2` = hız yönü ile nişan açısı farkı, `e1` ile karışık: `e2 = w·e_hız + (1−w)·e1`, `w = 0x576790(hız)` (0..1 rampası, eğrisi okunmadı).
- `e3` = `atan2(5,0 m, yanal_sapma)` (`0x783938` = 5; sapma `0x404AF0(konum)`) — **şeritten yanal sapmanın açısı** (Stanley'e benzer).
- Üçü de `0x576BB0` ile süzülüyor; süzgeç nesneleri değer (`+0x7C`), türev (`+0x30·n / (+8·+0x78)`) ve toplam (`+0x5C`) veriyor.

### Kural — `0x421030` [okundu]
İki dal, harekete (`+0x28`) göre:
- **Doğrusal** (trafik/diğer, tür4, ve sayaçlı kip 5): `δ = −(P[m]·e + I[m]·∫ + D[m]·ė)`, kip başına tablolar
  P `0x78395C` = 0,9 1,0 0,8 0,7 0,85 0,5 · I `0x783994` = 0,3 0,08 0,18 0 0,12 0 · D `0x783978` = 0,2 0 0,1 1,42 0,79 1,0
  (integral ±1,0'da kırpılıyor `0x783934`; türev ±`0x81E5AC`, çalışma anı).
- **Uyarlamalı PID** (PhysMov, MeloMvr): `AdaptivePIDController*` (`0x409CC0`…`0x40A0B0`), adım `0x4182C0`, çıktı `0x409DA0`.
  Kazanç takımı arabaya göre seçiliyor: PhysMov iki takım (`0x78385C` / `0x78388C`, araba+0xC+0x88A bayrağı), diğerleri iki
  takım (`0x78382C` / `0x7838BC`, `0x401930` bayrağı). Düşük hızda (<10, `0x7838E4`) kazançlar hıza göre eğrilerden:
  `0x7E9D44` 0,328→0,04 · `0x7E9D80` 0,21→0,344 · `0x7E9D58` 0→0,076→0,040. Hangi terimin P/I/D olduğu **[çıkarım değil, açık]**.
- Sonra, **duvar/araba itmesi**: son 0,1 s'de temas olan ve 0,6 s'den uzun süren her temas için (`0x783944`), karşı araba
  26,8 m/s'den yavaşsa (`60 × 0,447`), açıya `min((t−0,6)/1,0, 1) × kilit(v) × 1°` eklenip temastan uzağa döndürülüyor **[okundu]**.
- İstenen açı **±45°'de** kırpılıyor (`0x7844A0`/`0x78449C` = ±π/4).

### Kilit, oran ve çıkış [okundu]
- **Kilit L**: drift bayrağı açık değilse (normal yarış) `L = min(eğri_0x7E9E0C(|v|) × 1,0, 45°)`;
  eğri: **45 30 5,5 5 3 2 2 2 2 2 (°)**, x 0..160. Drift açıkken sabit 45°.
- Çıkış `u = clamp(δ/L, −1, 1)`; mevcut teker açısı `+0x1E0` (u16, ×360/65536).
- **Hız sınırı** (açı değişimi): yarış dalında ±100·dt (pratikte yok, `0x783A20`); doğrusal dalda
  `eğri_0x7E9E34(|ω|) × eğri_0x7E9E20(|v|) × dt`, merkeze dönüşte ×2 (`0x783A1C`). `0x7E9E20`: 4 3 2 1 0,45 0,14 0,1…
- Sonuç teker açısı = `(Δ + s/L)·L`; doğrusal dalda araba+0x20+0x270 (direksiyon oranı) ile çarpılıyor.

### 3. Hız hedefi — şerit hız baytı (yarışta hep 100: sınır yok)

> **Düzeltme (ana oturum, kurulum ölçümü).** Çatalın ilk hükmü "viraj hızı dosyadan geliyor, en büyük fark" idi; kurulumun tamamı ölçülünce çürüdü — aşağıda "Ölçüldü". Mekanizma doğru, ama yarış dosyalarında değer hiç sınırlamıyor.

Yarış hedefi `0x41C5E0` (Win road race, slot 13) **[okundu]**:

```
v_yol = FuturePoints.hız(sınıf = hedef+0x30C, mesafe)        // 0x411DD0
        0 çıkarsa 30 m/s (0x784478)
v     = v_yol × beceri_hız_çarpanı × araba+0x770 (lastik bandı)
        beceri_hız_çarpanı = 0x81CB60 bloğu +0x538 + beceri·0x68     (normal yarış)
                             +0x540 + beceri·0x68                    (0x89E7A0 bayrağı — drag?)
                             0,84 / 1,0 sabitleri (0x78357C/0x783598, oyun kipi 7)
mesafe = bloğun ileri bakışı + araba+0x704, +0x704 = rastgele ±X (X = blok +0x584 + beceri·0x68),
         her blok+0xAD4 saniyede yenilenir   → hızı okuma noktası rakip başına titriyor
özel: araba yol düğümündeyse (+0x750 ≠ −1) yolun kendi hızı (0x5D5CD0); bazı bayraklarda 99999 (sınırsız)
SetDrive(min(eylemin hızı, v, …), …)
```

`0x411DD0` **[okundu]**: FuturePoint'lerde mesafe `+0x54`, hız `+0x40` (sınıf 0) ve `+0x44..+0x50` (sınıf 2..5); farklı yol
parçasına geçen ilk noktayla **doğrusal ara değer**.

`0x40A8C0` **[okundu]** FuturePoint hızlarını dolduruyor:
`hız = bayt(kayıt + 0x2C + sınıf) << 16 × (1/65536)` (`0x784278` = 1,5259e−5) = **baytın kendisi** (16.16 sabit noktadan float'a; ana oturumda assembly'den doğrulandı — çatalın ilk okuması sabiti 2e−5 sanıp ×1,31072 yazmıştı). `kayıt = blok + 0x8C + nokta·0x38`, blok tablosu
`0x88A334[id]` (yönetici `0x88A320`). Tabloyu `0x5DE1C0` dolduruyor: **chunk `0x34121`**, blok başlığı `0x8C`, sayaç `+0x34`, id `+0xA`
— bu PryHUB'ın `routes.rs`'teki "84 B başlık + (sayaç+1)×56 B" okumasıyla aynı yürüyüş, yani **`Routes####F/B.bin` şerit noktaları**.

**[ölçüldü, 355 rota dosyası, 259 044 nokta]** Bayt indeksi `+0x2C + k` bir **şerit** indeksi: trafik kodu ileri
şeritleri 2..5, geri şeritleri 6..9 olarak kullanıyor (`0x410CA0`, §8).

- **Şerit 0/1 (`+0x2C`/`+0x2D`)**: `Routes####F.bin` ve `B.bin`'in **228 723 noktasının hepsinde 100**. 100'den farklı
  değerler yalnız serbest dolaşım dosyası `RoutesAll.bin`'de (14 588 nokta 247/239, 1 038 nokta 63–97) — ve onlar da
  virajda değil: 100 altı değerlerin eğrilik medyanı ≈0 (düz yol), 247/239'unki 0,003 (R≈320 m). Yani dosyada
  **yarışçılar için tasarlanmış viraj hızı yok**; rakibin viraj davranışı beceri/lastik bandı/eylemlerden geliyor.
- **Şerit 2..9 (`+0x2E..+0x35`)**: değerler 6..20 aralığında (16 farklı değer), dış şeritlerde (`+0x31`, `+0x32`, `+0x35`)
  %97'si 6. Trafiğin 24,59 m/s (55 mph) tavanının altında kalan **trafik şeridi hızları** (m/s okuması makul: 6 → 21,6 km/h,
  20 → 72 km/h; birim kodla doğrulanmadı).
- **`+0x36`/`+0x37`**: %99,7 sıfır.

PryHUB `LanePoint::lane_speeds` bu 12 baytı ham olarak taşıyor.

### 4. Gaz ve fren — `0x41D8D0` [okundu]

- Hedef hız |v*| < 5 m/s (`0x783A48`) ise aç-kapa: hız v*'ın 1 m/s altında → gaz 1; üstünde → fren 1.
- Aksi: hız hatası süzgecinden PI-D: `u = P·e + I·∫ + D·ė`, kip başına tablolar **çalışma anı veride** (`0x7E8FCC`…, exe'deki ilk
  değerler: P 0,1 · I 0,02 · D 0,02/0,01; kip 4: 0,005/0,005/0,01). Hız v*'ı 2 m/s'den fazla aşıyorsa (`0x783A4C`) "fren" takımı
  ve alt sınır −1. PhysMov/yarışta ve v* > 30 m/s iken (`0x784478`) bunun yerine uyarlamalı PID (`0x4183F0`).
- **Değişim hızı**: Δu ±`eğri(|v|)·dt` ile sınırlı; `u ∈ [−1, 1]`; u < 0 → gaz 0, fren −u.
- **Çekiş kontrolü**: `0x410040` doğruysa gaz tavanı `1 + 0,02·kayma + 0,02·kayma' + 0,005·∫` (frende −0,2/−0,02/−0,005) ile
  kırpılıyor; ≤0 olursa gaz kesiliyor (`0x7839FC..0x783A10`).
- Drift/özel kipte gaz × `(1 + cos(2·kayma + π/2))/2` [okundu; anlamı çıkarım: yan kayarken gaz azalıyor].
- Gaz tavanı `+0x22C` (eylem/kontrolcünün verdiği "en çok gaz").

### 5. Kaçınma, geçme, şerit

- `AvoidAvoidable` (başlamalı-mı `0x42BA80`, yürüt `0x42F330`) **[sabitler okundu, mantık çıkarım]**: engel boylamasına
  −10..+30 m (`0x785BAC`, `0x7835B4`; bir kolda 50 m `0x7835B8`) içindeyse; kaçınma ileri bakışı `min(v·0,75, 30 m)`
  (`0x783764/0x783768`); izleme hızı engelin hızı ±2 m/s (`0x783770`), 26,8 m/s'de (60 mph) kırpılıyor (`0x7835C4`).
  Yanal kayma DriveTarget'e ofsetli yol olarak yazılıyor (`0x419C60`, `0x4053B0`, `DriveAtOffset 0x40EEC0`).
- `Pass` (`0x42B940` / `0x42B7F0`), `Block` (`0x423A10`), `Draft`, `NudgeAvoidable` ayrı eylemler — sabitler `0x42B5B0` (5, 10,
  3, 3) civarında, **okunmadı**.
- Şerit seçimi: FuturePoints parça değişimini `+0x5C/+0x5E` (blok id, nokta) ile izliyor; seçimi yapan kod okunmadı.

### 6. Takılma ve kaza

- **GetUnstuck** başlamalı-mı `0x414440` **[okundu]**: `0x5ED580(araba)` > 2,0 s (`0x7836F8`; muhtemelen "yavaş kalınan süre",
  çıkarım) **ve** nişana yön ile gövde yönü nokta çarpımı < 0,9 (`0x7836E0`, ~26°). Son 0,4 s'de (`0x7836E4`) temas varken
  hız <5 m/s ise başlamıyor (o durumu kaza eylemine bırakıyor, çıkarım).
- **GetUnstuck** yürüt `0x420940` **[okundu]**: hedefe bir DriveTarget çiziyor (`0x404CD0`), sonra eylem hızı
  **−6 m/s** (`0x7836DC`), araba+0x24==3 ise **−15 m/s** (`0x7836D8`) — yani **geri vitesle** nişana yönelik yoldan geri çıkıyor.
- **AccidentRecovery**: başlamalı-mı `0x4033D0` → araba durumu `+0x424 == 4` ve yukarı vektör bileşeni > 0,707 (`0x403380`, 45°).
  Yürüt `0x424210`: tüm hızları sıfırla, eylemde 5 s (`0x783720`) geçince `0x5EBF60` (çıkarım: arabayı yola geri koy).
- **Yoldan uzaklaşma** (`0x42C0C0`): yoldan uzaklık `0x5F1160` ≥ 50 m (`0x783514`) ve şerit verisi koşulu (+0x6/+0x5, +0xC < 0,2)
  → `0x5EBF60` (sıfırlama) **[okundu, `0x5EBF60` adı çıkarım]**. Kontrolcü de eylemsiz kalınca aynı çağrıyı yapıyor (`0x42ABD0`).
- `IJustGotHit` (`0x7857B0`), drag'e özel çarpma/duvar eylemleri var; okunmadı.

---

### Bizim pilot/ray ile karşılaştırma

Bu repoda rakipler `rig::rail` ile rayda (konum çizgide, hız eğrilikten `v = √(a·r)`), `rig::pilot` ise ölçüm aracı
(saf takip: `LOOKAHEAD_PER_SPEED` 1,8 s, 12–40 m; `GRIP` 8 m/s²; `BRAKE_SPEED`/`BRAKE_GAIN`; `STALL_*`, `BACK_FOR`, `ESCAPE_*`).

| konu | oyun | biz | fark |
|---|---|---|---|
| viraj hızı | şerit baytı (yarış dosyalarında hep 100 = sınır yok) × beceri × lastik bandı; frenlemeyi eylemler ve PID belirliyor | `√(a·r)` (ray), `GRIP` 8 (pilot) | dosyadan alınacak bir viraj hızı yok — `√(a·r)` türetmesi kalıyor |
| okuma noktası | rakip başına rastgele ±X m titreşim, periyodik yenileniyor | yok | rakipler aynı noktada fren yapıyor |
| ileri bakış | 23–56 m tablosu × 1,1; virajda yol uzunluğuna köşe cezası ×1 / ×7 | 1,8 s·v, 12–40 m, kirişle | düşük hızda oyun daha uzak bakıyor (25 m vs 12 m) |
| direksiyon | 3 hata terimi (gövde/hız yönü/yanal sapma) üzerinde PID, kip başına kazanç; hızla daralan kilit 45°→2° | saf takip, kilidin %85'i, hızla daraltma **çürüdü** | oyun yanal sapmayı da kullanıyor (biz yalnız noktaya bakıyoruz); kilit 35 birimde 5,5° |
| direksiyon hız sınırı | trafikte eğriyle; yarışçıda yok | yok | — |
| gaz/fren | hız hatası PID + değişim hızı sınırı + çekiş kontrolü | eşik + kazanç | oyun gazı yumuşak değiştiriyor |
| temas | 0,6 s'den uzun temasta temastan uzağa direksiyon | duvar ışını `WALL_*`, kaçış `ESCAPE_*` | oyun basit: süreye bağlı itme |
| takılma | >2 s ve yön >26° sapmışsa **−6 m/s geri**, yol çizerek | durma 1,5 s → geri 1,2 s / kaçış 3,5 s | benzer; oyun yön koşulu da arıyor |
| kaza/devrilme | 5 s sonra yola koy; yoldan 50 m uzaklaşınca sıfırla | yok (rayda gerek yok) | pilot için eksik |
| düşünme sıklığı | 8 karede bir, ≥0,2 s | her kare | — |

**Oyunun yapıp bizim yapmadıklarımız:**
1. ~~Viraj hızlarını dosyadan almak~~ — çürüdü: yarış dosyalarında şerit 0/1 baytı hep 100 (yukarıda "Ölçüldü").
2. Rakip başına hız okuma noktasını rastgele kaydırmak (±X, blok `+0x584`) — rakiplerin aynı yerde frenlemesini bozar.
3. Hızla daralan direksiyon kilidi (45, 30, 5,5, 5, 3, 2°) ve yanal sapma terimi (`atan2(5 m, sapma)`).
4. Virajlı DriveTarget'te nişanı köşe fazlasıyla yaklaştırmak (`k` = 1 / 7).
5. Gaz/fren için değişim hızı sınırı ve çekiş kontrolü.
6. Yoldan 50 m uzaklaşan / devrilen arabayı yola geri koymak.

### Açık kalanlar
- `+0x42C`/`+0x430` eğrilerinin x biriminin m/s mi mph mi olduğu (0..160 aralığı).
- `hedef+0x30C` sınıfının yarışçıda değeri → `+0x2C..+0x30`'dan hangisi; FuturePoint vfunc +4/+8'in döndürdüğü bayt ofseti
  (taban vtable `0x7846F0` saf sanal; türev sınıf bulunmadı).
- Uyarlamalı PID'in (`0x4182C0`, `0x4183F0`, `0x409DA0`) iç formülü; P/I/D eşlemesi.
- `0x576790` hız-rampası eğrisi, `0x5ED580` (takılma süresi?), `0x5EBF60` (yola koyma?) adları.
- `Pass`, `Block`, `Draft`, şerit seçimi mantığı.
- Ölçümle doğrulama: rakiplerin çoğu `tür0`/`tür4` hareketinde (ROADMAP 2026-11-06) — bu dallarda direksiyon fiziği değil
  konumu sürüyor olabilir; `nfsu2-memlog.py` ile sürücü `+0x230` (hedef hız) ve FuturePoint hızları yarış sırasında okunmalı.

---

## 8. Trafik (2026-11-16)

Kanıt düzeyleri: **[kod]** = decompile/asm ile okundu, sabit exe'den; **[çıkarım]** = kod okundu ama adın/anlamın
bir kısmı bağlamdan; **[açık]** = okunmadı ya da yarım. Zaman sayacı `0x86518C` tik, 1 tik = `0x784268` = 0,00025 s
(4000 Hz) **[kod]**. Hız birimi m/s (`0x7842A8` = 0,44704 = mph→m/s).

### Sınıflar ve çapalar

| ad (dizge) | adres | rol |
|---|---|---|
| `StreamingTrafficCarManager` | nesne `0x81E810`; anlık görüntü `0x412720`; güncelleme `0x412950`; atama `0x412BE0` | trafik **modellerinin** yüklenmesi/boşaltılması (araba değil) |
| (trafik yerleştirici, adı yok) | güncelleme `0x42F910`, yerleştir `0x42F800` | arabaları oyuncunun önüne/kavşaklara **ışınlama** |
| `TrafficMover` | vtable `0x78456C`: dtor `0x408790`, Update `0x408820`, snapshot `0x4087B0` | kinematik hareket (mover fabrikası `0x5F46A0` tür **2**) |
| `TrafficController` | vtable ~`0x7853F8..0x78548C` (`0x408C40`, `0x408CA0`, `0x408ED0`, `0x429CF0`) | sürücü tarafı; mover seçimi |
| `TrafficIntersection`/`IncomingTrafficCar` | snapshot `0x410750`, kural `0x409110`, kare başı `0x4111C0` | kavşak geçiş hakkı |
| `TrafficTeleporter`/`CrossTrafficEmitter` | snapshot `0x417AB0`, seçici `0x409570` | kavşaktan çıkan çapraz trafik |
| `RandomEncounterManager` | `0x412E40` (yalnız snapshot okundu) | 3 yuvalı rastgele karşılaşma — **[açık]** |

### Veri: hangi tablolar

- **Rota/yol dosyası chunk'ları** (yükleyici `0x5DEBC6..0x5DEC3C`, boşaltıcı `0x5E4826..0x5E488E`) **[kod]**:
  - `0x34122` → `0x883DB0`, adet `(boy−4)/0x20` (32 B kayıt). Trafik kodu kullanmıyor gibi; ROADMAP'te "okunmadı" **[açık]**.
  - `0x34123` → `0x883DBC`, adet `boy/0x114` (276 B) = **kavşaklar**: `+4` iki `short` yol parçası no'su; kare başı
    `0x4111C0` ikisinin de parça bayrağı `+0x70` bit 6'sı açıksa `0x409110`'u çağırıyor **[kod]**.
  - `0x34124` → `0x883DC4`, adet `boy/0x108` (264 B) = **CrossTrafficEmitter**. `+0` parça no'su (`0x88A334[]`),
    `+4` short[] (yöne göre yol), `+0x10/+0xE0` int'ler × hız ölçeği (min 20 / 120 m, `0x783BB8`/`0x783BB4`),
    `+0x70/+0x90/+0xB0` float çiftleri trafik seviyesine göre (`+0xB0[lvl]·0,447` = hız mph→m/s), `+0xD0..` seviye/yön
    bayrak baytları, `+0x100` etkin araba sayacı (<6) **[kod; alan adları çıkarım]**.
- **Yol parçası tablosu** `0x88A334[id & 0x7FF]`: `+0x34` nokta sayısı, `+0x8C + i·0x38` noktalar, `+0x82/+0x84 + şerit`
  şerit varlık baytları (ileri 2..5, geri 6..9 — `0x410CA0`), `+0x2C + şerit` şerit hız sınırı baytı (m/s), `+0x70` bayraklar
  (bit 6 kavşak, bit 25/26 yöne göre trafiğe açık) **[kod; alan adları çıkarım]**.
- **Bölge poligonları** `0x88F298`, `0x5D9BF0(pos, tür 0x10)` ile sorgu: bölge kaydı `+0x3C` = CTI'de sütun ofseti,
  `+0x2C + lvl·4` = yoğunluk sınıfı **[kod]**. Büyük olasılıkla ROADMAP'teki `0x3414A` yönlü poligon bölgeleri (tür kodu `0x10`)
  **[çıkarım]**.
- **CarTypeInfo** (`0x8A1CCC`, 0x890'lık kayıt) **[kod]**: `+0x844 == 2` → araç trafik aracı; `+0x854 + bölgeSütunu` (signed
  byte) = o bölgede bu modelden en çok kaç araba; `+0x859 + sütun` ≠0 → model boşaltılmaz.
- **GLOBALB `0x34B00`** (yükleyici `0x408200`, gösterici `0x81CB60`; lastik bandı blob'unun aynısı) `+0xA8C + sınıf·8` =
  (min, max) float çifti: **trafik aralığı** **[kod; "aralık" çıkarım]**.
- **Etkinlik kaydı** (kariyer kurucusu `0x5322F0`, `esi` = etkinlik kaydı): `+0x36` (signed byte) → **trafik seviyesi**
  `0x89E7A8` **[kod]**. Hızlı yarış menüsü `MOTrafficLevel` (`0x7730A4`, `0x774290..`) ve `0x5267A6/0x52688C`, `0x53F1D0..0x5407EC`
  aynı global'e yazıyor **[kod; menü bağı çıkarım]**. Seviye **0 = trafik yok** (`0x42F910` hiç çalışmıyor). Yarış türü
  kurucuları `0x4FEF80/0x4FF070/0x4FF0E0/0x4FF160` seviyeyi 0'lıyor (drag kurucusu `0x4FEFF0` dahil) **[kod]**.
  `0x89E7F4` = global ayar `0x83AC30`, yerleştirmede `0x5FBED0`'a 9/4/0 olarak geçiyor; anlamı **[açık]**.

### (1) Doğma/silinme: aslında **havuz + ışınlama**

Trafik arabaları sürekli yaratılıp silinmiyor; havuzdaki arabalar uzaklaşınca **oyuncunun önüne ışınlanıyor** **[kod]**.

`0x42F910` (kare başı, `dt` alır), çalışma koşulu: `lvl = 0x89E7A8 ≠ 0`, havuz boş değil, oyun durumu `*0x890118 == 4`
(yarışta), insan oyuncu var:

1. Her **5 s** (`0x783BBC`) model akışı `0x412950`.
2. Geri sayım bitince (`0x4098D0`): **1,0 s** (`0x783BA0`), drag'de (`0x89E7D8`/`0x890108`) **0,3 s** (`0x783BA4`).
3. Her oyuncu için bir **aralık** çekilir (`0x418240`): oyuncunun bulunduğu bölgenin `sınıf = bölge[+0x2C + lvl]`'i →
   `U(min, max)` from `0x34B00+0xA8C`. Bölge yoksa 99999 (= orada trafik yok).
4. Havuzda round-robin, tur başına **1 araba** (başlangıç/zorunlu modda `0x4018F0` → **16**). Araba "boşta" sayılıyor
   (`0x417BB0`) eğer en yakın oyuncuya uzaklığı **≥ 250 m** (`0x78429C`; dünya uzaklığı `0x409840`/`0x5F0FB0`, rota üstünde
   `0x409890` |rota mesafesi| ≥ `0x7E9720` = 250) ya da sürücüsü "takıldım" bayrağı `+0x77C`'yi kaldırmışsa.
5. `0x42F800` yeri seçer; olmazsa araba gizlenir (`+0x77C = 1`).

`0x42F800` yerleştirme sırası **[kod]**: rastgele bir insan oyuncu seç; oyuncu **< 20 m/s** (`0x783BCC`) ise %35
(`0x783BD4`), değilse **%100** (`0x783BD0`) ihtimalle önce **çapraz trafik** (`0x42E790`): oyuncunun rota mesafesinin
±60 m (`0x783BD8`) penceresindeki emitter'lardan rastgele biri (`0x409570`), yavaş oyuncu yakında değilse (`0x4094A0`: <20 m/s
ve <50 m oyuncu yok). Olmazsa:
- drag'de `0x42E620`: oyuncunun yolunda önde bir nokta (`0x417C60`), **oyuncuyla aynı şeritte %50** (`0x410CA0`, `0x783BC8`);
- değilse `0x42E9F0`: oyuncunun önündeki yol ağında `aralık` uzunluğundan başlayıp her denemede ikiye katlanan
  adımlarla yürür (`0x415130`), **en çok 500 m** (`0x783BA8`) ve 100 deneme. Her adayda: şerit seç (`0x410CA0`; yön olasılığı
  `1 − ayar[+0x108 + lvl·4]` — karşıdan gelen oranı, `0x883DAC` yarış ayar nesnesi), yön bayrağı bit 25/26, ve
  `0x418110`: başka bir arabaya (oyuncu/rakip dahil, trafik hariç tutulmuyor) `aralık` yarıçapında yakın değil
  (`0x410FC0` dünya, `0x411060` rota) **ve** yarış rotasında oyuncu önünde `ayar[+0xF8 + yön·4]`'ten kısa değil.
  Başlangıç hızı **24,59 m/s = 55 mph** (`0x41C4B127`), şerit hız baytıyla sınırlı.

Işınlama `0x42C950` **[kod]**: şerit 2..5 ileri (yön +1), 6..9 geri (şerit−4, yön −1); kontrolcüye yol/şerit verir,
emitter sayacını günceller, `+0x7A4` = zaman damgası, `0x5FBED0` ile arabayı koyar, `0x5F46A0(2)` = **TrafficMover**.

**Araba sayısı sınırı:** havuz yarış kurulurken dolduruluyor (`0x57F996` → `0x412BE0`): yarış listesindeki CTI `+0x844 == 2`
olan her yuvaya bir trafik modeli atanır, model başına `CTI[+0x854 + bölge]` adede kadar, **en çok 7 ayrı model**. Model akışı
(`0x412950`): yüklü model < 7 ve < hedef ise rastgele bir yenisi; yükleneli **> 6 s** olan ve fazla örneği bulunan model
boşaltılır **[kod]**. Havuzun toplam araba sayısı statik okunamadı **[açık]** (`tools/nfsu2-memlog.py movers` 122 trafik
örneği saydı; o tek yarış).

### (2) TrafficMover: **kinematik**, fizik yok

`0x408820` **[kod]**: durum `x,y` (`+0xC,+0x10`), yön `+0x14`, hız `+0x18`.
- Direksiyon açısı kontrolcüden (`0x5EBF50`, u16 × `0x784240` = 2π/65536 rad). Açı ≈0 değilse bisiklet modeli: dönüş yarıçapı
  = dingil açıklığı / açı (dingil = `−(+0xA0) + (+0x10)` gövde verisinden), merkez etrafında döndürme; değilse düz.
- Hız: `v += (gaz(+0x20C)·f(v) − fren(+0x214)·f(v))·dt` (`f` = `0x5764D0`), **[0, 24,5865 m/s] ile kırpılır** — trafik 55 mph'yi
  aşamaz.
- `z` zeminden (`0x5F5010`), gövde konumu doğrudan yazılır, hız vektörü ve yaw hızı türetilip gövdeye verilir; tekerlek dönüşü
  `v / yarıçap`, yay sıkışmaları sıfırlanır. Yani çarpışmaya kadar dinamik yok.
- Yol takibi kontrolcüde (`TrafficController` + `AIActionDriveTrafficLane` `0x40EC20`, `AIActionAvoidAvoidableAsTraffic`
  `0x40ED80`, `AIActionObeyTrafficIntersections` `0x40EDE0`); bu eylemlerin gövdeleri **[açık]**.

### (3) Kavşak

Gerçek trafik ışığı yok; **ilk gelen geçer** **[kod]**. Kavşak (`0x34123` kaydı) `+0x10` = geçiş hakkı olan yaklaşım
(−1 = boş), 16 `IncomingTrafficCar` yuvası (etkin baytı, yaklaşım no'su, uzaklık). `0x409110`: hak sahibi yaklaşımda **12 m**
(`0x783B94`) içinde gelen araba kalmayınca hak bırakılır. Hakkı kimin nasıl aldığı (eylem `0x425140`) **[açık]**.
Çapraz trafik bu kavşaklardaki emitter'lardan (`0x34124`) besleniyor.

### (4) Çarpınca

`0x408DE0` (kontrolcü güncellemesi `0x429D90`'dan) **[kod]**: arabanın her temasında (`0x5A0820`) darbe büyüklüğü (`+0x60`)
**> 30** (`0x783B8C`) ise ya da araba neredeyse durmuşken (`+0x430 < 1`) mover başka bir kuvvet bildiriyorsa (`vt+0x74 > 1`):
nişan noktasını öne koyar (`0x4023E0`), sürücü nesnesine `vt+0x38(0,0,0,1)` ve `+0x710 = 1` ("çarpıldım"). Mover seçimi
`0x5F4400`: sürücü sınıfı `+0x24 == 4` → TrafficMover (2); aksi halde 1/3/0 (PhysMov/MeloMvr/tür0). Yani çarpılan trafik
arabası sürücü sınıfı değişince fiziğe geçiyor; `+0x38`'in sınıfı değiştirdiği **[çıkarım]** — ölçüm (ROADMAP "İki hareket
sınıfı") trafiğin `TrafficMover` 51 / `tür0` 33 / `MeloMvr` 25 / `tür4` 13 arasında geçtiğini zaten görmüştü. `0x408CA0(mod)`:
mod 3/4 → hız/şerit ayarı (`0x5ED5C0(1)`) + TrafficMover'a geri dönüş **[kod]**. Ses: `AUD: Traffic Horn` `0x4759C0`,
`AUD:TrafficWsh_Stch` (geçiş vınlaması) `0x475E10` — okunmadı.

### (5) Yarış türü ve yoğunluk

- Açık/kapalı: `lvl = 0x89E7A8`; 0 ise trafik yöneticisi hiç çalışmıyor. Kariyerde etkinlik kaydının `+0x36`'sı, hızlı yarışta
  `MOTrafficLevel` **[kod/çıkarım]**.
- Yoğunluk üç yerden: (a) seviye → bölgenin aralık sınıfı → `0x34B00+0xA8C` (min,max) metre; (b) karşıdan gelen oranı
  `0x883DAC+0x108[lvl]`; (c) bölge × model adet tablosu CTI `+0x854`. Emitter bayrakları da seviyeye göre (`+0xD0..`).
- Drag (`0x89E7D8`): ışınlama 0,3 s'de bir, oyuncunun önüne ve %50 **onun şeridine**; normal yarışta 1 s.
- Hız sınırı her durumda 55 mph.

### Bu repodaki "Trafik modeli" bölümüyle bağ

ROADMAP'teki "Trafik modeli — üç kez kendini gösteren eksik" **ortam trafiği değil**, rakip pilotların önlerindeki arabaya
(rakip/oyuncu) takip mesafesi tutmaması; oradaki kural (koridorda önde araba → gaz kes, fren) kendi icadımız. Oyunun ortam
trafiği ondan ayrı bir sistem ve henüz oyunda yok. Taşınırsa en ucuz sadık sürüm: havuz (≤7 model), 1 s'de bir **250 m'den
uzak** arabayı oyuncunun önünde `aralık`'tan başlayıp ≤500 m'ye kadar yol ağında bir şeride ışınla, 55 mph kinematik bisiklet
modeli, darbe > 30'da fiziğe devret. Gerekenler: `0x34123/0x34124` ve yol parçası şerit baytları PryHUB'da (format işi), ve
`0x34B00+0xA8C` tablosu.

### Açık kalanlar

`0x34B00+0xA8C` tablosunun gerçek değerleri (GLOBALB'den okunmalı); `AIActionDriveTrafficLane`/`ObeyTrafficIntersections`
gövdeleri; `0x89E7F4`'ün anlamı; havuz büyüklüğü; `RandomEncounterManager`'ın mantığı; `vt+0x38`'in sınıf değişimi olduğu.

---

## 9. Yarış kuralları (2026-11-16)

Kaynak: REA/Ghidra decompile + objdump, 2026-11-16. Kanıt düzeyleri: **okundu** (kod satır satır izlendi),
**çıkarım** (kod okundu, anlamı bağlamdan), **açık** (bulunamadı / okunmadı).
ROADMAP "Yarış türü bayrakları" (`0x5323AA`, drift/drag bayrakları, PhysMov) bilgisinin üstüne kuruldu; orada olan tekrar edilmedi.

### 0. Zaman tabanı ve nesneler

- Saat `DAT_0086518C` tik; saniye = tik × `0x784268` = **0,00025** → **4000 tik/s** (okundu: 30 s sınırı bu çarpanla karşılaştırılıyor).
- **Yarış nesnesi** `*0x890118` (sınıf adı yok; "RaceCoordinator" dizgesi `0x60A610`'da). Alanlar (okundu):
  - `+0x00` durum (3,4 …), `+0x10` başlangıç tiki, `+0x14` geçen süre, `+0x18` en iyi tur(?), `+0x1C`/`+0x20` **zaman sınırı başlangıcı / saniye** (aşağıda),
  - `+0x24` yarışçı sayısı, `+0x28` bitiren sayısı, `+0x30 + i·0x1F0` **RacingCar** kayıtları,
  - `+0xBB4[]` zamanlama çizgileri (Forward/Backward Start/Finish, SplitTimeBoundary1-3; `0x5EE620`), `+0xBF4` (short) çizgi sayısı,
    `+0xBF6`/`+0xBF7` (byte) başlangıç ve **bitiş çizgisinin indeksi**,
  - `+0xBD0[]` **sıraya dizilmiş** RacingCar göstericileri (lider ilk), `+0xBE8` **tur sayısı**, `+0xBEC/+0xBF0` "biri bitirdi" olayı (online),
  - `+0xC30` (byte) outrun başlat bayrağı.
- **RacingCar** (0x1F0 bayt; serileştirici `0x5EDF90`'dan ve kullanımlardan): `+0x00` araba nesnesi, `+0x04` short sektör (= çizgi + tur·5),
  `+0x06` en büyük sektör, `+0x08` char şu anki tur, `+0x09` char **tamamlanan en yüksek tur**, `+0x0A` char **sıra**, `+0x0B` char önceki sıra,
  `+0x0C` char **bitiş yeri** (0 = bitirmedi), `+0x0D` "bu kare tur bitti", `+0x0E` "yeni sektör", `+0x10` short **bitiş nedeni** (aşağıda),
  `+0x14` float ilerleme (tur + kesir), `+0x18` ilerleme hızı (EMA), `+0x1C` ham mesafe, `+0x20` **sıralama anahtarı** (= `+0x14`),
  `+0x24` outrun boşluğu, `+0x30` başlangıç tiki, `+0x44+i·4` çizgi zamanları, `+0x6C+…` en iyi tur, `+0x94+tur·4` tur süreleri,
  `+0xE8+tur·4` kümülatif, `+0x13C+tur·4` turdaki drift puanı, `+0x190` tur başı puan, `+0x198` **yarış süresi**, `+0x19C` "süre kesinleşti",
  `+0x1D4` stil puanlayıcı, `+0x1D8`/`+0x1DC` drift/stil nesneleri.

### 1. Sıralama nasıl hesaplanıyor — **rota ilerlemesi, kontrol noktası değil** (okundu)

- İlerleme `0x5F7C30` (normal kip), her yarışçı için her kare (`0x6016A0` → kipe göre `0x5F7C30` / outrun `0x5FC510` / kip 8 `0x56E6E0`):
  - `d = araba+0x8F8 × 100` (`0x784258`; araba alanının birimi okunmadı). `0x5EE150(&yön, d, çizgi)` her zamanlama çizgisi için geçişi soruyor.
  - **İleri geçiş bitiş çizgisinden:** `tur(+8)++`; `tur > +9` ise `+9 = tur`, `+0xD = 1` (tur tamamlandı). Sektör `çizgi + tur·5`
    yeni en büyükse tur/ara zamanları yazılıyor (`+0x44`, `+0x94+tur·4`, kümülatif `+0xE8`, en iyi `+0x6C`).
  - **Geri geçiş bitiş çizgisinden:** `tur--`, ama `+9 − 1`'in altına inmiyor. Yani ters yönde çizgiyi geçmek tur kazandırmıyor, tur sayacı kaybolan turu geri alıyor.
  - `+0x14 = +0x20 = tur + 0x5EE9B0(d)` (turun kesri, rota üzerinden). `+0x18` = ilerleme hızı, `α = 0,667` (`0x7A568C`) EMA.
- Sıralama `0x5EEC80`: `+0xBD0` üstünde **kabarcık sıralaması**, karşılaştırıcı `0x5EEC00`:
  - normal: ikisi de bitirdiyse **bitiş yeri** (`+0xC`) küçük olan önde; değilse **`+0x20` büyük olan önde**. (Bitiren/bitirmeyen karışık çiftte yalnız `+0x20` bakılıyor.)
  - outrun (kip 7): bitirmeyenler `+0x24` (lidere uzaklık) **küçük** olan önde.
  - Sonra `+0xB = eski +0xA`, `+0xA = 1..n`.
- Sıra değişince ses/kalabalık olayı `0x5EED10` ("GainPosition" 4/sıra+1, "LosePosition" 2/sıra; ikisi < 25 m ve hız > 40 iken; |Δhız| < 7 ise "ClosePass" 2,5). Oyun mantığına etkisi yok.

### 2. Tur, bitiş, zaman sınırı (okundu)

- Bitiş fonksiyonu **`0x606CB0(indeks, yer, ?, neden)`**: yarış süresini `+0x198`'e yazar, `+0x10 = neden`, yer verilmemişse
  `0x5EEF70` (ilk boş yer) veya `0x5EEFB0` (sondan boş yer) seçer, `+0x28` (bitiren sayısı) artar. Oyuncu bitirince sürüş AI'ya devredilir
  (`araba+0x24 = 3`, `0x4260B0`, 0x7B4 baytlık sürücü).
- **Bitiş nedenleri** (`+0x10`): 1 normal bitiş · 3 tur eleme (KO) · 6 outrun kazandı · 7 outrun kaybetti / geride kaldı ·
  8 drag'de motor patladı · 9 drag'de araba hurda · 0xB zaman doldu · 4/5 başka yollar (online; okunmadı).
- Ana güncelleme **`0x6076D0`** (her kare):
  - **Normal (kip 1):** `+9 (tamamlanan tur) == +0xBE8 (tur sayısı)` olan → neden 1.
  - **Lap KO (kip 3):** tur tamamlayan yarışçının `sıra + tur == yarışçı sayısı` ise **arkasındaki herkes** neden 3 ile elenir. Yani her turun sonunda son sıradaki düşer. (Hızlı yarışta kip 3 = Circuit + `0x83AAD4` seçeneği, `0x53F150`.)
  - **Zaman sınırı:** `+0x20 > 0` ve `(şimdi − +0x1C) × 0,00025 ≥ +0x20` ise bitirmemiş herkes sırasıyla yer alarak **neden 0xB**.
    `+0x1C/+0x20` `0x606CB0`'da kuruluyor: **insan oyuncu birinci bitirdiğinde** (`0x401820()==0` ve `0x5EFA00()`) sınır **30 s** (`0x7A5690`);
    drift'te `DAT_0089E7DC`. Yani **lider (oyuncu) bitirince rakiplere 30 saniye** veriliyor, sonra kalanlar sıralanıp yarış kapanıyor.
  - Bitiş yeri boşta kalan yarışçılar için `0x5F7EC0` kalan turları tahmini sürelerle dolduruyor (sonuç ekranı için).
- **Tur sayısı dosyada değil, olay kaydında:** kariyerde `0x5322E0` `DAT_0085EF48[2]`'den, hızlı yarışta `0x83AACC + kip·0x14` kaydının
  `ushort[1]`'inden `DAT_0089E7BC`'ye. Pist kaydı `0x5D3E40(pist)`: `+0x80 == 1` ise drift tek tur (noktadan noktaya), `+0x85` → `DAT_0089E7B4`.

### 3. Ters yön ve reset — **kısmen**

- **Okundu:** ters yönde bitiş çizgisini geçmek turu düşürüyor (yukarıda). HUD öğeleri `WrongWayImage` ve `WrongWay_CountDown` (`0x4F6420`,
  HUD maskesi `0x4000`) var: **geri sayımlı** bir ters-yön uyarısı. Sayacın süresi ve sonunda ne olduğu (reset mi?) **açık** — HUD
  öğeleri vtable'sız bağlı liste düğümü, güncelleyicileri bulunamadı.
- **Reset:** `PCCOReset*` tuş eşlemesi var; yoldan çıkınca konum/süre kuralı **açık**. AI tarafında `AIActionGetUnstuck` (`0x40E760`),
  `"Very Stuck Progress Average"` (`0x771808`) var, okunmadı.

### 4. Drag

- **Bayrak:** `0x89E7D8` = **drag** — artık doğrudan kanıt var (okundu): hızlı yarış kip 2 kurucusu `0x53F690` onu açıyor ve tur sayısını 1 yapıyor;
  FE kısayolu `0x4FEFF0` de aynı. (ROADMAP'teki "dolaylı" kaydı kapanabilir.)
- **Hurda (neden 9), okundu:** çarpışma işleyicisi `0x59FB70`: çarpma şiddeti `|v|` (40'ta kırpılıyor, `0x7A1858`)
  **≥ 30** (`0x7A186C`) ise **ya da** karşı cismin `+0x38` değeri ≤ −0,5 (`0x787B5C`; büyük olasılıkla çarpışma yönü — kafa kafaya; çıkarım)
  → `araba+0x964 = 1`. `0x6076D0` drag'de bu bayrağı gören yarışçıyı neden 9 ile bitiriyor (son boş yere).
  Bu yalnız drag/`0x890108` açıkken kuruluyor. Birimin m/s olduğu çıkarım (30 m/s ≈ 108 km/h).
- **Motor patlaması (neden 8), okundu:** `0x5A11E0` (motor adımı `0x5AA7F8`'den) bir **ısı sayacı** `+0x8C` tutuyor:
  `devir = ω × 9,5493 (rad/s→rpm)`; `devir > sınır` (`[+0x18]+4`) iken `ısı += 0,45·dt`, değilse `ısı −= 0,1·dt` (0'da taban);
  `ısı ≥ 1,2` → `ısı = 2,0` (kalıcı, "patladı"). Debriyaj/boş vites gibi bir koşulda (`[+0x20]+0x3C == 2` ve `+0x68 ≤ 0`) sayaç çalışmıyor.
  `0x5942C0` = `ısı == 2,0`. `0x6076D0` drag'de **insan oyuncunun** (`araba+0x28 == 1`) motoru patlayınca neden 8.
  Yani kırmızı çizginin üstünde ~2,7 s kesintisiz kalırsan motor gider. `dt` argümanı çıkarım.
- **Vites zamanlaması (Perfect/Good shift): açık.** HUD'da `OptimalRPM`, `PerfectShift`, `MissedShift`, `Shift_light`, `Redline`
  (takometre kurucusu `0x4F5100`, alanlar `+0x68 … +0x7C`) var; pencereyi hesaplayan güncelleyici bulunamadı. `PRPerfectShifts` sonuç istatistiği.
- **Şerit değiştirme / direksiyon: açık.** AI hedefi `"Steer player in drag race"` (`0x40CAD0` yakını; Ghidra fonksiyon değil diyor),
  `DragDriver` (`0x416AA0`), `DragController` (`0x42BFE0`, alanlar `+0x7B4..+0x7C0`), `AIActionHitWallDragRace`/`IJustGotHitDragRace` var; okunmadı.
- Drag zorluğu → şanzıman `+0xE4/+0xE8` ROADMAP'te zaten.

### 5. Outrun, Street X, URL, Drift

- **Kip değişkeni `DAT_0089E7B0`** (okundu): 0 serbest dolaşım · **1 normal yarış** (sprint, circuit, drag, drift, URL, Street X hepsi 1;
  türü ayrı bayraklar taşır) · 3 **lap KO** · **7 outrun** · 8 `0x4FF160` kısayolunun kipi (tur 0; ilerleme `0x56E6E0` ile — çıkarım: serbest/test sürüşü) ·
  2 kodda karşılığı var (çizgi zamanı `≤ DAT_0089E818` ise neden 1 — hedef süreli koşu) ama v1.2'de **hiçbir yerde 2 atanmıyor**; 5 online.
- **Tür baytı (+0xF) → bayrak** (kariyer `0x5322E0`, ROADMAP'e ek): 0 → `0x89E7E0`, 2 → `0x89E7E3`, 4 → drag `0x89E7D8`, 5 → drift `0x89E7D9`;
  `+0x34 == 2` ise hepsi kapalı ve `0x89E7E1 = 1`.
  Hızlı yarış eşlemesi (`0x53FC20`, kip `0x83AAB4`): 0,1,7 → `0x53F150` (circuit/sprint, `E0`); 2 → `0x53F690` **drag**; 3 → `0x53F830` **drift**;
  4 → `0x53FA40` (`E1`) ; 5,10 → `0x53F370` (`E3`); 9 → `0x53F580` (pist 4000 ise kip 0 = serbest dolaşım, değilse 1); 8 → `0x526830`; 11 → `0x526720`.
  **Çıkarım:** `E1` = **URL** (kariyerde ayrı sınıf `+0x34==2`, hızlı yarışta kendi kipi), `E3` = **Street X** (drift ile aynı HUD/kamera ailesi,
  stil puanı serileştirmeden çıkarılıyor, kısa pist `0x1264` drift ile paylaşılıyor), `E0` = **sprint/circuit** ailesi. İsimler eşleşme sırasından
  geliyor, dizge kanıtı yok.
- **Outrun (kip 7), okundu:** serbest dolaşımda `+0xC30` bayrağı `0x607180`'ü tetikliyor: kip 7, mevcut arabalardan yarış kuruluyor.
  `0x5FC510`: her yarışçının `+0x24`'ü **lidere uzaklık** (düz; aynı yol üstündelerse ve lider hızı > 10 m/s ise yol boyunca, `0x40CBC0`;
  30 m'den yakınsa ve geride değilse negatif). `0x6076D0`: lider dışındakilerin boşluğu **> 300 m** (`0x7A56AC`; `0x83AA1A ≠ 1` iken +4,9 —
  ayar baytı, anlamı açık) olunca o yarışçı **neden 7 (kaybetti)**. Tek kalan **neden 6 (kazandı)**. **Süre sınırı yok** — yalnız mesafe.
- **Drift yarışı puanı, okundu (stil puanlayıcısının üstü):** `0x5F7A00` her kare: puan kaynağı yarışçının stil nesnesi (`+0x1D4` → `+0x2C` float toplam).
  **Tur başına puan** `+0x13C + tur·4 = max(önceki, toplam − tur_başı(+0x190))`; tur bitince tur başı = toplam. Son turda `0x56D710` (bankala).
  Drift'te puan nesneleri `0x56EBE0`/`0x56CDF0`/`0x56C8F0` yaratılıyor. **Yarışı kim kazanır** — sıralamanın puana göre yapıldığı yer
  okunmadı (karşılaştırıcı `0x5EEC00` puana bakmıyor; bitiş yerleri `0x606CB0` + 0x89E7DC süre sınırıyla). **Kısmen açık.**
- Kip 1 + `0x89E7E7` (drift hızlı yarışında 0): insan yarışçıda puan farkı `/15 × 0,01` ile, AI'da saniyede bir mesafe farkıyla `0x592640`'a
  besleniyor — büyük olasılıkla **nitro dolumu**; okunmadı.

### 6. Bu repodaki yarışla karşılaştırma (`game/src/race.rs`, `nfs_race`/`nfs_cruise`)

| | SPEED2.EXE | bizde |
|---|---|---|
| sıralama | **tur + rota kesri** (`+0x20`), kabarcık sıralaması, bitirenler bitiş yerine göre | `Race::standings`: bitirenler bitiş sırasına göre, diğerleri `along` (yol noktası/metre) — **aynı model** |
| tur sayımı | yalnız **bitiş çizgisini ileri** geçince; geri geçiş turu geri alır | **zaten aynı**: tur `floor(ilerleme / tur boyu)`, ilerleme sargısız (`RaceLine::track` en yakın turu seçiyor), geriye sürmek turu düşürüyor |
| tur sayısı | olay kaydında (`DAT_0085EF48[2]`, hızlı yarış kaydı `ushort[1]`) | `Race::LAPS = 3` sabit ("hiçbir şeyde yok" yorumu **yanlış**: EXE kariyer olay kaydından okuyor; kaydın kaynağı (GLOBALB?) bulunmadı) |
| yarış sonu | insan normal yarışı **birinci** bitirince **30 s**, sonra kalanlar neden 0xB ile sıralanır (`+0xC` = bitiş yeri; ana oturumda assembly'den doğrulandı) | **taşındı**: `Race::GRACE`, `with_player`, `timed_out`; `nfs_cruise` "süre doldu" yazıyor |
| KO / outrun / drag / drift kuralları | yukarıda | yok |
| geri sayım | okunmadı | `COUNTDOWN = 3.0` |

Taşınmaya en hazır üç kural: (1) 30 s bitiş süresi — **taşındı**, (2) geri geçişin turu geri alması — **zaten sağlanıyor**, (3) lap KO (`sıra + tur == n`) — bizde kip seçimi olmadığı için bekliyor.
