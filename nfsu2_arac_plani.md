# NFSU2 Asset Aracı — Tasarım Planı

CarToolkit'ten (nfsu360) daha iyisini hedefleyen, Linux-native, açık kaynak,
genel amaçlı bir NFS Underground 2 dosya aracı için GUI ve mimari planı.

> Bu bir tasarım referansıdır — kod değil, GUI çizerken açık tutacağın harita.

---

## 0. Konumlandırma — neden "daha iyisi" olabilir

CarToolkit yıllardır ayakta çünkü **çalışıyor**. Onu taklit edersen geriden gelirsin.
Aşmak için kendi doğal avantajlarına yaslan:

| CarToolkit | Bu araç |
|---|---|
| Windows / .NET | Linux-native (+ çapraz platform) |
| Kapalı kaynak | Açık kaynak |
| Tek geliştirici | Topluluk katkısına açık |
| Sabit kodlu parser | Keşif / şema modu |
| Sessiz hata | Doğrulayan, uyaran |
| Ağırlıkla OBJ | glTF (materyal + hiyerarşi) |

**Strateji:** taklit değil, şu eksenlerde aşmak → *Linux-native + açık kaynak +
doğrulayan + keşfettiren.*

---

## 1. Mimari — "önce kütüphane"

Üç katmanlı workspace. Çekirdek hiçbir arayüz bağımlılığı taşımaz.

```
┌─────────────────────────────────────────────┐
│  nfsu2-gui  (egui + bevy)   nfsu2-cli        │  ← tüketiciler
├─────────────────────────────────────────────┤
│            nfsu2-format (çekirdek)           │  ← tüm mantık burada
│  • dosya okuma (mmap, zero-copy)             │
│  • chunk ağacı                               │
│  • parser'lar (mesh, tpk, config)            │
│  • doğrulama                                 │
│  • sıkıştırma (JDLZ aç/sıkıştır)             │
│  • dışa/içe aktarma                          │
└─────────────────────────────────────────────┘
```

**Altın kural:** Arayüz sadece çekirdeğin verdiği veriyi *gösterir* ve komut *iletir*.
Kendi başına format bilgisi taşımaz. Bu sayede 3D önizleme, keşif modu, CLI ve
toplu işlem — hepsi aynı çekirdekten beslenir.

Tasarlarken bunu varsay: her panel çekirdekten bir "görünüm nesnesi" alır ve çizer.

---

## 2. Ana ekran düzeni

Genel amaçlı araç → **dockable (yerleştirilebilir) panel** sistemi şart.
Kullanıcı model incelerken farklı, doku incelerken farklı düzen ister.

```
┌────────────────────────────────────────────────────────────────┐
│  Menü / Araç Çubuğu:  Aç  Dışa Aktar  Mod ▾  Sözlük  Ayarlar    │
├──────────┬──────────────────────────────────────┬──────────────┤
│          │  [ Hex ] [ 3D ] [ Doku ]  ← sekmeler │              │
│  AĞAÇ    │                                      │  INSPECTOR   │
│          │                                      │  (Denetçi)   │
│  ▸ Solid │       merkez içerik alanı            │              │
│    List  │       (seçime göre değişir,          │  seçili      │
│   ▸ Obj  │        sekmeler açık kalır)          │  chunk'ın    │
│     •Mesh│                                      │  parse       │
│   ▸ Obj  │                                      │  edilmiş     │
│     ...  │                                      │  alanları    │
│          │                                      │              │
├──────────┴──────────────────────────────────────┴──────────────┤
│  GÜNLÜK / DOĞRULAMA:  uyarılar, işlem çıktıları                 │
└────────────────────────────────────────────────────────────────┘
```

**Kilit ilke:** Paneller opsiyonel ve taşınabilir. Sadece çıkarma yapan
kullanıcı 3D'yi kapatır; keşif yapan hex'i büyütür.

### 2.1 Sol panel — Ağaç
- Chunk hiyerarşisi, genişlet/daralt
- Her düğümde **rozetler**:
  - `▸` container (0x80 biti)
  - `[JDLZ]` sıkıştırılmış
  - `✓` doğrulama geçti / `⚠` şüpheli
- Kullanıcı gözle tarayınca "burada sorun var" görebilmeli

### 2.2 Merkez — Sekmeli ana alan
Seçime göre içerik değişir, **sekmeler halinde açık kalır**:

| Sekme | Ne zaman | İçerik |
|---|---|---|
| **Hex** | her zaman | ham baytlar, seçili chunk bölgesi renkli vurgulu |
| **3D** | mesh seçili | bevy viewport, döndürülebilir model |
| **Doku** | TPK seçili | DDS önizleme, thumbnail ızgarası |

> Aynı anda açık kalabilmeleri önemli: hex'e bak → 3D'ye geç → geri dön.

### 2.3 Sağ panel — Inspector
- Seçili chunk'ın **parse edilmiş** alanları (ham hex'in insan-okuyabilir hali)
- MeshHeader → vertex/üçgen/submesh sayıları
- ObjectHeader → isim, bbox, matris
- **CarToolkit'te en zayıf yer — burayı güçlü yap**

### 2.4 Alt panel — Günlük / Doğrulama
- Dosyada bulunan sorunlar, uyarılar, işlem çıktıları
- "Ön tekerlek parçasında stride beklenmedik" gibi mesajlar burada akar

### 2.5 Üst — Araç çubuğu + menü
- Aç · Dışa Aktar · Mod değiştir · Sözlük · Ayarlar

---

## 3. Özel modlar (panellerin ötesinde)

Genel amaçlı araç tek bir görünümle yetinmez. Bunları ayrı mod/sekme/pencere düşün.

### 3.1 Keşif / Şema modu  ⭐ EN ÇOK AYRIŞTIRAN ÖZELLİK
- Bilinmeyen chunk'ı seç → "bu veriyi şu alanlarla, şu stride ile yorumla"
- Canlı tanım gir, stride değiştir → tablo anında yenilensin
- ImHex pattern deneyiminin **gömülü, kullanıcı-dostu** hali
- Genel amaçlı araçta olmazsa olmaz: format hiçbir zaman %100 çözülmüş olmayacak
- CarToolkit'te hiç yok (parser sabit kodlu) → **senin en somut katkı noktan**

### 3.2 Doğrulama modu  ⭐ EN BÜYÜK KALİTE FARKI
Konuşmadaki en sert dersin ürünü (sabit başlık varsayıldı, parçalar bozuldu, fark
edilmedi). Dosya açılınca **otomatik sağlık kontrolü**:

- stride mantıklı mı? (36/44/48 gibi makul bir değer mi)
- bbox araç ölçüsünde mi? (birkaç metre)
- normaller birim uzunlukta mı? (|n| ≈ 1)
- index vertex sayısını aşıyor mu?
- chunk size dosya sınırını aşıyor mu?

Sonuçlar → alt panelde liste + ağaçta kırmızı rozet.
**"Sessizce çöp üretmek yerine uyaran alet" = en büyük kalite farkın.**

### 3.3 Karşılaştırma (diff) modu
- İki dosyayı yan yana koy, chunk farklarını göster
- "Bu parça şurada var, burada yok; offsetler şöyle farklı"
- Modcuların "neden bu çalışıyor da öteki çalışmıyor" derdini çözer

### 3.4 Hash sözlüğü yöneticisi
- Dokular sadece **hash** olarak saklanıyor (TEXTURES.BIN'de görüldü)
- İsim ↔ hash eşleştirme tablosu + kullanıcının kendi isimlerini eklemesi
- Çoğu araçta iyi çözülmemiş → ayrışma fırsatı

---

## 4. Etkileşim akışları (tasarımın omurgası)

Kullanıcının %90'ı bunları yapacak. Tasarımı bunların etrafında kur.

**Akış 1 — Keşfet**
`Dosya aç → ağaç açılır → düğüme tıkla → hex'te bölge parlar + sağda alanlar görünür`
→ Senkronizasyon **anlık** olmalı; gecikme akışı bozar.

**Akış 2 — Önizle**
`Mesh seç → 3D sekmesi parçayı yükler → döndür/yakınlaştır`
`Doku seç → önizleme belirir`

**Akış 3 — Çıkar**
`Parça ya da tüm dosya seç → sağ tık/dışa aktar → format seç → hedef klasör`
→ **Toplu seçim** mümkün olmalı.

**Akış 4 — Çöz**
`Bilinmeyen chunk seç → keşif paneli → stride/alan dene → deseni bul → sözlüğe kaydet`

---

## 5. Özellik haritası — öncelik sırasıyla

Genel amaçlı hedefliyorsun ama hepsini birden yaparsan proje boğulur.
Bu sıralama değeri erken verir + mimariyi doğru kurar.

### Faz 1 — Okuma temeli  (değerin çoğu burada)
- [ ] Chunk ağacı (ID'ye göre, container biti, 4-byte hizalama)
- [ ] Hex görünümü + senkron vurgulama
- [ ] Inspector (parse edilmiş alanlar)
- [ ] **Doğrulama** (stride/bbox/normal/index kontrolleri)
- → "Aç, gez, gör, uyar." Çekirdeği sağlam kur, gerisi kolay.

### Faz 2 — Görselleştirme  (kullanıcıyı çeken kısım)
- [ ] 3D mesh önizleme (bevy viewport)
- [ ] Doku önizleme (DDS decode + thumbnail)

### Faz 3 — Çıkarma
- [ ] Model: **glTF** (öncelik) + OBJ
- [ ] Doku: PNG + DDS
- [ ] Toplu dışa aktarma
- → glTF'i öne çıkar: CarToolkit ağırlıkla OBJ'de; sen materyal/hiyerarşi taşıyan
  glTF ile ayrış.

### Faz 4 — Keşif gücü  (seni benzersiz yapan katman)
- [ ] Şema/keşif modu
- [ ] Hash sözlüğü
- [ ] Diff modu

### Faz 5 — Yazma (repack)  (en zor, en son)
- [ ] Doku değiştir → JDLZ ile yeniden sıkıştır → offsetleri kaydır
- [ ] Mesh değiştir → buffer'ları yeniden yaz
- → Neden zor (konuşmadan): TPK **mutlak offset** tutuyor; bir dokuyu değiştirince
  *compressor* yazman (decompressor yetmez) + tüm sonraki offsetleri yeniden
  hesaplaman + hizalamayı koruman gerekiyor.
- → **v1'e koyma**, ama çekirdeği okuma/yazma için **simetrik** kur.

---

## 6. Tasarım ilkeleri (her ekranda geçerli)

1. **Offset/stride sabit gösterme.** Her şey sayaçlardan türesin.
   `stride = vb_size / n_vtx`, `başlık = size − sayı × stride`.
   (Bunu sert öğrendik: 100 baytlık başlık 3 parçada 36/20 çıktı.)
2. **Chunk'ları ID'ye göre bul, sıraya göre değil.** Sıra tutarlı ama garanti değil.
3. **Sıkıştırmayı şeffaf yap.** Otomatik aç, ama "burada JDLZ var" rozetiyle göster.
   JDLZ kontrolünü **blok blok** yap, dosya geneli için varsayma.
4. **Her parse sonucunu doğrula, şüpheliyi işaretle.** Kullanıcının en çok güvendiği
   şey, aletin "bundan emin değilim" diyebilmesidir.
5. **Orijinal EA vs mod farkı.** Bu dosyalar nfsu360 Geometry/Texture Compiler ile
   derlenmiş modlar. Orijinal EA araçları farklı hizalama gösterebilir → aleti
   oyunun kendi `CARS` klasöründeki orijinal araçla da test et.

---

## 7. Doğrulanmış format bilgisi (çekirdek için referans)

Aracın çekirdeğine gömeceğin, bu projede bizzat çözülmüş yapı.

### Chunk temeli
```
[4 byte] ID   (little-endian u32)
[4 byte] size (kendisi hariç veri boyutu)
[size]   veri
ID & 0x80000000 == container  → içine gir (recursive)
Chunk'lar arası 4-byte hizalama
```

### GEOMETRY.BIN — model
```
0x80134000  SolidList (kök)
  0x80134001  SolidListHeader
    0x00134002  ListInfo   → "NFS:U2 Geometry Compiler by nfsu360" imzası
  0x80134010  SolidObject           ← PARÇA (her parça için tekrar)
    0x00134011  ObjectHeader (192 byte)
        +0x14  u32     üçgen sayısı
        +0x20  float3  bbox_min
        +0x30  float3  bbox_max
        +0x40  float16 4x4 birim matris
        +0xA4  char    parça ismi   ← "PEUGOT_KIT00_BODY_A"
    0x00134012  TextureRefs   (16 byte/doku → doku sayısı)
    0x00134013  MaterialInfo
    0x80134100  MeshData
      0x00134900  MeshHeader (68 byte)  ← ANAHTAR SAYAÇLAR
          +0x08  u32  sabit 0x10       ┐ vertex format tanımlayıcısı
          +0x0C  u32  sabit 0x4180     ┘ (stride 36'yı belirleyen; format
                                          değişirse stride değişir!)
          +0x10  u32  submesh sayısı
          +0x24  u32  üçgen sayısı
          +0x34  u32  vertex sayısı
      0x00134b01  VertexBuffer         → başlık = size − n_vtx×36, sonra:
          +0x00  float3  pozisyon
          +0x0C  float3  normal   (|n| ≈ 1, doğrulama için kullan)
          +0x18  u32     renk (BGRA)
          +0x1C  float2  UV       (DirectX; OBJ'ye çevirirken V = 1−v)
      0x00134b02  SubmeshTable         → başlık = size − n_sub×60, sonra 60 byte:
          +0x0C  u32  index sayısı
          +0x1C  u32  materyal indeksi
          +0x34  u32  index başlangıcı  (zincirleme toplanır)
      0x00134b03  IndexBuffer          → başlık = size − n_tri×6, sonra u16 üçlüler
```

**Kritik:** Başlık boyutları SABİT DEĞİL — hizalama dolgusu, parçadan parçaya
değişiyor. Her zaman `size − sayı × stride` ile hesapla.

Ölçek: 1 birim = 1 metre, **Z yukarı**. (bbox X≈4.4m, Y≈2m, Z≈1.36m)

### TEXTURES.BIN — dokular
```
0xB3300000  TexturePack (kök)
  0xB3310000  TPK InfoPart
    0x33310001  Header      → "NFS:U2/MW Texture Compiler by nfsu360" imzası
    0x33310002  TextureHashes   (8 byte/doku)
    0x33310003  CompDescTable   (24 byte/doku)  ← SIKIŞTIRMA HARİTASI
        +0x00 u32 hash
        +0x04 u32 offset        (dosya içi MUTLAK — repack'te sorun burada)
        +0x08 u32 sıkıştırılmış boyut
        +0x0C u32 açılmış boyut
        +0x10 u32 bayrak (0x100)
  0xB3312000  TPK DataPart  → peş peşe JDLZ blokları
```

- Doku verisi **JDLZ** ile sıkıştırılmış (ama zorunlu değil — sıkıştırılmamış TPK
  da olabilir; blok blok kontrol et).
- Doku formatı: 16 byte/blok gördüğümüz için **DXT3/DXT5** (DXT1 olsa 8 byte).
- Çıkarma: offset'e git → comp byte oku → JDLZ aç → DDS header ekle → .dds yaz.
- **İsim yok, sadece hash** → hash sözlüğü özelliğinin sebebi.

### .u2car — config
- Chunk değil. Başlıkta "NFS-CfgEd by nfsu360" imzası.
- Jant pozisyonları, parça yerleşimi, logo gibi ayarlar.
- Gövdesi sıkıştırılmış/karışık → NFS-CfgInstaller ile açılıyor.

---

## 8. Referans araçlar (JDLZ vb. için)

Kendi JDLZ'ini sıfırdan doğrulamaya çalışıp saatini yakma — test edilmiş kod var:

- **NFS404/nfs-toolbox** — Black Box parser'ları (JDLZ referansı)
- **nfs-tools / L5RA** — chunk ID `typeMap`
- **NFSTools/GlobalLib** — global dosya kütüphanesi
- **xan1242/xnfsmodfiles** — chunk override ASI plugin (canlı örnek)
- **NFS-CarToolkit, Binary (nfsu360)** — mevcut standart (kıyas noktan)
- **ImHex** — keşif için; format çözerken bunu kullan

---

## 9. Özet — hareket planı

1. **Çekirdeği önce yaz** (`nfsu2-format`): chunk ağacı + parser + doğrulama.
   Bölüm 7'deki doğrulanmış yapı hazır referansın.
2. **Faz 1 GUI**: ağaç + hex + inspector + doğrulama = "aç, gez, gör, uyar".
3. **Faz 2**: 3D + doku önizleme (etkileyici, kitle çeken).
4. **Kitle bul** (Linux-native + açık kaynak avantajı), sonra Faz 3-4 ile derinleş.
5. **Repack'i en sona** bırak, ama çekirdeği simetrik kur.

> CarToolkit'i tek hamlede geçemezsin — o yıllarca birikti. Faz 1-2 ile çık,
> doğrulayan ve keşfettiren araç olarak ayrış, sonra derinleş.
