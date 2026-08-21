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

### Bu sekizi bir şehrin sekiz parçası değil

Uzun süre "aynı haritanın yarışa özel sürümleri" diye çalıştık. Değiller. Hepsini birlikte
yükleyince ortaya çıkan şey — bir gök kubbenin içinde üst üste binmiş dünyalar — bir hata değil,
verinin kendisi: **altısı ayrı yerler ve hepsi aynı orijini paylaşıyor.**

Üç ölçüm aynı yere çıkıyor (`NFS_BUNDLES=1`, ve `PathsFreeRoam.bin`'in `0x3414A` boyutu):

| bölge | parkur | numaralar | free-roam verisi | zemin merkezi | zemin | ne |
|---|---:|---|---:|---|---|---|
| `L4RA` | 60 | 40xx 41xx | **69.628 B** | (−540, 606) | 5328 × 5188 | **şehir — free roam** |
| `L4RD` | 3 | 44xx | 32.164 B | (−540, 606) | 5328 × 5188 | şehir, yeniden paketlenmiş |
| `L4RB` | 9 | 42xx | 1.700 B | (490, −1250) | 3741 × 3977 | ayrı mekân |
| `L4RC` | 12 | 43xx | 700 B | (−157, 9) | 1820 × 2571 | ayrı mekân |
| `L4RF` | 8 | 46xx | 672 B | (1193, 960) | 1994 × 1507 | ayrı mekân |
| `L4RG` | 13 | 47xx | 1.600 B | (458, −533) | 3708 × 5411 | ayrı mekân |
| `L4RH` | 0 | — | 1.600 B | (−1, 1) | 435 × 1147 | test pisti |
| `L4RR` | 0 | — | 0 B | (0, 0) | 2800 × 2800 | test pisti |

Zemin ölçüsü yalnız `TRN_*` üzerinden; arka fon 15 km genişliğinde ve cevabı boğuyor.

- **Bayview `STREAML4RA.BUN`'dur.** Oyunun 105 parkurunun 60'ı orada, ve free-roam bölge verisi
  ötekilerin 40 katı.
- `L4RD` tek gerçek belirsizlik: zemini metresine kadar şehrin aynısı ve *daha fazla* nesne
  taşıyor (11.135'e 10.735). Ama fazlalığın 70'i kendi üç yarışının `ZPM_4401/4402` propları ve
  free-roam chunk'ı yarıdan küçük. Yani şehir A, D onun yarış paketlemesi.
- `L4RH` ve `L4RR` kendilerini iki kez ele veriyor: hiç etkinlik yok, ve zemin orijine oturmuş
  `TRN_ROADA`/`TRN_TERRAINA` karesi. Haritanın parçası değiller, oyunla gelen test pistleri.

**`PathsFreeRoam.bin` bir rota değil.** Sekizinin de düğüm tablosu yok — kurulumdaki tablosuz
sekiz dosya tam olarak bunlar — sadece `0x3414A` ile `0x3414D` taşıyorlar, ve ikisi de aynı
bölgenin her yarış dosyasındaki kopyalarıyla bayt-özdeş (L4RA'da 60/60). Yani bir yarış dosyası =
bölgenin free-roam verisi + AI ağı + etkinlik kataloğu. Free roam'da takip edilecek yol yok.

Tablo `nfsu2::world::REGIONS`'ta, `free_roam_bundle()` ile birlikte.

### Serbest dolaşım sürülebilir durumda

`NFS_FREEROAM=1` (`nfs_cruise`): `STREAML4RA.BUN` yüklenir, araba oyunun kendi free-roam
ızgarasında doğar, çizgi çizilmez ve "parkur dışı" uyarısı hiç çıkmaz. `NFS_SPOT=<n>` ile
şehrin 24 adlandırılmış noktasından birine gidilir.

Marker verisi (`ROUTESL4RA/TrackPosMarkersFreeRoam.bin` — içi dolu tek kopya, öbür yedisi 16
baytlık kabuk): **32 marker, 25 grup, hepsi track 4000.** Bir tam sekiz kişilik ızgara ve 24
tekil nokta. Grup numaraları hash: `540257916`/`…917`, `931508017`/`…018`,
`1608732748`/`…749`, `3585301327`/`…328` ardışık, ve `h*33 + byte` son harfi bir artan iki ad
için tam bunu üretir. Yani bunlar *adlandırılmış* yerler — dükkânlar, evler — anonim doğma
noktaları değil. Hangi ad hangisi, henüz çözülmedi; kırpılmış parça adı gibi aday hash'leyerek
çıkar.

**Marker yüksekliği ilk kez sınandı** (`NFS_GRIDS=1`). Bu dosyalarda yükseklik taşıyan tek kayıt
o ve bugüne kadar hiçbir şey onu kontrol etmemişti:

- altında yol olan **141 yarış ızgarasının 140'ı 1 m içinde** (medyan −0.0, p05 −0.5, p95 +0.3),
- free roam'un **24 tekil noktasının 22'si 1 m içinde**.

Alanın yükseklik olduğunu ve `remap`'in doğru olduğunu söyleyen ölçüm bu — 24 sayı bir şehrin
yüzeyine tesadüfen oturmaz. İstisnalar: tek yarış ızgarası (track 4301, kendi bölgesi yüklü
değilken 10.6 m) ve free roam'un **kendi ızgarası, 29.7 m havada**. O noktada — motor
çerçevesinde (884, −1695) — şehrin sunduğu tek yüzey 23.8; kaba kademeler geri açılsa da, sekiz
bundle birden yüklense de hâlâ 23.8. Nedeni bilinmiyor. Çağıran taraf sayıya güvenmek yerine
arabayı yere indiriyor (`nfs_cruise` zaten her doğuşta bunu yapıyor) ve araba dört tekerlek üstüne
inip sürüyor. Izgara yine de tekil noktalara tercih ediliyor, çünkü tek o bir **yön** taşıyor.

### `Routes####F/B.bin` çözüldü

226 dosya, ileri/geri çiftler hâlinde. Üç yaprak: `0x34121` 226'sında, `0x34122` 222'sinde (test
pistleri hariç), `0x34123` yalnız `ROUTESL4RA`'nın 122 dosyasında — hep tam 10.488 bayt ve
**122'sinde bayt-özdeş**.

**`0x34121` şeritlerdir, ve yürüyüş kendini bildiriyor:** blok = **84 baytlık başlık + (sayaç+1) ×
56 baytlık kayıt**, sayaç `+52`'deki `u16` — `+54`'te tekrar ediyor ve ikisi 17.226 blokta da
uyuşuyor. Bu yürüyüş 222 dosyanın **hepsinde** ödemeyi tam tüketiyor. Komşu okumaların hiçbiri
tüketmiyor: 80 ya da 88 baytlık başlık, 52 ya da 60 baytlık kayıt, `+50` ya da `+56`'dan sayaç —
altısı da 222 dosyanın 222'sinde uçtan sapıyor. `read_lanes` altısını da reddediyor.

| alan | ne |
|---|---|
| `+0`, `+4` | `11, 11` — biçimin imzası, marker kaydındakiyle aynı |
| `+10` `u16` | bloğun dosya içindeki numarası |
| `+16`, 16 B | ad: `TrackRoutesA10`…`A64`, 30 tane, `A{grup}{çeşit}` |
| `+52`, `+54` `u16` | nokta sayısı, iki kez |
| `+56`, `+60` `f32` | noktaların `along` değerlerinin min/max'ı (17.222'de 16.358, medyan hata 0) |
| `+68`…`+83` | 16 bayt `0xFF` |
| kayıt 0 `+12`…`+27` | bloğun kutusu — **17.222'de 17.222 blokta bütün noktalar içinde**, medyan pay tam 10 m |
| kayıt 1+ `+0`,`+4`,`+8` | x, y, ve yol boyu koordinat |

Ad her zaman `A` ile başlıyor, bölge harfiyle ilgisi yok: `ROUTESL4RG` de `TrackRoutesA50`
taşıyor, `ROUTESL4RB` de. Grup bir bölgeye değil bölge kümesine ait (`A30` sekizin altısında,
`A6x` yalnız `ROUTESL4RB`'de), yani bir yer değil bir şerit türü.

**F ile B'nin farkı.** Çiftin üç chunk'ından `0x34123` 60 çiftin 60'ında bayt-özdeş; `0x34121` ve
`0x34122` 105'inin hepsinde farklı ama 104'ünde aynı uzunlukta. `0x34121` içinde iki dosya **aynı
blokları, aynı adlarla, aynı sırada, aynı nokta sayılarıyla** taşıyor — ve sıra kesinlikle ters
değil: F'nin `i`. bloğunu B'nin `n−1−i`. bloğuyla eşleştirmek 7.399 denemede 9, düz eşleştirmek
5.912 tutuyor. Yalnız iki şey değişiyor: başlığın `+56`/`+60` aralığı ve her noktanın `+8`'i.
Toplamları blok içinde 7.399'un 5.912'sinde sabit. Yani çift, tek bir geometrinin ölçüsünün iki
uçtan alınmış hâli.

**Beklediğimizi vermedi, bunu böyle yazıyorum.** Bu dosyaların bir parkurun iki ızgarasından
hangisini kullandığını ve ızgaranın hangi ucunun ön olduğunu söylemesini bekliyorduk. Söylemiyor:
`+8` bir *blok* boyunca ilerliyor, blok ise yarışlar arasında paylaşılan bir yol parçası, bir
yarışın çizgisi değil. Doğrudan sorulduğunda 442 tam ızgaranın en yakın şerit noktası 116'sını
blok başına, 54'ünü sonuna, 212'sini ortaya koyuyor — cevap yok. `route::start_grid`'deki varsayım
duruyor ve hâlâ varsayım.

### Izgara sorusu: hangi ızgara çözüldü, ön/arka çözülmedi

Yön sorusunu `0x34122`'nin çözmesini bekledim; çözmüyor. Kayıt düzeni çıktı — **4 baytlık başlık +
32 baytlık kayıt, 222 dosyanın 222'sinde**, dört alternatif başlık boyu (0/8/12/16) 222'de 0 — ama
F ile B'de neredeyse her alan değişiyor (%93–99) ve `+8/+10` ne aynı ne takas (25.595 kayıtta
%0.4/%0.5). İki dosya bu tabloyu baştan kuruyor, sıra eşleşmiyor.

`0x34123` de çözmez, çünkü bölgenin 122 dosyasında bayt-özdeş — yarışa özel hiçbir şey taşıyamaz.
(Bir önceki turda "en umut verici yer" demiştim, yanlıştı.) Sabit adımlı bir dizi de değil: 16'daki
otokorelasyon zirvesi `FFFFFFFF, −1, −1, 0` dolgusundan geliyor, kalanlar hiçbir adımda toplanmıyor.

Cevap zaten elimizdeki **etkinlik anahattında** çıktı. Izgaraları anahatla karşılaştırınca:

- İki ızgarası olan **86 parkurun 74'ünde ikisi zıt yöne bakıyor** — yani bir parkurun iki ızgarası
  gerçekten iki yarış yönü.
- **82'sinde tam olarak biri** anahattın çizildiği yönle uyuşuyor.
- Ve o uyuşan, **82'nin 81'inde zaten ilk ızgara** — yazı tura 41 verirdi. Dosyadaki ızgara sırası
  kendi başına anlamlı; `start_grid`'in "ilkini al" kuralı kazara doğruymuş.

`start_grid_facing` bunu varsaymak yerine türetiyor ve kalan 1 parkuru düzeltiyor. Kazanç küçük;
değerli olan kuralın artık ölçülmüş olması. **Ön/arka yönünün küresel işareti hâlâ varsayım** —
slot `0..3` her yerde arka sıra olsaydı aynı 81/82 çıkardı. Ama bu tek bir küresel bit ve yanlışsa
her yarışta her araba ters bakar, yani bir koşuda görülür.

### `0x3414D` okundu, ama isimlendirilemedi

Paths dosyalarının en büyük okunmamış yaprağıydı; artık okunuyor. **36 baytlık kayıt, 111 dosyada
118.729 tane**, ve yaprak bölge geneli — bölgenin her dosyasında bayt-özdeş, `PathsFreeRoam.bin`
dahil.

Kayıt kendi tutarlılığıyla sabitleniyor: `+17`, `+20`'deki sıfır olmayan kelimelerin sayısı ve
**118.729'un 118.729'unda** öyle; sayacın ötesi hep sıfır, `+16` ve `+19` hep sıfır. 24 baytlık
okumada bu kural kayıtların %47.3'ünde, 48 baytlıkta %33.5'inde tutuyor; 32 ve 40 tek bir ödemeyi
bile bölmüyor.

İçerik: medyan **24.4 m** aralıklı iki nokta (p05 7.9, p95 75.1, en uzun 792.6) — kutu değil bir
**doğru parçası**, çünkü 118.729'un hiçbiri sıfır uzunlukta değil; bir bayrak (115.984 set,
2.745 clear); ve 1–4 adet 32-bit referans.

**Referanslar isimlendirilemedi, ama üç tahmin öldürüldü:** bölgenin nesne hash'leri değil, doku
yuvası anahtarları değil, `0x3414A` bölge kimlikleri değil — üçünde de 3.032'de 0. Bütün kurulum
**159 farklı değer** kullanıyor, dört sıkı kümede (93 · 60 · 5 · 1), küme içinde ardışık koşularla
— `h*33 + byte`'ın son harfi bir farklı iki ad için verdiği şey. Yani dört aileden ad hash'i gibi
duruyorlar. Hangi adlar, aday ad listesi olmadan çıkmaz.

`0x34122`'nin alan anlamları, `0x34123` ve `0x34149` okunmadı.

### Kare bütçesi — projenin hiç sahip olmadığı sayı

**Bayview'da serbest dolaşım, araba dahil: medyan 8,0 ms, p95 ~9 ms, en kötü 14,2 ms — 100–126 fps.**
(`nfs_cruise` artık `NFS_DIAG=1` ile ve HUD'da kare süresini yazıyor; pencere 60 kare.)

Bugüne kadar culling, detay kademesi ve streaming hakkındaki her tartışma **nesne sayısı** üzerinden
yürüdü, ve nesne sayısı kare süresi değil. Motorun kendi uzamsal indeksi bile belgesinde "BVH 8k
renderable'ın altında doğrusal taramaya kaybeder, kendi sahneni ölç" diyor. Ölçtük:

| ne | mesh | medyan kare |
|---|---:|---:|
| `NFS_TIERS` varsayılan (en ince kademe) | 5.672 | **8,0 ms** |
| `NFS_TIERS=all` (üç kademe birden) | 6.406 | **9,1 ms** |

%13 daha fazla mesh, %14 daha fazla süre — doğrusal ve küçük. p95 medyanın ~1,5 ms üstünde, yani
fizik kaynaklı ani sıçrama yok.

**Sonuç:** uzamsal indeks (MOTOR-NOTLARI 2) bu ölçekte alacak bir şey vermiyor ve kapandı; mesafeye
göre LOD seçimi (madde 8) en fazla 1,1 ms kazandırır, önceliksiz. Bu, M5'i gereksiz kılmaz — M5
bellek ve birden çok bölge meselesi, bu kare hızı meselesi değil.

### Gökyüzü geldi, ve iki belirtinin tek sebebi çıktı

`nfs_cruise` artık NFSU2'nun kendi boyalı backdrop'unu **varsayılan olarak** çiziyor
(`NFS_BACKDROP=0` kapatır). Buraya iki hatayı üst üste düzelterek gelindi:

1. **Kameraya kilit.** Motorun `MaterialType::Backdrop`'u üç davranışı ayrılamaz paketliyordu ve
   biri kameraya kilitti — bir birim-küp skybox için doğru, 12–18 km genişliğindeki dünya konumlu
   geometri için yıkıcı. Motora `BackdropPlaced` eklendi (`48ac99e`). Ölçüm: kamerayı 1.000 m
   kaydır, panelin ekranda aynı kalan piksel oranı **%45,4 → %0,0**.
2. **Dokular hiç yüklenmiyordu.** `nfs_city`'nin yükleme döngüsü yalnız şehrin mesh'lerini
   geziyordu; backdrop ondan sonra kuruluyordu, yani anahtarları hiç GPU'ya çıkmıyordu ve beyaza
   düşüyorlardı. Backdrop'un çoğu dokusu şehrin paketlerinde değil, paylaşılan katmanda
   (`TRACKS/LOC4DYNTEX.BIN`). Düzeltince **85/153 → 153/153** bağlandı, backdrop-only karenin
   medyanı **199 → 30**. (`nfs_cruise`'da bu hata yoktu, zaten zincirliyordu.)

Ve buradan beklenmedik bir kazanç çıktı: **zemindeki büyük beyaz düz yamalar ayrı bir hata
değilmiş.** Onlar yansıtıcı yüzeyler — ıslak asfalt, su — ve göğü aynalıyorlar. Beyaz bir
backdrop'u aynalayınca beyaz görünüyorlardı; gerçek gece silüetini aynalayınca doğru görünüyorlar.
Bir sebep, iki belirti.

Maliyeti ölçüldü: kare medyanı **8,0 → 8,6 ms**.

### Pencereler neden sönüktü — ve neden bu bir hata değil

Gece karesindeki binaların pencereleri siyah görünüyordu. Üç hipotez sırayla elendi, hepsi ölçümle:

1. **Doku bağlanmıyor mu?** Hayır. Pencere dokuları parlak: `ARC_*_WINDOW*` ailesinin p95'i ~250,
   max 254. Yani yanan pencere dokuda var.
2. **Yanan yüzeyleri ayıran bir işaret mi kaçırıyoruz?** Hayır, ve bu kesin: `NfsMaterialRange`
   bir shader hash'i taşıyor — arabalarda camı boyadan ayıran alan bu — ama şehirde **16.641
   run'ın hepsinde `00000000`**. Tek bir farklı değer yok. Şehir bu ayrımı yapmıyor.
3. **Vertex rengi yanlış mı okunuyor?** Hayır. Dağılım sağlıklı: 799.010 vertex üzerinde p05 9,
   p25 29, medyan 51, p75 93, p95 183, max 255. %4'ü 8'in altında, %3,2'si 200'ün üstünde. Kod
   hatası sıfıra ya da 255'e yığılırdı; bu yazılmış bir gece.

Yani karanlık veri sadık şekilde çiziliyor. Eksik olan **parlama**. Ve orada gerçek bir bulgu var:
motorun bloom eşiği `0.85` ve **bu şehirde hiçbir şey ona ulaşmıyor** — en parlak yüzey lineer
uzayda ~0,44. Parlama geçidi her kare koşuyor ve hiçbir şey çıkarmıyordu.

`scene::city_glare` eşiği `0.18`, yoğunluğu `1.2` yapıyor (`NFS_BLOOM="eşik[,yoğunluk]"` ezer).
Ölçüm: kare maksimumu 202 → 221, 200 üstü piksel oranı %0,00 → %0,06 — küçük bir sayı ve doğru
küçük sayı, çünkü kıpırdayan tek şey yanan pencereler ve neon. **Medyan hiç oynamıyor**, yani bu
bir parlaklık kolu değil parlama kolu; parlaklık kolu `city_lift`. Kare maliyeti değişmedi.

### Izgara doldu

`nfs_cruise` bir yarış yüklediğinde ızgaranın sekiz yerinin hepsine araba koyuyor
(`NFS_RIVALS=<n>` sayıyı değiştirir, `0` eski davranış). Şoförleri yok — yerlerinde duruyorlar —
yani bu henüz saha değil, formasyon.

Maliyeti ölçüldü ve yok sayılır: yedi rakip kare medyanını **8,0 → 8,1 ms** yapıyor. Her biri
kendi `spawn_car`'ı, yani model dosyası araba başına yeniden ayrıştırılıyor; israf, ve bilerek
öyle bırakıldı — ölçüm karenin oraya gitmediğini söylüyor, ortak geometri yolu ise `rig`'de bir
değişiklik ve ona ihtiyaç doğunca yapılmalı.

Formasyonun kendisi dört ayrı yarışta ölçüldü ve dördünde de aynı: **4 yan yana × 2 sıra**,
yanal 3,5 m, derinlik 5,7 m. Araba 1,64 × 4,39 m, yani sığıyor. Ve pole en önde — satırlar
`0.0` ile `−5.7` arasında ve slot 0 `0.0`'da — yani `start_grid`'in "slot 0..3 ön sıradır"
varsayımı artık geometriyle doğrulandı, kalan tek belirsizlik olan küresel ön/arka işareti de
kapandı.

### Rakiplere şoför — çalışıyor, ama 100 m sürüyor

`rig::Pilot` saf takip (pure pursuit): çizgide ileride bir noktaya nişan al, ona doğru dön. Oyuncunun
kullandığı `CarRig::drive`'ın aynısından geçiyor — fiziğe uzanan bir pilot, yarışılan arabadan başka
bir araba sürüyor olurdu.

Çalışan kısım ölçüldü: yedi rakip ızgarada doğuyor, gaz veriyor, 30 km/h'ye çıkıyor, direksiyon
kırıyor ve ilerleme sayacı ilerliyor. Kare maliyeti 8,0 → 8,1 ms.

**Sonra takılıyorlar, ~100 m'de.** Takıldıkları yeri çizdirdim: bir otoyol kavşağı, şeritlerin
arasında çarpışma bariyerleri. Sebep pilot değil, ona verdiğimiz çizgi: elimizde **yarış çizgisi
yok, bir yol ağı var** — 40 yol, aralarında kavşaklar — ve pilot ağın bir yolunu takip ediyor.
Nişan noktası bariyerin öbür tarafına düşebiliyor.

`relocate` artık yalnız arabanın baktığı yönle uyuşan yolları seçiyor (karşı şerit birkaç metre
ötede ve bariyerin arkasında), bu biraz uzattı ama kökten çözmedi.

**Çizgiyi ağdan yürüyerek çıkarmayı denedim, iki hipotez de yetmedi:**

1. *İlerleme sırasına diz.* Çürüdü: bütün düğümleri `progress`'e göre sıralayınca adımların
   %44'ü 100 m'nin üstünde. O ölçü yol-içi, ağ geneli değil.
2. *Kavşakları yürü.* Kısmen: düğümlerin `links`'ini takip edip her kavşakta ilerlemeyi sürdüren
   bağlantıyı seçmek **geometrik olarak sürekli** bir çizgi veriyor (üç yarışta da 100 m üstü
   sıçrama sıfır) — ama yalnız %1–16'sını kaplıyor, sonra duruyor.

Yolda çıkan asıl bulgu bu ikincisini ararken geldi ve parsere yazıldı: **dosya sırası sürüş sırası
değil.** Kurulumdaki 2.923 yolun **1.361'i** dosya sırasında `progress` düşerek bitiyor, yani
neredeyse yarısı sürüldüğü yönün tersine saklanmış. Bir yolun başı `+20`'nin sorusu, indeksin
değil.

**Yürüyüşün neden durduğu bulundu ve o hattı kapattım:** dördü de "hiçbir bağlantı ilerlemeyi
sürdürmüyor" diye duruyor. Ama sayılara bakınca kuralın önermesi çürüyor — 4041'de yürüyüş
`progress`'in tamamını (0→5911) kat ediyor ama yalnız 970 m yol gidiyor, altı kat fark; ve 4001'in
ilerlemesi 0'dan değil **804**'ten başlıyor. `progress` küresel bir tur koordinatı değil, yollar
farklı tabanlar taşıyor. Kavşakta "ilerlemeyi sürdür" kuralının dayanağı yok.

Bunun yerine okunmamış `0x34149`'a baktım ve orada gerçek bir şey çıktı — bkz. aşağısı.

### `0x34149` çözüldü: yol başına bir kayıt

**220 bayt / yol**, ve ödeme tam olarak `220 × yol sayısı` — leafi taşıyan 105 dosyanın hepsinde.
İki alan okundu ve ikisi de **2.923 kayıtta 2.923** doğru: `+116`/`+120` yolun dosya sırasındaki
ilk ve son düğümünün ilerlemesi, `+128`/`+132` en küçüğü ve en büyüğü.

İkisi birlikte **yolun hangi yöne sürüldüğünü dosyanın kendisi söylüyor** — `from > to` ise ters
saklanmış — ve çıkan sayı 1.361, onsuz yaptığımız ölçümle kayıt kayıt aynı.

Oyun tarafında `build_route` artık yolları ilerlemeye göre çeviriyor. Bu bir düzeltme, süs değil:
çizginin neredeyse yarısı ters çiziliyor ve ters sürülüyordu. Rakipler 100 m yerine ~140 m gidiyor,
yedisi de koridorda kalıyor — ama hâlâ duruyorlar, yani asıl sorun (yarış çizgisinin ağdan nasıl
çıkarılacağı) açık.

### Rakipler ağı sürüyor: 100 m → 700 m

Yarış çizgisini veriden çıkarma denemelerini bıraktım — üçüncüsü de (`0x34149`'un `+36` ardılı)
%95'i ancak 105 parkurun 43'ünde tutturuyor. Bunun yerine **elimizde çözülü olanı** kullandım.

İki parça: `world::Network` düğüm tablosunu sürülebilir bir grafa çeviriyor (yol içi komşuluklar +
kayıt bağlantıları, ikisi de çift yönlü), ve **yol noktaları etkinlik anahattından** geliyor.
Anahat sürülemez — 6 km'de 17 nokta, medyan adım 425 m — ama **17 köşesinin 14'ünün altında yol
var** ve tur sırasında. Yani sürüş çizgisi değil, ama yol noktası listesi.

Pilot artık grafı yürüyor ve her kavşakta bir sonraki yol noktasına yaklaştıran dalı seçiyor. Bu,
"bu yarış çizgisidir"den daha zayıf bir iddia ve verinin desteklediği iddia bu.

Sonuç: lider **89 km/h**'ye çıkıyor, **~700 m** gidiyor, yol noktası 1'den 2'ye geçiyor. Başlangıçtaki
100 m'nin yedi katı. `network: 341 düğüm · 810 bağ · çıkışı olmayan 0`.

**İlerleme kuralı üç denemede oturdu ve ikisi ölçümle elendi:**

- *"Yeterince yakınsa ilerle"* — kaçak. Adım attığı düğüm de genelde yeterince yakın, o yüzden pilot
  grafı kare hızında yürüyor: rakip başına yüz saniyede **6.600 kavşak**, oysa 96 km/h'de 29 m'lik
  düğümlerle saniyede bir tane sürülebilir.
- *"Yalnız düğüm arkada kalınca ilerle"* — tersi. Arabanın ulaşamadığı bir düğüm hiç arkada
  kalmıyor, pilot ilk ulaşamadığında duruyor: **65 m**.
- *"Bir sonraki düğüm arabaya daha yakınsa ilerle"* — yapısı gereği kendini sınırlıyor: duran araba
  ilerlemeyi durduruyor, hızlı olan yetişiyor. **106 kavşak, ~700 m.**

Hâlâ duruyorlar. Ama artık durdukları yer bir teşhis sorusu, mekanizma sorusu değil.

### Nerede durduklarını sorduk: bir yerde, ve iki hipotez daha eledik

`NFS_FIELD=1` her rakibin konumunu, düğümünü, kavşak sayısını ve yol noktasını saniyede bir
yazıyor. Cevap net: **yedinin altısı tam olarak aynı yerde** duruyor — düğüm 132, (-85, 1075),
hepsi 21 kavşak sonra. Yani sistemik bir sürücü sorunu değil, belirli bir engel.

Orayı çizdirdim: **çok katlı otoyol kavşağı**, ve yarış çizgisi şeridi birkaç katta birden
görünüyor. Buradan iki hipotez çıktı, ikisi de ölçüldü:

**1. Ağ yüksekliğe kör (doğru çıktı, ve ciddi bir hataydı).** `Network::of` düğüm yüksekliğini
`height_at`'e 2 m'den soruyordu. O fonksiyon "verilen noktanın altındaki en yüksek yüzey"i döndürür
— yükseltilmiş bir yolda bu **hiçbir şey**, ve düğüm sıfıra düşüyordu. Ölçüm: düğüm 132'nin
yüksekliği `0.0`, araba onun **10,3 m üstünde**. Pilot yer altındaki bir noktaya nişan alıyordu.

Düzeltme: yükseklikler artık çizilen çizginin kullandığı yolla çözülüyor — `route::follow`, yani
düğümün XZ'sindeki bütün aday yüzeyler ve yol boyunca en az tırmanan dizi. Paylaşıldı, yeniden
yazılmadı; araba ile şerit aynı kavşağın farklı katlarında olamasın diye.

**2. Dik bağları reddet (çürüdü).** "Şehir kat kat, düz düzlemde ölçen bir graf arabayı üst deste
yollar" fikri makul ama yanlış. Eşiği süpürdüm ve **her eşikte filtresiz durumdan daha kötü**:
0.30 → 34 kavşak, 0.60 → 29, 1.00 → 33, filtresiz → en uzağı, ikinci yol noktasına ulaşan.
Şehrin rampaları göründüğünden dik; arabaları köprüden uzak tutması beklenen sayı onları yoldan
uzak tutuyor. Filtre kaldırıldı, eğim yalnız rapor ediliyor.

Düğüm 132'deki engel hâlâ orada. Ama artık yüksekliği doğru, ve tıkanma yeri bir soru olarak
keskin: altı araba oraya varıyor, yedincisi ızgaradan hiç çıkamıyor.

### Duvar: doğru filtre, ve onu bulan şey süpürme

Düğüm 132'yi araba açısından çizdirdim: yolda, sağda **dalgalı beton istinat duvarı**, ve düğüm tam
o yönde. İkinci kez aynı desen — önce çarpışma bariyeri, şimdi duvar. Ağ yan yana giden yolları
birbirine bağlıyor ve aralarında ne olduğunu söylemiyor.

Parser bunu söyleyemez; bu şehir hakkında bir soru, ve şehir cevaplıyor. `Network::drop_walled`
her bağın üstünde yürüyüp `Ground`'a "burada araya giren yükseklikte bir yüzey var mı" diye soruyor.
Yol devam ediyorsa var, duvar ya da boşluk varsa yok.

**Ama toleransı süpürmeden koymadım, çünkü bir önceki "makul" filtre çürümüştü.** Sonuç:

| tolerans | kesilen bağ | kavşak | yol noktası |
|---:|---:|---:|---:|
| 0 (filtresiz) | 0 | 106 | 2 |
| 5 | 148 | 65 | 1 |
| **8** | **90** | **166** | **6** |
| 12 | 62 | 166 | 6 |
| 20 | 59 | 166 | 6 |

Dar tolerans gerçek yolları kesiyor (iki düğüm arasındaki düz çizgi tümseği ya da çukuru takip
etmez); 8'den sonra plato. Varsayılan 8, `NFS_WALL` ile değişir. Alan ızgaradan ~1,5 km uzağa,
(528, 972)'ye ulaşıyor.

### `nfs_sim`: penceresiz yarış

Yukarıdaki tablo bir öncekinin sekiz dakikası yerine **65 saniye** sürdü. `nfs_sim` aynı şehri,
aynı ağı, aynı ızgarayı ve aynı fiziği kuruyor ama kare çizmiyor: 120 saniyelik yarış **13 saniyede**
koşuyor, gerçek zamanın 9 katı. GPU yine açılıyor — `spawn_car` doku yüklüyor, yani araba gerçek —
ama sunulmuyor.

Sonunda makinece okunabilir tek satır bırakıyor (`SUMMARY junctions=… waypoint=…`), ki süpürmeler
döngüye girebilsin. Ölçüt olarak metre değil **kavşak ve yol noktası** seçildi: duvara sürtünen bir
araba da metre biriktirir.

### Üçü neden çıkamıyor: elenenler ve kalan sınır

Simülatör deterministik — üç koşu birebir aynı özet veriyor — yani aşağıdakiler gürültü değil.

**Elenen sebepler:**

- *Kötü slot.* Hayır: sekiz slotun sekizi de aynı marker yüksekliğinde (9.98), aynı zeminde (10.0),
  aynı iki yüzeyin üstünde.
- *Kötü yerleşim.* Hayır: sekiz pilotun sekizi de **aynı düğüme** (287) yerleşiyor.
- *Kalkışta birbirlerine çarpma.* Hayır. Izgarayı açtım (1×, 2×, 4×): 5/8, 6/8, 5/8 — düzelmiyor,
  ve 4×'te dıştaki arabalar yoldan çıkıp düşüyor. Aralıklı kalkış denedim (0 / 0,5 / 1 / 2 sn):
  5/8, 4/8, 3/8, 6/8 — gürültü.
- *Sabit bir slot.* Hayır: başarısız olan üçlü yapılandırmaya göre değişiyor.

**Kalan:** pilotun kurtulma davranışı yoktu — bir şeye sürtünen araba tam kilit ve %15 gazla orada
kalıyor. Ekledim (1,5 sn hareketsizlik → 1,2 sn geri vites, direksiyon ters). İlk hâli her şeyi
bozdu: kalkışta zaten duran arabaya "takıldı" dedi ve `|hız|` kullandığım için geri giderken de
takılı saydı — bütün alan altmış metre geriye, ters yöne gitti. Düzeltilmiş hâli **sayıyı
oynatmıyor**: kurtulmasız 166 kavşak, kurtulmalı 165, aynı beş araba gidiyor aynı üçü kalıyor.

Duruyor, çünkü sürtündüğü şeyden geri kaçan bir sürücü sürücüdür, sonsuza kadar öğüten değildir —
ama düzeltmediğini yazıyorum.

### O sınır yanlıştı: "sekizde beş" bir pilot gerçeği değil, tek parkur gerçeğiymiş

Aynı ölçümü sekiz parkurda koştum ve tablo tamamen değişti:

| parkur | giden | kavşak | yol noktası |
|---|---:|---:|---:|
| 4001 | 5/8 | 165 | 6 |
| 4002 | 0/8 | 4 | 0 |
| 4021 | 0/8 | 0 | 0 |
| 4041 | 2/8 | 47 | 2 |
| 4061 | 0/8 | 0 | 0 |
| 4081 | 1/8 | 21 | 2 |
| **4102** | **8/8** | **275** | 4 |
| 4121 | 0/8 | 0 | 0 |

**Dört parkurda hiç kimse kıpırdamıyor**, birinde sekizde sekiz gidiyor. Bütün ayarlarımı bu turda
tek bir şanslı parkurda (4001) yapmışım — ve o yüzden iki müdahalem de "işe yaradı" sonra
genellenmedi:

- **Yerleşimi öne koni ile sınırla.** 4001'de sebep mükemmel eşleşiyordu: giden beş araba düğüme
  4,5–11,0 m, takılan üçü 13,5–16,4 m uzaktaydı, yani başarısızlık mesafeye göre kusursuz
  sıralanıyordu. Koniyi süpürdüm (90°→25°): 5/8 → 3/8, 3/8, 2/8, 2/8. Korelasyon gerçek,
  önerdiği müdahale yanlış. Parametre silindi.
- **Direksiyon kilidini hıza bağla.** Gerekçe sağlam (raycast araç duruşta yanal kuvvet üretmez, ve
  iz takılan arabaları tam kilitte gösteriyordu) ve 4001'de işe yaradı: 5/8 → 6/8, 165 → 198.
  Dört parkurda takas: 4041 2/8 → 6/8 ama **4102 8/8 → 5/8**, 275 → 111 kavşak. Silindi.

Ağların şekli farkı açıklamıyor: 4001/4021/4102/4121 karşılaştırıldığında düğüm sayısı, bağ sayısı,
çıkışsız düğüm ve ilk arabanın düğüme uzaklığı hepsi benzer.

### Dört ölü parkurun sebebi: pilot yanlış yol noktasından başlıyordu

İzi açtım: ölü parkurlarda araba **geri geri** gidiyor ve yol noktası sayacı 0'da kalıyor. Kurtulma
davranışını suçladım, kapatıp sınadım — sebep o değil, dört parkur onsuz da ölü.

Asıl sebep tek bir satırdı: pilot her zaman **0. yol noktasından** başlıyordu. Oysa etkinlik anahattı
kapalı bir halka ve yazarının başladığı yerden çiziliyor; ızgarayla ilgisi yok. Ölçüm bunu kusursuz
sıralıyor:

| parkur | wp0'ın ızgaraya uzaklığı | giden araba |
|---|---:|---|
| 4001 | 10 m | 5/8 |
| 4102 | 13 m | 8/8 |
| 4081 | 33 m | 1/8 |
| 4041 | 75 m | 2/8 |
| 4121 | 160 m | 0/8 |
| 4021 | 493 m | 0/8 |
| 4002 | 785 m | 0/8 |
| 4061 | 844 m | 0/8 |

Yarım kilometre ötedeki bir noktaya nişan alan pilot arabayı ilk kareden itibaren parkurdan uzağa
sürüyor. `place` artık ızgaraya **en yakın** yol noktasını buluyor ve bir sonrakini hedefliyor —
en yakını ızgaranın üstünde olduğu için, orayı hedeflemek yerinde dönen bir araba demek.

Sonuç: 4021 **0/8 → 5/8** (0 → 109 kavşak, wp0 → wp11), 4061 0 → 12 kavşak wp5, 4121 0 → 13 kavşak
wp2, 4002 wp0 → wp9. **Artık sekiz parkurun sekizinde de yol noktası ilerliyor**, hiçbiri 0'da
kalmıyor. 4102 8/8'den 7/8'e indi, tek kayıp.

### Anahattı sıklaştır: 20 araba → 29, 6,7 km → 8,1 km

Kalan zayıflığın sebebi aynı ailedendi. Anahat kaba: 6 km'de 17 köşe, medyan adım 425 m. Izgara iki
köşenin ortasına düşerse **en yakın** yol noktası bile yüzlerce metre uzakta oluyor — 4002'de 133 m,
4061'de 197 m. Pilot oraya nişan alıyor ve parkurdan sapıyor.

`route::densify` anahattı 40 m'lik adımlara bölüyor. Bilgi eklemiyor — köşeler dosyanın söylediği
tek şey olarak kalıyor — sadece aralarındaki boşlukları sürücünün kaybolabileceği yer olmaktan
çıkarıyor.

Adım süpürmeyle seçildi, sekiz yarış üzerinden, iki ölçütle:

| adım | hareket eden araba | toplam mesafe |
|---|---:|---:|
| 25 m | 29/64 | 6,3 km |
| **40 m** | **29/64** | **8,1 km** |
| 80 m | 27/64 | 7,8 km |
| 200 m | 24/64 | 6,8 km |
| sıklaştırma yok | 20/64 | 6,7 km |

Ve `nfs_sim` artık **ızgaradan uzaklığı** da yazıyor. Kavşak sayısı parkura göre değişiyor ve yol
noktası sayısı sıklaştırmayla değişti; mesafe ikisinden de bağımsız, yani karşılaştırma yapılabilen
tek ölçüt o.

Parkur başına en uzağa varan araba: 4041 **1944 m**, 4061 1817 m, 4121 1624 m, 4001 991 m,
4081 746 m, 4102 640 m, 4021 358 m — ve 4002 **42 m**, tek gerçekten çakılı kalan.

Sınır: sekiz parkurun yedisinde alan yola çıkıyor ve bir kısmı kilometrelerce gidiyor; 64 arabanın
29'u sürüyor.

### 4002: dosyanın çizgisi ayırıcıların üstünden geçiyor

Tek gerçekten çakılı parkuru izledim. Araba kalkıyor (29,7 km/h), bir kavşak geçiyor, sonra
(882, 462)'de duruyor ve orada kalıyor. Devrilmemiş — `up` 0,97, yani ~15° eğik — ve zeminin
0,95 m üstünde, yani normal duruyor ama yatık.

Üstten bakınca sebep göründü: geniş bir otoyol, ortasından **sarı çizgili yükseltilmiş ayırıcılar**
geçiyor, ve yarış çizgisi keskin bir V yapıp onları kesiyor. Araba ayırıcıya tırmanıp kalıyor.

Duvar filtresi bunu göremiyor ve **göremez**: bordürün üstünde sürülebilir yüzey var, yani "yol
burada mı" sorusuna evet diyor. Basamağa bakan bir ek kural yazdım ve o da çürüdü — 4002'yi
düzeltmiyor (hâlâ 42 m) ve çalışanları bozuyor: 4001 991 → 268 m, 4081 746 → 333 m, sekiz parkur
toplamı 8,1 → 6,8 km, denenen her eşikte. Bordürü yakalayacak kadar keskin her kural, yol olan
tümsek ve çukurları da kesiyor.

**Bu oturumda ağ üzerine denenen dört geometrik filtrenin dördü de çürüdü** (eğim, koni, hıza bağlı
kilit, basamak). Deseni yazıyorum çünkü beşincisini denemek cazip: bu filtreler engelleri
kesmekten daha hızlı gerçek yolları kesiyor. Kalan açıklama, çizginin gerçekten oradan geçtiği —
NFSU2 yarışları karşı şeridi belirli boşluklardan kullanır, ve bizim ağ + yol noktası
yaklaşımımız o boşluğu bulmak yerine köşeyi kesiyor. Duruş noktası ya `seen_path` koruması ya da
"ilerlemeyi sürdüren bağlantı yok" — ikisi ayırt edilebilir ve ayırt edilmeli.

### M4: yarış artık yarış

`nfsu2::race` — geri sayım, tur sayımı, sıralama, bitiş. Hiç araba tutmuyor, fizik adımlamıyor,
kumanda üretmiyor: pilotların zaten izlediği şeyler üzerine aritmetik. Sonucuna güvenilmesi
gereken bir yarışın arabayı itebilmesi olmaz.

İki şekil dosyanın kendisinden: `0x3414C`'nin `circuit` bayrağı (4.067 kayıtta anlaşmazlık yok)
kapalı devreyi açık sprintten ayırıyor. **Tur sayısı hiçbir yerde çözülü değil** — `Race::LAPS = 3`
bir seçim, okuma değil, ve sabit olarak duruyor ki bir sonraki kişi varsayımla karşılaşsın, onu
miras almasın.

Bağlarken bir ölçüt hatası çıktı ve ciddiydi. İlerleme "yol noktasına 60 m'den yakınsan geçtin
say" diye ölçülüyordu. Çizgiden sapan bir araba o yarıçapa hiç girmiyor, yani **sayaç duruyor ama
araba sürmeye devam ediyor**: bir alan 1.843 m kat ederken sayaç 5 yol noktası (200 m) okuyordu.
Onun üstüne kurulan sıralama uydurmadır.

Ölçüt ağda işe yarayan kurala geçti — bir sonraki yol noktası daha yakınsa ilerle. Sekiz parkurda
etkisi: hareket eden araba **29/64 → 32/64**, toplam mesafe **8,1 → 6,5 km**. Sürüşe net bir
kazanç değil; ama yarışın anlamlı olması için ilerlemenin doğru ölçülmesi pazarlık konusu değil.

`nfs_sim` artık bitiş sırasını yazıyor. Henüz kimse bitirmiyor: bir devre 5 km, üç turu 15 km, ve
alan en iyi ihtimalle 2 km'ye ulaşıyor.
### Tahmin etme, gözlemle: sürücülerin en büyük kazancı

Çürüyen dört geometrik filtrenin ortak kusuru şuydu — hepsi bir bağın sürülebilir olup olmadığını
**önceden** söylemeye çalışıyordu, ve yarım metrelik bir bordürü yakalayacak kadar keskin her test
yol olan her tümseği de kesiyordu.

Pilot artık tahmin etmiyor, hatırlıyor. Takıldığında (1,5 sn hareketsizlik) gitmeye çalıştığı
düğümü **kara listeye alıyor**, geri viteste kaçıyor, ve geldiği yerden başka bir dal seçiyor.
Gerçekten bir yere gidemeyen bir arabanın elinde hiçbir geometrik testin sahip olmadığı kanıt var.

Sekiz parkurda ölçüm, sürücülerin şimdiye kadarki en büyük tek kazancı:

| | önce | sonra |
|---|---:|---:|
| hareket eden araba | 32/64 | **41/64** |
| toplam mesafe | 6,5 km | **7,9 km** |

Ve en çok, geometrik kuralların beceremediği yerde: hiç kimsenin kıpırdamadığı 4002 **2/8 40 m →
5/8 170 m**, 4081 **0/8 29 m → 2/8 264 m**, 4121 5/8 → 7/8, 4021 1481 → 1917 m. Yalnız 4061 hafif
geriledi.

Kara liste pilotun kendi deneyimi, grafın özelliği değil — o yüzden `step_avoiding`'e parametre
olarak geçiyor: aynı yolda iki araba farklı şeylere çarpmış olabilir.
### Ölçütüm yanlıştı: düşen arabayı ilerleme sayıyordum

Geçen turda "toplam mesafe 7,9 km" dedim. Yanlış. Arabaların hızına bakınca çıktı: bir parkurda
sekizin altısı **−219 ile −310 km/h**'de, ve biri haritanın 2,4 km dışında. Bu geri vites değil,
**düşüş**. `en uzak` düz mesafeyi ölçüyordu, yani tespit etmesi gereken hatayı ödüllendiriyordu.

Düzeltilmiş tablo, sekiz parkur, 90 sn:

| | |
|---|---|
| giden araba | 41/64 |
| **haritadan düşen** | **20/64** |
| mesafe (yalnız ayakta olanlar) | **2404 m** — 7924 değil |

Yani sürücüler bildirdiğimden çok daha kötüymüş. `nfs_sim` artık düşenleri ayrı sayıyor ve
mesafeye katmıyor; ölçüt "ızgara yüksekliğinin 50 m altına inen araba düşmüştür".

### Fren

Pilotun hız politikası yalnızca gaz kesmekti ve yetmiyordu: 90 km/h'yi viraja taşıyıp dışarı
çıkıyordu. Fren eklendi — dönme sertliği × hız, arabanın tutabileceğine karşı. Eşik yine süpürmeyle,
ve bu sefer **üç ölçütte birden** kazanan bir değer çıktı:

| eşik | giden | düşen | mesafe |
|---|---:|---:|---:|
| frensiz | 41/64 | 20/64 | 2404 m |
| 20 m/s | 41 | 18 | 2418 m |
| 14 m/s | 41 | 17 | 2354 m |
| **9 m/s** | **44** | **13** | **2594 m** |
| 6 m/s | 41 | 14 | 2242 m |

Ayrıca pilot artık kaç **ayrı** düğüm gördüğünü sayıyor; kavşak sayısına karşı bu, parkuru dolaşan
bir arabayla daireler çizen arabayı ayırıyor.

### Düşenler: onu da yoldan çıkıyor, hiçbiri zeminden geçmiyor

Ölçüt yine yanlıştı, bu sefer ters yönde. "Izgaranın 50 m altına inen araba düşmüştür" der demez
`Paths4061` onu çürütüyor: parkur **y = 323**'te başlıyor ve rotası altmış metre iniyor, yani aşağı
inip duran üç araba düşmüş sayılıyordu. Üçü de son adımda dört tekerlek yerde, dik ve 0 km/h. Geçen
tur ölçüt hatayı ödüllendiriyordu; bu ölçüt hata **uyduruyordu**. Sayı **13 değil 10**.

Yeni ölçüt oyunun kendi kuralı: arabanın **en son durduğu** yerin 60 m altı (`rig::FALL_DEPTH`).
Yerel, hiçbir zemine ya da ızgaraya ihtiyacı yok, her parkurda aynı şeyi söylüyor. Ve artık tek yerde
duruyor — `keep_in_world` ile `nfs_sim` aynı `CarRig::watch_ground`'u çağırıyor, biri kurtarıyor
öteki yalnız izliyor, ikisi ayrışamaz.

Sonra sebep. Her düşen araba, tekerleklerin dünyayı en son tuttuğu ana geri sarılıyor ve şehre
"bu arabanın **gittiği yönde** ne var" diye soruluyor — `Ground::gap_along`, `drop_walled`'ın kendi
içinde kapalı duran yürüyüşü, artık ortak ve testli.

| | |
|---|---:|
| düşen | **10/64** |
| yol bitiyor | **10** |
| zeminden geçen | **0** |
| hiç yere basmamış | 0 |

**Hiçbiri tünelleme değil.** Onunun da havada kazandığı yükseklik **0,0 m** — kimse rampadan
fırlamamış — ve hepsi 39-55 km/h'de düz düz yoldan çıkmış. ROADMAP §6'daki `NarrowPhase::
shape_trimesh` iç-kenar maddesi bunun sebebi değil; MOTOR-NOTLARI'na yazılacak bir şey yok.

Ve boşluk **bizim montajımızın açtığı delik değil.** Paketin backdrop dışındaki her üçgeni çarpışmaya
koyarak ölçüldü (`NFS_COLLIDE=all`: 582.304 → 734.880 üçgen, 2.100 kaba kademe geri geldi): sekiz
parkurda sonuç **birebir aynı**, aynı on araba aynı üç yerden düşüyor. `*_WORLD_LOD` proxy'leri
hipotezi ise test *edilemedi* — bu bölge hiç taşımıyor, sıfır tane. O yüzden `nfs_sim` artık
filtrelerin ne attığını yazıyor: etkisi görünmeyen bir kol, hiç değiştirmediği bir küme üzerinde
gün boyu süpürülebilir.

`NFS_FALLMAP=<yarıçap>` boşluğun resmini çiziyor, ve resim dar bir dikiş değil onlarca metrelik bir
hiçlik gösteriyor — araba `#` alanının kenarında duruyor, gideceği yön düpedüz `.`:

```
..############...########################
.#############...########################
############......#######################
..................#######################
...................######################
................>..######################      # = basılabilir zemin
...................######################      . = hiçbir şey
....................#####################      O = arabanın bıraktığı yer
....................O####################      > = gittiği yönde 10 m
.....................####################
...............##########################
..........###############################
......###################################
```

Düşüşler üç kümede: 4041'de (1900, 650) altı araba, 4021'de (−247, 1480) üç, 4102'de (80, −458) bir.
Kalan beş parkurda kimse düşmüyor.

Yani şehrin yolları **kenarlarında zemin olmadan** geliyor, ve arabayı üstünde tutacak bariyer
chunk'ı dosyaların hiçbirinde yok (`0x0003410B`, §M4). Sürücü hatası değil, fizik hatası değil,
bizim filtremiz değil — eksik veri. Kalan iş ikisinden biri: `keep_in_world`'ün oyunda zaten yaptığı
şey, ya da §M4'ün ağdan türetilmiş bariyerleri.

### Nişan noktası arkada kalıyordu — pilotun sessiz kilidi

Düşenlerden birini izlerken çıktı, ve düşüşten büyük. `Paths4041`, car 3: t=9,5'te düğüm 163'ün
**1,2 m** yanına varıyor, o andan sonra düğüm bir daha hiç ilerlemiyor, araba 39 km/h'de düzgün bir
yay çizip t=14,3'te dünyadan çıkıyor. İki kuralın birbirine kilitlenmesi:

- **Düğüm yapışıyor.** İlerletme kuralı "bir sonraki düğüm arabaya, tutulandan daha yakınsa ilerle".
  Araba düğümün 1,2 m yanındayken hiçbir komşu bundan yakın olamaz — yani düğümün *üstünde* duran
  araba oradan hiç çıkamıyor.
- **Nişan noktası çöküyor.** İleri bakış yürüyüşü `walked`'ı arabanın tuttuğu düğüme olan
  mesafesiyle başlatıyordu. Araba düğümden bir lookahead'den fazla uzaklaşınca döngü **ilk testte**
  kırılıyor ve nişan noktası düğümün kendisinde kalıyor. O da artık arkada.

Arkadaki bir noktaya pure pursuit tam kilit ister; tutulan tam kilit de dairedir. Ve hiçbir şey
kurtarmıyor: takılma manevrası 0,7 m/s'nin altını bekliyor, araba on bir yapıyor.

Düzeltme tek cümle: **arkadaki nokta nişan noktası değildir.** Yürüyüş, nokta öne geçene kadar da
sürüyor.

| | önce | sonra |
|---|---:|---:|
| giden araba | 52/64 | **62/64** |
| toplam mesafe | 2.594 m | **3.652 m** |
| kavşak | 718 | **848** |
| haritadan düşen | 10 | 10 |

Temiz bir kazanç değil, takas: 4081 **2/8 ve 21 kavşak → 8/8 ve 159**, 4061 4/8 ve 237 m → 8/8 ve
735 m, 4121 142 → 724 m; karşılığında 4002 172 → 51 m, 4102 452 → 333, 4021 388 → 348. Üç ölçütte
birden kazandığı için duruyor — ve daire çizerek kazanmıyor: kavşağın ayrı düğüme oranı 1,29 → 1,30
sabit, mesafe kaybeden 4102'de ise 1,28 → **1,00**, yani orada aldığı her kavşak yeni bir yer.

Düşen sayısı kıpırdamadı, ki beklenen: bu pilotun nereye baktığını düzeltiyor, şehrin yol kenarına
zemin koymuyor.

### Bariyer onun sekizini yakalardı — ikisini yakalamazdı

Bariyeri yapmadan önce sorulacak soru: düşen arabalar koridoru **önceden** terk ediyor mu?
`Corridor` zaten duruyor — `nfs_cruise` "parkurun dışı" ile "dünyanın dışı"nı onunla ayırıyor,
yarı genişliği 12 m ve ölçülmüş (kaldırımdan yana yürüyerek: Bayview'ın şeritleri üç rota dosyasında
medyan 9-11 m açılıyor). `nfs_sim` artık her düşen için, dünyanın onu bıraktığı anda parkurun kaç
metre dışında ve **kaç saniyedir** dışında olduğunu yazıyor.

| araba | parkurdan | ne kadardır |
|---|---:|---:|
| 4041 car 6 | **8 m** | **0,0 s** |
| 4041 car 3 | **10 m** | **0,0 s** |
| 4021 ×3 | 15-16 m | 0,3 s |
| 4041 car 5 | 27 m | 2,1 s |
| 4041 car 1 | 41 m | 3,2 s |
| 4121 car 6 | 47 m | 4,8 s |
| 4102 car 5 | 52 m | 3,3 s |
| 4041 car 2 | 95 m | 7,4 s |

**İkisi hâlâ parkurun içindeyken düşüyor.** 4041'de yarış çizgisinin kendisi boşluğun kenarından
geçiyor; 12 m'lik bir çit o iki arabayı kurtarmazdı. Kalan sekiz için yeterdi — üçünü kıl payı
(0,3 s dışarıda), beşini rahat rahat (2-7 saniyedir dışarıdalar).

Yani bariyer yapılmaya değer **ve tek başına yetmiyor.** Kenarı olmayan yol kenarı ayrı bir sorun ve
kendi çözümünü istiyor.

Bu arada `COURSE_HALF_WIDTH` artık `nfs_cruise`'un özel sabiti değil, kütüphanede: şehir hakkında
ölçülmüş bir sayı, ve artık iki çağıranın aynı fikirde olması gerekiyor.

### Çit — şehrin kendi kenarından türetildi

Bariyer chunk'ı hiçbir dosyada yok, yani çit bir yerden türetilecek ve iki aday vardı. Ölçüm rotayı
eledi: koridor onun sekizini yakalardı, **ikisi hâlâ parkurun içindeyken** düşüyordu. Rota şehrin
nerede bittiğini bilmiyor. Zemin biliyor.

`Ground::edge_at` on iki yöne bir halka sondaj atıyor; **arabanın kendi yüksekliğinde** basılabilir
zemin bulamadığı yönlerin toplamı, dışarıyı gösteren normal. Yükseklikte sorulması şart, çünkü
Bayview katmanlı: XZ'ye çıkarılmış düz bir "burada zemin var mı" haritası köprünün kenarını altından
geçen yoldan ayıramaz — yolu çitler ve köprüden düşmeye izin verir. Testin son iki iddiası tam bu.

`CarRig::hold_at_edge` bunu duvara çeviriyor: hızın şehirden **dışarı** bakan bileşenini siliyor,
gerisine dokunmuyor. Kenara değen araba boyunca kayıyor ve yoluna devam ediyor — bir bariyerin
davranışı, çarpıp durmak değil. Direksiyona hiç karışmıyor; o yüzden oyuncuya da rakibe de aynı
şekilde uygulanabiliyor, ve `nfs_cruise`'da ikisine de uygulanıyor. Yeri de kararın parçası:
kuvvetlerle integrasyonun **arasında** (`Driver::step_physics_with`), yani arabayı dudağın üstünden
taşıyacak olan adım, taşımayan adım oluyor.

Tek ayarı sondajın şasi merkezinden ne kadar önden atıldığı, ve süpürüldü:

| pay | düşen | mesafe | kavşak |
|---|---:|---:|---:|
| çit yok | 10 | 3.652 m | 848 |
| 0 m | 5 | 3.934 | 865 |
| 1,0 | 3 | 3.946 | 851 |
| 2,2 | 2 | 3.746 | 813 |
| 2,6 | 2 | 4.019 | 833 |
| **3,0** | **1** | **3.881** | **846** |
| 3,4 | 1 | 3.623 | 822 |
| 3,8 | 2 | 3.252 | 783 |
| 4,5 | 0 | 3.482 | 771 |

Düşüş payla tekdüze azalıyor, mesafe 3,4'e kadar düz duruyor, ondan sonra çit meşru yolu reddetmeye
başlıyor: 4,5'te kimse düşmüyor ama alan çitsiz hâlden **170 m az** yol alıyor. Üç metre bu platonun
ortası — ondan biri yerine bir araba kaybediliyor, çitsizden fazla yol alınıyor, kavşak sayısı
kıpırdamıyor (846'ya karşı 848).

**Ve ilkeli değer denendi, kaybetti.** Sondajı ön akstan atmak — desteği gerçekten ilk kaybeden temas
noktası, arabanın kendi tekerlek bağlantılarından okunuyor — gerekçesi olan sayı, ve her sütunda
daha kötü: 3 düşen, 3.686 m, 819 kavşak. Bir dahakine "iyileştirme" diye yeniden türetilmesin diye
yazıldı.

Oturumun toplamı:

| | oturum başı | şimdi |
|---|---:|---:|
| giden araba | 52/64 | **62/64** |
| haritadan düşen | 10 | **1** |
| toplam mesafe | 2.594 m | **3.881 m** |
| kavşak | 718 | **846** |

### Alan parkuru sürmüyor — ve iki makul çözüm daha çürüdü

Çit düşüşleri bitirince kalan soruyu ölçtüm: alan neden ilerlemeyi bırakıyor, süre mi yetmiyor?
Süre değil. 64 arabanın **39'u t=30'dan önce** parkurda ilerlemeyi bırakıyor ve yalnız 4'ü son
saniyeye kadar ilerliyor. Dahası 54'ü tuttuğu düğümden 60 m'den fazla sapıyor, en fazlası **954 m**.
Arabalar sürüyor (62/64, 4,1 km) ama parkuru dolaşmıyorlar.

**Birinci deneme: "yol noktası arkada kaldıysa ilerlet."** Nişan noktasında işe yarayan cümlenin bir
üst katmanı. Kaçtı: bir araba 90 saniyede **92 tur ve 5999 yol noktası** sürdü ve iki araba
"BİTİRDİ". Sebep artık net ve düğümdeki kaçakla aynı: **"arkada" döngüsel bir dizide sonlanamaz.**
Parkur kapalı bir halka, yani ondan yüzünü çevirmiş arabanın arkasında halkanın koca bir yayı var;
sayaç o yayı yürüyor, başa sarıyor, bir tur sayıyor, ve baştaki noktalar da arkada. İki kez
denendi, iki kez aynı sebeple öldü.

**İkinci deneme: kaybolduysa parkuru yeniden bul.** İlerletmek olmuyorsa geriye kalan: bir süredir
yol noktası kazanamayan pilot ızgaradaki soruyu tekrar sorsun — en yakın yol noktası, en yakın
düğüm. Tanım gereği sınırlı, kaçamaz. Yine de kaybediyor, hem de her aralıkta.

Ve **şişiremediği tek ölçütte** kaybediyor. `along` yol noktasının *indeksi*, yani indeksini ileri
zıplatan pilota aradaki her şey yazılıyor: senkron 5,5 → **122,8** gibi yirmi iki katlık bir kazanç
gösteriyordu. `Pilot::covered` — arabanın gerçekten 40 m yakınından geçtiği ayrı yol noktası — bunu
söktü:

| | giden | düşen | mesafe | kavşak | **geçilen yol noktası** |
|---|---:|---:|---:|---:|---:|
| **senkron kapalı** | **62/64** | **0** | **4.129 m** | 864 | **462** |
| 4 sn | 53/64 | 2 | 2.955 | 913 | 369 |
| 8 sn | 59/64 | 0 | 3.878 | 906 | 412 |
| 16 sn | 62/64 | 1 | 3.640 | 934 | 435 |

Kazandığı tek sütun kavşak, ki düğümü yeniden seçmekle şişen sütun tam olarak o.

Bu arada bayat bir okuma düzeldi ve bedava kazanç getirdi: `toward` bu karenin yol noktası
ilerlemesinden **önceki** hedeften hesaplanıyordu. Ilerlemeden sonrasına alınınca sekiz parkurda
düşen 1 → **0**, mesafe 3.881 → **4.129 m**, kavşak 846 → **864**.

Kalan gerçek: alan parkurun ortalama **7 yol noktasını** geçiyor (rotaya göre 65-126 tanesinden).
Sürüyorlar, düşmüyorlar, ama parkuru sürmüyorlar — ve bunun sebebi ne ilerletme kuralı ne de
senkron. Sıradaki iş burada.

### Ağdaki en kısa yol da çürüdü — ve çürüme şekli asıl bulgu

Alanın parkuru sürmemesinin en makul açıklaması pusulaydı: pilot her kavşakta **düz çizgi**
mesafesine göre dal seçiyor, ve şehir bu pusulanın en kötü olduğu yer. Hedefe *bakan* her şey
seçilir — çıkmaz sokak, karşı şerit, denizde biten cadde — ve graf bunu kuş uçuşundan ayıramaz.

Yerine dürüst soru kondu: yol noktasının kendi düğümünden dışa doğru Dijkstra, her dal "kaç metre
yol kaldı" ile puanlanıyor (`Network::guide_to`, kurs başına bir alan). Sekiz parkurda **her ölçütte
daha kötü**:

| | düz çizgi | ağda en kısa yol |
|---|---:|---:|
| geçilen yol noktası | **462** | 391 |
| toplam mesafe | **4.129 m** | 2.779 m |
| t<30'da duran | **39/64** | 44/64 |

**Ve bozuk bir uygulama değil** — asıl bulgu bu. Dört parkurda ölçüldü: alanlar grafın **tamamını**
çözüyor (341/341, 340/341, 154/154, 227/227) ve her yol noktası, alanının çözüldüğü düğüme medyan
17-19 m uzakta, en kötüsü 107 m. Pusula doğruydu ve araba onunla daha kötü sürdü.

Kalan okuma: **bu graf tam olarak izlenebilecek bir yol haritası değil.** Yan yana giden yolları
birleştiriyor — `drop_walled` zaten aralarında duvar olan iki bağ yüzünden var — yani en kısa yol,
arabanın fiziksel olarak alamayacağı birleşmelerden geçiyor ve o tek rotaya kilitleniyor. Düz çizgi
pusulası ise geziniyor, ve gezinme yalnızca kabaca doğru olan bir grafı tolere eden şey. Sıra:
**rotayı optimize etmeden önce grafı doğru yap.**

### Kara listeleri grafa geri yazmak: hasat gerçek, sinyal değil

Geçen turun çıkardığı iş şuydu — bir bağın sürülebilirliğini *tahmin* etmek dört kez çürüdü, ama
pilotların kara listeleri **sürülemediği kanıtlanmış** yerlerin kaydı. Bir arabanın vazgeçmesi kaza
olabilir; birkaç arabanın aynı düğümde vazgeçmesi graf gerçeği olmalı.

Hasat gerçekten var: sekiz parkurda **369 düğüm**, 64 arabanın **63'ü** en az bir tane bırakıyor. Ve
paylaşmak her eşikte kaybediyor:

| kaç araba anlaşınca | giden | mesafe | kavşak | **geçilen yol noktası** | yasaklanan |
|---|---:|---:|---:|---:|---:|
| **kapalı** | **62/64** | **4.129 m** | 864 | **462** | 0 |
| 2 | 60/64 | 3.217 | 1671 | 412 | 70 |
| 3 | 62/64 | 3.848 | 991 | 448 | 54 |
| 4 | 62/64 | 3.974 | 841 | 458 | 39 |

Eğri tekdüze kapalıya doğru gidiyor — yani paylaşım arttıkça kötüleşiyor, ki bir mekanizmanın
"faydası negatif" demenin en temiz hâli.

**Sebep ölçüldü ve trafik.** İki arabanın anlaştığı **ilk** düğümler ızgaraya medyan **7,6-47,7 m**
uzakta, sekiz parkurun hepsinde. Alan dört yan yana, önde arkada 1,3 m boşlukla başlıyor ve pilotun
hiç trafik modeli yok: oradaki "anlaşma" yolun kapalı olduğunu değil, arabaların **birbirini**
tıkadığını ölçüyor.

Açıklama doğrulandı: kanıtın ızgaradan en az 60 m ötede toplanması şartı konunca 412 → **461**'e
dönüyor, yani kaybın tamamı bulaşmaymış. Ama temizlenmiş hâliyle bile kazanç yok — 461'e karşı 462,
ve mesafe 3.947'ye karşı 4.129. Yani gözlem kanalı kaynağında kirli, ve temizlendiğinde bu örneklem
büyüklüğünde grafa dair söyleyecek bir şeyi kalmıyor.

Kalan: kara liste pilotun **kendi anlık** deneyimi olarak değerli (o arabaya başka yol denetiyor),
graf gerçeği olarak değil. Vazgeçilen düğüm sayısı artık araba satırlarında yazıyor.

### Trafik modeli — üç kez kendini gösteren eksik

Aynı şey üç ayrı yerden çıktı: alanın bir kısmı ızgarayı hiç terk edemiyor, kara liste hasadının
üzerinde anlaşılan düğümleri ızgaranın dibinde toplanıyor, ve pilotun kendi modül dokümanı
"önündekinin arkasına seve seve girer" diye yazıyor. Üçüncüsünden sonra bu bir üslup notu değil,
ölçülmüş bir eksik.

En kaba kural kondu: kendi genişliğinde bir koridorda, **önde**, hıza orantılı bir takip mesafesi
içinde araba varsa gaz kesiliyor ve yaklaştıkça fren geliyor. Etrafından dolanmıyor — yavaş bir
arabanın arkasında kuyruk olursa kuyruk kalıyor, ki dürüst olan da bu.

Karar ölçütü **geçilen yol noktası**: trafik modelsizliğinin bedelini ödeyen sütun o, ve
şişirilemiyor.

| çarpan | giden | düşen | mesafe | **geçilen yol noktası** | t<30 duran |
|---|---:|---:|---:|---:|---:|
| yok | 62/64 | 0 | **4.129 m** | 462 | 39 |
| 0,6 | 63/64 | 2 | 3.965 | 584 | 33 |
| 1,2 | 63/64 | 3 | 3.401 | 526 | 36 |
| **2,0** | **63/64** | **0** | 3.611 | **589** | **31** |
| 3,0 | 63/64 | 3 | 3.612 | 587 | 34 |
| 4,5 | 63/64 | 3 | 3.787 | 572 | 28 |

Kapsama, herhangi bir takip mesafesi olur olmaz sıçrıyor ve sonra platoya oturuyor; 2,0 platonun
en iyisi. **Mesafe sütunu ters yönde ve bu sürpriz değil**: `furthest` her parkurun en öne geçen
arabası, ve öndeki arabanın birinin arkasına takılması takip mesafesinin ta kendisi. Alanın parkuru
*sürdüğünü* söyleyen sütun %27 arttı.

Düşen sütunu bu ölçekte sinyal değil gürültü (0, 2, 3, 0, 3, 3); 2,0'ın sıfıra denk gelmesi şans.

Yan etkiler açıklamayla tutarlı: ızgarayı hiç terk edemeyen araba **2 → 1**, pilotların vazgeçtiği
düğüm **369 → 308**, t=30'dan önce duran **39 → 31**.

### Ve etrafından dolanmak — kuyruğun bedelini geri alıyor

Takip mesafesi dürüsttü ama kuyruk bırakıyordu: öndeki takılırsa arkasındaki yedi araba da duruyor.
Tıkayan arabanın hangi yanda olduğu zaten ölçülüyordu, geriye onu kullanmak kaldı — nişan noktasını
ters yana, yaklaştıkça daha çok kaydır. Başka hiçbir şey değişmiyor, yani yol açılınca araba çizgiye
kendi kendine dönüyor.

| kaydırma | giden | düşen | mesafe | geçilen yol noktası | t<30 duran |
|---|---:|---:|---:|---:|---:|
| yok | 63/64 | 0 | 3.611 m | 589 | 31 |
| **1,5 m** | **63/64** | **0** | **4.187** | **605** | 32 |
| 2,0 | 63/64 | 3 | 4.260 | 605 | 28 |
| 2,5 | 63/64 | 4 | 3.559 | 577 | 27 |
| 3,0 | 63/64 | 2 | 3.866 | 617 | 27 |
| 5,0 | 62/64 | 3 | 4.029 | 609 | 30 |

Kapsama 1,5'ten yukarısı için 605-617 platosunda, yani geniş kaydırmalar en fazla bir düzine yol
noktası alıyor — karşılığında iki ilâ dört arabayı haritadan atarak. **1,5 m tek ayarsız seçenek**:
kimseyi düşürmeden bir şey değiştiren tek değer, ve arkasında ölçüm dışında bir okuma da var — bir
araba genişliğinin yarısından biraz fazlası, aynı şeritteki arabayı temizleyen en küçük kayma.

Ve takip mesafesinin bedelini geri veriyor: mesafe 3.611 → **4.187 m**, hiç trafik modeli yokken
alınan 4.129'un bile üstünde. Kuyruk bedeldi; dolanmak onu ödemeyi bırakmak.

**Yapışan düğüm için "geçtiyse bırak" ise çürüdü — bin kat.** Nişan düzeltmesinin simetriği gibi
duruyor (düğüm arabanın arkasında kaldıysa ilerlet) ve sekiz parkurda kavşağı 848 → **854.536**
yapıyor, mesafeyi 3.652 → 2.699 m'ye düşürüyor. Bu, kodda zaten yazılı olan "yeterince yakınsa
ilerlet" kaçağının öbür kapıdan gelmiş hâli: arkadaki bir düğümün yerine geçen düğüm de genellikle
arkada oluyor, yani döngü yarışın sonuna kadar her karede tavanına vuruyor. Yapışan düğümü ne
çözecekse bu değil, ve düğüm hâlâ yapışıyor.

Bir de ölçütün kendisi kodda değil awk'taymış. "Giden araba" `ROADMAP`'te sürücüler var olduğundan
beri alıntılanıyor ama hiçbir zaman kodda olmamış — her seferinde araba satırlarından elle
sayılmış, ve iki ayrı girdinin rakamı da bugünkü çıktıdan üretilemiyor. Tanım artık `nfs_sim`'de:
en az bir kavşak almış araba, `SUMMARY away=` diye yazılıyor.

### Bariyer filtresi çürüdü — iki kez, ikincisi doğru aletle

`drop_walled` "bu bağ boyunca yol devam ediyor mu" diye soruyor, bunu iyi yapıyor, ve bilerek bir
sınıfı kaçırıyor: refüj, iki gidiş yönü arasındaki bariyer, rampanın yanındaki istinat duvarı.
Hepsinin iki yanında da, aradaki çizgi boyunca da sapasağlam yol var — yol testi geçiyor ve ağ
sürücüye önünde bariyer olan bir dal veriyor. `drop_walled`'ı doğuran iki bağ da zaten tam bu
şekildeydi; yalnızca oralarda yol *da* bittiği için yakalanmışlardı.

Öyleyse soru doğrudan sorulacak. Her `Surface::Wall` üçgeni 64 m'lik hücrelere indekslendi
(`Walls`), ve bir bağın iki ucu arasındaki doğru parçası **araba yüksekliğinde** taşınıp üçgenlerle
kesiştirildi (Möller-Trumbore, parçaya sınırlı). Yükseklik işin bütünü: `Surface::Wall` bir normal
testi, yani 15 cm'lik kaldırımı bina cephesiyle aynı şekilde yakalıyor, ve bu projede dört filtre
tam da kaldırıma duyarlı oldukları için çürümüştü.

**Birinci uygulama yanlış aletti, ve bunu süpürmeden önce karakterizasyon söyledi.** Kesilen bağ
sayısı yükseklikle düşmüyor (4001, 720 bağ):

| lift | 0,15 | 0,3 | 0,5 | 0,75 | 1,0 | 1,5 | 2,0 | 3,0 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| **düz kiriş** | 126 | 132 | 140 | 136 | 137 | 108 | 116 | 123 |
| **profil yürüyüşü** | 168 | 168 | 168 | 168 | 151 | 127 | 121 | 77 |

3 m'de 15 cm'dekinden fazla şey kesen bir test "şu yükseklikte duran duvar" ölçüyor olamaz. Sebep
`drop_walled`'ın kendi yorumunda yazılı: iki düğüm arasındaki düz çizgi tümseği takip etmez. Yol
iki ucun arasında yükseliyorsa kiriş **yerin altında** kalıyor ve gömüldüğü şevin üçgenlerine
"duvar" diyor. İkincisi bağı yolun kendi profilini izleyerek yürüyor: her 3 m'de yerel zemin —
yürüyüşün bulunduğu yüksekliğe *en yakın* yüzey, en yükseği değil, çünkü düz bir çatı da
`Surface::Drivable` — ve test edilen şey iki komşu örneğin kendi zemininin `lift` üstündeki kısa
parçası. Eğrisi tekdüze, yani iddia ettiği şeyi ölçüyor.

**Ve doğru alet de kaybetti, süpürülen her yükseklikte** (sekiz rota, sekizer araba, 90 sn):

| lift | giden | düşen | mesafe | **geçilen yol noktası** | kavşak | ayrı düğüm | kesilen bağ |
|---|---:|---:|---:|---:|---:|---:|---:|
| **0 (kapalı)** | 63/64 | 0 | 4.187 m | **605** | 1245 | **926** | 0 |
| 0,5 | 63/64 | 1 | 4.385 | 564 | 1401 | 876 | 399 |
| 1,0 | 63/64 | 0 | 3.804 | 577 | 1181 | 858 | 332 |
| 2,0 | 63/64 | 0 | 4.105 | 597 | 1203 | 891 | 259 |
| 3,0 | 63/64 | 0 | 4.062 | 595 | 1189 | 884 | 191 |

Karar sütunu bir kez bile kapalıyı geçmiyor, ayrı düğüm de öyle. Yükseklik büyüdükçe sayıların
düzelmesi bir eğilim değil, filtrenin kendini kapatması. Düz kiriş sürümü de aynı yerde bitiyor:
562 / 556 / 561 (0,5 / 1 / 2 m). 0,5'teki kavşak sıçraması (1245 → 1401) ayrı düğüm *düşerken*
geliyor — ilerleme değil, aynı yerde salınım.

Asıl bulgu etkinin nerede olduğu: **sekiz rotanın altısında bütün sayılar birebir aynı.** Kalan iki
rota da birbirini götürüyor:

| rota | mesafe (0 → 0,5) | geçilen yol noktası |
|---|---|---|
| 4002 | 176 → **739 m** | 27 → 26 |
| 4001 | 965 → **567 m** | 141 → **103** |

4002 tam da "dosyanın çizgisi ayırıcıların üstünden geçiyor" diye teşhis edilmiş rota, yani
mekanizma teşhis konmuş yerde gerçekten iş yapıyor — ama yaptığı iş **öndeki tek arabanın metresi**,
alanın kapsaması kıpırdamıyor. 4001'deki kayıp ise kapsamada ve gerçek. Sebebi de ölçüldü: 4001'in
çıkışsız düğümleri **1 → 6**. Sürülemeyen bir bağı silmek onun **etrafından dolanan yolu** da
siliyor, ve bu graf onu kaldıramıyor — bariyere dayanıp sürtünerek ilerleyen bir pilot, dalı hiç
görmeyen pilottan iyi çıkıyor.

Kod silindi. Kontrol iki kez koştu: `lift=0` hem mekanizma dururken hem silindikten sonra son
commit'in tablosunu birebir veriyor (605 yol noktası, 4.187 m, 926 ayrı düğüm), yani ölçülen fark
filtrenin kendisiydi.

**Açık kalan:** 4002'nin ayırıcıları hâlâ orada. Bir sonraki denemenin bunu **bağı silmeden**
yapması gerekiyor — dalı grafta bırakıp pilota "bunun üstünde bir şey var" demek gibi, ki dolanma
yolu kaybolmasın.

### Fiyatlamak da çürüdü — ama aleti akladı, ve dosya hakkında bir şey söyledi

Yukarıdaki girdinin bıraktığı iş yapıldı: bağ grafta kalıyor, yalnız pahalanıyor. `mark_barriered`
işaretliyor ve silmiyor; `step_avoiding` artık mesafenin karesiyle değil **mesafenin kendisiyle**
sıralıyor — aynı sıra, ama üstüne metre eklenebilen tek biçim — ve bariyerli dala
`NFS_BARRIER_COST` metre bindiriyor. Yapısal olarak çıkışsız düğüm üretemiyor: bütün dallar
bariyerliyse hepsi aynı bedeli aldığı için aralarındaki sıra değişmiyor, yani her zaman bir yol
kalıyor. Geçen turu batıran şey tam da buydu.

**Bedel süpürmesi** (yükseklik 0,5 m):

| bedel | giden | düşen | mesafe | **yol noktası** | kavşak | ayrı düğüm |
|---|---:|---:|---:|---:|---:|---:|
| **0 (kapalı)** | 63/64 | 0 | 4.187 m | **605** | 1245 | 926 |
| 10 | 63/64 | 0 | 4.191 | 586 | 1225 | 902 |
| 30 | 63/64 | 1 | 4.246 | 593 | 1235 | 910 |
| **100** | 63/64 | 1 | **4.717** | 593 | **1291** | **973** |
| 300 | 63/64 | 1 | 4.717 | 593 | 1291 | 973 |

100 ile 300 birebir aynı, yani bedel 100'de **doyuyor**: alternatifi olan her seçimi çeviriyor,
üstüne koymak bir şey değiştirmiyor. Ve tablo ilk kez ikiye bölünüyor — mesafe, kavşak ve ayrı
düğüm kapalıyı geçiyor (4.717'ye karşı 4.187, 973'e karşı 926), yol noktası geçmiyor.

**Yükseklik süpürmesi** (bedel 100'de sabit):

| lift | mesafe | **yol noktası** | ayrı düğüm |
|---|---:|---:|---:|
| kapalı | 4.187 m | **605** | 926 |
| 0,5 | **4.717** | 593 | **973** |
| **1,0** | 4.115 | **609** | 893 |
| 1,5 / 2,0 | 4.105 | 597 | 891 |
| 3,0 | 4.062 | 595 | 884 |

En iyi hücre 609'a karşı 605. Bu bir kazanç değil: kara liste turunda 461'e karşı 462 için "kazanç
yok" denmişti ve büyüklük aynı — üstelik o hücrede mesafe ve ayrı düğüm düşüyor. Yani mekanizma
silmekten her sütunda iyi, ve yerini yine de hak etmiyor.

**Asıl kazanım aletin aklanması.** İşaretlerin ne cinsten olduğu dosyanın kendisine soruldu: grafta
iki tür bağ var — bir yolun ardışık iki düğümü (dosya "burası tek bir yol, bu sırayla sürülüyor"
diyor) ve iki yolu birleştiren kavşak bağı (dosya yalnız "bunlar buluşuyor" diyor). Birincisinin
üstünde bariyer olması çelişki, ikincisinin üstünde olması ise refüjün ta kendisi:

| rota | tek yolun içinde | iki yolu birleştiren |
|---|---:|---:|
| 4001 | 14 | **154** |
| 4002 | 21 | **80** |
| 4021 | 5 | 21 |
| 4061 | **0** | **0** |

Yani mekanizma aradığı şeyi buluyor; çelişkili işaret %8'de kalıyor. 4061'in sıfırı da beş rotanın
her ayarda kılını kıpırdatmamasını açıklıyor — oralarda işaretlenecek hiçbir şey yok.

Buradan çıkan iki gerçek, mekanizma silinse de duruyor: **rota dosyası yan yana giden taşıt
yollarını, aralarında ne olduğuna bakmadan birleştiriyor** (4001'de 154 kavşak bağında bir şey
duruyor), ve **4001'in kaybı yanlış işaretten değil, dolanmanın kendi bedelinden geliyor** — bedelle
birlikte beş araba (80, 23, 1195) civarına toplanıp 51-61. saniyede duruyor, bedelsiz hâlde ise üçü
79. saniyeye kadar yol noktası kazanmaya devam ediyor. Bariyerden kaçınmak onları başka bir
tıkanıklığa sokuyor, ve o tıkanıklık ölçülmedi.

**Açık kalan:** 4002 hâlâ çözülmedi, ama artık ne olmadığını biliyoruz — ne bağı silmek, ne
fiyatlamak. Sıradaki adayın grafa değil **pilotun nişan almasına** dokunması gerekiyor.

### Ve nişan aldığı yere bakınca oldu: "oradan değil"

Grafın iki kapısı da kapanınca kalan yer pilottu, ve mekanizmadan önce ölçüm geldi: **araba ile
nişan aldığı nokta arasında ne sıklıkla bir şey duruyor?** Alan hiç değiştirilmeden, her onuncu
adımda soruldu (`NFS_AIMWALL=<metre>`, indeks yalnız istenince kuruluyor):

| rota | 90 sn boyunca | **ilk 20 sn** | alanın gittiği mesafe |
|---|---:|---:|---:|
| 4081 | %53,6 | **%0,0** | 233 m |
| 4002 | %42,4 | **%71,7** | 176 m |
| 4001 | %13,3 | **%0,0** | 965 m |
| 4021 | %6,4 | %0,0 | 596 m |
| 4102 | %3,1 | %0,0 | 426 m |
| 4121 | %2,1 | %0,0 | 667 m |
| 4061 / 4041 | %0,0 | %0,0 | 859 / 265 m |

İki zaman penceresi tek başına bir teşhis: 4081'in %53,6'sı ve 4001'in %13,3'ü **sonuç**, sebep
değil — ilk yirmi saniyede sıfırlar, yani bunlar zaten kaybolmuş arabaların binaların içinden
nişan alması. **Yalnız 4002 ızgaradan itibaren duvarın arkasına nişan alıyor** (%71,7), ki
"dosyanın çizgisi ayırıcıların üstünden geçiyor" teşhisinin sayıya dönmüş hâli.

Kural, öndeki arabayı geçmenin aynı şekli — nişanı yana kaydır — ama varsayamadığı bir şey var:
**hangi yana.** Refüjün iki yanında da yol var, dosyanın oradan bağ kurmasının sebebi zaten bu. O
yüzden yön aranıyor: kaydırma adımı kadar sağa ve sola bakılıyor, açılmazsa dört katına kadar
genişletiliyor, ilk açılan alınıyor. Karar on tikte bir yenileniyor (her karede verilse direksiyon
titrer, oysa dolanılan şey yüz metrelik beton), ve kaydırma dışında hiçbir şey değişmiyor — beton
bitince araba çizgiye kendiliğinden dönüyor.

| kaydırma | giden | düşen | mesafe | **yol noktası** | kavşak | ayrı düğüm |
|---|---:|---:|---:|---:|---:|---:|
| **0 (kapalı)** | 63/64 | 0 | 4.187 m | **605** | 1245 | 926 |
| 2 | 63/64 | 1 | 4.163 | 615 | 1235 | 931 |
| 4 | 63/64 | 1 | 4.217 | 630 | 1263 | 948 |
| **5** | **63/64** | **0** | **4.251** | **653** | **1358** | **980** |
| 6 | 63/64 | 0 | 4.272 | **663** | 1325 | 968 |
| 7 | 63/64 | 0 | 3.990 | 584 | 1267 | 893 |
| 8 | 63/64 | 0 | 4.315 | 642 | 1276 | 974 |
| 10 | 63/64 | 1 | 4.787 | 636 | 1265 | 962 |

**Sekiz ayardan yedisi kapalıyı geçiyor**, yani kazanç kuralın kendisi, bir sayının şansı değil.

Seçim tepeden yapılmadı: 6 en çok kapsıyor (663) ama komşusu 7 bir çukur (584, kapalının altında),
ve çukurun yanındaki tepe "ayarlanmış sayı"nın tarifidir. **5** kapsamada ondan %1,5 geride, ama
tüm süpürmenin en çok kavşağını (1.358) **en çok ayrı düğüm** (980) üzerinden alıyor — daire çizen
arabayla gerçek ilerlemeyi ayıran sütun o — kimseyi düşürmüyor, ve iki yanı da sağlam (4'te 630,
6'da 663). Çukur da açıklandı: 7'de 4001 (141 → 117) ve 4002 (27 → 22) aynı anda kötüleşiyor,
ikisi de 8'de toparlıyor.

Rota kırılımı kuralın nereye dokunduğunu söylüyor — ve sürpriz:

| rota | kapalı | 5 | 6 | 8 |
|---|---:|---:|---:|---:|
| 4001 | 141 | 154 | **179** | 158 |
| 4021 | 68 | **92** | 83 | 89 |
| 4002 | 27 | **38** | 35 | 24 |
| 4041 / 4061 / 4081 / 4102 | değişmiyor | | | |

**En büyük kazanç 4002'de değil, 4001'de.** Yani ölçümün "sebep değil sonuç" dediği şey — kaybolmuş
arabanın binaların içinden nişan alması — müdahale edilince kazanca dönüyor: kural kaybolan arabayı
geri getiriyor. 4002'nin ızgaradan gelen kendi sorunu da düzeliyor ama daha az.

Ve grafın iki kapısıyla farkı burada: bu kural **hiçbir şeyi silmiyor ve hiçbir şeyi
pahalandırmıyor**, yalnızca aynı hedefe başka bir çizgiden gidiyor.

Yukarıdaki iki zaman penceresi tablosu kural **kapalıyken** ölçüldü. Kural artık varsayılan açık,
yani `NFS_AIMWALL` bugün koşulursa ölçtüğü şey aynı sayı değil, kaydırmadan *sonra* geriye kalan —
"kural iş görmesine rağmen hâlâ duvara nişan alıyor mu". İkisi de faydalı, ama karşılaştırırken
`NFS_AIMCLEAR=0` ile koşmak gerekiyor.

### Yapışan şey düğüm değil yol noktasıymış — ve ızgaranın nerede kurulduğuna bağlı

"Düğüm yapışıyor" diye üç girdidir açık duran madde yanlış adlandırılmıştı. `NFS_WATCH` ile 4081'in
1 numaralı arabası izlendi: düğüm gayet ilerliyor (45 → 46 → 47 → 54 → 49 → 52 → 51), araba 66
km/h'ye çıkıyor, 7 kavşak alıyor — ama satırların hepsinde aynı sayı duruyor, **`waypoint 125`**,
45 saniye boyunca. t=15'te tuttuğu düğüm 3,8 m ötedeyken nişanı 41,8 m **geride**; t=21'de 128 m
geride; t=30'dan sonra hız eksiye dönüyor, yani araba geri geri gidiyor. Çalışan bir rotada aynı
sütun ilerliyor: 4061'de 46 → 47 → 50 → 51 → 53 → 55 → 56.

Sebep kurulum satırında yazılıymış:

```
4061:  waypoint 0 is 844 m from the grid · nearest is Some("#45 at 42 m")
4081:  waypoint 0 is  33 m from the grid · nearest is Some("#124 at 8 m")
```

4081'in ızgarası **126 yol noktalı parkurun sonuncusunun 8 m yanında** kuruluyor, yani sekiz pilot
da hedefini arabanın dibinde tutarak doğuyor. İlerleme kuralının tek kolu vardı — "sıradaki,
tutulandan yakınsa ilerlet" — ve bu böyle bir durumda asla evet olamaz: tutulan sıfırda, sıradaki
(halka başa sarıyor, #0) 33 m ötede. Kilit kalıcı, `toward` arabanın kendi konumu oluyor, graf
yürüyüşünün yön tercihi kalmıyor.

İkinci kol kondu: **tutulan noktaya varıldıysa da ilerlet.** Kayıtlı iki çürütmeyle karışmasın diye
farkı yazılı — resync hedefi *yeniden seçiyordu* (geriye bile atabiliyordu, ve her aralıkta
kaybetmişti), "arkada kaldıysa ilerlet" ise halkanın bütün yayını yürüyüp 92 tur saymıştı; bu ise
yalnız bir adım ileri, yalnız bırakılan noktaya varılmışken, tikte en fazla üç kez.

| reached | giden | düşen | mesafe | **yol noktası** | kavşak | ayrı düğüm | t<30 duran |
|---|---:|---:|---:|---:|---:|---:|---:|
| **0 (kapalı)** | 63/64 | 0 | 4.251 m | 653 | 1358 | 980 | 30 |
| 8 | 63/64 | 0 | 4.865 | 707 | 1453 | 1102 | 28 |
| 12 | 63/64 | 1 | 4.961 | 703 | 1773 | 1068 | 26 |
| 15 | 63/64 | 1 | 5.145 | 780 | 1732 | 1172 | 20 |
| **18** | 63/64 | 2 | **5.394** | **799** | 1566 | **1247** | **18** |
| 20 | 63/64 | 3 | 5.389 | 788 | 1504 | 1228 | 11 |
| 25 | 63/64 | 1 | 4.800 | 678 | 1284 | 1044 | 16 |
| 40 | 63/64 | 4 | 4.660 | 697 | 1503 | 1049 | 16 |

**Sekiz ayarın sekizi de kapalıyı geçiyor.** 15-20 plato, altında 12 (703) ve üstünde 25 (678); 18
hem ortası hem en iyisi. Ve tavanın sebebi var: yol noktaları 40 m aralıklı, yani **yarıda (20 m)
iki kol birbirine değiyor** — düzgün aralıklı bir parkurda tutulana 20 m'den yakınsan sıradaki
zaten daha uzaktır, ki o birinci kolun kendi testi. Yarının ötesinde kural "vardım" demeyi bırakıp
"atla" demeye başlıyor, sayılar da onu söylüyor (25'te 678, 40'ta dört araba haritadan düşüyor).
Yani 18 mutlak bir sayı değil, **adıma bağlı**: `NFS_WPSTEP` değişirse bu da değişmeli.

Tur sayısı sekiz ayarın hepsinde 64 arabada da sıfır — "92 tur" şişmesi geri gelmedi.

Rota kırılımı, dürüstlük payıyla: beş rota kazanıyor (**4121 67 → 152**, **4081 50 → 100**,
**4041 44 → 79**, 4021 +6, 4061 +6), üçü kaybediyor (**4001 154 → 132**, 4102 105 → 93, 4002 −2).
4081'in eşiği de öğrenildi: 8 ve 15'te hiç kıpırdamıyor, 18'de açılıyor (233 → 583 m) — yani
teşhis doğruydu ama eşiği rotanın kendi geometrisi belirliyor.

Bir de yolda görüldü: kod `Self::lost`'a yönlendiriyor ama **öyle bir metot yok** — "kursu
kaybetmiş arabanın cevabı" yazılmamış, yönlendirme boşa gidiyor.

### Çit hakkını veriyor ama nişanı kötü: dört rotada kimseyi kurtarmıyor, ikisinde ağır ödetiyor

Sekizin en zayıfı hâlâ 4002 (223 m). Duvar artık sebebi değil — nişan kuralı orada işini yapıyor,
duvara nişan oranı **%65'ten %14'e** düşüyor (`NFS_AIMCLEAR=0` ile karşılaştırıldı). Bağlayıcı kısıt
başka çıktı, ve bir sütun onu ele veriyordu: `held`, yani çitin kaç kez devreye girdiği. 4002'de
**122**, üstelik toplam 38 kavşak alınırken. Kıyas: 4001'de 13 tutuş / 150 kavşak, 4102'de 3 / 322.

Çit kurulduğunda süpürülmüş ve hakkını vermişti (10 düşen → 1), ama o günden beri bir daha
ölçülmemişti — ve bugünkü iki yeni pilot kuralı arabaları başka yerlere götürdüğü için bedeli de
değişmiş olabilirdi. Ölçüldü:

| | giden | **düşen** | mesafe | yol noktası | ayrı düğüm | t<30 duran |
|---|---:|---:|---:|---:|---:|---:|
| **çit açık** | 63/64 | **2** | 5.394 m | 799 | 1.247 | 18 |
| çit kapalı | 63/64 | **9** | 5.885 | 861 | 1.287 | 12 |

Yani hâlâ hakkını veriyor: yedi arabayı haritadan kurtarıyor, karşılığında 62 yol noktası ve 491 m
alıyor. Ama nişanı kötü olduğu rota rota bakınca çıkıyor:

| rota | düşen (açık) | düşen (kapalı) | mesafe (açık) | mesafe (kapalı) | held |
|---|---:|---:|---:|---:|---:|
| **4041** | 2 | **6** | 966 | 921 | 73 |
| 4021 | 0 | 1 | 633 | 610 | 1 |
| 4061 | 0 | 1 | 898 | 898 | 84 |
| 4121 | 0 | 1 | 704 | 711 | 18 |
| 4081 | 0 | 0 | 583 | 593 | 34 |
| 4102 | 0 | 0 | 503 | 503 | 3 |
| **4001** | 0 | 0 | **884** | **1.086** | 13 |
| **4002** | 0 | 0 | **223** | **563** | **122** |

**Dört rotada kimseyi kurtarmıyor**, ve ikisinde bedeli ağır: 4001 −202 m, 4002 −340 m. Kurtardığı
yer zaten teşhisi konmuş olan 4041 — "yarış çizgisi boşluğun kenarından geçiyor" — orada 6 → 2.
4002'deki 122 tutuşun hiçbir şey önlemediği **kanıtlı**, çünkü çit kapalıyken orada da düşen sıfır.

**Mekanizma koddan okunuyor.** `hold_at_edge` önce gidiş yönünde zeminin bitip bitmediğine bakıyor;
bitiyorsa `edge_at`'in on iki yönlü halkası boşluk yönlerini toplayıp dışa bakan normal `n`'i
veriyor, ve hızın `n` üzerindeki bileşeni siliniyor. Tasarım "kenara değen araba boyunca kayar"
diyor ve yanal bir kenarda öyle de oluyor. Ama boşluk **ileride** olduğunda — dar bir viyadükte
sağ ve sol birbirini götürür, geriye ileri kalır — silinen bileşen arabanın **bütün ileri hızı**
oluyor. 4002'nin arabalarının 0 km/h'de durmasının biçimi bu.

Boşluğun neden ileride göründüğü ise **henüz ölçülmedi**; en makul aday yolun bitmesi değil
*dönmesi*: sondaj düz bir çizgi ve yükseklik penceresi 8 m, yükseltilmiş bir yolun dönen dış
kenarında bu ikisi "yol burada bitiyor" der. Bunu ayıracak ölçüm yazılmadı, ve çiti düzeltecek
şeyin önce bunu ayırması gerekiyor: **"yol bitiyor" ile "yol dönüyor" aynı sondajda aynı görünüyor.**

Çit bu yüzden olduğu gibi bırakıldı — kaldırmak yedi arabayı haritadan atmak demek, ve nişanını
düzeltmek ayrı bir tur.

### Duran 18 araba tek bir hastalık değilmiş — ve ağ vardı, sadece oyuncuya bağlıydı

En büyük tek kayıp kalemi "t=30'dan önce ilerlemeyi kesen 18 araba"ydı ve kodun kendi yorumu
"özet bunları ayırt edemez, farklı işler isterler" diyordu. Ayırt eden ölçüm yazıldı: her araba için
yarışın yüzde kaçında hiç kıpırdamadığı, bunun ne kadarının **önündeki arabanın arkasında** geçtiği,
yüzde kaçını **yan yatmış** geçirdiği, ve **çitin kaç kez** tuttuğu. Eşikler uydurulmadı, pilotun
kendi stall kuralından ödünç alındı (0,7 m/s, 3 sn yerleşme) — ölçen ile ölçülen aynı şeyi kastetsin
diye.

| sebep | araba |
|---|---:|
| önündeki arabanın arkasında (>%50) | 3 |
| çit çivilemiş (10+ tutuş) | 5 |
| yan yatmış | **2** |
| **hiçbiri** | **8** |

**Ve burada kendi ölçümüm beni yanılttı, kaydı düzeltiyorum.** İlk sayım "25/64 araba devriliyor,
devrilmişler sayılan kavşakların %52'sini tutuyor" dedi ve bu bir felaket gibi okundu. Gevşekmiş:
`rolled > 0` yalnızca "bir tik 60°'yi geçti" demek, ve viraj içinde savrulan araba da buna giriyor.
Erken duran 18 arabanın **14'ünde yan yatma süresi %0**; gerçekten devrilip kalan iki tane
(%92 ve %64). Kurtarma mekanizması bunu bağımsız olarak doğruladı: sekiz rotada yalnız **7 kez**
ateşledi.

Kirlenme iddiasının doğru kalan kısmı şu: **devrilmiş araba durur, pilotu bunu bilmez.** İzlenen bir
araba (4102, no 7) aynı koordinatta **72 saniye** durdu ve bu sırada kavşak sayacı 10'dan **115'e**
çıktı. Yani `junctions` sütunu tek bir taş gibi duran arabayla şişebiliyor — kararların
`geçilen yol noktası` sütununda verilmiş olması bu yüzden şans değil: hareketsiz araba yeni yol
noktasına yaklaşamaz.

Devrilmenin **hiçbir cevabı yokmuş**, ve sebebi asıl bulgu: `keep_in_world` üç pencereli binary'de de
yalnız `state.rig` için, yani **oyuncunun arabası** için çağrılıyor. Rakip düşerse düşmüş kalıyor,
devrilirse devrilmiş kalıyor — sim'de değil, **oyunda**. Ağ baştan beri vardı ve tek bir arabaya
bağlıydı.

İki şey eklendi. `Rescue::Righted` — dört saniye tolerans (virajda iki teker havalanabilir, o
sürüştür), ve ayrı bir hâl olarak, çünkü devrilen araba dünyadan çıkmadı ve öyle raporlamak sonraki
okuyucuya yalan olur. Ve sim'de `NFS_RESCUE`, çünkü "sahayı hiç yakalamamak" hiç ölçülmemiş bir
karardı:

| | giden | **düşen** | mesafe | yol noktası | ayrı düğüm | yakalama |
|---|---:|---:|---:|---:|---:|---:|
| rakiplere ağ yok | 63/64 | **2** | 5.394 m | 799 | 1.247 | — |
| ağ var | 63/64 | **0** | 5.372 | 806 | 1.271 | 7 |

Düşen 2 → 0, gerisi düz (+7 yol noktası bu ölçekte kazanç değil — 609/605 için de öyle denmişti).
Mütevazı ama bedava. **Sim'in varsayılanı kapalı bırakıldı**: bu dosyadaki bütün tablolar ağsız
ölçüldü ve varsayılanı sessizce değiştirmek onları kıyaslanamaz yapardı. Rakiplere ağı `nfs_cruise`
tarafında da bağlamak açık iş.

**En büyük kova — ve sorusu aynı turda cevaplandı.** Dik duran, önü boş, çitin dokunmadığı, ama
yarışın %33-93'ünde kıpırdamayan sekiz arabaya üç soru soruldu ve üçü de beklediğimin tersini
söyledi:

| soru | cevap |
|---|---|
| bir şeye mi oturmuşlar (yüksek merkezli) | **hayır** — durdukları sürenin %99-100'ünde dört teker yerde, ortalama 4,0 temas |
| duvara tam gazla mı yaslanıyorlar | **hayır** — duran arabalarda ortalama gaz **0,00** civarı (−0,10 ile +0,02), fren sıfır |
| pilot hiç konuşmuyor mu olabilir | **hayır** — `drive` hiç `None` dönmüyor, sessizlik oranı **%0** |

Üçü birleşince tek bir açıklama kalıyor, ve aritmetiği de tutuyor: pilotun **stall → geri git**
döngüsü. Kural 1,5 saniye durgunluktan sonra tetikleniyor ve 1,2 saniye `-0,7` gazla geri gidiyor,
sonra baştan; bu döngünün ortalaması `(1,5 × ~0,5 + 1,2 × −0,7) / 2,7 ≈ −0,03`, ki ölçülen aralık
bu. Yani araba "sürülmüyor" değil — **sürekli kurtulmaya çalışıyor ve hiç kurtulamıyor.**

Kural tam bu durum için yazılmıştı ("bir daha aynı engele koşmasın diye düğümü de kara listeye al")
ve sekiz arabada doksan saniye boyunca çalışmıyor. Artık ne olmadığını biliyoruz: zemin değil,
temas değil, sessizlik değil, trafik değil, çit değil, devrilme değil.

**Ve bir soru daha kapandı: geri vites çalışıyor.** Negatif gazın vitesi çevirmeyip fren gibi
davranması makul bir şüpheydi ve yanlış çıktı — geri gitme komutu verilen arabaların burnu boyunca
aldığı yol ölçüldü ve gerçekten geri gidiyorlar: −21,0 · −22,4 · −28,7 · −12,1 · −10,4 m, biri
−78,8 m. Yani manevra icra ediliyor.

Asıl sayı onun yanında: bu arabalar **doksan saniyenin 21-38 saniyesini geri geri giderek**
geçiriyor — yarışın dörtte biri ile yarısı arası. Geri git, ileri dön, aynı şeye çarp, baştan.
İki alt grup ayrılıyor: uzun mesafe geri gidip aynı yere dönenler, ve **geri viteste bile
kıpırdamayanlar** (otuz saniyede −0,0 ile −6 m; arkalarında da bir şey var).

Yani kusur kurtulma manevrasında değil, **kurtulduktan sonra nereye gidildiğinde**. Ve o da aynı
turda ölçüldü: vazgeçme anında, terk edilen düğümün ve yerine seçilenin arabadan görülen yönleri
karşılaştırıldı (45°'den dar açı "aynı yön" sayıldı).

| araba | vazgeçme | aynı yön |
|---|---:|---:|
| 4001 no 7 | 126 | **125** |
| 4001 no 3 | 126 | **124** |
| 4002 no 2 | 115 | 110 |
| 4021 no 5 | 91 | **91** |
| 4041 no 2 | 89 | **89** |
| geri kalan on üç | 39-117 | **%94-100** |

Tek istisna devrilmiş araba (117 vazgeçme, 0 aynı yön) ve o zaten hiçbir yere gitmiyor.

**Vazgeçme, arabayı %94-100 oranında aynı yöne gönderiyor** — ve sebebi yapısal, tesadüf değil:
`step_avoiding` aynı `toward` hedefine en yakın dalı seçiyor, kara liste ise yalnız **bir düğümü**
eliyor. Hedef değişmediğine göre ikinci en iyi dal da doğal olarak aynı yöne bakar. Yani manevra
"bu düğümden vazgeç" diyor, oysa arabanın çarptığı şey düğüm değil **o yön**.

Sayının büyüklüğü de bunu tamamlıyor: arabalar doksan saniyede **39 ilâ 126 kez** vazgeçiyor, yani
saniyede yarım kez. Kara listenin uzunluğu (sekiz rotada 334 düğüm) bunun çok altında, çünkü
vazgeçmelerin çoğu zaten listede olan düğümleri yeniden seçiyor.

### Ve kaçış grafın dışından geldi: şehre sormak

Üç varyantın da çürümesi bir şeyi söylüyordu — cevap düğüm seçme makinesinin dışında. Ama
mekanizmadan önce onun varsaydığı şey ölçüldü: sıkışan araba gerçekten kapana mı kısılmış?

`nfs_sim` sıkışan her araba için on iki yönde zemin ne kadar tutuyor ve üstünde bir şey
duruyor mu diye sordu. Cevap: **6-10 yön, yirmi metre boyunca temiz.** Araba açık alanın
ortasında duruyor ve hiçbirini kullanmıyor, çünkü pilotun sorabildiği tek soru "hangi
düğüm" ve o düğümlerin hepsi aynı tıkanıklığa bakıyor.

Kural: pilot sıkıştığını anlayınca **grafı bırakıyor** ve şehre soruyor — en iyi yön
kazanıyor, araba sınırlı süre oraya sürüyor, süre dolunca tuttuğu düğüm/kara liste/yol
noktası **hiç dokunulmamış** hâlde beklediği için tam kaldığı yerden devam ediyor. Üç
çürütülmüş kuralın hiçbiri geri alınmadan denenebilmesinin sebebi bu: onlar makinenin
içindeydi, bu dışında.

**İlk sürüm çürüdü, ve nasıl çürüdüğü düzeltmeyi yazdı.** "En açık yön" seçilince:

| kaçış | giden | yol noktası |
|---|---:|---:|
| 0 | 63/64 | **799** |
| 1,0 | 61/64 | 781 |
| 2,0 | **64/64** | 757 |
| 3,5 | **64/64** | 742 |
| 6,0 | **64/64** | 737 |

Mekanizma yapması gerekeni yapıyordu — 2 saniyeden itibaren **64/64 araba kavşak alıyor**,
yani kimse sıkışıp kalmıyor — ama kurtarılan araba parkurdan uzaklaşıyordu. Tek satırlık
düzeltme: açıklığı, gidilmesi gereken yönle ağırlıkla (`reach × (0,5 + 0,5·cos)`). Aynı
süpürme tersine döndü:

| kaçış | giden | düşen | **yol noktası** | ayrı düğüm | t<30 |
|---|---:|---:|---:|---:|---:|
| **0 (kapalı)** | 63/64 | 2 | 799 | 1.247 | 18 |
| 1,0 | 61/64 | 1 | 802 | 1.262 | 15 |
| 2,0 | **64/64** | 2 | 827 | 1.314 | 18 |
| **3,5** | **64/64** | **1** | **833** | **1.329** | **15** |
| 6,0 | **64/64** | 2 | 838 | 1.319 | 19 |

3,5 seçildi: 2-6 platosunun ortası ve ayrı düğümde en iyisi (1.329), erken duran en az
(15), düşen 1. 6,0 beş yol noktası daha kapsıyor ama ikisini de geri veriyor.

Bu, dört turdur açık olan maddeyi kapatıyor: **kaçış manevrası pilotun düğüm makinesinin
dışından geldi ve işe yaradı.**

**Ama kimlere yaradığı ayrı bir bulgu, ve kalan kovayı yeniden tanımlıyor.** Kural girdikten
sonra erken duran araba 18 → 15'e indi, fakat "başka hiçbir şey açıklamıyor" kovası **sekizde
kaldı**. Kaçışın kendi sayacı takıldı ve cevap net:

| araba | kaçış sayısı | kaçış boyunca alınan yol |
|---|---:|---:|
| çoğu | 44-126 kez | **0,0 m** |
| en iyisi | 95 kez | 5,5 m |

Yani kural **ateşleniyor** (araba başına yüze yakın kez) ve arabayı hiçbir yere götürmüyor. Bu
kolu ayırt etmek önemliydi: "hiç ateşlenmiyor" ile "ateşleyip başaramıyor" zıt düzeltmeler
ister.

Ve başaramamasının sebebi kaçışta değil: kaçış **nereye sürüleceğini** değiştirir, oysa bu
arabalar hiç hareket edemiyor — daha önce ölçülmüştü, bazıları otuz saniye geri viteste
kalıp **−0,0 m** alıyor. Ne ileri ne geri. **Kamalanmışlar**, ve kamalanmış bir arabaya
hiçbir pilot kuralı yaramaz.

Alan genelindeki kazanç bundan bağımsız ve gerçek: kaçış, yanlış nişan alan ama *hareket
edebilen* arabalara yaradı. Kalan sekiz araba artık bir pilot sorunu değil, bir **fizik/
geometri** sorunu olarak sınıflanmalı — ve bir sonraki tur onu öyle ele almalı.

### Sekiz araba fizik sorunu değildi: manevra kendi tetikleyicisini üretiyordu

Yukarıdaki son cümle yanlış çıktı ve nasıl yanlış çıktığı, bulgunun kendisinden değerli.

Sekiz arabanın "fiziksel olarak kamalanmış" olduğu bir çıkarımdı, ölçüm değil. Ölçülünce sırayla
düştüler: **karnı yerde değil** (dört tekerin taşıdığı yük, kendi ağırlığının 0,95-1,00'i),
**kutuda değil** (on iki yönün sekizi 8 m'den açık, biri 20 m), **önü kapalı değil** (burnu
boyunca on metre zemin profili dümdüz), **aktarma bozuk değil** (boşta geçen süre %0, ikinci
vites, kesintisiz 1200 Nm), **lastik modeli tükenmiş değil** (`ref_vel` tabanı sayesinde durgun
halde kayma oranı 0,33, yani tepeye yakın; teker traksiyon kontrolüne kırpıldığı için 0,5 rad/s).

Zinciri kesen şey ölçüm değil **deney** oldu, ve baştan yapılmalıydı: pilotu devreden çıkarıp
gazı basılı tutmak (`NFS_FLOOR`). Araba **0'dan 71 km/h'ye** çıktı ve 180 m gitti. Tam kilitle
tekrarlandığında da kalktı (29 km/h) — yani direksiyon da suçlu değil. Araba kusursuzdu; onu
sürmeyen pilottu.

Sebep, takılma kuralının tek satırındaydı:

```rust
if self.age > SETTLE && speed < STALL_SPEED && self.hold <= 0.0 { self.stalled += TICK; }
```

`speed` **işaretli**. Geri vitesini yeni bitirmiş araba negatif hız taşır, `speed < 0,7` daha ilk
tikte doğrudur. Geriden +0,7 m/s'ye dönmek tam gazda bile iki saniyeden fazla sürer, `STALL_FOR`
ise 1,5 — yani **her geri vites bir sonrakini garantiliyordu**. Manevra kendi tetikleyicisini
üretiyor, araba pilota itaat ettiği için cezalandırılıyordu. 90 saniyenin 35'i geri viteste,
115 kaçış, hiçbiri tamamlanamaz.

Geriye yuvarlanan araba duruyor sayılmasın diye konan muafiyet, sekiz rotada **her ayarda** tabanı
geçti:

| eşik (m/s) | kapsanan | ayrık düğüm |
|---|---|---|
| kapalı | 833 | 1329 |
| 0,2 | 840 | 1345 |
| **0,5** | **869** | **1364** |
| 1,0 | 860 | 1342 |
| 2,0 | 851 | 1335 |

Yükselip inen tek tepeli eğri, gerçek bir sınır bulunduğunda beklenen şekil: küçükte hâlâ
sürünen araba mahkûm ediliyor, büyükte gerçekten sıkışmış olan bir tutam geri vitesin arkasına
saklanıyor. **0,5 girdi.**

### Aynı turda çürüyen: viraj gaz kesmesini hızla rampalamak

İlk bulduğum mekanizma buydu ve aritmetiği hâlâ doğru: tam kilitte gaz `1 − 0,85·0,75 = 0,36`'ya
iner, bu durgun halde ~0,29 m/s² eder, 1,5 saniyede 0,44 m/s — eşik ise 0,7. Yani sert direksiyon
tutan araba kendi takılma eşiğini aşamaz. Doğru, ama arabaların takılma **sebebi bu değildi**.

Gerçek sebep düzeldikten *sonra* ölçülünce rampa çöktü: 4 ve 8 m/s'de **821** ve **690**, düz
kesmenin **869**'una karşı. Durgun halde tam kilitte tam gaz arabayı kurtarmıyor, fırlatıyor. İki
mekanizma izden bakınca birbirine benziyordu; ayıran tek şey süpürme oldu. Çürütme
`CORNER_LIFT`'in yanına yazıldı, kod silindi.

**Kalıcı alet:** `NFS_FLOOR=<k>` (+ `NFS_FLOORSTEER`) `nfs_sim`'de kaldı. Pilotun akıl yürütmesi
ile arabanın hareket edebilmesi iki ayrı iddia, ve `pilot.drive`'dan geçen her ölçüm ikisini
birlikte sınıyor. Bu turu çözen şey oydu.

### Yönü elemek (çürüdü)

Bunun üzerine yazılan mekanizma — **düğümü değil yönü elemek** — çürüdü, ve nasıl çürüdüğü asıl
bulgu. Vazgeçme anında, terk edilen düğümün yönüne `shun` derece içinde kalan bütün dallar o seçim
için elendi, kalanların hedefe en yakını alındı; hiçbiri kalmazsa eski davranışa düşüyordu.

| shun | 0 | 30° | 45° | 60° | 90° |
|---|---:|---:|---:|---:|---:|
| yol noktası | 799 | 817 | 795 | 790 | 814 |
| ayrı düğüm | 1.247 | 1.287 | 1.258 | 1.239 | 1.278 |
| **aynı yön oranı** | **%90** | %89 | **%90** | **%90** | **%89** |

Karar sütununda eğilim yok — 817, 795, 790, 814 — yani 800 civarında gürültü. Ama çürütmeyi
kesinleştiren şey ikinci satır: **shun=90'da bile aynı yön oranı %89.** Doksan derece, terk edilen
düğüme bakan bütün ön yarıküreyi elemek demek; buna rağmen seçimlerin %89'u hâlâ o düğümün 45°
içinde kalıyorsa, filtre **neredeyse her seferinde geri düşüyor** demektir. Ve sayı bunu üstten
sınırlıyor: shun ≥ 45'te filtre başarsaydı seçim 45°'nin dışına düşer ve sayaca hiç girmezdi, yani
filtre **en fazla onda bir** iş görüyor.

**Yani kusur pilotun seçiminde değil, seçecek bir şey olmamasında.** Araba sıkıştığında geldiği
düğümün başka yöne çıkan bir dalı yok — o noktada graf fiilen bir koridor. Düğümü elemek de yönü
elemek de aynı yere çıkıyor, çünkü ikisi de *alternatif varsayıyor.*

Buradan "demek ki dal yok" diye bir sonuç çıkardım, ve **o bir çıkarımdı, ölçüm değildi.** Ölçünce
yanlış çıktı — vazgeçme anında düğümün **4,22 dalı** var ve **1,56'sı başka yöne bakıyor** (%37).
Alternatif var.

Doğru cevap üçüncü ölçümde geldi: o dalların **%92'si zaten kara listede** (vazgeçme başına 1,56
başka-yöne dalın yalnız **0,12'si** listede değil). Yani araba kendi geçmişi tarafından köşeye
sıkıştırılmış. Kara liste hiç unutmuyordu — `blocked` yalnızca büyüyen bir liste — ve hepsi
tükenince `step_avoiding`'in son çaresi listeyi tümden yok sayıp arabayı yine aynı yöne gönderiyor.
Kendi kendini besleyen bir kilit.

**Ve kara listeye unutma eklemek de çürüdü.** Girişler 3, 6, 12 ve 25 saniye sonra düşürüldü:

| unutma | giden | düşen | mesafe | **yol noktası** | kavşak | ayrı düğüm | kara liste |
|---|---:|---:|---:|---:|---:|---:|---:|
| **0 (hiç)** | **63/64** | 2 | **5.394 m** | **799** | 1566 | 1247 | 334 |
| 3 sn | 60/64 | 1 | 5.209 | 785 | 2551 | 1230 | 56 |
| 6 sn | 60/64 | 0 | 5.218 | 758 | 2128 | 1218 | 109 |
| 12 sn | 61/64 | 1 | 5.074 | 787 | 2226 | 1281 | 166 |
| 25 sn | 61/64 | 3 | 5.172 | 773 | 2038 | 1315 | 202 |

Dördü de yol noktasında, giden arabada, mesafede ve erken durmada kaybediyor. Nasıl kaybettiği de
tanıdık: kavşak 1.566'dan 2.038-2.551'e fırlarken ayrı düğüm kıpırdamıyor — bu projede o ikili
"ilerleme değil, aynı yerde salınım" demek. Araba vazgeçtiği düğüme dönüyor, yine takılıyor, yine
vazgeçiyor. **Kara listenin kalıcılığı boşuna değilmiş: daha kötü bir salınımı tutuyormuş.**

Böylece kaçış dizisi kapanıyor ve vardığı yer yapısal: **ne farklı seçmek, ne unutmak işe yarıyor,
çünkü ikisi de aynı makinenin içinde oynuyor.** Pilotun düğüm seçme makinesi yerel bir optimumda —
düğüm eleyen, yön eleyen ve liste unutan üç varyantın üçü de çürüdü. Sıradaki adayın bu makinenin
*dışından* gelmesi gerekiyor: kaçış boyunca grafı tamamen bırakıp arabayı serbest sürdürmek, ya da
kaçış yönünü düğümlerden değil şehrin geometrisinden türetmek.

Bir de alet dersi, iki tane: sayacın eşiği mekanizmanın eşiğinden **bağımsız** olmalı (shun ≥ 45'te
45°'lik sayaç tanım gereği doğru çıkıp ölçmeyi bıraktı), ve bir çıkarımı ölçüm yerine koymamalı —
bu turda "dal yok" sonucu tam olarak öyle üretilmişti ve yanlıştı.
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

### Sekiz bundle'ı birden yüklemek bir kusurdu — bölme hiç yapılmamış

Bu bölümün geri kalanı "şehir görünür oldu, sürülebilir hâle geldi" diye ilerliyor ve hepsi doğru,
ama altında duran şey yanlıştı: **`TRACKS`'teki sekiz `STREAM*.BUN`'ın hepsi birden yükleniyordu.**
Bu bir stopgap sanılmıştı; değil, üst üste bindirme.

Ölçüm: 13.986 ayrı (obje, konum) çiftinin **sekizinin hepsinde bulunanı yalnız 1 tane**,
**5.377'si tek bir dosyada**. Yani bunlar aynı şehrin sekiz sürümü de değil, bitişik parçalar da
değil. İkisi kendi zeminini taşıyor: `STREAML4RB` başka hiçbir yerde olmayan 397 `TRN_GRASS` +
359 `TRN_RDP`, `STREAML4RG` 542 + 340 — her biri ~3,7 × 8 km'ye yayılmış. Yani aynı haritanın
üstüne birkaç rotaya özgü **arazi ve yol katmanı** seriliyordu.

Bir yarışın 341 güzergâh düğümünün altındaki yol yüzeyi yığını:

| | üçlü yığın | ikili yığın | hiç yol yok |
|---|---|---|---|
| sekiz bundle | **16** | 56 | 10 |
| yalnız kendi bundle'ı (L4RA) | **0** | 21 | 53 |

Yükseklik çözümünün en kötü basamağı da 20,4 m → **13,1 m**. `world::bundle_for_route` artık rota
dosyasının dizin adından bölgeyi çıkarıyor (`ROUTESL4RA/Paths4001.bin` → `STREAML4RA.BUN`) ve
`nfs_cruise` yarış yüklüyken yalnız onu okuyor.

Bedeli açıkça yazılı: tek başına o bundle'da 53 düğümün altında yol adlı obje yok (sekiziyle 10).
Bir bundle **tam bir dünya değil**; hangi bölgelerin bir yarışa yettiğini seçmek ROADMAP'in M5
dediği iş, ve bu onun tek satırlık, tek yarış sürülebilecek hâli.

**Bu ayrıca daha önceki bir ölçümü de açıklıyor.** `world::route`'un "her düğümün altında en az iki
sürülebilir yüzey, 341'in 179'unda dört ve fazlası, tepe-taban 79 m" bulgusu çatılara ve otopark
güvertelerine yazılmıştı; payının bir kısmı buymuş — üst üste binmiş zemin katmanları.


Güncel durum burası; 2026-08-09 ve 2026-08-04 bölümleri tarihsel kayıt.

### DÜZELTME (2026-08-11, aynı gün): kaba kademeler **eleniyor**

Aşağıdaki karar yanlıştı ve nasıl yanlış olduğu asıl kayda değer olan. Ölçümler doğruydu:
kademeler yere en ince kademeyle aynı oranda oturuyor (%67/%70) ve komşularının içinden ondan
fazla geçmiyor (%22/%26). Bundan "yerde duran objeleri siliyoruz" sonucu çıkarıldı ve varsayılan
"hepsini çiz" bırakıldı.

Sonra biri şehirde araba sürdü. Kaba kademe bir **uzak taklidi** — fotoğraf kaplı bir kutu, beş
yüz metreden okunmak için yapılmış — ve arabadan bakınca binasının *olmadığı* açık arazide duran
bulanık bir levha. `XB_LANDMARKTOWER_1Z_RB_00`, temsil ettiği kuleden 544 m ötede, otoyol
kavşağının yanındaki çimenin üstünde duruyor.

Ölçümlerde yanlış olan bir şey yok. Onlar **"dosya bu objeleri bina gibi mi yerleştiriyor"** diye
sordu, cevap evetti. Soramadıkları şey **"geometri sokaktan görülecek kadar bitmiş mi"** idi — ki
asıl soru oydu. Bu bölüm "karar bir bakışa bakıyor" diye başlamıştı; bakış sayılarla ezildi ve
sayılar başka bir şeyi cevaplıyordu.

Varsayılan artık `keep_finest`; geri dönüş yolu `NFS_TIERS=all`.

---

### Karar verildi: `_1A/_1B/_1Z` "raf" değil LOD kademesi — ve elenmiyorlar (ÜSTTEKİ DÜZELTMEYE BAK)

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

#### `Routes####F/B.bin` — açılmadı, ama nereden devam edileceği belli

`Paths####.bin`'in yanında ikinci bir aile var ve hiçbir kaynakta geçmiyor: etkinlik başına, yön
başına bir dosya (`F`/`B`), toplam **226**. OpenUG'un `FORMATS.md`'inde yok.

Ölçülenler:

| chunk | dosya | boyut | gcd |
|---|---|---|---|
| `0x00034121` | 222 | 1.848 .. 781.844 | **28** |
| `0x00034122` | 222 | 132 .. 120.100 | 4 |
| `0x00034123` | 122 | **her zaman tam 10.488** | — |

`0x34121` kayıtları **`11, 11` imzasıyla** başlıyor — başlangıç işaretçilerinin (`0x34146`)
imzasının aynısı, yani bunlar tek bir aile. Her kayıt `+0x10`'da 16 baytlık bir ad taşıyor:
`TrackRoutesA21`, `A30`, `A31`, `A32`, `A33`, `A34`, `A40`, `A43`, `A44`. `Routes4001F.bin`'de
**136 kayıt** var ve ardışık kayıt başlangıçları arasındaki her aralık **28'in tam katı**
(812, 1372, 1148, 700, 1540 → 29, 49, 41, 25, 55 × 28).

**Çözülemeyen:** kayıt sınırı. Kaydın ilk 112 baytındaki hiçbir `u16`/`u32`, aralığı da
aralık/28'i de öngörmüyor (135 kayıtta sıfır isabet, ±4 kaymayla da). Yani uzunluk ya daha
ileride, ya iç içe bir yapının içinde, ya da hiç saklanmıyor.

**Denenip elenen:** `0x34123`'ün indeks tablosu olması. Sabit boyutu (her dosyada 10.488) bunu
düşündürüyor ama içeriği seyrek — 81 farklı `u32`, en sıkları `0xBF800000` (−1,0f, 1.216 kez),
`0xFFFFFFFF` (646) ve sıfır (608). Ofset gibi duran değer yok.

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

**Bağımsız tanık (2026-08-11, internet araması).** `yugecin/nfsu2-re` bu chunk'ı okuyor ve
`nfsu2-re-binfiles/main.c` içinde **aynı şekilde yürüyor**: `part_size = *(short*)(pos + 0x42);
pos += part_size;`, kutu `0x20..0x2C`'de, ilk köşe `0x44`'te — aynı yerden sayılan aynı ofsetler.
Uzunluk alanı burada en son ve en zor türetilen şeydi; iki okuyucunun aynı kayıt sınırına ayrı
yollardan varması tek başına her ikisinden de değerli.

Önündeki alanı ise yanlış tiplemiş — `0x40`'a `short radius` diyor. O köşe sayısı, ve
`uzunluk = 68 + 8 × sayı`'nın 42.698 kaydın hepsinde tutması bunun tartışma değil bilgi olmasını
sağlıyor. Ayrıca yalnız ilk baytı `0x12` olan kayıtları basıyor — on dört türden biri, üstelik
8 m'lik işaretçi — yani chunk'ın şekli oradan hiç görünmemiş.

`0x34148`, `0x34149`, `0x3414C` ve `0x3414D`'yi **hiç kimse belgelememiş**: o proje beş bölüm
işliyor ve yalnız biri bizimki.

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

`nfs_cruise NFS_ROUTE=<Paths*.bin>` artık bir yarış yükleyip hattını yola çiziyor; HUD hangi
güzergâhta olduğunu, koridora uzaklığını ve dosyanın kendi kümülatif mesafesini gösteriyor, parkur
dışına çıkınca da uyarıyor. Bu uyarı harita-kenarı uyarısından **ayrı bir soru**: harita kenarı
dünyanın bittiği yer, koridor yarışın bittiği yer — ve ilki arabanın *önüne*, ikincisi *altına*
soruluyor, çünkü haritadan çıkmak geri dönülmez, parkurdan çıkmak değil.

Koridor kendini doğruluyor: bir rota dosyasının kendi düğümleri o dosyanın koridorunda **0 kaçak,
en kötü uzaklık 0,000 m, en kötü mesafe hatası 0,000** ile bulunuyor (üç dosyada). Çerçeve dönüşümü
ya da bir indeks kaysa bu tutmazdı.

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

### "Kimse tur tamamlamıyor" yanlış soruymuş — alan bir yere varıp dolanmaya başlıyor

Duran hedef aylardır böyle yazılıydı. Üzerine gidince üç kez şekil değiştirdi ve sonuncusu asıl
iş listesini değiştiriyor.

**Önce aritmetik.** Bir tur 144 waypoint, waypoint'ler 40 m aralıklı, yani 5.760 m. Alanın en iyi
arabası 90 saniyede 26 waypoint yapıyor — ortalama 42 km/h. O hızda bir tur **494 saniye** sürer.
Yani 90 saniyelik ölçüm penceresinde tur tamamlamak *kurgu gereği* imkânsızdı; hedefin kendisi
ölçülemez yazılmıştı.

**Sonra pencereyi açtım ve o açıklama da çürüdü.** 4121'de 600 saniye (6,7 kat) koşturunca en iyi
araba 26 → **29** waypoint yaptı. 6,7 kat zaman, %12 kazanç. Sınır zaman değil.

**Alan bir yere varıp orada kalıyor.** 600 saniyenin sonunda sekiz arabanın yedisi
`(-560…-754, 19–35, 886…1047)` içinde, ~200 m'lik tek bir bölgede, hepsi 0 km/h. Ve rotalarından
**58-126 m sapmış** durumdalar. Sekizincisi (araba 4) `y = -38223`'te — dünyadan düşmüş.

**Düşme teşhisi bir dünya kusuru gösterdi:**

```
car 4: let go at t=101.9s (-725, 32.8, 938) doing 4 km/h, 0.0 m of air
       · 41 m off the course for 4.7 s · now 38256 m down · inside the map
```

4 km/h'de, sıçramadan, haritanın içinde. Zemin taraması onayladı — o noktanın çevresinde 16-30 m
genişliğinde, çapraz uzanan zeminsiz bir bant var (`NFS_HOLE=x,z` ile haritalanıyor, alet kaldı).

**Ama delik kursun üstünde değil, ve bu bulguyu tersine çeviriyor:** en yakın rota düğümü **41 m**
ötede ve kurs koridorunun yarı genişliği **12 m**. Yani dünyanın, arabaların sürmesi gereken
yerinde bir sorun yok.

**Kalan soru bu yüzden dünya değil, dolanma:** 12 m'lik bir koridordan 40-130 m sapan arabalar,
sapınca haritasız araziye giriyor ve orada bitiyor. Delik onları öldüren şey, oraya götüren şey
değil.

Sıradaki tur bunu sormalı: **arabalar 25-29. waypoint civarında koridoru neden bırakıyor?** Alan
toplamı (869) değil, *sapma* ölçülmeli — hangi waypoint'te, hangi yöne, ve kurs orada ne yapıyor.
**Ve sıradaki tur hemen geldi: kayıp waypoint 25'te değil, 6'da.** Yukarıdaki "koridoru neden
bırakıyorlar" sorusu için sapmanın *ilk* anı ölçüldü — üç saniye boyunca koridor dışı, yani viraj
kesme değil kalıcı kayıp. Sonuç tek bir noktayı gösteriyor:

| araba | kursu bıraktığı an | waypoint | konum |
|---|---|---|---|
| 6 araba | t = 36-50 s | **6** | (−334…−337, 11-12, 1372…1376) |
| 1 araba | t = 86 s | 25 | (−697, 33, 926) — deliğin yanı |
| 1 araba | hiç bırakmadı | — | — |

Sekizin altısı **dört metrelik tek bir noktada** kaybediyor. Önceki turda 25-29 civarını
işaretlemem hataydı: arabaların *bittiği* yere bakmıştım, *yanlış gittiği* yere değil — bu ölçüm
tam o ayrım için kuruldu ve ayrımı hemen gösterdi.

Orada dünya sağlam: zemin taraması 13×13'lük ızgarada tek boşluk göstermiyor, en yakın rota düğümü
22 m ötede ve koridor uzaklığı 15 m (sınır 12). Yani araba sağlam zeminde, izin verilenden yalnız
üç metre fazla açılıyor — ama bir daha toparlanmıyor.

İzleme o anı yakalıyor. Araba tuttuğu düğümü `…113 → 114 → 274 → 276 → 37 → 38` diye değiştiriyor
ve tam o sırada 65 km/h'den **16 km/h**'ye düşüyor. Numaraların 270'lerden 30'lara atlaması, ağın
**başka bir koluna** geçtiğini söylüyor.

Buradan çıkan hipotez — ve bir sonraki turun sınaması gereken şey — şu: **pilot yarış rotasını
değil yol ağını sürüyor.** Koridor `Paths4121`'in kendi hattından kuruluyor, oysa pilot
`Network`'ün tamamında yürüyor; waypoint 6'daki kavşakta yarış hattından çıkıp komşu bir yola
sapmak, pilotun kendi ölçütlerine göre tamamen meşru bir hamle. Öyleyse kusur "dolanma" değil,
**pilotun neyi takip ettiği**.

Sınaması ucuz: o kavşakta ağın kaç kolu var, hangisi yarış hattında, ve pilot hangisini seçiyor.
**Hipotez çürüdü: alacak yanlış kol yok.** Kayıp noktasındaki kavşak (düğüm 37, hat 4) üç kol
taşıyor ve **üçü de koridorun içinde**, uzaklıkları 0,0 m:

| kol | hat | konum | koridora |
|---|---|---|---|
| 36 | 4 | (−314, 1392) | 0,0 m |
| 38 | 4 | (−391, 1378) | 0,0 m |
| 277 | **3** | (−312, 1394) | 0,0 m |

Üçüncüsü komşu bir hatta ait ama koridor onu da kapsıyor — yani koridor tek bir `Paths` dosyasının
hattı değil, birden çok hattı örtüyor. "Pilot ağı sürüyor, yarışı değil" okuması bu yüzden yanlış:
pilotun burada seçebileceği yanlış bir kol **yok**.

**Geriye kalan şey rota seçimi değil, virajı geniş almak.** Düğüm konumları ve iz birlikte
okununca: araba kuzeydoğudan (−255, 1436) → (−292, 1380) diye güneybatıya iniyor, sonra
(−339, 1379) ile batıya dönüyor — yaklaşık **50°'lik bir viraj** — ve tam orada 65 km/h'den
**16 km/h**'ye düşüyor. Kayıp konumu (−334, 1374), yolun ekseninden ~15 m güneyde.

Yani altı araba aynı virajda, 12 m'lik koridordan üç metre taşarak çıkıyor ve bir daha
dönemiyor. Sıradaki turun sorusu artık dar ve pilotun kendi kollarına bakıyor: bu viraja giriş
hızı (`BRAKE_SPEED`, `BRAKE_GAIN`) ve viraj gaz kesmesi (`CORNER_LIFT`) neyi veriyor, ve taşma
frenle mi direksiyonla mı kapanıyor. Ölçüt yine sekiz rotalık süpürme — ama artık yanına
"kursu bıraktığı waypoint" de yazılıyor, ki bir kazanç alan toplamını değiştirmeden bu virajı
düzeltmiş olabilsin.
**İki kaldıraç denendi, ikisi de çürüdü.**

*Fren.* `NFS_BRAKE` iki yönde süpürüldü ve varsayılan 9 yerel optimum çıktı: 3 ve 5'te alan çöküyor
(kapsama 48), 7'de 166, **9'da 178**, 12'de 121, 18 ve üstünde yine 48. Daha çok fren de daha az
fren de kötüleştiriyor — yani viraj kaybı bir **giriş hızı sorunu değil**.

*Direksiyon hızı.* `self.steer += (want − self.steer) * 0.35` sabiti hiç süpürülmemişti ve
hipotez sağlamdı: 65 km/h'de 50°'lik virajda geciken direksiyon zaten "geniş almak" demek. Tek
rotada da öyle göründü — 0,5 ile 4121'de kursu bırakan araba **6/8'den 3/8'e**, hiç bırakmayan
1'den 3'e.

Alan genelinde tutmadı. Sekiz rotada 0,5 → **773** kapsanan (869'a karşı) ve 1.149 ayrık düğüm
(1.364'e karşı), üstelik düzeltmesi beklenen viraj ölçütü neredeyse sabit: 64 arabanın 53'ü iki
ayarda da kursu bırakıyor, hiç bırakmayan 11'e karşı 12. **Tek rotadaki iki kat iyileşme rotanın
kendisiydi, kuralın değil.** Süpürmenin var olma sebebi tam bu.

Geriye pilotun kolları arasında `CORNER_LIFT`'in büyüklüğü kaldı (rampalaması ayrıca çürütülmüştü)
ve bakılmamış olan **nişan mesafesi**: pilot 18-20 m ötesine nişan alıyor, ki 18 m/s'de bir saniyelik
öngörü eder — 50°'lik bir viraj için kısa olabilir. Sıradaki tur oraya bakmalı.
**Üçüncü çürütme: nişan mesafesi de değil — ve ön eleme ikinci kez yanılttı.** `LOOKAHEAD_PER_SPEED`
(kaç saniyelik yol kadar öne bakılacağı) hiç süpürülmemişti. Dört rotalık elemede eğri temiz ve tek
tepeliydi — 0,6 → 511, 0,9 → 528, 1,3 → 558, **1,8 → 587**, 2,5 → 551, 5,0 → 547 — yani varsayılana
göre %11.

Sekiz rotada düştü: 1,5 → **724**, 1,8 → **858**, varsayılan **869** (ayrık düğüm 1.122 / 1.276 /
1.364).

Aritmetik, elemenin neden yanılttığını tam söylüyor: 1,8 elenen dört rotada (4001, 4121, 4041, 4102)
gerçekten **+59** kazanıyor, elenmeyen dörtte **−70** kaybediyor. Gürültü değil — **alt küme temsil
etmiyormuş.** Ve bu, üst üste ikinci kez oldu (önce direksiyon hızı, şimdi bu). Yarım süpürme,
süpürmenin ucuz sürümü değil; **farklı ve güvenilmez bir alet.** Bundan sonra karar yalnız sekiz
rotadan çıkacak.

**Viraj cephesinde bilanço: üç aday, üç çürütme, kazanç yok.** Fren, direksiyon hızı ve nişan
mesafesi elendi. Alan 869'da duruyor. Geriye `CORNER_LIFT`'in büyüklüğü kalıyor — ve artık şunu da
biliyoruz: bu virajı düzeltmek alan toplamını yükseltmiyor, çünkü üç ayarın hiçbirinde iki ölçüt
birlikte iyileşmedi.
### Oyuncu "yolda binalara çarpıyorum" dedi, ve ölçüm onu doğruladı

Üç pilot sabiti üst üste çürüdükten sonra gelen bu gözlem, aramayı bambaşka bir yere taşıdı:
**belki sorun sürücüde değil, kursun kendisinde.**

Yeni alet (`NFS_BLOCKED=1`) yarış hattının kendisini yürüyor: koridorun içindeki her **komşu** düğüm
çifti için (uzun kestirme bağlantılar hariç — yol kıvrıldığı için onların düz çizgisi doğal olarak
bina keser) önü kesen geometri var mı diye soruyor, ve engeli **yüksekliğine göre** ayırıyor.

| rota | engelli kenar | 1,5 m'de açılan | 3 m'de açılan | **3 m'de bile kapalı** |
|---|---|---|---|---|
| 4001 | 113 / 459 (%24,6) | 40 | 43 | **30** |
| 4002 | 67 / 476 (%14,1) | 18 | 13 | **36** |
| 4121 | 12 / 356 (%3,4) | 6 | 3 | **3** |
| 4041 | 31 / 593 (%5,2) | — | — | — |

Yani yolun kendi segmentlerinin **%6-8'inde** üç metrelik kaldırmanın açmadığı bir şey duruyor.
Bordür ya da rampa değil.

**Bu, üç turdur yaptığım şeyin zeminini kaydırıyor.** Pilot sabitlerini, segmentlerinin bir kısmı
fiziksel olarak kapalı olan bir kursa karşı süpürüyormuşum. Bir sürücü, önü duvarla kesilmiş bir
yolu hiçbir fren/direksiyon/nişan ayarıyla süremez — ve üç ayarın da "iki ölçütü birlikte
iyileştirememesi" bununla tutarlı.

**Açık kalan çekince, ve sınaması bir sonraki turun işi:** şehir çok katlı (tek bir noktada üç
yüzey ölçülmüştü) ve `Walls::across` zemini takip ederken yanlış kata atlayıp *altındaki* katı engel
sayabilir. Yani bu oranların bir kısmı köprü/alt geçit olabilir. Ayırmanın yolu belli: engelli
segmentlerin altında kaç yüzey var, ve engel gerçekten kursun kotunda mı.
### Pilotun saati dört kat hızlı koşuyormuş — ve kanıtı sabahtan beri bu dosyadaydı

Çok mercekli bir inceleme, üç turdur göremediğim şeyi buldu. `Pilot::drive` kendi zamanlayıcılarını
`TICK = 1/60` ile ilerletiyordu ve belgesi *"kare başına bir kez çağrılır ve fizik altmışta koşar,
yani önemli olan yerde doğru"* diyordu. Oysa `nfs_sim`, `drive`'ı **240 Hz'lik fizik döngüsünün
içinde, kapısız** çağırıyor. Yani bu dosyadaki her zamanlayıcı **gerçek zamanın dört katı** hızda
doluyordu: `STALL_FOR`'un 1,5 saniyesi 0,375'te, `BACK_FOR`'un 1,2'si 0,3'te, `ESCAPE_FOR`'un 3,5'i
0,875'te.

**Kanıt bu dosyanın kendi kayıtlarındaydı ve okumadım.** 4002 için *"90 saniyenin 35'i geri viteste,
115 kaçış"* yazmışım. 35/115 = **0,304 s** geri vites başına — `BACK_FOR`'un tam dörtte biri. Üstelik
varsayılan hızda bir takılma→geri döngüsü en az `STALL_FOR + BACK_FOR` = 2,7 saniye sürer, yani 90
saniyeye **33** tane sığar; ben 115 ölçmüşüm. Aynı şekilde *"126 kaçış"* × 3,5 s = **441 saniye**,
90 saniyelik bir yarışın içinde. Üç sayı da yalnız 4× ile açıklanabiliyordu.

**Düzeltme:** `drive` artık `dt` alıyor ve çağıranlar kendi adımlarını veriyor. Ölçüm hemen
oturdu — geri vites süresi 19,2 s / 16 kaçış = **1,200 s**, yani `BACK_FOR`'un kendisi.

**Ama alan toplamı değişmedi: 865 (bozuk saatte 869), düğüm 1.354 (1.364), kursu bırakan 51/64
(53/64).** Yani 4× saat platonun sebebi değil. Temiz bir olumsuz sonuç.

Buna karşılık üç şey kazanıldı ve üçü de sayıdan bağımsız:

1. **Teşhisler artık okunabilir.** "126 kaçış" gibi imkânsız sayılar üretmiyor.
2. **Simülasyon ile oyun ilk kez aynı şeyi ölçüyor.** `nfs_cruise` pilotu kare hızında (100-126 fps)
   çağırıp 1/60 sayıyordu, yani oyun da ~1,7-2,1× hızlıydı — farklı bir kat. Süpürme sonuçları
   oyuna aktarılamıyordu.
3. **Zaman sabitleri artık ne dediklerini yapıyor** — ve bu, açık bir soru bırakıyor: `STALL_FOR`,
   `BACK_FOR`, `ESCAPE_FOR`, `SETTLE` yıllardır *çeyreklenmiş* değerleriyle ayarlanmıştı. Şimdi
   dürüst olduklarına göre dört kat uzun olabilirler. Yeniden süpürülmeleri gerekiyor.
**Düzeltme: o engel taraması sonuç vermiyor, ve "doğrulandı" demek fazla iddialıydı.**

Çekinceyi sınamak için iki ölçüm daha yaptım ve ikisi de kendi bulgumun aleyhine çıktı.

*Birincisi.* 3 m'de bile kapalı kalan segmentlerin ezici çoğunluğu **çok katlı yerde**: 4002'de
36'nın **32'si**, 4121'de 3'ün 3'ü. `Walls::across` yürürken zemini takip ediyor ve her adımda
"önceki yüksekliğe en yakın" yüzeyi seçiyor — yani üst üste binmiş katların olduğu yerde köprüye
çıkıp altındaki yolu "engel" sayabilir. Taramanın vuruşlarının çoğu tam o bölgelerde.

*İkincisi, ve bu benim ikinci ölçümümü de çürüttü.* "Kursun kotu zeminden 2 m uzak" diye
raporladığım sayı yanlıştı: düğümler zeminle **birebir** uyuşuyor (dört rotada en kötü sapma
**0,0 m**, yalnız iki düğümün altında hiç zemin yok). Ölçtüğüm şey düğümler değil, iki düğüm
arasına çektiğim **düz çizgiydi** — yol kot olarak kıvrıldığı için o çizgi zeminden ayrılıyor,
oysa `across` zemini takip ediyor. Kendi lerp'imi ölçmüşüm.

*Ve elimde zaten aleyhte bir veri vardı.* Yolun %6-8'i gerçekten kapalı olsaydı arabalar o
segmentlerde dağınık biçimde tıkanırdı. Oysa ölçtüğüm şey bunun tersi: 4121'de sekiz arabanın
**altısı tek bir dört metrelik noktada** kursu bırakıyor, 6-8'e yayılmış onlarca yerde değil.

**Durum: sonuçsuz, doğrulanmış değil.** Oyuncunun "yolda binalara çarpıyorum" gözlemi kendi
başına geçerli ve ciddi; çürüyen şey benim onu kanıt diye sunduğum ölçüm. Kesin cevap, engelin
kendisini görmeyi gerektiriyor: `across` bir boolean döndürüyor, hangi üçgene çarptığını değil.
Onu döndürmek — çarpan üçgenin yükseklik aralığını ve yola göre kotunu — bu soruyu tek seferde
kapatır, ve sıradaki turun işi odur.

### `across` artık ne çarptığını söylüyor — ve tarama ilk kez ayırıyor (2026-08-20)

Bir önceki turun bıraktığı iş buydu. `Walls::across_hit` artık bir `Hit` döndürüyor: çarpan üçgen,
çarpma noktası, yürüyüşün o adımda inandığı zemin, ilk **zemin bulduğu** yükseklik, zemin bulunan
iki ardışık örnek arasındaki en büyük kat değişimi, o adımın altındaki yüzey sayısı, ve **aynı adım
düz taşınsaydı yine bir şeye çarpar mıydı**. `across` duruyor, aynı yürüyüşün `is_some()`'ı.

**Üç eleme, ve neden o sabitler.** Bir vuruş, şu üçünden biri olduğunda yürüyüşün kendi işi sayılıp
atılıyor: adım düz taşındığında indeksteki *hiçbir şeye* değmiyorsa (`!flat_too`); tek adımda kat
**2,5 m**'den fazla değiştiyse (üç metrelik bir adımda bu 40°'lik bir yüzey demek, yol değil); ya da
yürüyüşün zemini kursun kendi kotundan **3 m**'den uzaksa. Kursun kotu düğümlerden okunuyor, çünkü
düğümler zeminle birebir oturuyor — aynı taramanın başında basılan sayı, dört rotada en kötü
sapma 0,0 m. Elemeden geçen vuruşlar sonra araca göre ayrılıyor: tepesi **0,4 m** altında kalan
bordür, en altı **1,3 m**'nin üstünde kalan (240SX'in tavanı) arabanın altından geçen şey.

**Ölçüm** — sekiz rota, `NFS_BLOCKED=1`, motor pini `58dc2623`. Kenar sayıları geri çekilen
taramanınkiyle **birebir aynı** (459 / 476 / 356 / 593), yani karşılaştırma dürüst:

| rota | kenar | ilk vuruş | elenen vuruş | temiz çıktı | devamda bulundu | bordür | üstünden | **DUVAR** | çok katlı | eksende |
|---|---|---|---|---|---|---|---|---|---|---|
| 4001 | 459 | 113 | 59 | 42 | 2 | 6 | 8 | **57** | 12 | 18 |
| 4002 | 476 | 67 | 48 | 32 | 2 | 0 | 0 | **35** | 27 | 23 |
| 4021 | 256 | 20 | 8 | 8 | 0 | 2 | 0 | **10** | 6 | 10 |
| 4041 | 593 | 31 | 11 | 9 | 0 | 15 | 0 | **7** | 0 | 6 |
| 4061 | 52 | 0 | 0 | 0 | 0 | 0 | 0 | **0** | 0 | 0 |
| 4081 | 343 | 25 | 1 | 1 | 0 | 1 | 0 | **23** | 21 | 19 |
| 4102 | 289 | 1 | 0 | 0 | 0 | 1 | 0 | **0** | 0 | 0 |
| 4121 | 356 | 12 | 6 | 6 | 0 | 0 | 0 | **6** | 5 | 5 |
| **TOPLAM** | **2.824** | **269** | **133** | **98** | **4** | **25** | **8** | **138** | **71** | **81** |

("eksende" = çarpma noktası yol ekseninden 6 m'den yakın.)

**Üç sonuç.**

1. **Geri çekilen bulgunun gerekçesi fazla kabaymış.** "Vuruşların çoğu çok katlı yerde" doğruydu,
   ama eleme sebebi değil: ayakta kalan 138 duvarın **71'i** çok katlı yerde. Çok katlılık şüphe
   sebebiydi; ayıran şey yürüyüşün kat değiştirip değiştirmediği, ve o artık ayrı ayrı ölçülüyor.
2. **Oyuncunun gözlemi ilk kez ölçülmüş bir desteğe kavuştu.** Yarış hattının kenarlarının
   **%4,9'unda** (138/2.824) yolun kendi kotunda, arabanın çarpacağı yükseklikte duran geometri
   var; **81'i** yol ekseninin 6 m'sinde, yani kirişle açıklanamaz. Bu, oyuncunun çarptığı şeyin
   *bunlar* olduğunu kanıtlamıyor — ama "yolda bina var" ifadesinin karşılığı sahiplerde duruyor.
3. **Ve 4121'in virajını açıklamıyor.** O rotada sekiz arabanın yedisi waypoint 6'da,
   `(-334, 1374)` civarında dört metrelik bir noktada kursu bırakıyor. Rotanın altı duvarının en
   yakını oraya **225 m** uzakta. Yani bu tarama viraj sorusunu kapatmıyor, bir adayı daha eliyor:
   orada duran bir şey yok.

**Ölçümün kendi ilk hâlinde iki hata vardı, ikisi de sayıyı değiştirdi** — ve ikisi de tarama
koşulmadan önce, kodun kendisine bakan çok mercekli bir incelemede çıktı.

- **`flat_too` yalnız çarpılan üçgeni yeniden soruyordu.** Bir bariyer tek üçgen değil, ve yürüyüş
  hücre sırasındaki *ilk* üçgeni döndürüyor. Alt yoldaki yüzey önce indekslenmişse, düz adım onu
  ıskalıyor ve üst katta yolun tam karşısına dikilmiş duvar "kat değiştirme" diye atılıyordu.
  Artık indeksin tamamı soruluyor. 4001'de duvar sayısı **37 → 57**, `!flat_too` elemesi **47 → 17**.
- **Elenen bir vuruş kenarı temize çıkarmaz.** `across_hit` çarptığı ilk şeyde dönüyor ve
  yürüyüşün geri kalanı hiç koşmuyor; vuruşu atan bir çağıran yolun açık olduğunu değil, yalnız
  *o* engelin gerçek olmadığını öğrenmiş oluyor. Tarama artık vuruşun 1,5 m ötesinden devam
  ediyor. Sekiz rotada 98 kenar bu şekilde temize çıktı ve **4 kenarda gerçek engel ancak devam
  edilince bulundu**.

Yürüyüşte iki düzeltme daha var, ikisi de teste bağlandı: zemin bulunmayan adımlar artık `climb`
ve `start_y`'ye karışmıyor (delik kat değişimi değildir), ve yatay uzanımı olmayan bir sorgu düz
sayılıyor (eskiden dejenere segment "hiç değmedi" cevabı veriyordu).

**Bunu tekrar yanlış yapacak şey, ve çıktıda ilk bakılacak yer.** Kalan tek büyük alternatif
açıklama **kiriş**: tarama düğümden düğüme düz gidiyor, yol ise arada kıvrılıyor, ve virajın
dışındaki bina hiçbir arabanın sürmeyeceği bir çizginin önünde duruyor. Onun için her duvarın yol
eksenine uzaklığı basılıyor — 138'in **57'si** 6 m'den uzakta ve 4001'in 57 duvarının 39'u öyle.
4001'in yüksek oranını kanıt diye kullanmadan önce oraya bakılmalı. İkinci sınırlama: koridor
uzaklığı plan görünümünde ölçülüyor, yani üst geçidin altındaki bir duvar da "eksende" görünebilir;
`zemin N kat` sütunu o satırlar için uyarı işaretidir.

### 4121'in virajı: fren geç kalmıyor, hiç gelmiyor — ve iz bunu tek koşuda söyledi (2026-08-20)

Aynı gün engel taraması virajda duran bir şey olmadığını gösterdikten sonra (en yakın duvar 225 m),
geriye kalan tek yer pilottu. Üç kol o virajda süpürülüp çürütülmüştü — fren eşiği, direksiyon
hızı, nişan mesafesi — ve dördüncüyü körlemesine süpürmek yerine `NFS_LOST=1` yazıldı: her arabanın
**kursu bıraktığı anın çevresindeki sekiz saniye**, 20 Hz'de, pilotun kendi terimleriyle.

**Altı araba aynı noktada, aynı sayılarla çıkıyor.** `(-284, 1383)`, düğüm 276, **66 km/h**,
nişan 30 m ileride **−48°**, direksiyon **0,45**, gaz 0,66 ve **fren 0,00**. Bu sayılar tesadüf
değil, kuralın kendisi: `over = (hız / 9) · |direksiyon| = (18,3/9) · 0,45 = **0,92**`, eşik ise
`1,0`. Yani 48°'lik bir viraja 66 km/h ile giriliyor ve **fren eşiğin %8 altında kalıyor**.

**Ama asıl bulgu büyüklük değil, öncülük.** Aynı izden türetilen fiziksel büyüklük — saf takipte
nişan yayının istediği yanal ivme, `2·v²·sin|açı| / L` — arabanın kursu bırakmasından çok önce
lastiğin verebileceğinin üstüne çıkıyor:

| t | koridora | hız | nişan | `over` | gereken yanal |
|---|---|---|---|---|---|
| 37,3 | 3,0 m | 64 km/h | 31 m, −28° | 0,51 | **9,6 m/s²** |
| 37,8 | 0,7 m | 65 km/h | 24 m, −27° | 0,52 | **12,3 m/s²** |
| 38,3 | 4,2 m | 66 km/h | 16 m, −31° | 0,61 | **21,6 m/s²** |
| 39,8 | **12,1 m** | 66 km/h | 28 m, −45° | 0,86 | 17,0 m/s² |
| 40,8 | 17,7 m | 63 km/h | 18 m, −68° | **1,24** | 31,6 m/s² |

Gereken ivme **t=37,3'te** zaten 9,6 m/s² — bir yol arabasının tutamayacağı değer — ve fren ancak
**t=40,8'de**, araba koridorun 17,7 m dışındayken geliyor. Arada **3,5 saniye** ve yaklaşık 60 m
var. Mevcut kural arabanın *şu an* ne kadar döndüğüne bakıyor; virajın kendisi nişan noktasında,
30 m ileride, saniyelerce önce görünüyor ve o bilgi atılıyor.

Bu, `NFS_BRAKE` süpürmesinin sekiz rotada neden kaybettiğini de açıklıyor: eşiği düşürmek **her
yerde** fren yapar, oysa eksik olan şey "daha sert" değil "daha erken".

**İkinci ve ayrı bir arıza da izde görünüyor.** Araba 5 ve araba 2'de nişan noktası bir düğümde
**180° dönüyor**: düğüme varılıyor (nişan 0 m), bir sonraki örnekte nişan **55 m, 177°** — yani
arkada. Pilot anında tam kilit (−0,85) ve tam fren istiyor, tuttuğu düğüm 111'de çakılı kalıyor
(mesafe 0 → 31 m büyürken düğüm değişmiyor) ve araba tam kilitle kursun dışına kayıyor. Bu,
`pilot.rs`'in "arkadaki nişan noktası nişan noktası değildir" notunda tarif edilen arızanın ta
kendisi; düzeltme "yürüyüş nişan arkadayken de devam etsin" idi ve burada yürüyüş 55 m devam edip
yine arkayı gösteriyor — yani ağ yürüyüşü geriye gidiyor. Araba 0'ın izinde t=34,8'de aynı şey bir
kez daha var (nişan 144 m, −160°), o seferinde toparlanıyor.

**Durum:** viraj sorusu artık "arabalar neden açılıyor" değil, **"fren neden virajı görmeden
bekliyor"**. Sıradaki iş bu iki arızayı ayrı ayrı ele almak; ikisi de ölçülebilir ve ikisi de
sekiz-rota süpürmesiyle yargılanacak.

### Fren virajı nişan yayından görüyor artık — ve bu, çürütülmeyen ilk viraj kolu (2026-08-20)

İzin söylediği şey açıktı: eski fren kuralı arabanın *şu an* ne kadar döndüğüne bakıyor, direksiyon
ise viraj gelene kadar dönmüyor. Yeni terim aynı nişan noktasının **yayını** kullanıyor. Saf takip
`L / (2 sin θ)` yarıçaplı bir yay izler, dolayısıyla o çizgiyi o hızda tutmanın bedeli
`2·v²·sin θ / L` — m/s² cinsinden, yani ayarlanmış bir orana değil **lastiğin verebileceğine**
karşı kıyaslanabilir bir sayı.

**Sekiz rota, dört kol.** Karar veren ölçü "waypoints driven past": şişirilemeyen tek sayı.

| grip | waypoint | distinct nodes | kursu hiç bırakmayan | fallen | junctions |
|---|---|---|---|---|---|
| kapalı | 883 | 1.352 | 9 / 64 | 3 | 1.445 |
| 6 | 602 | 953 | **22 / 64** | 1 | 1.024 |
| **8** | **927** | 1.280 | 14 / 64 | 2 | 1.349 |
| 12 | 815 | 1.192 | 13 / 64 | 1 | 1.287 |

Sekiz, kural kapalıyken alınan alanı geçen **tek** değer ve eğri onun etrafında tek tepeli.

**Altı, tuzağın kendisi.** 64 arabanın 22'sini kursta tutuyor — bugüne kadarki en iyi rakam — ve
kolların en kötüsü. Çünkü o arabalar kursta *sürünerek* kalıyor: kursta kalan araba başına **10,2**
waypoint, sekizde 20,1 ve kural kapalıyken 18,8. Arabayı yolda tutmak hedef değil; yoldan
geçirmek hedef.

**Kaydın dürüst kısmı.** Rota rota sekiz, dördünde kazanıyor (4001 +95, 4041 +26, 4102 +18,
4121 +11) ve üçünde kaybediyor (4061 −48, 4021 −34, 4081 −24). Yani +44'lük net, tek bir rotanın
sırtında ve bunu okuyan biri ona güvenmemekte haklıdır.

Junctions (−%6,6) ve distinct nodes (−%5,3) de düşüyor. Bunun kayıp ilerleme **olmadığı**
ölçüldü: ikisini de "araba kursu bıraktı mı" diye ayırınca, **kursta kalanlar 9 arabadan 187 düğüm
ve 169 waypoint'ten, 14 arabadan 279 düğüm ve 282 waypoint'e** çıkıyor; kaybolan nüfus ise 55'ten
50'ye iniyor. Düşüşün tamamı, grafta dolanan daha az kaybolmuş arabadan geliyor — `nfs_sim`'in
kendi notunun söylediği şişme ("takla atmış bir araba kavşak saymaya devam eder"). Junctions /
distinct nodes oranı yerinde duruyor: 1,069 → 1,054.

**4121'i çözmüyor.** Sekiz arabanın yedisi hâlâ waypoint 6'da çıkıyor — ama virajın **19 m
ilerisinde ve 66 yerine 50 km/h ile**, ve biri artık hiç bırakmıyor. İzden okunan kalan eksik
direksiyonda: −54°'de pilot kilidin yalnız 0,50'sini istiyor, çünkü açı→kilit haritası yayın
kendi geometrisi değil düz bir rampa (`(açı·2/π)·STEER_LIMIT`). Saf takibin kendi formülü
`δ = atan(2·L_dingil·sin θ / L)`; sıradaki aday o, ve o da sekiz rotayla yargılanacak.

### Alanın en büyük tek kaybı viraj değilmiş: arkasındaki bir noktaya direksiyon kırmak (2026-08-20)

`NFS_LOST=1` sekiz rotanın hepsine koşuldu. Kursu bırakan **50 arabanın 28'i (%56)**, bırakmadan
önceki saniyelerde yarım saniyeden uzun süre **arkasındaki** bir noktaya direksiyon kırıyor: nişan
açısı |θ| > 120°. Neredeyse hepsi *yol ekseninin üstünde* başlıyor — koridora 0,4-1,0 m, 43-88 km/h
— ve direksiyon tam kilide yakın çakılıyken 20-50 m dışarıda bitiyor. `Paths4102`'de bunu **altı
arabanın altısı** bir saniye içinde yapıyor.

**Anatomi, 4102 araba 0.** Sayılar izin kendisinden:

| t | konum | hız | koridora | direksiyon | nişan | hedef waypoint |
|---|---|---|---|---|---|---|
| 27,0 | (−39, −138) | 79 | 1,1 m | 0,02 | 67 m, −2° | 11 @ 46 m, **−61°** |
| 28,6 | (−19, −107) | 86 | 0,6 m | 0,01 | 30 m, −1° | 11 @ 42 m, **−108°** |
| 29,8 | (−2, −83) | 88 | 0,1 m | 0,04 | **1 m**, −5° | 11 @ 58 m, −136° |
| 30,2 | (3, −75) | 83 | 1,5 m | **0,85** | 9 m, **−179°** | 11 @ 65 m, −140° |
| 32,6 | (31, −42) | 50 | **22,6 m** | 0,85 | 52 m, −173° | 12 @ 89 m, −127° |
| 35,0 | (55, −23) | 45 | **49,1 m** | 0,82 | 190 m, −87° | 13 @ 99 m, −112° |

Araba hattın tam üstünde, saatte 88 km ile ve dümdüz gidiyor. Hedef waypoint 11 ise yolun
**40 m yanından** geçiyor ve arkaya süpürülüyor (−61° → −136°) — ilerlemiyor, çünkü ilerletme kuralı
"bir sonraki daha yakın mı" diye soruyor ve araba ikisine de yaklaşmıyor. Sonra tutulan düğüm
207'ye atlıyor, nişan **9 m, −179°** oluyor ve pilot tam kilit + tam fren istiyor. Fren zaten
sonuna kadar basılı (eski `over` kuralı hızda tam kilidi görüp her şeyi istiyor); arabayı kurstan
çıkaran şey **fren değil kilit** — 88'den 45 km/h'ye inerken koridoru 1,5 m'den 49 m'ye yana
sürterek geçiyor.

**Kök sebep ağ yürüyüşünde:** greedy adım (`step_avoiding`) hedefe *düz mesafede* en yakın komşuyu
seçiyor; hedef yana ve arkaya düştüğünde bu, arabayı geldiği yöne çeviren kolu seçmek demek. Nişan
yürüyüşü de sadık biçimde geriye yürüyor. Yani kusur pilotun cevabında değil, cevabın **fiziksel
olarak imkânsız** olmasında: 88 km/h'deki bir araba dönemez.

**Arabanın gerçek yanal kapasitesi ölçüldü** — ve bu, dünkü `GRIP = 8`'in ne olduğunu da söylüyor.
İzdeki konumlar tam hassasiyetle saklandığı için `v·dψ/dt` doğrudan hesaplanabiliyor: 4121'de
>28 km/h'deki 1.347 adımda **%50: 3,0 · %90: 5,2 · %95: 5,6 m/s²**. Yani `GRIP = 8` arabanın **hiç
ulaşmadığı** bir tavan; fren ancak talep zaten ulaşılamaz olduğunda, yani araba açılmaya başladıktan
sonra devreye giriyor. `grip = 6`'nın 64 arabanın 22'sini kursta tutması da bu yüzden şaşırtıcı
değil — gerçek sınıra yakın tek değer oydu.

**Düzeltme: dünkü "sıradaki aday" yanlıştı.** Commit'te sıradaki iş olarak saf takibin kendi
direksiyon yasasını (`δ = atan(2·L_dingil·sin θ / L)`) yazmıştım. Aritmetiği yapınca ters çıkıyor:
o viraj 21 m yarıçap istiyor ve doğru geometri bunun için kilidin ancak dörtte birini ister —
yani viraj bir **direksiyon** değil **hız** problemi, ve doğru yasa oraya kilit *eklemez*. Aday
geri çekildi.

**Ve bir olumsuz sonuç daha: yolun kendi eğriliği, kirişten daha kötü.** Fren şu an nişan noktasına
çekilen kirişin eğriliğine bakıyor (`2·sin θ / L`) ve o açı iki şeyi birden taşıyor: yolun virajı
**ve** arabanın kendi yönelme hatası. Daha "saf" olanı denendi — yürüyüşün düğüm çoklu-doğrusundan
okunan, arabadan tamamen bağımsız yol eğriliği — ve sekiz rotada kaybediyor:

| kol | waypoint | kursta kalan |
|---|---|---|
| kiriş, GRIP 8 (mevcut) | **927** | 14 |
| yol eğriliği, 5 | 909 | 12 |
| yol eğriliği, 6.5 | 848 | 17 |

Okuma: hatayı da içine katan sayı daha iyi çalışıyor, çünkü zaten hattan açılmış bir arabayı
yavaşlatmak istediğimiz şeydir — "kendi hatan için fren yapma" kulağa doğru gelen ama alanın
reddettiği bir arıtma. Silindi; kayıt `GRIP`'in yanında duruyor.

**Bunun bir yan sonucu var ve `GRIP = 8`'i açıklıyor.** Araba gerçekte 5,2 m/s² tutuyorsa, "fiziksel
olarak doğru" tavan 5-6 olmalıydı — ama ölçümde `grip = 6` kolların en kötüsü. Çelişki değil:
kiriş açısı yolun virajını **abartıyor** (arabanın yönelme hatasını da içerdiği için), dolayısıyla
8 sabiti lastiğin kapasitesi değil, **kapasite bölü abartma katsayısı**. Sabiti arabanın gerçek
sınırına çekmek, ancak eğrilik tahmini de düzeltilirse anlamlı olur — ve o düzeltme (yukarıdaki yol
eğriliği) alanda kaybediyor. İkisi birlikte ayarlanmadıkça 8 kalır.

**Denenmemiş varyant, kayıt için:** yukarıdaki sınır her hızda uygulanıyor, oysa arkadaki noktaya
dönmek *duran* bir araba için doğru manevra — ve pilotun kaçış/geri vites makinesi tam da onu
yapar. Hıza bağlı bir tavan (durmuşken tam kilit, 60 km/h'de neredeyse sıfır) aynı fikrin daha
doğru biçimidir ve ölçülmedi.

**Ve arka nişanın kendi düzeltmesi de çürüdü.** Açık olan şey deneniyordu: 88 km/h'deki araba
dönemeyeceğine göre, hedef arkadayken kilidi sınırla — araba hızını atarken yolda kalsın, dönüşü
sonra yapsın. Sekiz rota:

| kol | waypoint | kursta kalan | dünyadan düşen |
|---|---|---|---|
| sınır yok (mevcut) | **927** | 14 | **2** |
| 0,35 | 728 | 19 | 5 |
| 0,15 | 739 | 16 | 6 |

Gerçekten daha çok arabayı kursta tutuyor, ve bunun bedeli alanın ilerlemesinin beşte biri. Ama
kapatan sütun sonuncusu: **dönemeyen araba kenardan da dönemez**, ve dünyadan düşen araba üçe
katlanıyor. Kısıtlanan manevra, arabayı sınırda kurtaran manevranın ta kendisi.

### Bir örüntü çıktı: "daha az yap" kollarının hepsi aynı takası yapıyor

Bugün üç ayrı kol denendi ve üçü de aynı sınırda duruyor — kursta kalan arabayı artırıyor, waypoint
kaybediyor:

| kol | waypoint | kursta kalan |
|---|---|---|
| kural yok | 883 | 9 |
| **GRIP 8** | **927** | **14** |
| GRIP 6 | 602 | 22 |
| arka kilit 0,35 | 728 | 19 |
| arka kilit 0,15 | 739 | 16 |

`GRIP = 8` bu sınırın **dışında** kalan tek nokta: ikisini birden aldı. Geri kalan her "daha az
yap" hamlesi (daha çok fren, daha az kilit) ilerlemeyi güvenlikle takas ediyor, ve takas oranı
kötü.

**Okuma, ve sıradaki turun yönü:** arka nişan arızası pilotun *cevabında* değil, **rotalamada**
doğuyor. Araba hattın üstünde, düz, 88 km/h giderken hedef waypoint yolun 40 m yanından geçiyor —
yani ağ yürüyüşü arabayı kursun gitmediği bir yola sokmuş durumda, ve nişanın geriye dönmesi bunun
*sonucu*. Cevabı yumuşatmak (fren, kilit) semptomu tedavi ediyor ve alan bunu her seferinde
reddediyor. Kazanç, yanlış yola hiç girmemekte.

### Düzeltme: arka nişanın sebebi rotalama değilmiş — sayaç, arabanın *yanından geçtiği* waypoint'i bırakamıyor (2026-08-20)

Bir önceki bölümde "kazanç yanlış yola hiç girmemekte" yazmıştım: yani ağ yürüyüşünün arabayı kursun
gitmediği bir yola soktuğunu. **Ölçüldü ve yanlış.** `NFS_WRONGWAY=1` her düğüm geçişini izliyor ve
koridorun içindeki bir düğümden dışındaki bir düğüme atılan adımı sayıyor; 4102 ve 4121'de bu sayı
**sıfır**. Yürüyüş kurstan hiç çıkmıyor.

**Gerçek mekanizma, aynı izin bir sütunu daha eklenince görüldü.** İze "bir sonraki waypoint ne
kadar uzakta" konunca 4102 araba 0 şunu diyor:

| t | koridora | hız | tutulan hedef | sonraki |
|---|---|---|---|---|
| 27,0 | 1,1 m | 79 km/h | 11 @ **46 m**, −61° | **81 m** |
| 28,2 | 0,7 m | 84 km/h | 11 @ **40 m**, −95° | **67 m** |
| 29,4 | 0,3 m | 88 km/h | 11 @ 51 m, −128° | 63 m |
| 30,2 | 1,5 m | 83 km/h | 11 @ 65 m, −140° | 67 m | ← tam kilit burada

Araba hattın tam üstünde ve waypoint 11'in **kırk metre yanından** geçiyor. İlerletme kuralının iki
kolu da ölü: tutulan waypoint'e en yakın mesafe **40 m** (eşik [`REACHED`] = 18 m) ve bir sonraki
**hep daha uzak** (81 → 67 → 63 m), yani "sonraki daha yakın mı" hiç doğru olmuyor. Sayaç takılıyor,
hedef −61°'den −140°'ye süpürülüyor, nişan yürüyüşü sadık biçimde onu takip ediyor ve pilot 88
km/h'de tam kilit istiyor.

**Neden kırk metre yanından:** waypoint halkası ile arabanın sürdüğü şerit, aynı rota dosyasının
**farklı hatları**. Koridor bunları birleştiriyor (yarı genişlik 12 m, arabaya "kurstasın" diyor);
waypoint sayacı birleştirmiyor. 4102'de bunu altı arabanın altısı aynı yerde yapıyor, çünkü bu
arabanın değil rotanın geometrisi.

**Denenen kural, ve neden çürütülmüş olanla aynı şey değil.** Yeni kol: tutulan waypoint **arkadaysa**
ve bir sonraki **öndeyse** ilerlet. Eskiden çürütülen "arkadayken ilerlet" bir halkada duramıyordu —
halkadan uzağa bakan araba için arkada koca bir yay kalır, sayaç yayı yürür, sarar, tur sayar (bir
araba için 92 tur ve 5.999 waypoint). Buradaki koruma yapısal: adım **önde** olan bir waypoint'e
inmek zorunda, yani kol ateşlediği anda kendini siliyor — bir sonraki tikte tutulan waypoint önde
olur, `behind` yanlıştır ve araba onu da geçene kadar hiçbir şey olmaz. Arkadaki bir waypoint dizisi
asla yürünemez.

**İlk hâli çürüdü, ve nasıl çürüdüğü kuralın kendisini düzeltti.** "Tutulan waypoint burnun
arkasına düştüyse bırak" sürümü sekiz rotada:

| rota | GRIP 8 | + burnun arkasında | fark |
|---|---|---|---|
| 4001 | 297 | 309 | +12 |
| 4002 | 36 | 36 | 0 |
| 4021 | 76 | 71 | −5 |
| 4041 | 108 | **171** | +63 |
| 4061 | 41 | **76** | +35 |
| 4081 | 80 | 97 | +17 |
| 4102 | 121 | 103 | −18 |
| **4121** | 168 | **48** | **−120** |
| toplam | **927** | 911 | −16 |

Beş rotada kazanıp birinde yıkılıyor — ve yıkılma sebebi öğretici: 4121'de arabalar virajda
**duruyor** (araba başına 6 waypoint, 28 yerine 10 kavşak, 0 km/h). Viraj dışına savrulan bir
arabanın hedefi burnunun arkasına düşer ama araba onu *geçmemiştir*; kural bırakır, yeni hedef
virajın karşısına düşer, araba oraya sürüp takılır. Bu, `REACHED`'in belgesinde zaten yazılı olan
"bırakma değil atlama" hatasının aynısı.

Ayrım kuralın içine yazıldı: waypoint'i **yolun kendi yönünde** geçmiş olmak gerekiyor
(`(araba − w[i]) · (w[i+1] − w[i]) > 0`), burna göre değil — ve yalnızca ona yakınken, çünkü
tuttuğu waypoint'ten uzaktaki araba kaybolmuş arabadır ve bu kolun asla onun için halkayı
yürümemesi gerekir. Turlar kontrol edildi: her iki sürümde de sıfır, yani halka yürüme geri
gelmedi.

**Ve düzeltilmiş hâli alanı geçiyor — bugünün ikinci kalıcı değişikliği.** Sekiz rota:

| kol | waypoint | kursu hiç bırakmayan | düşen | away |
|---|---|---|---|---|
| kural yok | 883 | 9 / 64 | 3 | 64 |
| GRIP 8 | 927 | 14 / 64 | 2 | 64 |
| burnun arkasında | 911 | 16 / 64 | 3 | 64 |
| yolun yönünde, <30 m | 883 | 14 / 64 | 4 | 64 |
| **yolun yönünde, <60 m** | **986** | **24 / 64** | 4 | 64 |
| yolun yönünde, <100 m | 956 | 24 / 64 | 4 | 64 |

Rota rota: dördünde kazanıyor (4001 +25, 4061 +33, 4102 +26, 4021 +1), üçünde kaybediyor
(4041 −1, 4081 −9, 4121 −16), birinde aynı. Net +59 ve tek rotanın sırtında değil — üç ayrı rotaya
yayılmış. `away` 64'te sabit, tur sayısı sıfır (halka yürüme yok).

**Zayıf yeri, açıkça:** dünyadan düşen araba 2 → 4. İki araba, 64'te. Sayaç arabayı daha uzun süre
kursta ve hızlı tuttuğu için kenara daha çok araba ulaşıyor olması makul, ama bu bir tahmin;
`fallen` bir sonraki süpürmelerde izlenecek — bugün zaten `BASELINE-SEKIZ-ROTA.md`'de 4041 için
açılmış bir izleme var.

### Düzeltme tam da hedeflediği nüfusu aldı — ve kalanların nerede durduğu ölçüldü (2026-08-20)

Alan sayısı iyileşti diye mekanizmanın doğrulandığı varsayılmaz. `NFS_LOST=1` sekiz rotaya
düzeltmeden **önce ve sonra** koşuldu, ve kursu bırakan her arabanın koridoru geçtiği andan geriye
iki saniyelik pencerede imzası sınıflandırıldı:

| imza | önce (50 araba bıraktı) | sonra (40 araba) |
|---|---|---|
| **arka nişan** (\|θ\| > 120°) | **23** | **16** |
| tam kilide yakın (\|dir\| > 0,8) | 10 | 7 |
| zaten yavaş (< 25 km/h) | 8 | 8 |
| hiçbiri — düz açılma | 9 | 9 |

Kurtarılan 10 arabanın 7'si arka-nişan grubundan, 3'ü tam-kilit grubundan geldi; **diğer iki
kategori sayıca birebir aynı kaldı**. Yani değişiklik yalnızca hedeflediği arızayı kaldırdı, alanı
genel olarak yavaşlatıp "daha az araba çıksın" diye takas yapmadı — bugün üç kez çürütülen desen
tam olarak buydu.

**Kalan 16 arka-nişan vakası eşiğin hemen dışında duruyor.** Vakanın ilk anındaki hedef uzaklığı
**medyan 66 m**, ve 16'nın yalnız 2'si 60 m'nin altında. Yedisi tek bir rotada (4102) ve tek bir
waypoint'te: alan artık waypoint 11'i geçiyor (düzelen kısım) ama waypoint 12'de 65-66 m'de
takılıyor, 88-91 km/h ile. Yani eşiğin kendisi, kalan arızanın tam sınırında.

**Düşen araba artışı (2 → 4) da yerelleşti:** ikisi 4021'de, biri 4102'de, ve düşen arabaların
tuttukları düğümden sapmaları 348-1009 m. Bunlar kursu çoktan bırakmış, uzağa gitmiş ve dünyanın
kenarını bulmuş arabalar; yeni bir arıza değil, daha uzağa gidebilen bir alan.

**Ve eşiği genişletmek onları kurtarmıyor.** Kalanların medyanı 66 m olduğu için sınırı 80 m'ye
çekmek doğal görünüyordu; sekiz rotada 973 waypoint (60 m'de 986) ve asıl mesele, **o yedi vakanın
bulunduğu 4102'de sayı 147'den 126'ya düşüyor**. 100 m'de de 126. Yani o arabaları tutan şey sınır
değil: takıldıkları waypoint, bırakılması bir bedeli olan bir waypoint.

| sınır | waypoint | kursu hiç bırakmayan | düşen |
|---|---|---|---|
| kapalı | 927 | 14 / 64 | 2 |
| 30 m | 883 | 14 / 64 | 4 |
| **60 m** | **986** | **24 / 64** | 4 |
| 80 m | 973 | 23 / 64 | 3 |
| 100 m | 956 | 24 / 64 | 4 |

**Sıradaki turun soruları, artık tek bir baskın sebep olmadığı için üç tane:** kalan 16 arka nişan
(sınır değilse ne?), 8 "zaten yavaş" araba (durma/kaçış makinesi), ve 9 "düz açılma" (viraj, hâlâ).

### Kalan on altı vaka aynı arıza, ve kapıyı iyileştirmenin iki yolu da çürüdü (2026-08-20)

Kalan 16 arka-nişan vakasının **16'sında da hedef takılı** — bir öncekiyle aynı mekanizma, sadece
eşiğin dışındaki mesafelerde. Uzaklığı bileşenlerine ayırınca iki ayrı şekil çıkıyor:

| rota | hedef uzaklığı | yanal | boyuna (geride) |
|---|---|---|---|
| 4102 | 66 m | **64 m** | 17 m |
| 4041 | 76 m | 71 m | 26 m |
| 4121 | 85 m | 65 m | 55 m |
| 4021 | 49 m | 44 m | 21 m |
| 4081 | 119 m | 6 m | **119 m** |

Yani çoğunda araba waypoint'i **yalnız 17-26 m boyunca geçmiş ama 44-71 m yanında** — halka komşu
şeritte. 4081'inki bambaşka: 6 m yanda, 119 m geride; dönmüş bir araba.

"Toplam mesafe bu ikisini ayıramıyor, **boyuna aşım** ayırır" diye düşünmek doğal, ve denendi:
serbest bırakmayı `0 < aşım < 40 m` ile kapılamak. 4021'de +27, 4041'de +43 kazandırıyor — ve
**4121'de 152 → 48**, 4102'de −21, toplamda 927 (mevcut kural 986). Mesafe sınırının erdemi, kusuru
sandığım şeymiş: **yanal olarak uzak bir waypoint'i reddediyor**, ve 4121'in virajında dışarı
savrulan araba waypoint'ini bir iki metre geçmiş ama on metrelerce yanındadır. Orada bırakmak, rotayı
boşaltan atlamanın ta kendisi.

| kapı | waypoint | kursta kalan | düşen |
|---|---|---|---|
| kural yok | 927 | 14 | 2 |
| **mesafe < 60 m** | **986** | **24** | 4 |
| mesafe < 80 m | 973 | 23 | 3 |
| boyuna aşım < 40 m | 927 | 23 | 2 |

İki bağımsız rota kuralı iki yandan sıkıştırıyor: **4121 fazla istekli her bırakmayı cezalandırıyor,
4102 ise kendi waypoint 12'sini bırakmayı.** 60 m ikisini birden sağlayan nokta, ve iki farklı
genişletme denemesi de birinde ya da diğerinde yıkıldı. Kalan 16 vaka bu kuralla çözülmüyor —
çözümü kapıda değil, halkanın arabanın sürdüğü şeritle uyuşmamasında aramak gerekiyor.

### "Kursu bıraktı" sayacı, kursa hiç girmemiş arabaları da sayıyormuş (2026-08-20)

Kalan üç sorudan ikincisi — kursu bırakırken **zaten sürünen 8 araba** — ize bakılınca soru olmaktan
çıktı. Sekizinin sekizi de tek bir örnekten ibaret: **t = 0,0, hız 0 km/h, koridora 12,3-19,4 m.**
Yani bunlar kursu bırakmadı; **hiç girmediler.** Gridleri koridorun dışında.

`Paths4002`'nin çıkış çizgisi en yakın waypoint'ten **21 m** uzakta (dosyanın kendi çıktısı:
*"waypoint 0 is 785 m from the grid · nearest is #87 at 21 m"*), ve sekiz arabanın altısı 12,3-19,4 m
dışarıda başlıyor. Kalıcı ayrılma kuralı "üç saniye koridorun dışında" olduğu için altısı da
t = 3'te, daha hiçbir yere sürmeden "kursu bıraktı" sayılıyordu. `Paths4001`'de aynı şeyden iki
araba var.

**Düzeltme:** bir araba, koridorun içine **en az bir kez girmeden** onu bırakmış sayılamaz. Üçüncü
bir durum eklendi (`kursa hiç girmedi`) ve sayaç ona göre kapılandı.

**Sonuç, ve manşet ölçütü etkiliyor:** düzeltilmiş sayımda 4001 ve 4002'de sekiz arabanın **sekizi
de** kursu hiç bırakmıyor (eskiden 6 ve 2 sayılıyordu). Alan sekiz rotada:

| kol | waypoint | kursu hiç bırakmadı | bıraktı |
|---|---|---|---|
| kural yok (eski sayım) | 927 | 14 / 64 | 50 |
| **kural yok (düzeltilmiş)** | 927 | **22 / 64** | 42 |
| `PASSED_NEAR` 60 (eski sayım) | 986 | 24 / 64 | 40 |
| **`PASSED_NEAR` 60 (düzeltilmiş)** | **986** | **32 / 64** | 32 |

waypoint toplamı değişmiyor (986) — bu bir raporlama düzeltmesi, sürüşe dokunmuyor.

**Bugünkü kayıtlar için ne demek:** "kursu hiç bırakmayan araba" sayısı gün boyu **eksik**
raporlandı, ve eksiklik iki rotanın gridinden geliyor. Kollar arası **farklar** geçerli (aynı grid
her kolda aynı), ama mutlak sayılar düşük. Bugünün kolları düzeltilmiş sayaçla yeniden koşuldu ve
sayılar aşağıda yerine kondu.

### 4121'in "virajı" bir viraj değilmiş: arabalar 250 m önce yanlış kola sokuluyor (2026-08-20)

Üç açık sorunun sonuncusu — düpedüz açılan arabalar — kapandı, ve cevap bugüne kadarki bütün viraj
çalışmasının zeminini kaydırıyor.

**Önce yolun kendi hız sınırı ölçüldü** (`NFS_CURVE=1`): ardışık üç waypoint'ten geçen çemberin
yarıçapı, ve `v = √(a·r)` ile alanın gerçekten tuttuğu 5,2 m/s²'ye karşılık gelen hız. 4121'in en dar
yerleri **22-28 m yarıçap, yani 39-43 km/h**. Ama arabaların çıktığı yer bunların hiçbiri değil.

**Sonra "waypoint 6" ifadesinin bir konum olmadığı fark edildi.** Rapor satırındaki sayı, arabanın o
ana kadar **geçtiği waypoint sayısı**; indeks değil. Gerçek yer `(-334, 1374)` ve orada — `NFS_CURVE=x,z`
ile sorulunca — **80 m yarıçapında tek bir waypoint bile yok.** Yarış hattı oradan geçmiyor.

**Ve kapatan ölçüm.** `NFS_WRONGWAY`'in ilk hâli düğümü *koridora* karşı soruyordu; koridor rota
dosyasının bütün hatlarını birleştirdiği için, yarıştan 150 m uzaktaki bir şeritte giden araba da
"kursta" çıkıyordu. Sıfır ölçmüştü ve sıfır gerçekti ama işe yaramazdı. Soru **en yakın waypoint'e**
uzaklık olarak sorulunca:

```
4121: sekiz arabanın sekizi de  düğüm 110 → 111 · yeni düğüm hattan 50,9 m · hatta kalan kol: 2
4102: yedi araba               düğüm  11 →  10 · yeni düğüm hattan 58,6 m · hatta kalan kol: 1
toplam 21 çıkış · 21'inde de hatta kalan bir kol VARDI · 0'ında yoktu
```

**Yirmi bir vakanın yirmi birinde de yarış hattında kalan bir kol vardı ve seçilmedi.** Arabalar
sonra 250 m boyunca koridorun içinde, 60 km/h ile, hedef waypoint'leri 150-159 m uzaktayken sürüyor
ve nihayet açılıp koridoru terk ediyorlar. Üç süpürmenin "viraj" diye saldırdığı yer, yanlış kola
sokulmuş bir arabanın o yolun büküldüğü noktada dışarı çıkması.

**Sebep `Network::step_avoiding`'in kendisi:** hedefe **düz mesafede** en yakın komşuyu seçiyor.
Yarış hattı ile paralel giden bir şerit, hedefe düz mesafede pekâlâ daha yakın olabilir — özellikle
hedef ileride ve yanda ise. Graf da o iki şeridi birbirine bağladığı için seçenek gerçekten var.

**Sıradaki iş, ve neden çürütülenlerden farkı:** kol seçimine "yarış hattına yakın kal" terimi
eklemek. ROADMAP'te çürütülmüş üç rotalama mekanizması (kara liste, yönü şucu, listeyi eskitme) hep
*kurtarma* mekanizmalarıydı — araba çuvalladıktan sonra ne yapmalı. Bu ise seçimin kendisi, ve
ölçüm ilk kez seçilmeyen doğru kolun **var olduğunu** gösteriyor. Sekiz rota yargılayacak.

### Doğru kol vardı, ama onu almak çok daha kötü — dördüncü rotalama fikri de çürüdü (2026-08-20)

Bir önceki ölçüm, 21 vakanın 21'inde yarış hattında kalan bir kolun **var olduğunu ve
seçilmediğini** gösteriyordu. Bundan çıkan aday açıktı: kol seçimine "hatta yakın kal" terimi ekle.
Kuruldu (`Network::mark_line`, düğüm başına bir kez, kurulum zamanında) ve iki biçimde denendi.

**Sert biçim — kolları hattakilerle sınırla.** 4121'de alan boşalıyor: junctions **204 → 36**,
waypoint **30 → 10**, furthest 730 → 215. Sebebi anlaşılır: geldiği kol hariç tek hat-üstü kolu olan
bir kavşakta yürüyüşe alacak bir şey kalmıyor.

**Yumuşak biçim — maliyete ceza ekle, eleme yok.** Aynı sonuç: 40 m cezada waypoint 30 → **11**
(ve fence arabaları 96 kez tutuyor), 100 m'de 30 → **10**.

Sekiz rotanın tamamında da kaybediyor: **waypoint 986 → 815, kursu hiç bırakmayan 32 → 24.** Rota
rota 4041 −53, 4102 −38, **4121 −102**; tek kazanan 4081 (+24).

Yani hattaki kolu tercih etmek, hangi biçimde olursa olsun, tam da tasarlandığı rotayı üç kat
kötüleştiriyor. **Bulgu duruyor, çıkarım düşüyor:** arabalar gerçekten tek bir kavşakta hattan
çıkarılıyor, ama grafın "yanlış" dediği kol pratikte sürülebilir olan; hattın kendi devamı
sürülemiyor ya da arabanın o hızda yapamayacağı bir manevra istiyor.

Bu, aynı yöne bakan **dördüncü** çürütme (kara liste, yönü şucu, listeyi eskitme, ve şimdi hatta
kalma). Ortak dersi şu olabilir: **bu graf bir yol haritası değil**, ve `network.rs`'in kendi modül
belgesi bunu zaten yazıyor — "önce grafı doğru yap, sonra rotayı en iyi yap". Sıradaki iş rotalama
politikasını bir kez daha ayarlamak değil, hattın kendi devamının neden sürülemediğini görmek:
düğüm 110'un hat-üstü kollarının nereye gittiğini ve orada ne olduğunu ölçmek.

### Ve kök sebep: pilotun sürdüğü "yarış hattı"nın yarısı yol değil (2026-08-20)

Dördüncü rotalama çürütmesinden sonra kalan tek soru şuydu: düğüm 110'un hat-üstü kolları nereye
gidiyor? `NFS_ARM=<düğüm>` ile kavşak tek tek soruldu:

```
düğüm 110 · (-424,1611) · hat 2 · yarış hattına 12 m · 3 kol
   →  109 · hat 2 ·  37 m · hatta  14 m (HATTA) · zemin tam · önü açık   ← geldiği yön (batı)
   →  111 · hat 2 ·  54 m · hatta  51 m         · zemin tam · önü açık   ← seçilen
   →  294 · hat 1 ·  65 m · hatta  15 m (HATTA) · zemin tam · önü açık   ← (-488,1620), yine batı
```

**Hat-üstü kolların ikisi de arabanın geldiği yöne gidiyor.** Yarış ise waypoint 8'de (-420,1622)
**güneye dönüyor** — ve düğüm 110'un güneye giden kolu yok. `NFS_HOLE=-419,1583`: oraya en yakın
düğüm 28 m ötede ve o da 110'un kendisi. Güneyde düğüm yok, çünkü orada yol yok: zemin taraması
z ≈ 1545'te bitiyor, ve waypoint 10 tam orada.

**Sebep `densify`.** Waypoint halkası, olay anahatının köşeleri arasına çekilen **düz çizgi**:
17 nokta, 6 km, medyan adım 425 m. İki köşe arasında ne varsa — bina, arazi kenarı, başka bir
şerit — kiriş onun üstünden geçiyor. Ölçüldü:

| rota | waypoint | en yakın düğümden >40 m | altında zemin yok | koridorun dışında |
|---|---|---|---|---|
| 4001 | 159 | 15 (%9) | 4 (%2) | 64 (**%40**) |
| 4002 | 144 | 32 (%22) | 11 (%7) | 83 (**%57**) |
| 4021 | 65 | 10 (%15) | 2 (%3) | 38 (**%58**) |
| 4041 | 126 | 23 (%18) | 33 (**%26**) | 67 (**%53**) |
| 4061 | 74 | 39 (**%52**) | 4 (%5) | 62 (**%83**) |
| 4081 | 126 | 33 (%26) | 15 (%11) | 90 (**%71**) |
| 4102 | 104 | 6 (%5) | 14 (%13) | 58 (**%55**) |
| 4121 | 130 | 19 (%14) | 25 (%19) | 56 (**%43**) |
| **TOPLAM** | **928** | **177 (%19)** | **108 (%11)** | **518 (%55)** |

**Pilotun nişan aldığı çizginin yarıdan fazlası kursun dışında, ve her onda biri havada.** 4061'in
her kolda tuhaf davranmasının sebebi de burada: waypoint'lerinin %52'si en yakın düğümden 40 m'den
uzak, %83'ü koridor dışı.

**Bugünün dört rotalama çürütmesi bu ışıkta okunmalı.** Kara liste, yönü şucu, listeyi eskitme ve
hatta kalma — dördü de arabayı *bu çizgiye* yaklaştırmaya çalışıyordu. Çizgi yol olmadığı için
dördü de kaybetti, ve "hatta kal" en sert biçimde kaybetti çünkü en doğrudan denedi.

**Sıradaki iş, ve artık kanıtlı:** waypoint halkası anahat köşeleri arasında **ağda yürünerek**
üretilmeli, lerp'lenerek değil. `network.rs`'in modül belgesi zaten "graf + anahat = sürülebilir
rota" diyor; eksik olan, halkayı da o yürüyüşten türetmek. O yapılmadan pilot tarafında ölçülen
her şey, yarısı hayalî bir hedefe göre ölçülüyor.

### Halka ağda yürünerek kuruldu: geometri düzeldi, sürüş kötüleşti (2026-08-20)

Bir önceki bölümün işaret ettiği iş yapıldı. `Network::path` (plan görünümünde Dijkstra, testleri
yazıldı) ve `route::along_roads`: anahat köşeleri arasındaki yol **ağda yürünerek** bulunuyor,
sonra o çoklu-doğru densify ediliyor. `NFS_WALKLINE=1`.

**Halkanın geometrisi neredeyse mükemmel oldu:**

| | kiriş | ağda yürünmüş |
|---|---|---|
| waypoint | 928 | 1.426 |
| en yakın düğümden >40 m | 177 (%19) | **0** |
| altında zemin yok | 108 (%11) | **3** |
| koridorun dışında | 518 (%55) | **5** |

**Sürüş yine de kötüleşti** (sekiz rota):

| halka | furthest | kursu hiç bırakmayan | düşen |
|---|---|---|---|
| kiriş | **5.413 m** | **32 / 64** | 4 |
| yürünmüş | 5.193 m | 15 / 64 | 2 |

Rota rota: 4002 +175 m, 4081 +52, 4041 +33, 4021 +14; buna karşılık **4121 −363**, 4001 −117.
Kursta kalan araba 4001'de 8 → 0, 4021'de 3 → 0, 4081'de 5 → 2.

**Sebep, ve grafın kendi başlığının uyardığı şey.** Bu ağ yan yana giden yolları birbirine
bağlıyor; **taahhüt edilmiş** bir en-kısa-yol da arabanın geçemeyeceği bağlantılardan geçiyor —
`guide_to`'nun sürüş rolünde neden kaybettiğini açıklayan cümlenin aynısı. Halkanın kendisi
üzerinde sayıldı, ardışık waypoint'ler arasında araç boyunda engel olanlar:

```
4001: 15    4081: 14    4121: 8    4021: 7    4061: 0
```

Bunlar tam olarak arabaların kursta kalmayı bıraktığı rotalar, ve halkası temiz olan tek rota
(4061) zaten hareket etmeyen rota. **Pilot orta refüjün içinden geçirilmeye çalışılıyor.**

**Durum:** `NFS_WALKLINE` varsayılan olarak kapalı; kod, ölçümü ve kusuruyla birlikte duruyor.
Sıradaki adım başka bir arama değil, bu aramaya **duvarı olan bağlantının yol olmadığını
öğretmek**: `drop_walled` zaten bir bağlantı boyunca *zeminin* devam edip etmediğini soruyor, ama
hiçbir şey içinde bir şey **durup durmadığını** sormuyor.

### Aramaya duvar öğretildi: kaybın üçte biri geri geldi, teşhis doğrulandı (2026-08-20)

Bir önceki bölümün sıradaki adımı uygulandı: halka kurulurken **üzerinde araç boyunda bir şey duran
bağlantılar yol sayılmıyor** (`Network::path_where`, ve bağlantı başına bir kez ölçülen duvar
testi). Graf değişmedi — `drop_walled`'a filtre eklemek bir kez süpürülüp atılmıştı — test yalnız
kurs kurucusuna verildi.

| halka | furthest | kursu hiç bırakmayan | düşen |
|---|---|---|---|
| kiriş (mevcut) | **5.413 m** | **32 / 64** | 4 |
| yürünmüş | 5.193 m | 15 / 64 | 2 |
| yürünmüş, duvarlı bağlantıyı reddederek | 5.155 m | **21 / 64** | 3 |

4001'de halkanın kendi duvardan geçen ardışık çifti **15 → 8**, alanda kursta kalan araba
**15 → 21**. Bedeli: dört bacak artık birleştirilemeyip kirişe düşüyor. Yani teşhis doğru, düzeltme
yetersiz.

**Kalan fark iki rotada:** 4121 −363 m, ve 4001'in sekiz arabasının sekizi de kursta kalmıyor. Daha
fazla duvar filtresi değil, çünkü **4081 aynı işlemle kirişi geçiyor** (kursta kalan 5 → 8). O iki
rotanın halkasında başka bir şey var, ve onu bulacak alet bunu bulan alet: halkanın kendisini rota
rota saymak.

`NFS_WALKLINE` varsayılan kapalı, `NFS_WALKWALLS` onunla birlikte açık.

### İki halka iki farklı kurs: sayılar bunları sıralayamaz (2026-08-20)

Yürünmüş halkanın 4001 ve 4121'de neden kaybettiği soruldu ve halkanın kendisi rota rota sayıldı.
Cevap ikisini de kapsıyor ve karşılaştırmanın kendisini geçersiz kılıyor.

| rota | uzunluk (kiriş → yürünmüş) | <80 km/h sınırı dayatan waypoint | katlanma | dönüş |
|---|---|---|---|---|
| 4001 | 6.038 → 7.159 m (+%19) | 10 → **26** | 0 → 4 | 1 → **9** |
| 4002 | 5.368 → 6.581 m (+%23) | 12 → **43** | 0 → 1 | 1 → 9 |
| 4021 | 2.347 → **5.128 m (+%118)** | 10 → **61** | 0 → 3 | 1 → **80** |
| 4041 | 4.700 → 5.823 m (+%24) | 10 → **51** | 0 → 2 | 1 → 13 |
| 4061 | 2.784 → 3.077 m (+%11) | 4 → **24** | 0 | 1 |
| 4081 | 4.825 → 5.085 m (+%5) | 9 → **33** | 0 → 2 | 1 → 0 |
| 4102 | 3.856 → 4.237 m (+%10) | 13 → **52** | 0 → 1 | 0 |
| 4121 | 4.816 → 5.571 m (+%16) | 11 → **59** | 0 | 0 |

**Her rotada yürünmüş halka daha uzun ve üç ilâ altı kat daha virajlı** — çünkü yolları takip
ediyor, kiriş ise blokların üstünden kesiyor. Yani iki halka aynı kursun iki ölçümü değil, **iki
farklı kurs**: biri şehrin gerçek yolları, öteki haritanın üstüne çizilmiş düz çizgiler.

**Bunun sonucu, bugünkü kıyasların çoğunun geçersiz olması.** `furthest` (kat edilen metre) ve
"geçilen waypoint" dürüst kursu cezalandırır: bloklar arasına çekilmiş düz çizgide giden araba,
gerçek virajları süren arabadan daha çok metre yapar. Ayakta kalan tek kıyaslanabilir sayılar
`away` (her kolda 64) ve `fallen` (4 → 3); "kursu hiç bırakmayan" bile etkileniyor, çünkü virajı
olmayan bir kursta koridoru terk etmek için fiziksel olarak savrulmak gerekir.

**Ve bir gerçek kusur çıktı:** 4021'in yürünmüş halkası **80 waypoint'te daha önce geçtiği yere
dönüyor** ve uzunluk iki katından fazla artıyor — ardışık iki anahat köşesinin en kısa yolu bir
öncekinin üstünden geri geçiyor. Bacakları birleştirirken geldiği yönü hesaba katmayan bir arama,
sokağı iki kez sürüyor. Bu, halkanın kendi hatası ve düzeltilebilir.

**Durum ve karar:** varsayılan kiriş olarak kalıyor — daha iyi olduğu için değil, **bugüne kadarki
bütün ölçümlerin ona göre ayarlanmış olduğu için.** Yürünmüş halkaya geçmek, alanın tabanını ve
pilot sabitlerinin (özellikle `GRIP`, çünkü bugün kurgusal virajlara karşı ayarlandı) yeniden
ayarlanmasını gerektirir. Bu bir süpürme sonucu değil, bilerek verilecek bir proje kararı.

### Halkanın üç kusuru bulundu ve düzeltildi — alan kıpırdamadı (2026-08-20)

Bir önceki bölüm halkanın kendi kusurlarını saymayı bırakmıştı; üçü de bulundu, ikisi düzeltildi ve
üçüncüsü zaten yoktu.

**1. Bacak, bir öncekinin üstünden geri dönüyordu.** İki anahat köşesi arasındaki en kısa yol,
çoğu zaman yeni gelinen yolun ta kendisi. Düzeltme: bir bacak, bir öncekinin **girdiği düğüme geri
dönen adımı** kullanamıyor — sadece o tek adım, çünkü bir kurs bir yoldan sonra yine geçebilir ve
bunu bütünüyle yasaklamak, çürütülmüş "halkada duramaz" hatasının başka kılığı olurdu.

`Paths4001`: daha önce geçilen yere dönen waypoint **9 → 1**, uzunluk 7.159 → 6.238 m.

**2. Bir bacağın yolu kirişinin 22 katı olabiliyor.** `NFS_WALKLEGS=1` ile bacak bacak bakıldı ve
`Paths4021`'in 6. bacağı çıktı: **kirişte 108 m, yolda 2.436 m — ×22,6.** Tek başına 5.192 m'lik
halkanın yarısı, ve o rotanın 80 dönüşünün kaynağı. Böyle bir sapma "graf bu iki köşe için yol
tanımıyor, bunun yerine bütün bir ada sistemini dolaşıyorum" demektir. Düzeltme: yol, kirişin
**3 katından** uzunsa o bacak kirişte bırakılıyor (`NFS_WALKDETOUR`).

`Paths4021`: uzunluk **5.192 → 2.811 m**, dönüş **81 → 3**, halka 205 → 115 waypoint.

**3. 4121'in halkasında zaten kusur yoktu** — sıfır katlanma, sıfır dönüş, ilk günden beri.

**Ve alan hiç kıpırdamadı:**

| halka | furthest | kursu hiç bırakmayan | düşen |
|---|---|---|---|
| kiriş (varsayılan) | **5.413 m** | **32 / 64** | 4 |
| yürünmüş + duvar | 5.155 m | 21 / 64 | 3 |
| + geri dönme yok | 5.157 m | 21 / 64 | 2 |
| + sapma sınırı | 5.157 m | 21 / 64 | 2 |

**Ve `Paths4021` bunu en sert biçimde gösteriyor:** halkası yarıya indi, 90 waypoint eksildi, ve
sürüş sonucu **bayt-birebir aynı** kaldı (623 m, sıfır araba kursta). Sebep de bulundu: bacak 0-5'in
kiriş toplamı 619 m ve arabaların `furthest`'ı 623 m — yani **arabalar tam olarak grafın tarif
edemediği bacağın başladığı yerde duruyorlar.** Halkanın o bacaktan sonrası hiçbir arabanın
ulaşmadığı yer; düzeltmenin ölçüye yansımaması bu yüzden.

**Okuma:** halkanın şeklini düzeltmek alanı hareket ettirmiyor. Bu, bir önceki bölümün vardığı
yerin doğrulanması — iki halka iki farklı kurs ve sınırlayıcı olan halkanın düzgünlüğü değil,
**pilotun gerçek bir kursu sürebilmesi**. Halka artık dürüst; sıradaki iş orada değil.

**Sıradaki turun somut sorusu:** `Paths4021`'de arabalar 620 m'de, altıncı anahat bacağının
başında duruyor — grafın 108 m'lik kirişe karşılık 2.436 m'lik yol bulduğu bacağın. Orada ne var?
`NFS_HOLE` ve `NFS_ARM` o noktayı sormak için hazır.

### Düzeltme, ve arkasına nişan alan arabanın yerinde sallanması (2026-08-20)

**Önce bir düzeltme.** Bir önceki bölümde "4021'in arabaları tam olarak grafın tarif edemediği
bacağın başladığı yerde duruyor" yazmıştım: bacak 0-5'in kiriş toplamı 619 m, arabaların
`furthest`'ı 623 m. **İki farklı büyüklüğü karşılaştırmışım.** `furthest`, çıkış çizgisinden
**düz-çizgi uzaklık** (`nfs_sim.rs`'de `(at - line).length()`'in en büyüğü), kat edilen yol değil.
619 ile 623'ün örtüşmesi tesadüf. Bacak 6 `(-436, 976) → (-336, 1014)` arasında; arabalar ise
`(-247, 1380)` ve `(-586, 1593)` civarında duruyor — ikisi de o bacak değil.

**Doğru yer sorulunca dünya kusursuz çıktı.** `(-247, 1380)`: 13×13'lük zemin taramasının 169
hücresinin 169'unda zemin var, en yakın düğüm 17 m ötede ve **üç kolunun üçü de yarış hattında**.
Araba 0'ın kendi özeti: yarışın **%52'sinde duruyor**, dört tekeri yerde ve tam ağırlığını
taşıyor, **12 yönün 11'i 8 m'den uzağa açık**, ve dururken **istenen gaz 0,03**.

**Takılan arabanın izi yoktu, çünkü iz yalnız kurs kaybını yakalıyordu** — kursu hiç bırakmayan bir
araba için `lost` hiç ateşlemiyor. Aynı halka tamponu artık "bu araba altı saniyedir duruyor"
tetiğiyle de donduruluyor (`NFS_STUCK`). İlk koşuda cevap geldi:

```
koridora 2.6 m · direksiyon -0.85 · gaz  0.36 · nişan 20 m  180° · düğüm 5, 82 m · vazgeçti 6 · KAÇIŞ
koridora 2.6 m · direksiyon +0.85 · gaz -0.70 · nişan 20 m  180° · düğüm 5, 82 m · vazgeçti 6 · KAÇIŞ
koridora 2.6 m · direksiyon +0.85 · gaz -0.70 · nişan 20 m  179° · düğüm 5, 83 m · vazgeçti 6 · KAÇIŞ
koridora 2.7 m · direksiyon -0.85 · gaz  0.36 · nişan 19 m  177° · düğüm 5, 83 m · vazgeçti 6 · KAÇIŞ
```

Araba kalıcı bir **kaçış** içinde ve kaçışın hedefi **20 m, tam arkada**. Tam arkadaki bir noktanın
**yanı yoktur**: `atan2`, hedefi burnun hangi tarafına bir milimetre kaydırdığınıza göre +179° ya da
−179° döndürür, `want` bir kilitten ötekine atlar, gaz da ileri-geri gider. Arabanın kendi özeti:
**17 kaçış, 0,0 m.** Ve 102 çıkış yolundan yalnız 7'si henüz kara listede değil.

**Düzeltme, tanımsız soruyu sormayı bırakmak:** `BEHIND`'ın (150°) ötesinde açı kullanılabilir bir
yan taşımıyor, o yüzden **zaten seçilmiş olan yan korunuyor** — tekerlek bir tarafa bağlanıyor ve
manevra bitebiliyor. `NFS_BEHIND=0` yazı-turayı geri getirir.

**Süpürme, ve genişlik fikirden daha önemli çıktı:**

| tutma açısı | waypoint | furthest | kursu hiç bırakmayan | düşen |
|---|---|---|---|---|
| kapalı | 986 | 5.413 m | 32 / 64 | 4 |
| 150° | 980 | **5.522 m** | **34 / 64** | 4 |
| **170°** | **1.000** | 5.381 m | 33 / 64 | 4 |

Rota rota 170°: 4081 +8, 4061 +6, 4121 +2, 4041 +1, 4001 ve 4002 aynı, 4021 −1, 4102 −2. **Hiçbir
rota incinmiyor.** 150° daha uzağa gidiyor ve iki araba fazla tutuyor ama tek başına `Paths4102`'de
33 waypoint ödüyor: geniş bir koruma, saf takibin gayet iyi becerdiği dörtte üçlük dönüşlerde de
tekerleği bağlıyor.

`BEHIND = 2.97` (170°) varsayılan; `NFS_BEHIND=0` yazı-turayı geri getirir. Alan **986 → 1.000
waypoint**.

### Kaçış kendi kendini yeniden tetikliyormuş — ve aritmetiği zaten kodda yazılıydı (2026-08-20)

`BEHIND` düzeltmesi takılan arabaya ne yaptı diye bakıldı: kaçış başına ilerleme **0,0 m → 3,0 m**.
Salınım söndü, araba çıkamadı — yarışın hâlâ %56'sında duruyor. Yeni iz sebebi gösterdi ve sebep
`CORNER_LIFT`'in kendi belgesinde duruyordu:

> Tam kilitte pedal `1 − 0,85·0,75 = 0,36`'ya iner, bu duruştan ~0,29 m/s²'dir; `STALL_FOR`'un
> 1,5 saniyesinde 0,44 m/s'ye ulaşır ve `STALL_SPEED` 0,7'yi **hiç geçemez**.

Kaçış arabayı tam kilide sokuyor (hedef arkada), tam kilit gazı 0,36'ya indiriyor, araba kendi
durma eşiğini geçemiyor, yeniden takılıyor, geri viteste 1,2 saniye harcıyor, yeni bir kaçış
başlatıyor — ve baştan. Arabanın kendi özeti: **18 kaçış, 3 m**, dururken istenen gaz **0,03**.

**Düzeltme:** kaçış bir viraj değil. Gaz kesme, virajda kaybedilecek hızı olan araba içindir;
kaçışta ne viraj vardır ne kaybedilecek hız. `self.escape` varken kesme uygulanmıyor.

Araba 0'ın sayıları, üç düzeltme boyunca:

| | kaçış başına ilerleme | dururken istenen gaz | tork | junctions |
|---|---|---|---|---|
| başlangıç | **0,0 m** | 0,03 | 966 Nm | 142 |
| + `BEHIND` | 3,0 m | — | — | 148 |
| + kaçışta tam gaz | **6,8 m** | **0,23** | 1.568 Nm | 167 |

Hâlâ çıkamıyor, ama üç ölçünün üçü de doğru yönde ve her adım bir öncekinin açtığı kapıdan geçti.

**Süpürme, ve daraltma:**

| kaçış gazı | waypoint | kursu hiç bırakmayan | dünyadan düşen |
|---|---|---|---|
| her zamanki gibi kesilir | 1.000 | **33 / 64** | **4** |
| her hızda tam | 1.038 | 27 / 64 | 7 |
| **2 m/s altında tam** | **1.035** | 30 / 64 | 6 |

Kazanç mekanizmanın söylediği yerde: `Paths4021` +17, `Paths4041` +19 — takılan arabaların olduğu
iki rota. **Düşen artışının tamamı 4041'in iki arabası.** Bu bedel gizlenmiyor: bu kurulumda
**hiç bariyer yok** (`world::collide::Bounds`), yani yeniden hareket eden araba er geç çitsiz bir
kenar buluyor. Arka-kilit kolunun kusuruyla aynı şey değil — orada tekerleği kısıtlamak arabanın
kenardan **kaçmasını** engelliyordu, yani sürüş hatasıydı; burada araba çitsiz bir haritada daha
uzağa gidiyor, ki o bekleyen bariyer maddesi.

`ESCAPE_FULL = 2.0` varsayılan. Alan **1.000 → 1.035 waypoint**.

### Alanın güncel arıza sayımı, ve gaz kesmenin üçüncü kez çürütülmesi (2026-08-20)

Bugünün dört düzeltmesinden sonra 64 arabanın tamamı, yarışlarının **nasıl bittiğine** göre
sayıldı — takılanlar da dahil, çünkü artık onların da izi var:

| son | araba | pay | araba başına waypoint |
|---|---|---|---|
| kursu bıraktı (takılmadan) | **31** | %48 | 15,6 |
| sorunsuz | 14 | %22 | **21,7** |
| **takıldı, kursta kaldı** | 13 | %20 | 12,1 |
| dünyadan düştü | 6 | %9 | 15,0 |
| takıldı *ve* kursu bıraktı | 0 | — | — |

Yani alan artık üçe ayrılıyor: yarısı kursu bırakıyor, beşte biri kursun üstünde takılıyor, beşte
biri sorunsuz sürüyor.

**Takılanların izi bir sonraki katmanı gösterdi ve o katman çürüdü.** `ESCAPE_FULL` düzeltmesinden
sonra araba 0'ın izi aynı tuzağı kaçışın *dışında* gösteriyor: direksiyon 0,84, gaz **0,37**, nişan
69-96 m ötede −88°, ve araba kaçış çağrılmadan önce takılıyor. `CORNER_LIFT`'in kesmesini 4 ve
8 m/s'nin altında iptal etmek zaten çürütülmüştü (821 ve 690'a karşı 869) — ama o ölçüm 869'luk bir
alandaydı ve eşikler genişti. **2 m/s** ile, 1.035'lik alanda tekrar denendi:

| kol | waypoint | kursta kalan | düşen |
|---|---|---|---|
| mevcut | **1.035** | 30 | 6 |
| 2 m/s altında kesme yok | 919 | 29 | 7 |

4021'de +16 (takılan arabanın rotası), buna karşılık **4121 −56, 4102 −35, 4061 −34**. Üç eşik —
8, 4 ve 2 m/s — üçü de çürük. **Düşük hızda gaz kesmek göründüğü gibi genel bir kusur değil;
yalnızca kaçışın içinde yanlış**, çünkü orada ortada viraj yoktur.

### Bugünkü pilot düzeltmeleri kirişe özel: kazanç taşınmıyor, bedel taşınıyor (2026-08-20)

Kursu bırakan 31 arabanın imzası yeniden çıkarıldı: **arka nişan hâlâ en büyüğü** (imzası okunan 36
arabanın 15'i). `BEHIND` yalnız 180°'deki salınımı söndürdü; hedefin arkaya düşmesini engellemedi ve
o, halka tarafına bağlanan bilinen zor vaka.

Bunun üzerine açık bırakılan döngü kapatıldı: **yürünmüş halka, bugünün dört düzeltmesinden önce
ölçülmüştü.** Aynı halka, aynı üç halka düzeltmesi, iki farklı pilotla:

| | waypoint | halkanın oranı | furthest | kursta kalan | düşen |
|---|---|---|---|---|---|
| kiriş · sabahki pilot | 986 | %13,3 | 5.413 | **32** | **4** |
| kiriş · bugünkü pilot | **1.035** | %13,9 | 5.305 | 30 | 6 |
| yürünmüş · sabahki pilot | 1.665 | **%15,3** | 5.157 | 21 | **2** |
| yürünmüş · bugünkü pilot | 1.662 | %15,3 | 5.104 | 15 | 3 |

**İki okuma, ve ikincisi bir uyarı.**

1. **Yürünmüş halka kendi kursunun daha büyük payını sürdürüyor** — aynı pilotla %15,3'e karşı
   %13,9 — üstelik daha uzun ve üç ilâ altı kat daha virajlı bir kurs olduğu hâlde.
2. **Bugünkü iki pilot düzeltmesinin kazancı taşınmıyor, bedeli taşınıyor.** Kirişte +49 waypoint
   ve −2 araba; yürünmüş halkada **waypoint'te sıfır fark** (1.665 → 1.662) ve **−6 araba**. Dört
   sabit de kirişe göre süpürüldü, ve kiriş kursun %55'inde yol değil.

*(Bu bölümün ilk hâli `walk2` ile `walknow`'u kıyaslıyordu; o ikisi arasında halka düzeltmeleri de
değişiyordu. Yukarıdaki tablo pilot değişikliğini yalıtan `walk5` ile alınmıştır.)*

**Sonuç, ve bugünün en geniş dersi:** pilot sabitlerinin hangi kursa göre ayarlandığı, sabitlerin
kendisi kadar önemli. Yürünmüş halkaya geçme kararı verilirse dört sabit de yeniden süpürülmeli —
ve o zaman kirişte kazandıran şeyin orada da kazandırdığı varsayılmamalı.

## Nerede kaldık (2026-08-14 sonu)

**Alan (2026-08-20 sonu, motor pini `58dc2623`, `GRIP`, `PASSED_NEAR`, `BEHIND` ve `ESCAPE_FULL`
açıkken): 1.035 geçilen waypoint, 64 arabanın 30'u kursu hiç bırakmıyor, 6'sı dünyadan düşüyor.**
Sabah 883 waypoint ve 22 arabaydı. Günün ortasında, yalnız `GRIP`
varken: 927 waypoint, 50 araba bırakıyordu. Bir önceki hâli, aşağıdaki paragrafın ölçüldüğü gün:
865 waypoint, 1.354 düğüm, 51 araba. (Bugün 869
diye geçen sayı, pilot saati 4× hızlıyken ölçülmüştü; düzeltilince 865 oldu — yani saat platonun
sebebi değildi.) En iyi araba 144'ün **29'unda**, ve 600 saniye vermek onu 26'dan 29'a taşıyor:
**sınır zaman değil.**

### Hedefin bugünkü hâli

"Kimse tur tamamlamıyor" üç kez daraldı ve şuraya indi: **4121'de sekiz arabanın altısı, waypoint
6'daki ~50°'lik virajda, 12 m'lik koridordan 15 m açılarak kursu bırakıyor ve bir daha
toparlanmıyor.** Orada dünya sağlam (13×13 zemin taramasında boşluk yok), kavşağın üç kolu da yarış
hattında (koridora 0,0 m), yani alacak yanlış kol yok.

### Çürütülenler — tekrar denenmesin

| aday | sonuç |
|---|---|
| fren eşiği (`NFS_BRAKE`) | varsayılan 9 yerel optimum; iki yönde de kötüleşiyor |
| direksiyon hızı (0,35) | 4121'de 6/8 → 3/8, **alanda 869 → 773** |
| nişan mesafesi (`LOOKAHEAD_PER_SPEED`) | dört rotada +%11, **sekizde 869 → 858** |
| pilotun 4× hızlı saati | düzeltildi, ama alan 869 → 865 — sebep değilmiş |

**Yöntem dersi, iki kez ödendi:** yarım süpürme (dört rota) iki kez yanlış pozitif verdi, ve
ikisinde de gürültü değil **temsil etmeyen alt küme** yüzünden — nişan mesafesi elenen dörtte +59,
elenmeyen dörtte −70. Karar yalnız sekiz rotadan çıkar.

### Açık kalanlar

- **`CORNER_LIFT`'in büyüklüğü** — viraj cephesinde denenmemiş tek pilot kolu (rampalanması ayrıca
  çürütülmüştü).
- **Zaman sabitleri artık dürüst.** `STALL_FOR`, `BACK_FOR`, `ESCAPE_FOR`, `SETTLE` yıllarca
  çeyreklenmiş değerleriyle ayarlanmıştı. Dördü birden dört katına çıktığında alan 869 → 865 gitti,
  yani muhtemelen bağlayıcı değiller — ama tek tek süpürülmediler.
- **Oyuncunun gözlemi: sürerken yolda binalara çarpmak.** 2026-08-20'de ölçülmüş desteğe kavuştu
  (yukarıya bak): sekiz rotada 2.824 kenarın 138'inde yolun kendi kotunda, arabanın çarpacağı
  yükseklikte geometri duruyor, 81'i yol ekseninin 6 m'sinde. Kapanmayan kısım: bunların hangisine
  gerçekten çarpıldığı, ve 4001'in 57 duvarının 39'unun eksenden uzak olması (kiriş şüphesi).
- **4121'in "virajı" viraj değilmiş** (2026-08-20 sonu, yukarıya bak): sekiz arabanın sekizi de
  düğüm 110 → 111 adımıyla yarış hattından **50,9 m** uzağa sokuluyor, ve o kavşakta hatta kalan
  **iki kol** vardı. 4102'de aynı şey (58,6 m, bir kol). 21 vakanın 21'inde doğru kol mevcut ve
  seçilmiyor — **ama o kolu almak çürütüldü** (yukarıya bak): iki biçimde de alan düşüyor
  (986 → 815 waypoint) ve 4121 üçe katlanarak kötüleşiyor. Grafın "yanlış" dediği kol pratikte
  sürülebilen olan. **Ve sebep bulundu** (yukarıya bak): hat-üstü kollar geriye gidiyor, yarış ise
  ağın yol tanımadığı bir yöne dönüyor — çünkü waypoint halkası `densify` ile düz çizgi olarak
  üretiliyor ve **928 waypoint'in 518'i (%55) koridorun dışında, 108'inin altında hiç zemin yok.**
  **Yapıldı ve ölçüldü** (yukarıya bak): halka düzeldi (koridor dışı 518 → 5) ama sürüş kötüleşti
  (furthest 5.413 → 5.193, kursta kalan 32 → 15), çünkü en-kısa-yol duvarlı bağlantılardan geçiyor.
  Sıradaki adım aramaya duvarı öğretmek.
- **Viraj cephesinin eski kaydı, artık bu ışıkta okunmalı:** Duran bir şey yok (en yakın duvar 225 m); iz,
  frenin eşiğin %8 altında kalarak hiç gelmediğini gösterdi ve nişan yayının eğriliğinden fren
  yapan terim (`GRIP = 8`) sekiz rotada **883 → 927 waypoint**, kursta kalan araba **9 → 14**
  getirdi. 4121 hâlâ çözülmedi: yedi araba virajın 19 m ilerisinde, 50 km/h ile çıkıyor. Kalan
  eksik direksiyonda — açı→kilit haritası düz bir rampa, saf takibin kendi formülü değil.
- **Alanın en büyük tek kaybı — arkadaki bir noktaya direksiyon kırmak — sebebinden düzeltildi.**
  Rotalama değilmiş (`NFS_WRONGWAY=1`: yürüyüş kurstan hiç çıkmıyor, ölçülen sıfır); waypoint
  sayacı arabanın *yanından geçtiği* hedefi bırakamıyormuş. `PASSED_NEAR = 60` ile alan
  **927 → 986 waypoint**, kursu hiç bırakmayan **14 → 24/64**. Cevabı yumuşatan kollar (arka kilit)
  çürütüldü ve öyle kaldı.
- **Arabanın ölçülmüş yanal kapasitesi 5,2 m/s²** (%90; %95'te 5,6). `GRIP = 8` bunun üstünde ve
  bu bilerek: kiriş açısı virajı abarttığı için sabit, kapasite değil *kapasite bölü abartma*.
  İkisi birlikte düzeltilmedikçe 8 kalır — yol eğriliğine geçme denemesi çürütüldü.
- **Motor pini `58dc2623`'te** (2026-08-20'de taşındı). `BASELINE-SEKIZ-ROTA.md` yükseltme öncesi
  tabloyu tutuyor; yükseltme sonrası süpürme onunla yan yana konmalı.

### Kalan aletler

`NFS_HOLE=x,z` (zemin haritası + en yakın düğüm + kavşak kolları), `NFS_BLOCKED=1` (hat boyunca
engel taraması: kat değiştirmeleri eleyip kalan engeli bordür / arabanın üstünden geçen / duvar
diye ayırır, her duvarın yol eksenine uzaklığını basar — ve artık arabaların takılıp takılmadığına
bakmadan koşar, yani `NFS_SECONDS=1` ile saniyeler içinde alınır),
`NFS_FLOOR`/`NFS_FLOORSTEER` (pilotu devreden çıkarıp gazı basılı tutmak — bir turu bu çözdü),
`NFS_LOST=1` (kursun bırakıldığı anın çevresindeki 8+2 saniye, 20 Hz: hız, koridora uzaklık,
direksiyon/gaz/fren, nişan mesafesi ve açısı, tutulan düğüm — üç süpürmenin göremediğini tek
koşuda söyledi), ve her arabanın kursu **ilk kalıcı olarak bıraktığı** an/waypoint/konum.


### `GRIP` dürüst halkada yeniden süpürüldü: fiziksel doğru değer yine kaybetti (2026-08-20)

Bir önceki bölümün kapanışı bir iddia taşıyordu: *"bir pilot sabitinin hangi kursa karşı
oturtulduğu, sabitin kendisi kadar önemli."* Bunun en keskin örneği `GRIP`'ti. Belgesinde
yazdığım gibi 8, arabanın ölçülmüş 5,2 m/s²'lik yanal ivme tavanının üstünde duruyor ve orada
durmasının gerekçesi kirişin açısının yolun eğriliğini **abartması**: `GRIP` aslında "kapasite"
değil, "kapasite bölü o abartı". Öyleyse abartının olmadığı — ağ üzerinde yürünmüş — halkada
fiziksel değer kazanmalıydı.

**Kazanmadı.** Sekiz rota, `NFS_WALKLINE=1`, 8 araba, 90 s:

| `GRIP` | geçilen waypoint | halkanın oranı | kursu hiç bırakmayan | düşen |
|---|---|---|---|---|
| **8 (bugünkü)** | **1.662** | %15,3 | 15 / 64 | 3 |
| 6,5 | 1.610 | %14,8 | 18 / 64 | 3 |
| 5 (fiziksel) | 1.570 | %14,4 | 18 / 64 | **6** |

Karar ölçüsünde sıralama tek yönlü: 8 > 6,5 > 5. Daha sert fren **kursta kalmayı** artırıyor
(15 → 18) ama ilerlemeyi azaltıyor — bu, süpürmelerin baştan beri gösterdiği aynı takas.

**Ve düşenlerin ikiye katlanması bir kusur değil, halkanın kendi kusurunun ölçüsü.** Sekiz rotanın
altı düşüşünün beşi *"yol 3–9 m ileride bitiyor"* diyor; altıncısı 15 km/h'de **yüzeyin içinden**
geçmiş (bu ayrı bir çarpışma kusuru, kayda geçti). Halkayı ne kadar sadık takip eden araba varsa,
yolun bittiği yeri bulan araba o kadar çok oluyor. `GRIP` 5'in 6 düşüşü, 18 arabayı kursta
tutmasının bedeli.

**Alan toplamı asıl şekli gizliyor.** Rota rota:

| rota | 8 | 6,5 | 5 | 6,5 − 8 |
|---|---|---|---|---|
| 4001 | 362 | 371 | 365 | **+9** |
| 4002 | 75 | 75 | 73 | 0 |
| 4021 | 193 | 205 | 188 | **+12** |
| 4041 | 272 | 279 | 247 | **+7** |
| 4061 | 303 | 237 | 238 | **−66** |
| 4081 | 79 | 82 | 99 | **+3** |
| 4102 | 235 | 219 | 222 | −16 |
| 4121 | 143 | 142 | 138 | −1 |
| **toplam** | **1.662** | 1.610 | 1.570 | **−52** |

6,5 sekiz rotanın **dördünde 8'i yeniyor**, birinde berabere, üçünde kaybediyor; toplamı tek rotada,
`Paths4061`'de kaybediyor — ve oradaki fark (−66) bütün alan farkından (−52) büyük. Bir arabanın
bankacılığı da değil: 4061'in sekiz arabasının altısı birden düşüyor (42→30, 43→32, 45→33,
40→15, 42→33).

**Okuma — ve bir önceki bölümün iddiasının düzeltilmiş hâli.** Mesele "8, kirişin abartısı yüzünden
doğru" değilmiş; dürüst halkada da 8 kazanıyor. Mesele daha kötüsü: **tek bir doğru `GRIP` yok.**
Rotalar farklı değerler istiyor ve alan sayısı bir uzlaşma. 8 yerinde kalıyor — karar ölçüsünü
kazandığı ve zaten yürürlükte olduğu için, süpürme onu doğru bulduğu için değil. Aynı şüphe
diğer üç sabit (`PASSED_NEAR`, `BEHIND`, `ESCAPE_FULL`) için de açık duruyor: alan ortalaması,
rota başına dağılımdan küçükse, o ortalama bir sonuç değil bir tesadüf olabilir.

**Süpürme okumasına yeni kural:** bir kolun alan farkı, o farkın rotalar arası yayılımından
küçükse, sonuç "kazandı" diye yazılmaz. Rota tablosu da yazılır.

### Yeni kural dört sabite geriye dönük uygulandı: süpürmeler dört mekanizma kurdu, dört sayı değil (2026-08-20)

Bir önceki bölüm şu kuralı yazdı: *bir kolun alan farkı, o farkın rotalar arası yayılımından
küçükse, sonuç "kazandı" diye yazılmaz.* Kuralı yazıp geçmek dürüst olmazdı — günün dört kabul
edilmiş sabitinin süpürme log'ları hâlâ duruyor, hepsi yeniden okundu. Ölçü aynı: geçilen
waypoint, rota rota.

| sabit | karşılaştırma | alan farkı | en büyük tek rota | rota rota | okuma |
|---|---|---|---|---|---|
| `GRIP` 8 | fren **hiç yok**a karşı | +44 | **4001: +95** | 4/8, 1 berabere | **taşınıyor** |
| `GRIP` 8 | 6'ya karşı | +325 | −120 | 7/8, 1 berabere | sağlam |
| `GRIP` 8 | 12'ye karşı | +112 | −61 | 4/8, 1 berabere | sağlam |
| `PASSED_NEAR` 60 | **kapalı**ya karşı | +75 | **4121: +104** | 4/8, 1 berabere | **taşınıyor** |
| `PASSED_NEAR` 60 | 30'a karşı | +103 | −60 | 3/8, 4 berabere | sağlam |
| `PASSED_NEAR` 60 | 80'e karşı | +13 | **−21** | 3/8, 4 berabere | **taşınıyor** |
| `PASSED_NEAR` 60 | 100'e karşı | +30 | **−47** | 4/8, 3 berabere | **taşınıyor** |
| `BEHIND` 170° | **kapalı**ya karşı | +14 | −8 | 4/8, 2 berabere | sağlam |
| `BEHIND` 170° | 150°'ye karşı | +20 | **−31** | 4/8, 3 berabere | **taşınıyor** |
| `ESCAPE_FULL` | kaçışta **da kesme**ye karşı | +35 | −19 | 4/8, 2 berabere | sağlam |
| `ESCAPE_FULL` | **hep tam gaz**a karşı | **−3** | +7 | 4/8, 2 berabere | **ayırt edilemez** |

**Sütunun kendisi de bir bulgu:** on bir karşılaştırmanın dokuzunda kazanan taraf sekiz rotanın
yalnız **dördünde** önde; gerisi ya berabere ya karşı tarafın. Yani "alan kazandı" dediğimiz her
şey, aslında yarı yarıya bölünmüş bir alanın toplamı. `PASSED_NEAR` 60 ile 80 sekiz rotanın
**dördünde bayt-birebir aynı** sonucu veriyor — o iki sayı arasındaki fark, sahanın yarısında
hiç yok.

**Şekil dördünde de aynı ve tek cümleyle söylenebilir: mekanizmanın *varlığı* sağlam, sayının
*değeri* değil.**

* Eğriliğe fren **olması** 4001'e dayanıyor: o rota olmasa frensiz pilot alanı kazanırdı, ve zaten
  8 rotanın 3'ünde kazanıyor. Ama 8'in 6 ve 12'ye üstünlüğü tartışmasız — yani "fren olsun mu"
  sorusunun cevabı tek rotalık, "ne kadar" sorusununki değil.
* Geçilen waypoint'i bırakmak 4121'e dayanıyor; 60'ın 80 ve 100'e üstünlüğü ise gürültü
  (+13 ve +30, tek rotalık sapmalar −21 ve −47). 60 ile 80 arasında ölçüyle seçim yapılamaz.
* Bir yanı seçip tutmak sağlam; **170° ile 150° arasında seçim yapılamaz** (+20, ama 4102 tek
  başına −31).
* Kaçışta gaz kesmemek sağlam; **2 m/s eşiği ise hiçbir şey**: hep tam gaz waypoint'te +3 önde,
  kursta kalan arabada 3 geride (27'ye 30), düşende 1 geride (7'ye 6). Üç ölçünün ikisi eşiği
  tutuyor, biri tutmuyor, üçü de gürültü içinde. Eşik yerinde kalıyor — çünkü "gürültüde
  kıpırdama" kuralı iki yöne de işler ve yürürlükte olan o.

**Bu, sabitlerin geri alınması demek değil.** Dördü de alanı 883'ten 1.035'e taşıyan zincirin
halkaları ve hiçbiri geri alındığında daha iyisini vermiyor. Demek olan şu: **bu dört sayı
ölçülmüş değil, ölçülmüş bir mekanizmanın makul birer ayarı.** İnce ayarları (60'a karşı 80,
170°'ye karşı 150°, eşiğe karşı eşiksiz) savunulacak bir bulgu gibi yazmak, bu tablodan sonra
yanlış olur; belgelerinin her birine bu ayrım eklendi.

**Ve asıl sonuç yön veriyor:** sekiz rotalık alan, ince ayarları ayırt edecek kadar duyarlı değil.
Bir sonraki kazanç ayar aramakta değil, hâlâ kursu bırakan 31 arabanın (%48) ve altı düşüşün
kaynağında — orada farklar rota gürültüsünün çok üstünde.

### Düşen arabalar: dünyada delik var, ama deliğin dörtte üçü halkanın kendi kurgusu (2026-08-20)

Denetim "sıradaki kazanç ayar aramakta değil, kursu bırakan 31 arabada ve altı düşüşte" diye
bitmişti. Düşüşlerden başlandı. `nfs_sim`'in kendi raporu altı düşüşün beşi için *"yol 3–9 m
ileride bitiyor"*, biri için *"yol 80 m sürüyor — **yüzeyin içinden geçti**"* diyor (bu sonuncusu
ayrı bir çarpışma kusuru, 15 km/h'de). Ve `Paths4041`'in **iki arabası aynı noktada** düşüyor:
`(1984, −121)`, kursun 7 m yanında.

**Önce o nokta soruldu — ve `NFS_HOLE` hiç çalışmadı.** Sorgu `if !early.is_empty()`'nin içine
gömülüymüş: dünyaya sorulan bir soru, herhangi bir arabanın takılmasına bağlıydı. Dışarı alındı.
Cevap geldiğinde delik gerçekti:

```
   z=   -129  #####.#######
   z=   -121  ###...#######
   z=   -113  ###....######
   z=   -105  ###..########
   en yakın rota düğümü: 12 m ötede · koridora uzaklık: 6.6 m (yarı genişlik 12)
   kavşak 113: hat 6 · 4 kol — dördü de YARIŞ HATTI
```

Yaklaşık **30 × 30 m zemin yok**, dört kollu bir kavşağın dibinde, koridorun **içinde**.

**Tek nokta mı desen mi? `NFS_HOLE=course` yazıldı:** halkanın her waypoint'inde, koridorun kendi
genişliği boyunca beş örnek (−10, −5, 0, +5, +10 m). Sekiz rota:

| halka | yolun üstünde (düğüme <20 m) | kiriş dolgusunda |
|---|---|---|
| kiriş (varsayılan) | 2.250 örnek · **%3,2 zemin yok** | 2.350 örnek · **%18,4 zemin yok** |
| yürünmüş | **5.155 örnek** · %1,6 zemin yok | 1.605 örnek · %7,1 zemin yok |

Üç şey birden okunuyor:

1. **Halkanın kaçta kaçı gerçek yolda:** kirişte %49, yürünmüşte **%76**. `along_roads`'ın
   ölçülmüş asıl kazancı bu — waypoint sayısı ya da furthest değil.
2. **Eksik zeminin dörtte üçü halkanın kurgusu.** Kiriş halkasında yol dışı hücrelerin **%18,4'ünün
   altında hiçbir şey yok**; toplamda %11,0 olan eksik zemin, yürünmüş halkada **%2,9**'a iniyor.
   Yani "yol 3–9 m ileride bitiyor" diyen beş düşüş, çoğunlukla dünyanın değil halkanın kusuru.
3. **Ama bir kalıntı gerçekten dünyanın:** yürünmüş halkada bile gerçek yolun üstündeki örneklerin
   **%1,6'sında zemin yok** ve bu rotaya göre yığılıyor — `Paths4041` tek başına %4,3, ötekilerin
   hepsi %0,4–1,8. İki arabanın düştüğü kavşak orada. Dağılımı bir blok değil (üç ayrı yerde),
   yani "bir bölüm yüklenmedi" açıklaması tutmuyor; en yakın hipotez, o yüzeylerin çarpışma
   kümesine hiç girmediği — "yüzeyin içinden geçti" diyen düşüşle aynı hipotez.

**Sıradaki somut iş bu kalıntıda:** `Paths4041`'in üç deliğinin altında hangi collider'ın olması
gerektiği. Halkanın kurgusu artık ölçülmüş ve çaresi de belli (`NFS_WALKLINE`); dünyanın kendi
eksiği ölçülmemiş bir şey ve düşüşlerin geri kalanını o taşıyor.

### Deliğin altında ne var: yükleyici değil, filtre değil, sınıflandırma değil — ve "bütün şehri yükle" bir tuzak (2026-08-20)

Bir önceki bölüm yürünmüş halkada bile gerçek yolun üstünde %1,6 zemin eksiği bıraktı ve
`Paths4041`'i (%4,3) işaret etti. Dört şüpheli tek tek elendi.

**1. LOD filtresi değil.** `NFS_COLLIDE=all` 2.100 kaba parçayı geri koyuyor; aynı halkada yoldaki
28 deliğin **hiçbiri** kapanmıyor (kiriş dolgusunda yalnız 4 hücre). Filtre suçsuz.

**2. Sınıflandırma değil.** `NFS_HOLE` artık üç sembol basıyor: zemin, *geometri var ama duvar
sayılmış*, ve *hiçbir şey yok* (`Ground::of_everything`, yalnız teşhis için — duvarları da
indeksliyor). Sekiz rota, yürünmüş halka: yolun üstündeki **84 boş hücrenin 82'sinde hiçbir
üçgen yok**, ikisinde duvar. `surface_of` suçsuz.

**3. `dedup` değil.** `NFS_DEDUP=off` 10.735 parçanın hepsini tutuyor; delik bayt-birebir aynı
kalıyor.

**4. Ve "bütün TRACKS'i yükle" delikleri kapatıyor — ama bu bir düzeltme değil, kirlenme.**
`NFS_BUNDLE=all` ile sayılar çarpıcı: yürünmüş halkada zemin yok olan hücreler sekiz rotada
**198 → 12**, ve 4041'in kavşağının yanındaki 30 m'lik kare tamamen doluyor. Bunu bir kazanç diye
yazmak üzereydim. `NFS_BUNDLE=<bölge>` eklenip **tek tek soruldu**:

| bölge | ne | `(1984, −121)` satırı |
|---|---|---|
| **L4RA** | şehir (4041 bu bölgenin rotası) | `###...#######` |
| **L4RD** | aynı şehrin başka paketlemesi, 400 obje fazla | `###...#######` |
| **L4RB** | **arena** — ayrı bir mekân | `#############` |
| L4RC · L4RF · L4RG · L4RH · L4RR | arena / test pisti | `.............` |

Deliği dolduran şey şehir değil, **L4RB — orijini paylaşan bağımsız bir arena**. `REGIONS`
belgesinin zaten söylediği şey bu: sekiz bölge tek haritanın parçaları değil, aynı orijini
paylaşan ayrı yerler. `all` deliği doldurmuyor, **üstünü örtüyor**.

**Kalan cevap, veri.** Şehrin iki bağımsız paketlemesi — L4RA ve L4RD — bütün kurs boyunca
**rakam rakam aynı**: yolun üstünde 745 örnekten 32'sinde zemin yok, ikisinde de. İki ayrı
paketlemenin aynı yerde aynı boşluğu göstermesi, kaynak veriye bu ölçüyle varılabilecek en yakın
şey: orada gerçekten zemin yok.

**Ve bu, düşüşlerin sorusunu değiştiriyor.** Oyunun kendi dünyasında oyuncunun gidemeyeceği
yerlerde zemin olmaması normaldir; oyuncuyu orada tutan şey **bariyerdir**. Düşen arabaların
raporu ise baştan beri *"dünyada hiç bariyer yok"* diyor. Yani eksik olan zemin değil, zemini
gereksiz kılan şey. Sıradaki iş `Paths4041`'in kavşağının çevresinde bariyer geometrisinin
bundle'da olup olmadığı — `Walls`'ın gördüğü ama `across_hit`'in elediği bir şey mi, yoksa hiç
yok mu.

**Ve bir okuma kuralı daha:** bir düğme ölçüyü büyük ölçüde iyileştiriyorsa, önce **neyin**
iyileştirdiği sorulur. `NFS_BUNDLE=all`'ın %94'lük kazancı, bir arenanın zeminiydi.

### Zemin haritası şehrin planını değil, yollarını kaplıyor — ve koridora giren boşluklar sayıldı (2026-08-20)

Delik veriymiş; sıradaki soru "bu normal mi" idi. `NFS_HOLE=city` yazıldı: bölgenin zemini 8 m'lik
ızgarayla örneklenip **boşluklar taşırma ile bileşenlere ayrılıyor**, kutunun kenarına değenler
(dünyanın bitişi) atılıyor, kalan her boşluğun yarış hattına uzaklığı ölçülüyor.

İlk sürümü yanlıştı ve kendi sorusunu göremiyordu: hücrenin ±16 m'deki komşularına bakıyordu, yani
16 m'lik deliği buluyor 30 m'liği ıskalıyordu — 4041'in kavşağındaki, bütün soruyu doğuran delik
tam da 30 m'lik olan. Taşırmaya çevrildi.

| rota | ayrı boşluk | toplam alan | koridora (12 m) girenler |
|---|---|---|---|
| 4001 | 183 | 623.616 m² | 6 |
| 4002 | 161 | 453.056 m² | 7 |
| 4021 | 90 | 136.064 m² | 2 |
| 4041 | 85 | **576.320 m²** | **8** |
| 4061 | 33 | 34.368 m² | 1 |
| 4081 | 148 | 586.304 m² | 4 |
| 4102 | 111 | 514.176 m² | 2 |
| 4121 | 107 | 253.312 m² | **10** |

**Boyutlar cevabı veriyor.** 4041'in en yakın altı boşluğu 12.032 ile 86.400 m² arasında — 300 m
kenarlı bir kare bina ayak izi değildir. Yani `Ground` "şehrin zemini" değil, **sürülebilir
yüzeyin haritası**: yollar ve kaldırımlar modellenmiş, blokların içi hiç modellenmemiş. Bir
yarışta düşmek de bu yüzden bir kenardan uçmak değil, **bloğun içine girmek**.

**Karşı kanıt da kaydedilsin:** eğer boşluklar blok içiyse kenarlarında bina cepheleri, yani duvar
üçgenleri olmalı. `(1984, −121)`'in çevresindeki **104 × 104 m'lik karede tek bir duvar üçgeni
yok** — üstelik şehrin bütünü %42,7 duvar (333.846 sürülebilir, 248.458 duvar üçgeni). O boşluk
en azından cephesiz; ya boş bir arsa ya da eksik bir obje. Izgara ayrıca tek katman gösteriyor
(`NFS_HOLE` artık '#' yerine **katman sayısı** basıyor), yani üst geçit altı da değil.

**Düşüşlerin bütün zinciri artık yazılabiliyor:** halka bloğun üstünden geçiyor (kirişte %18,4'ünün
altında hiçbir şey yok) → araba halkayı takip edip bloğun içine giriyor → orada zemin yok ve onu
durduracak cephe de yok → düşüyor. Zincirin ilk halkası `NFS_WALKLINE` ile zaten kırılıyor
(%18,4 → %7,1); ikincisi için yarış koridoruna giren 1–10 boşluk rota başına **sayılı ve yerli**,
yani teker teker bakılabilir.

### Çit neyi çitliyordu: iki adımdan beşi biten yolu değil, kot değiştiren yolu (2026-08-20)

`CarRig::hold_at_edge`'in belgesi bir yerde açıkça duruyordu:

> Zeminin ileride bittiğinin **neden** öyle göründüğü ölçülmedi, ve düzeltme oradan başlamalı:
> 8 m'lik kot penceresi içindeki düz bir sonda, **biten** yolla **dönen** yolu ayırt edemez.

Ayırt etmek bir sorgu tutuyormuş. `gap_along` bir boşluk bildirdiğinde, o boşluğun tam XZ'sinde
**herhangi bir kotta** sürülebilir zemin var mı diye soruluyor. Varsa yol bitmiyor, kot
değiştiriyor — rampa, çukur, tepenin arkası. `Fence { held, off_level }` bunu taşıyor.

**Ölçü, sekiz rota, bugünkü pilot:** çit 1.121 adım tuttu, **451'i (%40)** kot değiştiren yolda.
Ve dağılımı rastgele değil — belgenin "mesafeye mal oluyor, kimseyi kurtarmıyor" diye mahkûm
ettiği rotaların ta kendisi:

| rota | çit tuttu | kot değiştiren yolda |
|---|---|---|
| 4002 | 381 | **303 (%80)** |
| 4081 | 132 | **132 (%100)** |
| 4061 | 48 | 16 (%33) |
| 4021 · 4041 · 4102 · 4121 | 365 · 97 · 93 · 5 | **0** |
| 4001 | 0 | — |

**Düzeltme:** kot değişiminde çit ateşlemiyor (`NFS_FENCE_LEVEL=0` eskisini geri getirir).

| | waypoint | kursta | düşen | çit adımı |
|---|---|---|---|---|
| eski çit | 1.035 | 30 / 64 | 6 | 1.121 |
| **kot farkında çit** | **1.046** | 30 / 64 | 6 | **785** |

**Ve asıl argüman waypoint değil.** Alan farkı +11 ve en büyük tek rota +10 (4002) — kendi
kuralımın kıyısından geçiyor, yani ilerleme kazancı tek rotalık sayılmalı. Argüman şu: **336 daha
az müdahale, hiçbir ölçüde kayıp yok.** `Paths4081`'de çit 132 ateşlemeden **sıfıra** iniyor ve
`furthest` bayt-birebir aynı kalıyor (509 m) — yani o 132 müdahalenin tamamı saf gürültüymüş.
`Paths4002`'de 381 → 180 ve furthest 241 → 235 m. Kalan altı rotanın hepsi değişmiyor.

Bir bariyerin işi arabayı dünyada tutmaktır; aynı sonucu daha az dokunarak veren bariyer daha iyi
bariyerdir. Kalan 785 müdahale artık gerçekten zeminin bittiği yerlerde.

**Ek ölçü — ve çit değişikliğinin asıl kanıtı bu.** Waypoint farkı (+11) küçüktü; duran arabalara
bakılınca sebep de görüldü:

| | duran araba | bunun çitle tutulanı |
|---|---|---|
| eski çit | 13 | **8** |
| kot farkında çit | 10 | **1** |

Eski çitin çiviledikleri, kot değiştiren yolu çitlediği iki rotanın ta kendisi:

```
4002 araba 3: yarışın %43'inde duruyor · çit 65 kez
4002 araba 4: yarışın %61'inde duruyor · çit 62 kez
4002 araba 6: yarışın %84'inde duruyor · çit 40 kez
4081 araba 2: yarışın %54'inde duruyor · çit 49 kez
4081 araba 3: yarışın %42'inde duruyor · çit 39 kez
4081 araba 7: yarışın %47'inde duruyor · çit 44 kez
```

Yeni çitte bu listede yalnız `4121 araba 2` kalıyor (4 kez, %22) — ve o gerçek bir kenarda.
Yani belgenin *"4002'nin alanı yarışı 0 km/h'de bitiriyor, kenar boyunca kayarak değil"* kuşkusu
doğruymuş, sebebi de bulundu: kayacak bir kenar yoktu, **yol oradan aşağı iniyordu**.

Waypoint kazancının yine de küçük olması şaşırtıcı değil: kurtulan yedi araba 4002 ve 4081'de, ve
o iki rota zaten alanın en az ilerleyen rotaları (75 ve 79 waypoint). Çivilenmemek ilerlemek
değil — ama çivilenmek kesinlikle ilerlememek.

### Halka-bağımsız ölçüler: yürünmüş halka *sürüşte* daha kötü, ve sebebi nişan açısı (2026-08-20)

Bugünkü bulgular yürünmüş halkayı dünya tarafında haklı çıkardı (gerçek yolda geçen oran %49 → %76,
altında zemin olmayan hücre %11,0 → %2,9). Sıradaki soru, tabanın oraya taşınıp taşınmayacağıydı.
İki halka farklı sayıda waypoint taşıdığı için "geçilen waypoint" bu karşılaştırmada anlamsız;
`tools/ring-neutral.py` yazıldı ve **halkadan bağımsız** olanlara bakıldı — kursu hiç bırakmayan
araba (koridora karşı ölçülüyor, halkaya değil), o arabaların bastığı ayrık düğüm, `furthest`,
ve düşen.

| kol | kursta | kursta kalanın düğümü | furthest | düşen |
|---|---|---|---|---|
| kiriş (bugünkü çitle) | **30 / 64** | **20,0 / araba** | **5.299 m** | 6 |
| yürünmüş (bugünkü çitle) | 14 / 64 | 7,5 / araba | 5.237 m | **3** |

**Yürünmüş halka sürüşte açık ara kötü.** Rota rota: 4001 **8/8 → 0/8**, 4041 4/8 → 0/8,
4081 8/8 → 6/8. Buna karşılık 4002'nin `furthest`'ı 235 → **627 m** (2,7 kat) ve düşen yarıya
iniyor.

**İlk hipotez çürüdü.** "Halka yarışın kendi yollarını değil, herhangi bir yolu yürüyor" dendi;
`NFS_WALKONLY=1` eklendi (bacak yalnız koridorun içindeki düğümlerden geçebilir) ve sonuç
**bayt-birebir aynı** çıktı — halka zaten yarışın koridorunda.

**İzler mekanizmayı gösterdi.** 4001'in sekiz arabası da aynı noktada, `(468, 1096)`'da, 0 km/h'de
yığılıyor. Donmuş iz o anın 20 saniye öncesini gösteriyor:

```
t= 62.5 ( 391, 1148)  77 km/h · koridora 1.3 m · direksiyon -0.62 · gaz 0.46 fren 1.00
        · nişan  49 m  66° · hedef 44  34 m  -3° · sonraki 64 m
```

Araba **koridorun 1,3 m içinde**, hedef waypoint **−3°'de yani tam önünde**, ama **nişan noktası
66°'de**. Ön-takip noktası virajın arkasına düşmüş; araba düz yolda tam frenle direksiyonu kırıyor.

**Ve bu sayılabilir bir şey.** `nişan açısı` sayımı eklendi (her araba, her adım):

| rota | halka | ortalama | %>45° | %>90° | kursta |
|---|---|---|---|---|---|
| 4001 | kiriş | **9°** | %6,4 | %0,4 | **8/8** |
| 4001 | yürünmüş | **21°** | %20,0 | %2,0 | **0/8** |
| 4041 | kiriş | 44° | %34,6 | %19,8 | 4/8 |
| 4041 | yürünmüş | 43° | %35,9 | %17,0 | 0/8 |
| 4121 | kiriş | 45° | %35,2 | %17,9 | 0/8 |
| 4121 | yürünmüş | **60°** | %50,6 | %30,6 | 0/8 |

Kursu koruyan tek örnek (4001 kiriş) aynı zamanda nişan açısı **tek haneli** olan tek örnek. Bir
saf-takip denetleyicisinin düz yolda birkaç derecede çalışması beklenir; 44-60° ortalama, hattın
sürülebilir olmadığının değil, **nişanın yanlış yere düştüğünün** ölçüsü.

### Ölçüyü iki kez bozan tuzak: boş değer "açık" demek (2026-08-20)

`NFS_WALKLINE=` ve `NFS_BUNDLE=` — yani düğmeyi *boşaltmak* — `std::env::var(..).is_ok()` için
**set** demek. Kontrol kolunu böyle temizleyen bir kabuk döngüsü iki özdeş kolu koşar, ve çıktı
"iki halka bayt-birebir aynı" gibi görünür. Bugün iki ölçü bu yüzden yanlış okundu (biri
`NFS_BUNDLE` boşken hiç çalışmadı, biri iki kiriş kolunu yürünmüş sanıp karşılaştırdı).

`nfs_sim`'de artık tek bir `knob(name)` var: **boş değer = ayarlanmamış**. 26 okuma ona çevrildi;
`NFS_RESCUE` ve `NFS_SHOTCAM` de aynı tuzaktaydı, onlar da kapatıldı. Sayısal düğmeler zaten
güvenliydi (boş değer `parse` edilemez, varsayılana düşer).

### Nişan açısı hipotezi çürüdü: on altı noktada korelasyon sıfır (2026-08-20)

Bir önceki bölüm şu ilişkiyi kurmuştu: kursu koruyan tek örnek (4001 kiriş) nişan açısı tek haneli
olan tek örnekti, ve yürünmüş halkada açı 9° → 21°'ye çıkarken kursta kalan 8/8 → 0/8'e iniyordu.
İlişki iki uç noktadan okunmuştu; on altı noktada test edildi.

| rota · halka | ortalama nişan | %>45° | kursta |
|---|---|---|---|
| 4001 · kiriş | **9°** | %6,4 | **8/8** |
| 4001 · yürünmüş | 21° | %20,0 | **0/8** |
| 4002 · yürünmüş | 28° | %15,6 | 7/8 |
| 4061 · yürünmüş | 28° | %24,7 | 0/8 |
| 4041 · kiriş | 44° | %34,6 | 4/8 |
| 4081 · kiriş | 60° | %45,2 | **8/8** |
| 4121 · yürünmüş | 60° | %50,6 | 0/8 |
| 4002 · kiriş | **71°** | %51,3 | **7/8** |

**r = −0,01.** İlişki yok. Ve karşı örnekler tek tek yıkıcı: alanın **en kötü** nişan açısına sahip
kolu (4002 kiriş, 71°, adımların yarısından çoğu 45° üstünde) sekiz arabanın **yedisini** kursta
tutuyor; 4081 kirişte 60° ile **8/8**; buna karşılık 4001 yürünmüşte 21° ile **0/8**.

**Ders:** bir rotanın iki ucu bir ilişki değildir. Korelasyonu on altı noktada sormak beş dakika
sürüyor ve mekanizma anlatısını yazmadan **önce** yapılmalıydı. Bu, günün "alan farkı rotalar arası
yayılımdan küçükse kazandı denmez" kuralının aynısının başka kılığı: **tek rotadan çıkarılan hiçbir
şey alan hakkında bir iddia değil.**

Gözlemin kendisi yanlış değil — 4001'in arabası gerçekten koridorun 1,3 m içinde, hedefi −3°'de
iken nişanı 66°'de ve tam frende. O araba için tanı doğru. Alanı açıklamıyor, o kadar.

**Yine de saf-takip yasası uygulandı ve süpürülüyor** (`NFS_PURSUIT=0` rampayı geri getirir), çünkü
gerekçesi korelasyon değil geometri: gerçek bir virajda iki yasa neredeyse aynı şeyi istiyor
(21 m yarıçap, 20 m'de 45° → rampa 0,42, geometri 0,41), yalnız ön-takibin virajın arkasına
düştüğü hâlde ayrılıyorlar (49 m'de 66° → rampa 0,62, geometri **0,19**). Karar süpürmenin.

### Ve saf takip çürüdü: rampa haklı, dürüst geometri burada yanlış (2026-08-20)

`NFS_PURSUIT=1` ile süpürüldü. Her ölçüde birden kaybediyor:

| | waypoint | kursta | kursta kalanın düğümü | furthest | düşen |
|---|---|---|---|---|---|
| **rampa** (kalan) | **1.046** | **30 / 64** | **20,0** | **5.299 m** | **6** |
| saf takip geometrisi | 839 | 27 / 64 | 15,1 | 4.772 m | 7 |

−207 waypoint, **sekiz rotanın yedisinde geride**, hiç beraberlik yok, ve en büyük tek rota −87 —
yani bugünkü kurala göre sağlam bir alan sonucu, tek rotanın taşıdığı bir şey değil.

**Yanlış yasanın niye kazandığı, bu pilotun ne olduğunu söylüyor: bir hattı *takip etmiyor*, bir
noktayı *kovalıyor*.** Saf takip "o noktaya hangi yay ulaşır" sorusunu cevaplar; 49 m ötede 66°'deki
bir noktaya ulaşan yay yumuşaktır — oraya *varmayı* amaçlayan bir araba için doğru, bir saniye
içinde yeniden yola bakması gereken bir araba için işe yaramaz. **Rampanın fazla direksiyonu,
toparlanmanın kendisi.**

Varsayılan rampaya döndürüldü ve doğrulandı (4001 waypoint=46, 4121 waypoint=30, `lvl` koluyla
bayt-birebir aynı). Geometri `NFS_PURSUIT=1` ile duruyor, kaydıyla birlikte.

### Bütün gün kullandığım ölçü yanıltıcıymış: kursu bırakanların 34'ünden 33'ü geri dönüyor (2026-08-20)

Kursu bırakan 34 arabanın ayrılma **anı** kaydedilmeye başlandı — üç saniyelik eşiğin dolduğu an
değil, koridorun kenarını geçtiği adım (`çıkarken:` satırı, `tools/leaving.py` ile sayılıyor):

| ayrılma anında | kaç | oran |
|---|---|---|
| tam kilitte (\|direksiyon\| ≥ 0,84) | 18 | %52,9 |
| **nişan arkada (>90°)** | 17 | %50,0 |
| **hedef arkada (>90°)** | 15 | %44,1 |
| frende (>0,5) | 12 | %35,3 |
| hızlı (>60 km/h) | 8 | %23,5 |
| vazgeçtiği düğüm var | 5 | %14,7 |
| yavaş (<20 km/h) | 3 | %8,8 |
| kaçışta | 2 | %5,9 |

Ayrılmaların yarısında pilot **zaten geriye dönmeye çalışıyor**. Ama asıl bulgu bir sonraki soruda
çıktı: *geri dönüyorlar mı?*

**34 ayrılmanın 33'ü koridora geri dönüyor** — ve azı değil: 22, 32, 38, 47, 50, hatta **55
saniye** (90 saniyelik yarışta). Yani gün boyu başlık sayısı olarak kullandığım *"kursu hiç
bırakmadı"*, bir kez üç saniye çıkıp kalan seksen yedi saniyeyi kursta geçiren arabayı, çıkıp bir
daha dönmeyenle **aynı** sayıyor.

**Doğru ölçü eklendi: kursta geçen sürenin oranı.**

| rota | kiriş: süre / hiç bırakmayan | yürünmüş: süre / hiç bırakmayan |
|---|---|---|
| 4001 | %93,7 · 8/8 | %82,9 · **0/8** |
| 4002 | %92,5 · 7/8 | %90,0 · 7/8 |
| 4021 | %78,9 · 1/8 | %58,2 · 0/8 |
| 4041 | %87,3 · 4/8 | %83,6 · **0/8** |
| 4061 | %67,0 · 1/8 | **%77,6** · 0/8 |
| 4081 | %99,5 · 8/8 | %93,6 · 6/8 |
| 4102 | %62,5 · 1/8 | %54,4 · 1/8 |
| 4121 | %74,5 · 0/8 | %61,4 · 0/8 |
| **ALAN** | **%82,0** · 30/64 | **%75,2** · 14/64 |

**İkili ölçü 30'a 14 diyor — 2,1 kat, felaket gibi. Süre %82,0'a %75,2 diyor — 1,09 kat.** Ve
4001'de sekiz arabanın sekizi de bir kez çıkıyor ama yarışın **%83'ünü** kursta geçiriyor. 4061'de
ikili ölçü 1/8 → 0/8 ile kötüleşme gösterirken süre %67 → %78 ile **iyileşme** gösteriyor: ölçü
o rotada verdiği hükmü ters çeviriyor.

**Karar değişmiyor ama gerekçesi düzeliyor.** Taban kirişte kalıyor: süre farkı −6,8 puan ve rota
başına en büyük sapma 20,7 puan, yani günün kuralına göre sağlam bir alan sonucu. Ama "yürünmüş
halka sürüşte açık ara kötü" ifadesi fazla sertti; doğrusu **ölçülü biçimde kötü, ve bir rotada
daha iyi**.

**Ve bu, günün dördüncü okuma tuzağı:** bir olayın *olup olmadığını* sayan ölçü, o olayın
*sürdüğünü* varsayar. Kursu bırakma sürmüyor.

### Ayrılmalar 34 araba değil, **12 yer** — ve ilk dördü %68'ini açıklıyor (2026-08-20)

Ayrılma noktaları kümelenince mesele başka bir şeye dönüştü. 60 m'lik kümelerle:

| rota | araba | yer | ne zaman |
|---|---|---|---|
| 4102 | **7** | (52, −26) | t=33-44 s |
| 4061 | **6** | (−2431, 1748) | t=20-28 s |
| 4121 | **6** | (−315, 1372) | t=37-49 s |
| 4021 | **4** | (−268, 1395) | t=37-46 s |
| … | 1-3 | 8 yer daha | |

**34 ayrılma, 12 ayrı yer, ve ilk dördü 23 arabayı (%68) açıklıyor.** Dahası 4121'in ve 4021'in
noktaları birbirine **52 m** — aynı kavşak, iki farklı rotada, on araba.

Ve 4102'nin yedi arabası aynı yerde **bayt-birebir aynı durumda** çıkıyor: 63-66 km/h, tam kilit
+0,85, **tam fren 1,00**, nişan 37 m'de **−176°**, hedef 80 m'de −123°. Yani kusur arabanın değil
**yerin** bir özelliği; sekiz farklı araba aynı yerde aynı şeyi yapıyor.

O ortak kavşağa bakıldı (`NFS_HOLE=-290,1385`): 4121 için kavşak 276'nın iki kolu **2,8 m arayla**
farklı hatlarda — `Network` başlığının uyardığı paralel şerit birleşimi. Koridor testi ikisini de
"YARIŞ HATTI" sayıyor, yani koridora dayanan hiçbir kural bunları ayıramaz.

**Ve hedefin 80 m'de olması, `PASSED_NEAR`'ın 60 m'lik bırakma yarıçapının dışında** demek: araba
hedefini geçmiş olsa bile bırakamıyor, hedef arkada kilitli kalıyor.

**Buradan çıkan kural yeniden yazılmadı, çünkü zaten çürütülmüş.** "Bir süredir waypoint
kazanmayan pilot yerini yeniden bulsun" `pilot.rs`'de kayıtlı: 4, 8 ve 16 saniyelik aralıkların
üçünde de kaybediyor (462 → 369 / 412 / 435 waypoint), ve `covered()` zaten o denemenin yirmi iki
katlık sahte kazancını yakalamak için yazılmış. Kodu okumak deneyi yeniden yapmaktan kurtardı.

**Denenmemiş olan tek aday, aynı yorumda kayıtlı:** *hıza göre değişen kilit sınırı* — dururken tam
kilit (kaçış makinesi ona bağlı), 60 km/h'de az. Düz sınır çürütülmüştü çünkü **duran** arabadan da
kilidi alıyor ve kenardan dönemeyen araba düşüyor (düşen üç katına çıkmıştı). Bugünkü sayım tam
buraya işaret ediyor: 34 ayrılmanın **18'i tam kilitte**, 8'i 60 km/h üstünde. `NFS_CAPFAST`
eklendi ve süpürülüyor.

### Hıza göre kilit sınırı da çürüdü — ve iki deney artık aynı şeyi söylüyor (2026-08-20)

`NFS_CAPFAST` süpürüldü. Üç ayarın üçü de her ölçüde kaybediyor:

| 60 km/h'de kalan kilit | waypoint | kursta süre | hiç bırakmayan | düşen |
|---|---|---|---|---|
| **sınır yok** (kalan) | **1.046** | **%82,0** | **30** | **6** |
| %70 | 872 | %74,2 | 26 | 6 |
| %50 | 861 | %76,0 | 27 | 6 |
| %35 | 682 | %70,7 | 26 | 8 |

Günün kuralına göre sağlam: −174, −185, −364 ve en büyük tek rota −107, −82, −120.

**Ama teşhis doğruymuş.** Düz sınırın çürütülme sebebi düşenlerin üçe katlanmasıydı (2 → 5, 6);
hıza göre olan bunu **tekrarlamıyor** (6, 6, 6, 8). Yani "duran arabadan kilidi alma" teşhisi
doğruydu ve düzeltilmesi fikri kurtarmadı — fikir kendi başına yanlış.

**Ve bugün iki bağımsız deney aynı şeye varıyor.** Nişan yana savrulunca daha az kilit isteyen saf
takip geometrisi −207 ile kaybetti; hızda daha az kilit isteyen sınır −174 ile −364 arasında
kaybetti. **Bu pilotu daha az direksiyon kırdıran her yol daha kötü.** Sebebi direksiyon yasasının
yanında yazılı: bir hattı takip etmiyor, bir noktayı kovalıyor, ve fazla direksiyon toparlanmanın
kendisi. **Kalan kaldıraç direksiyon tepkisi değil.**

Sabit `CAP_FAST = 1.0` olarak, kaydıyla birlikte duruyor (`NFS_CAPFAST` ile geri açılır).

### 4102'nin yedi arabası: kusur pilotta değil, halkada — ve halkayı dürüstleştirmenin bedeli hep aynı (2026-08-20)

Ayrılmaların en büyük öbeği (4102, yedi araba) izlendi. Ayrılmadan sekiz saniye önce araba
**kusursuz sürüyor**:

```
t= 25.4 ( -42, -140)  88 km/h · koridora 3.4 m · direksiyon 0.06 · gaz 0.96 fren 0.00
        · nişan 23 m -6° · hedef 10  19 m -90° · sonraki 50 m
```

Koridorun 3,4 m içinde, direksiyon neredeyse düz, nişan tam önünde. Ama **hedefi 19 m yanında
duruyor** ve açısı −60° → −116° diye kayıyor: araba yanından geçiyor, "geçilmiş" sayılmıyor
(`PASSED_NEAR`'ın kuralı kursun kendi yönünde geçmeyi istiyor), `sonraki` de hedeften uzak olduğu
için ilerleme de olmuyor. Hedef orada kilitleniyor, sonra arkaya düşüyor, ve tam kilit + tam fren
gelip arabayı dışarı atıyor.

**Halkaya sorunca sebep göründü:** `NFS_CURVE=-45,-145` → waypoint 9'un **yarıçapı 25 m, yani
41 km/h**. Araba oradan 89 km/h ile geçiyor çünkü **yol düz** — koridorun 3,4 m içinde. Halka
dönüyor, yol dönmüyor. Ve tek köşe değil: o rotanın **104 waypoint'inin 58'i koridorun dışında**.

**Yeni aday, kırpma (`NFS_TRIM=1`):** koridorun dışındaki waypoint'ler yarışın gitmediği yerler,
atılsınlar. Halka 104 → 46, 159 → 95, 130 → 74 waypoint'e iniyor; en büyük boşluk 272-394 m.

| | kursta süre | düşen | furthest | hiç bırakmayan |
|---|---|---|---|---|
| kırpılmamış | **%82,0** | 6 | 5.299 m | **30** |
| koridora kırpılmış | %73,6 | **1** | **5.723 m** | 20 |

**Düşen 6 → 1.** Bu, düşüşlerin bugün kurulan zincirinin bağımsız doğrulaması: halka blokların
üstünden geçiyor → araba takip ediyor → zemin yok → düşüyor. Koridor dışı waypoint'leri atmak
zincirin ilk halkasını kesiyor ve düşüşler neredeyse bitiyor. `furthest` de günün en iyisi.

**Ama kursta geçen süre 8,4 puan düşüyor** (rota rota −34'e kadar; en büyük tek rota alan
farkından küçük, yani sağlam). Sebebi de belli: hayatta kalan waypoint'ler arası 394 m'lik boşluğun
düz çizgisi koridordan çıkıyor.

**Ve buradaki asıl bulgu üçüncü kez aynı şey:** halkayı dürüstleştirmenin bedeli hep yaklaşık aynı.
Ağda yürünmüş halka %75,2, koridora kırpılmış halka %73,6, kurgusal kiriş halkası **%82,0**.
Kurgusal halka kursta kalmayı *daha iyi* sağlıyor — çünkü köşeleri kesiyor, yani **daha yumuşak**,
ve koridor (bütün hatların birleşimi) cömert olduğu için o yumuşak çizgi çoğunlukla içeride
kalıyor. Dürüst çizgilerin keskin dönüşlerini bu pilot süremiyor.

Kırpma varsayılan yapılmadı; düşen sayısındaki 6 → 1 kazancı kaydıyla `NFS_TRIM=1` altında duruyor.
Bir sonraki adayın şekli de buradan belli: **koridor içinde kalan ama yumuşak** bir halka — yani
kırpılmış waypoint'ler arasını düz çizgiyle değil, ağı yürüyerek doldurmak.

### Halkayı atmak yerine **çekmek**: alan 1.046 → 1.317 waypoint, düşen 6 → 3 (2026-08-20)

Kırpma bölümünün kendi sonucu "koridor içinde ama yumuşak bir halka" diyordu. Waypoint'i atmak
yerine **koridora çekmek** ikisini birden veriyor: halkanın şekli korunuyor, noktalar kursun
içine giriyor. `Fix` artık en yakın noktayı da taşıyor (`Fix::at`), yani izdüşüm bir sorguya
düşüyor.

Nereye çekileceği ölçüldü ve fark büyük:

| halka | waypoint | kursta | kursta kalanın düğümü | furthest | düşen | kursta süre |
|---|---|---|---|---|---|---|
| kiriş, dokunulmamış | 1.046 | 30 | 20,0 | 5.299 m | 6 | **%82,0** |
| **merkeze çekilmiş** (yeni varsayılan) | **1.317** | **32** | **22,1** | **5.784 m** | **3** | %78,9 |
| kenara çekilmiş | 1.292 | 20 | 22,6 | 5.030 m | 4 | %72,2 |
| koridora kırpılmış | — | 20 | 23,6 | 5.723 m | **1** | %73,6 |

**Kenar ile merkez arasındaki fark, deneyin kendi kusurunu gösterdi:** ilk sürüm waypoint'i
koridorun *kenarına* çekiyordu, yani halka tam sınır boyunca gidiyor ve en küçük hata arabayı
dışarı atıyor. Kursta kalan 20, süre %72,2. Merkeze çekince aynı fikir 32 araba ve %78,9 veriyor.
Bir ölçüm kötü çıktığında önce fikrin mi yoksa uygulamanın mı ölçüldüğü sorulmalı.

**Caveat, açıkça:** "geçilen waypoint" yakınlıkla ölçülüyor, dolayısıyla halkayı arabaların zaten
sürdüğü yere doğru taşımak o sütunu kendiliğinden şişirir — deney kendi ödevini not veriyor.
Bağımsız olanlar: **`furthest` +485 m** (sekiz rotanın altısında ileride, en büyük tek rota +306,
yani kurala göre sağlam), **düşen 6 → 3**, **kursta kalan 30 → 32**, ve kursta kalan araba başına
düğüm 20,0 → 22,1. Dört bağımsız ölçü aynı yöne gidiyor; bir tanesi — koridorda geçen süre,
%82,0 → %78,9 — ters yöne.

Bugün halkayı dürüstleştirmenin dört yolu denendi (ağda yürüme, kırpma, kenara çekme, merkeze
çekme) ve yalnız sonuncusu sürüşü ileri taşıdı. Ortak ders: **halkanın yumuşaklığı, doğruluğu
kadar önemli** — ve çekmek doğruluğu yumuşaklığı bozmadan veren tek işlem.

### Süpürmenin gürültü tabanı ölçüldü: ±37 waypoint (%2,8) — ve tek bir rotanın çatallanması (2026-08-20)

Yeni varsayılan `PULL_TO = 0` ile daha önce `NFS_PULL=0.001` ile koşulan kol arasında 35 waypoint
fark çıktı. Aradaki tek fark, waypoint konumunda **1,2 santimetre**. Bu, ölçünün kendisi hakkında
bir soru: fiziksel olarak anlamsız bir kayma alanı ne kadar oynatıyor?

Dört hedef (0 · 0,0005 · 0,001 · 0,002 — en fazla **2,4 cm**), sekiz rota:

| rota | 0 | 0,0005 | 0,001 | 0,002 | yayılım |
|---|---|---|---|---|---|
| 4001 | 366 | 366 | 366 | 366 | **0** |
| 4002 | 56 | 56 | 56 | 56 | **0** |
| 4021 | **100** | **142** | **142** | **142** | **42** |
| 4041 | 252 | 252 | 252 | 253 | 1 |
| 4061 | 162 | 155 | 155 | 156 | 7 |
| 4081 | 154 | 154 | 154 | 154 | **0** |
| 4102 | 108 | 108 | 108 | 108 | **0** |
| 4121 | 119 | 119 | 119 | 119 | **0** |
| **TOPLAM** | 1.317 | 1.352 | 1.352 | 1.354 | **37** |

**Simülasyon büyük ölçüde kararlı** — sekiz rotanın altısı bayt-birebir aynı. Gürültünün tamamına
yakını `Paths4021`'in **iki sonuç arasında çatallanması**: 100 ya da 142, arada bir değer yok. Bu
kayan nokta hatasının birikmesi değil, bir arabanın bir kavşakta öbür kolu seçmesi.

**Sonuç, bugünün bütün sayılarını kalibre ediyor: alan farkı ~40 waypoint'in altındaysa sonuç
değildir.** Bu ölçüyle:

* Bugün kabul edilen çekme (+271) ve çürütülen saf takip (−207) ile kilit sınırı (−174…−364)
  gürültünün çok üstünde — hepsi sağlam.
* **Çitin +11'i gürültünün içinde.** Zaten öyle yazılmıştı: çitin gerekçesi waypoint değil, 336 az
  müdahale ve çivilenen arabanın 8 → 1 olmasıydı. İyi ki ona dayanmamışız.
* **`PASSED_NEAR` 60'ın 80'e (+13) ve 100'e (+30) üstünlüğü de gürültünün içinde.** Bu, aynı gün
  rota dağılımıyla varılan sonucun bağımsız ikinci kanıtı: o ince ayarlar ölçülmüş değil.

Ve yeni varsayılanın (1.317) çatallanmanın şanssız tarafına düşmesi bir kusur değil, gürültü;
0,0005 ile 1.352 veriyor. Peşine düşülecek bir şey yok.

### Halka değişince ayrılmanın **imzası** değişti — ve `GRIP` gürültünün içine düştü (2026-08-20)

Çekilmiş halka varsayılan olunca ayrılma sayımı yeniden koşuldu. Sayı neredeyse aynı (34 → 32) ama
**sebep bambaşka**:

| ayrılma anında | kiriş | çekilmiş |
|---|---|---|
| frende (>0,5) | 12 (%35) | **21 (%66)** |
| hızlı (>60 km/h) | 8 (%24) | **19 (%59)** |
| hedef arkada | 15 (%44) | 7 (%22) |
| tam kilitte | 18 (%53) | 6 (%19) |
| nişan arkada | 17 (%50) | 4 (%13) |

**Çekme, teşhis ettiği kusuru düzeltmiş:** "hedef yanda kalıyor, arkaya düşüyor, tam kilit geliyor"
imzası yarıdan beşte bire indi. Yerine geçen imza bir **hız** imzası: arabalar 43-68 km/h ile,
nişanı 45-96° yanda olan bir virajı alırken çıkıyorlar.

Ayrılma yerleri ise **aynı kaldı** (4102 hâlâ 7 araba, 4061 hâlâ 6, 4121 8'e çıktı). Yani o dört
yer pilotun ne yaptığından bağımsız olarak arabaları düşürüyor; değişen yalnız *nasıl* düştükleri.

**Yeni imza `GRIP`'i işaret ediyordu; süpürüldü ve gürültünün içine düştü:**

| `GRIP` | waypoint farkı | kursta süre | hiç bırakmayan | düşen |
|---|---|---|---|---|
| 5 (fiziksel) | **±0** | %75,6 | 24 | 2 |
| 6,5 | −26 | %78,6 | 27 | 3 |
| **8 (kalan)** | — | **%78,9** | **32** | 3 |
| 12 | −50 | %76,1 | 27 | 2 |

Kiriş halkasında 8 ile 5 arasında ~100 waypoint vardı; çekilmiş halkada fark **tam sıfır**. Bu,
`GRIP`'in belgesinde yazılı hikâyenin nihayet göründüğü yer: kiriş, kirişin açısı yolun eğriliğini
abarttığı için yüksek bir eşik istiyordu; halka dürüstleşince eşik **önemsizleşti**. Fiziksel değer
kazanmıyor, ama artık kaybetmiyor da. 8 yerinde kalıyor (ikincil ölçülerde önde) ve kaydı düzeldi.

### Ön-takip mesafesi tersine döndü: 0,9 s → 1,8 s, alan +100 waypoint (2026-08-20)

`LOOKAHEAD_PER_SPEED` kiriş halkasında süpürülmüş ve 0,9 s'de sabitlenmişti; belgesinde 1,5 ve
1,8'in **kaybettiği** yazılı (724 ve 858'e karşı 869). Halka çekilince aynı süpürme tersine döndü:

| saniye | waypoint | kursta süre | hiç bırakmayan | düşen | furthest |
|---|---|---|---|---|---|
| 0,6 | 1.302 | %77,0 | 33 | 3 | 5.514 m |
| 0,9 (eski) | 1.317 | **%78,9** | 32 | 3 | 5.784 m |
| 1,3 | 1.378 | %73,0 | 16 | **0** | 5.878 m |
| 1,5 | 1.375 | %70,2 | 12 | 5 | 5.785 m |
| **1,8 (yeni)** | 1.417 | %77,7 | 19 | 1 | **6.023 m** |
| 2,4 | **1.433** | %72,0 | 15 | 7 | 5.377 m |

**+100 waypoint** — ölçülen gürültü tabanının (±37) iki buçuk katı — ve en büyük tek rota +86, yani
alan sonucu. `furthest` +239 m, düşen 3 → 1.

**Koridorda geçen süre karşı çıkıyor gibi görünüyor ve çıkamıyor.** Alan ortalaması 1,2 puan
düşüyor ve bunun **tamamı** `Paths4081`'in çöküşü (%99,1 → %53,6). Sekiz rotanın beşinde süre
*artıyor*, üçünde büyük ölçüde: 4102 **+19,0**, 4061 **+10,3**, 4121 **+7,0**. 1,2'lik bir farkın
karşısında tek rotanın 45,5'i — bu, günün kuralının tanımı gereği taşınan bir sonuç.

**Bedel gerçek ve tek bir viraj.** 4081'de 0,9 ile 1,3 arasında bir uçurum var (%99,1 → %50,3 ve
orada kalıyor: 49,6 · 53,6 · 48,1) ve arabaları `(−354, −180)`'de, 48 km/h ile, nişanı 33 m'de 52°
iken çıkıyor — kısa ön-takibin aldığı virajı kesiyorlar. Sıradaki iş o viraj; alanın taşındığı bir
sabiti tutmak için sebep değil.

**Ve buradaki asıl ders bugünün üçüncü tekrarı:** bir sabitin hangi kursa karşı oturtulduğu,
sabitin kendisi kadar önemli. `GRIP` halka değişince **önemsizleşti** (5 ile 8 arasında tam sıfır
fark), ön-takip ise **tersine döndü**. İkisi de kiriş halkasının kurgusuna oturtulmuştu.

### Nişan noktası halkadan gelmiyor: **ağdan** geliyor — ve bu, çekmenin niye işe yaradığını açıklıyor (2026-08-20)

4081'in çöküşü kovalanınca pilotun nasıl çalıştığı hakkında bugüne kadar örtük kalan bir şey
çıktı. Nişan noktası bir waypoint **değil**: pilot `self.at`'ten başlayıp **ağı yürüyor**
(`step_avoiding`), ve halka yalnız o yürüyüşe hangi yöne gideceğini söylüyor (`toward` = hedef
waypoint). Yani "halkayı koridora çektik" demek nişanı koridora sokmak değil.

Ölçü bunu doğruladı: halkanın %100'ü koridorun içindeyken **4081'de nişan adımların %36,7'sinde
koridorun dışında** (4061 %0,7, 4001 %0,0). O da tam olarak uzun ön-takibin kırdığı rota.
Kavşak 206'nın beş kolundan üçü 30-40 m doğudaki paralel bir hatta; yürüyüş uzayınca oraya
sapıyor.

**Ve bu, çekmenin niye bu kadar işe yaradığını açıklıyor:** halkayı taşımak nişanı taşımıyor,
**kol seçimini** taşıyor. `step_avoiding` bir sonraki düğümü `toward`'a en yakın olana göre seçer;
`toward` koridora çekilince seçim de koridorun içindeki kola kayıyor.

**Kural denendi ve alanda çürüdü.** "Zaten önünde kullanılabilir bir nişan varsa, hattın dışına
çıkan adımı atma" (`NFS_AIMLINE=1`):

| | waypoint farkı | kursta süre | hiç bırakmayan | düşen | furthest |
|---|---|---|---|---|---|
| bugünkü | — | **%77,7** | 19 | **1** | 6.023 m |
| hatta kal, 20 m | −49 | %76,3 | 18 | 2 | 5.940 m |
| hatta kal, 12 m | −94 | %75,5 | 21 | **1** | **6.323 m** |

Motive eden rotada **tam istendiği gibi** çalışıyor: 4081 **%53,6 → %96,5** (20 m'de), nişanın
koridor dışında kalması %36,7 → %2,1. Ama düzeltmediği dört rotayı bozuyor: 4021 %94 → %77,
4041 %91 → %82, 4061 %65 → %51, 4102 %68 → %57.

**İlk denemenin hiçbir şey yapmaması da ayrı bir ders:** işaretin varsayılan genişliği 40 m, ve
paralel hat 30-40 m ötede — yani "hatta" sayılıyordu. Koşu bayt-birebir aynı çıktı. Bir kuralın
etkisiz görünmesi, kuralın yanlış olduğu anlamına gelmiyor; önce **eşiğinin ölçtüğü şeyi ölçüp
ölçmediği** sorulmalı.

### Havada duran siyah levhalar: şehir opak çiziliyor, oysa Bayview'in ağaçları kesim kartları (2026-08-20)

Oyun çalıştırıldı ve iki görsel şikâyet geldi: *havada duran cisimler* ve *binaların içleri
gözüküyor*. İkisi de tek bir kareyle üretildi (`nfs_city`, `STREAML4RA`, şehir merkezi) ve ikisi de
tek bir sebebe indi.

**"Binaların içleri" değil.** `NFS_DOUBLE=1` — arka yüzleri de çiz — ile aynı kare alındığında
921.600 pikselin yalnız **383'ü** değişiyor (%0,04). Winding doğru; hiçbir binanın içi görünmüyor.

**Havada duranlar ağaç.** Yakınlaştırınca siyah levhaların içinde ağaç siluetleri seçiliyor:
Bayview'in bitki örtüsü **düz kartlara kesilmiş dokular**. Şehir `Material::new(...).with_baked_lit(...)`
ile **opak** doğuyor, yani kartın saydam olması gereken kısmı koyu dolu çiziliyor — gökyüzünde
duran koyu levhalar tam olarak bu.

**Ve düzeltmesi bu motorda yok.** Bu yolda tek kaldıraç `with_transparent`, yani alfa
*karıştırma*. Bir kesimin istediği alfa *testi* (`alpha < cutoff` → `discard`) renderer'ın
G-buffer shader'ında **var** ama yalnız glTF yükleyicisinden besleniyor, ve `with_baked_lit`
G-buffer'dan geçmiyor.

Karıştırma ağaçları düzeltiyor, **decal'ları kaybettiriyor**: dairesel taş deseni, yol kiri, şerit
kaplamaları — hepsi altındaki yüzeyle eş düzlemde, ve saydam geçiş onları orada tutmuyor; meydan
düz griye dönüyor.

**Eşikle ayrılamıyor, ölçüldü.** `STREAML4RA`'nın 1.500 dokusunun 235'i saydamlık taşıyor ve
oranları %0 ile %100 arasında **düzgün** dağılmış (çeyrekler %48 / %68 / %82). 11 dokuda decal'lar
doğru ve ağaçlar levha; 235'te ağaçlar doğru ve meydan gri; aradaki her değer ikisinden biri.

**Sonuç:** `CUT_SHARE = 0` ile kapalı gidiyor (siyah levhalar kalıyor, çünkü zemin karenin çok daha
büyük kısmı), `NFS_CUT=1` takasın öbür yüzünü gösteriyor. Şehrin motordan istediği tek şey belli
ve dar: **`baked_lit` materyalinde bir alfa kesim eşiği.** Motor bilerek pinli olduğu için burada
değiştirilmedi; bu, karşı tarafa geçirilecek bir istek.

### "Araba hızı tatmin etmiyor": tavan 103 km/h, ve sebebi vitesler değil (2026-08-20)

Oyun sürüldü ve hız şikâyeti geldi. Ölçüldü — `nfs_sim` artık her arabanın **en yüksek hızını**,
o andaki vitesini, devrini ve tekerlek hızını basıyor; `NFS_FLATOUT=1` gazı kökleyip freni
kapatıyor, `=2` direksiyonu da sıfırlıyor.

**Alan 95-105 km/h'de tavan yapıyor** ve `Paths4001`'de sekiz arabanın sekizi de tam **102 km/h**.
Aynı sayıda buluşan sekiz araba yol değil, tavan demektir.

| ölçü | değer |
|---|---|
| tepe hızda vites | **4** (dizide 0=geri, 1=boş, 2=1.) → yani **3. vites** |
| tepe hızda devir | 4.768 / 6.500 |
| tepe hızda tekerlek torku | 1.142 Nm |
| yarış boyunca en yüksek vites | **4** — 5. ve 6. hiç kullanılmıyor |
| viteslerde geçen süre | 1.:%27 · 2.:%57 · 3.:%16 |
| tekerlek jant hızı / yol hızı | 104 / 103 km/h → **kayma %0** |

**Vitesler suçsuz.** Oranlar `[-3,657 · 0 · 3,321 · 1,902 · 1,308 · 1,000 · 0,900]`, son sürat
her viteste kırmızı çizgide: 2.'de 98, 3.'de **142**, 4.'de 186, 5.'de **207 km/h**. Araba 3.'de
4.800 devirde takıldığı için 6.500'e ulaşamıyor ve **4. vitese hiç geçmiyor** — yani "vites
atmıyor" bir sonuç, sebep değil.

**Pilot da suçsuz.** `NFS_FLATOUT=1` (gaz kökte, fren yok) tavanı 103 km/h'de bırakıyor;
`=2` (direksiyon da sıfır) daha da düşürüyor, çünkü araba yoldan çıkıyor.

**Ve fizik tarafında dört şüpheli tek tek eleniyor:**

* **Sürükleme değil.** `Cd 0,32 · alan 2,2 m²` → 103 km/h'de **353 N**. İtiş 1.142 Nm / 0,31 m =
  **3.684 N**. Aradaki 3.331 N, 1.220 kg'da 2,7 m/s² eder; araba 50 saniye boyunca 3. vitede ve
  hızlanmıyor.
* **Patinaj değil.** Tepe hızda kayma **%0** (jant 104, yol 103 km/h).
* **Çekiş kontrolü kelepçesi değil.** `TC_TARGET_SLIP = 0,2` ile üst sınır `(v+0,2v)/r` = 110 rad/s;
  tekerlek 93'te, yani kelepçeye değmiyor.
* **Fren değil.** `NFS_FLATOUT` freni tamamen kapatıyor ve tavan değişmiyor.

**Geriye kalan tek yer, sürüş torkunun lastiğe bindiği yol.** 3.684 N'luk itiş, 353 N'luk
sürüklemeye karşı, %0 kaymada dengeye oturuyor — oysa kuvvet üretmek kayma ister ve motorun kendi
testi *"sıfır kaymada boyuna kuvvet sıfırdır"* diyor. Denge orada kurulamaz. Bu, `gizmo-physics-
dynamics`'in tekerlek-lastik eşlemesine ait bir soru; motor **bilerek pinli** olduğu için burada
değiştirilmedi, ölçüyle birlikte karşı tarafa geçirilecek.

Ölçüm aletleri kaldı: `NFS_FLATOUT`, ve her koşuda basılan en yüksek hız / vites / devir / kayma
satırları.

### Ve hız tavanının sayısı çıktı: aktarma 4.700-8.900 N sunuyor, arabaya 800-3.200 N ulaşıyor (2026-08-20)

Bir önceki bölüm tavanı bulmuştu; şimdi **kaç newton** olduğu da ölçüldü. `NFS_RESIST=1` araba 0'ın
hareketinden direnci çözüyor: saniyede bir hız, ivme ve aktarmanın sunduğu tork alınıyor, `m·a` ile
karşılaştırılıyor.

**Önce kendi okumamı düzelttim.** İlk tabloda `itiş − m·a` sütununa "direnç" demiştim; o direnç
değil, **sunulan itişle arabaya ulaşan kuvvetin farkı**. Sürükleme onun onda biri.

| t (s) | hız km/h | itiş N | ulaşan N | kayıp N | tekerlek yükü |
|---|---|---|---|---|---|
| 10 | 32 | 7.243 | 3.227 | 4.016 | 0,89 |
| 12 | 51 | 7.160 | 2.202 | 4.958 | 1,01 |
| 14 | 63 | 5.247 | 1.746 | 3.501 | 1,06 |
| 16 | 73 | 5.244 | 1.304 | 3.941 | 1,06 |
| 18 | 80 | 4.992 | 887 | 4.105 | 1,35 |
| 19 | 83 | 4.760 | 821 | 3.939 | 1,07 |

**Kayıp hızdan bağımsız ve sabit: ~3.800 N.** Bir v² sürüklemesi değil (o 83 km/h'de 230 N eder),
yuvarlanma direnci de değil (bu Crr ≈ 0,32 demek olurdu, normalin yirmi katı).

Ve tavan tam buradan çıkıyor: 3. viteste 4.800 devirde itiş **3.684 N**, kayıp **~3.800 N** — itiş
kaybın altına düştüğü anda ivme biter. Araba 103 km/h'de durur, 6.500 devre ulaşamaz, 4. vitese
geçemez.

**Dört şüpheli daha elendi:**

* **Sürtünme çemberi değil:** süspansiyon yükü boyunca **1,0 × ağırlık** (tabloda 0,89–1,35), yani
  `μ·Fz` tavanı aç değil.
* **Fren değil:** `brake_torque = brake_input · max · bias` ve `NFS_FLATOUT` girdiyi sıfırlıyor.
* **Aero değil:** `q = ½ρv²` doğru yazılmış, 103 km/h'de 353 N.
* **Tekerlek sönümü değil:** viskoz sönüm yalnız *havadaki* tekerleğe uygulanıyor (motorun kendi
  yorumu bu hatanın bir zamanlar var olduğunu ve düzeltildiğini yazıyor).

**Geriye kalan:** tekerleğin tork dengesi ile şasiye uygulanan boyuna kuvvet birbirini tutmuyor.
Tekerlek `drive_torque = reaction_torque` noktasına oturuyor — orada tepki torkunun ima ettiği
kuvvet 3.684 N — ama arabaya ulaşan 821 N. Bu `gizmo-physics-dynamics`'in tekerlek→şasi
eşlemesine ait ve motor bilerek pinli; ölçüsüyle birlikte karşı tarafa geçirilecek.

Alet repoda: `NFS_RESIST=1`.

### DÜZELTME — motor suçsuz: düz zeminde araba 179 km/h yapıyor (2026-08-20)

Bir önceki bölüm "aktarma 4.700-8.900 N sunuyor, arabaya 800-3.200 N ulaşıyor · sabit ~3.800 N
kayıp · `gizmo-physics-dynamics`'in tekerlek→şasi eşlemesine ait" diye bitiyordu. **Bu hüküm
yanlıştı ve şimdi çürütüldü.**

Önce kodu okumak iki hipotezi eledi: `tire_force` ile `reaction_torque` **aynı** `final_long`'dan
türüyor (yani birbirini tutmamaları imkânsız), ve `apply_force_at_point` doğrusal bileşeni
tam uyguluyor (`vel.linear += force * inv_mass * dt`).

Sonra `nfs_top` yazıldı: düz zemin, tam gaz, direksiyon sıfır, şehir yok, varlık yok — aracın
sayıları (1.220 kg, oranlar, son sürüş 4,083, tepe 216 Nm, kırmızı çizgi 6.500, r = 0,31) doğrudan
kurulup motorun `update_vehicle`'ı çağrılıyor. Sonuç:

| ölçü | değer |
|---|---|
| 0-100 km/h | **13 s** |
| 0-150 km/h | 27 s |
| en yüksek hız | **179 km/h** |
| vitesler | 1 → 2 → 3 → 4, sırayla ve zamanında |

**Yani şehirdeki 103 km/h tavanı aktarmanın değil, kursun.** Ölçtüğüm "sabit ~3.800 N kayıp"
viraj sürüklemesiydi: yarış hattındaki araba sürekli dönüyor, yanal lastik kuvveti sürtünme
çemberinden boyuna kuvveti yiyor, ve bu hıza kabaca bağımsız göründüğü için sabit bir kayıp gibi
okundu. `NFS_FLATOUT=1` freni kapatıyor ama **direksiyonu kapatmıyor** — o yüzden tavan orada da
103'te kalmıştı.

**Ders, ve bugün üçüncü kez:** bir ölçü "sabit ve açıklanamaz" görünüyorsa, önce ölçünün içinde
kalan değişken aranır. Kurstaki bir arabadan aktarma sorusu sorulamaz; sorunun cevabı ancak kursun
olmadığı yerde alınır.

**Geriye kalan gerçek soru bir ayar sorusu:** 0-100 için 13 saniye, NFSU2'nin kendi 240SX'i için
fazla. Oyunun kendi verisiyle sürülen bir araba arcade hissi vermiyorsa bakılacak yer tork eğrisi
ve `GLOBALB`'den okunan katsayılar — motor değil.

### Kök: araba yavaş değil, **stok bir 240SX** — ve veri doğru okunuyor (2026-08-20)

Zincirin sonuna gidildi. `nfs_top` artık arabanın kendi dokuz noktalı eğrisini ve oyunun kendi
yükseltme verisini kullanıyor (`NFS_ENGINE`, `NFS_GEARBOX`), ve tutuşla torku ayrı ayrı oynatan
iki düğmesi var (`NFS_GRIPD`, `NFS_TORQUE`).

| kurulum | 0-100 | 0-150 | en yüksek |
|---|---|---|---|
| **stok** | **11 s** | 19 s | **231 km/h** |
| motor 3 | 10 s | 16 s | 254 km/h |
| şanzıman 3 | 11 s | 19 s | 232 km/h |
| tam yükseltilmiş | 10 s | 16 s | 249 km/h |

**Neyin bağladığı ölçüldü:** tutuş ×1,5 tek başına **hiçbir şey** değiştirmiyor (11 s), tork ×1,5
orta menzili açıyor (0-150: 19 → 15 s), **ikisi birden 0-100'ü 8 saniyeye** indiriyor. Yani stokta
ne tutuş ne tork tek başına bağlıyor; araba sadece güçlü değil.

**Ve veri doğru okunuyor — birim hatası yok.** Eğri 5.450 devirde **155 hp** veriyor:

```
  3900 rpm · 200 Nm → 110 hp      5450 rpm · 203 Nm → 155 hp
  4675 rpm · 216 Nm → 142 hp      6225 rpm · 170 Nm → 149 hp
```

Gerçek KA24DE: **155 hp / 210 Nm / ~1250 kg / 0-100 ~9 s**. NFSU2'nin stok 240SX'i de 155 hp.
Eğri ft·lb olsaydı 216 ft·lb = 293 Nm = 193 hp çıkardı, ki stok bir 240SX için fazla — yani
N·m okuması doğru.

**Kök bu: kusur yok.** Araba yavaş, çünkü **stok bir 240SX yavaş**, ve fizik dürüst davranıyor
(11 s'ye karşı gerçeğin ~9 s'si; aradaki fark aktarma kayıpları ve lastik modeliyle açıklanır).
Şehirde 103 km/h'de tavan yapması da ayrı bir şey değil — o kursun virajları.

**Geriye kalan bir tasarım kararı, hata değil:** NFSU2 bir simülatör değil ve kendi arabaları
gerçekten olduğundan sert hızlanır. O his isteniyorsa gereken çarpan **ölçüldü**: tork ×1,5 **ve**
tutuş ×1,5 → 0-100 **8 s**, son sürat 278 km/h. Bu, oyunun kendi verisini bozmadan bir "arcade
katsayısı" olarak eklenebilir; ama veriye sadakatten vazgeçmek proje kararıdır, ölçüm sonucu değil.

### Rakipler oyuncunun ağını almıyormuş: yan yatarak geçen süre %7,0 → %0,8, düşen 1 → 0 (2026-08-20)

Kalan tek düşüş kovalanırken çok daha görünür bir şey çıktı: **8 arabanın 5'i yan yatıyordu**, ve
kimi yarışın %87-92'sinde. Sebep koddaydı ve kod bunu zaten açık bir soru olarak yazmıştı:

> `keep_in_world` üç pencereli binary'de de yalnız `state.rig` için çağrılıyor, başka kimse için
> değil — yani düşen rakip düşük kalıyor, devrilen rakip devrik kalıyor, hem burada hem oyunda.
> … "alan hiç yakalanmıyor" hiç ölçülmemiş bir karar.

Ölçüldü. Önce doğru ölçü kuruldu: **"yan yatan araba sayısı" yanlış sayaç** — doğrultulan araba
yeniden devrilebildiği için ağ açılınca o sayı *artıyor* (28 → 32). Görünen şey süre:

| | yan yatarak | kursta süre | düşen | waypoint |
|---|---|---|---|---|
| ağ yok | **%7,0** | %77,7 | 1 | — |
| **rakiplere de ağ** | **%0,8** | %73,8 | **0** | **+59** |

Yan yatarak geçen süre dokuzda birine iniyor (4081 %22,9 → %2,6, 4102 %21,7 → %1,3), sekiz
arabanın son düşüşü de gidiyor, ve waypoint farkı +59 ile gürültü tabanının (±37) üstünde ve tek
rotalık değil (en büyük tek rota +37, üç rotada önde beşinde eşit).

**Koridor süresindeki −3,9 puan ise kayıp değil, ölçünün kendi kusuru.** Yan yatmış bir araba yolun
ortasında duruyorsa koridorda sayılıyor — bedava. Düşüşün **tamamı** `Paths4102` (68,4 → 43,4),
yani arabaların yarışın beşte birini yatarak geçirdiği rota; öteki yedi rota ±2,7 ya da hiç
oynamıyor. Ağ, bedava koridor süresini gerçekten süren arabalarla değiştiriyor.

**Açıldı, iki tarafta birden:** `nfs_sim`'de varsayılan (`NFS_RESCUE=0` geri alır) ve `nfs_cruise`'da
rakipler artık oyuncunun aldığı `keep_in_world`'ü alıyor. Sekiz-rota tablosu: **1.476 waypoint ·
yan yatarak %0,8 · kursta %73,8 · düşen 0 · furthest 5.951 m.**

### Direksiyon freni yeni halkada süpürüldü: 9 kalıyor, ve sebebi devrilme (2026-08-20)

İki davranış değişikliğinden (halkayı çekme, rakiplere ağ) sonra ayrılma sayımı yeniden alındı ve
imza sabit kaldı: **49 ayrılmanın %49'u frende, %41'i 60 km/h üstünde.** Yer dağılımı da aynı —
13 yer, ilk beşi 34 arabayı (%69) açıklıyor.

Bu doğrudan `BRAKE_SPEED`'in alanı ve o sabit yeni halkada hiç süpürülmemişti.

| 60 km/h'de sınır | waypoint | kursta süre | yan yatarak | furthest |
|---|---|---|---|---|
| 6 (erken fren) | −83 | %69,4 | %1,3 | 5.727 m |
| 7,5 | −14 | %73,6 | **%0,7** | 5.790 m |
| **9 (kalan)** | — | %73,8 | %0,8 | 5.951 m |
| 10,5 | +44 | %74,6 | %1,8 | 6.251 m |
| 12 (geç fren) | +70 | **%74,5** | %1,6 | **6.217 m** |

**Geç fren gerçekten kazandırıyor** — +44 ve +70 waypoint, 300 m'ye varan `furthest`. İki şey
durduruyor. Kazançlar **taşınıyor** (10,5'in en büyük tek rotası −55'e karşı +44 alan, 12'ninki
+76'ya karşı +70), ve bedel bugünkü devrilme işinin yarattığı sütunda tek yönlü ve büyük:
**yan yatarak geçen süre ikiye katlanıyor**, %0,8 → %1,8. Geç fren yapan araba, zaten dışarı
çıkacağı viraja daha çok hızla giriyor ve devriliyor.

6 ise her sütunda birden kötü, yani kullanılabilir aralık dar ve 9 onun içinde.

**Ve bu, bugün ölçülen ilk "daha az yaparak kazandıran" kaldıraç** — sabah ne yaptıysak
(saf takip, kilit sınırı) daha az direksiyon daha kötüydü; burada daha az fren daha çok ilerleme
veriyor. Reddedilme sebebi de **bu sabah var olmayan bir ölçü**: yan yatarak geçen süre.

### Halkayı yumuşatmak kırıkları alıyor ama alanı kıpırdatmıyor (2026-08-20)

Çekme waypoint'leri **tek tek** taşıyor, yani kirişin dışarı taştığı yerde komşusu taşınmayınca
kırık kalabilir — ve kırık, pilotun almak zorunda olduğu bir viraj. Çekilmiş halka gerçekten de
rota başına 7-12 waypoint'te 60 km/h'nin, ~30 waypoint'te 80'in altında sınır dayatıyor, oysa alan
95-105 gidiyor.

`NFS_SMOOTH=<n>` eklendi: üç noktalı ortalamanın n geçişi, **her geçişten sonra aynı çekme** ile
koridora geri alınıyor (ikisi dönüşümlü olmalı; yoksa ortalama virajı keser ve waypoint kurstan
çıkar).

**Kırıklar gerçekten çekmenin:** `Paths4102`'de 80 km/h altı waypoint **29 → 16** (2 geçiş),
60 altı **12 → 5** (6 geçiş).

**Ama sürüş umursamıyor:**

| | waypoint | kursta süre | yan yatarak | furthest |
|---|---|---|---|---|
| **yumuşatma yok** (kalan) | — | %73,8 | **%0,8** | 5.951 m |
| 2 geçiş | −39 | %74,0 | %1,2 | **6.373 m** |
| 6 geçiş | −83 | %71,6 | %1,3 | 5.752 m |

−39 gürültü tabanının (±37) kıyısında ve taşınıyor (en büyük tek rota −81); koridor süresi
kıpırdamıyor; yan yatma biraz kötüleşiyor. Tek gerçek kazanç 2 geçişin `furthest`'ı (+422 m,
günün en iyisi), ve o bir **maksimum** — tek arabanın sürebileceği bir sayı.

**Okuma:** halkanın yumuşaklığı sabah bağlayıcıydı (kiriş %82 koridor süresi veriyordu çünkü
köşeleri kesiyordu, yürünmüş ve kırpılmış halkalar keskin dönüşleriyle %75'e düşüyordu) — ama
çekmeden **sonra** artık bağlamıyor. Çekme o kısıtı zaten kaldırmış; üstüne yumuşatmak boş.
Varsayılan değişmedi, düğme kaydıyla duruyor.

### Ayrılmanın beş yeri haritalandı — ve "bulunduğun hatta kal" çürüdü (2026-08-20)

Kural aramayı bırakıp yerlere bakma sırasıydı. 49 ayrılmanın 34'ünü (%69) açıklayan beş yerin
kavşakları, kolların **kotu da basılarak** çıkarıldı (`Corridor::locate` plan görünümü olduğu için
kot hiçbir yerde görünmüyordu):

| yer | araba | kavşak | kollar |
|---|---|---|---|
| 4081 `(−352,−159)` | 8 | **206 · 5 kol** | 2'si yarışın hattında (5), **3'ü 30 m doğudaki paralel hatta (33)**, hepsi y≈5 |
| 4121 `(−249,1423)` | 8 | 275 · **2 kol** | seçim yok |
| 4001 `(528,973)` | 7 | 13 · 4 kol | **3'ü aynı noktada, üç ayrı hatta**, y = 24,1-24,2 |
| 4102 `(31,−53)` | 6 | 209 · 3 kol | 2'si `(46,−75)` ve `(47,−75)` — aynı yer, iki hat |
| 4061 `(−2358,1838)` | 5 | 40 · 3 kol | kotlar **278,7 · 293,5 · 302,2** — 24 m fark, dik rampa |

**Bir güverte hipotezi kuruldu ve hemen çürüdü:** 4001'in altı katlı kavşağında üç kolun da aynı
kotta (10 cm içinde) olduğu görüldü — farklı güverteler değil, aynı yolun paralel şeritleri.

**Desen:** beş yerin **üçünde** kollar *aynı yere ve aynı kota* giden farklı hatlar, yani grafın
sunduğu bir seçim gerçek bir seçim değil. Buradan dar bir kural çıktı ve denendi
(`NFS_SAMEPATH=1`): iki kol plan ve kotta 6 m içinde birbirinin ikiziyse, aralarından **arabanın
zaten üstünde olduğu hattı** seç.

| | waypoint | kursta süre | yan yatarak | furthest | ayrılma |
|---|---|---|---|---|---|
| **bugünkü** | — | **%73,8** | **%0,8** | **5.951 m** | 49 |
| hatta kal | **−87** | %71,1 | %1,1 | 5.856 m | 49 |

**Çürüdü, ve kurala göre sağlam** (en büyük tek rota −51, alan farkı −87). Üstelik en çok
kaybettiği iki rota `Paths4001` (−51) ve `Paths4121` (−49) — yani düzeltmeye çalıştığı yerlerin
ta kendisi. Ayrılma sayısı da hiç oynamıyor (49 → 49).

**Okuma:** ikiz kollar bir kusur değil. Grafın düz-çizgi mesafeyle yaptığı seçim, hattın kimliğine
bakan seçimden iyi — bu, aynı günün "hattın üstünde kal" ve "hatta yakın kolu tercih et"
çürütmeleriyle **üçüncü kez** aynı yere varıyor: kol seçimi bu alanda bir kaldıraç değil.

**Beş yerin haritası duruyor**, ve sıradaki iş orada: 4121'de seçim bile yok yani sebep başka;
4061 dik bir rampa; 4001'in yedi arabası 29 km/h'de, hedefi 89°'de, alabilecekleri bir virajı
(gereken yarıçap ~16 m, arabanın minimumu 5,5 m) alamıyor. Üçü üç ayrı soru ve hiçbiri kural
değil.

### Ölçü düzeltildi: arabalar koridordan çıkarken hattı **medyan 28 m** önce kaybetmiş (2026-08-20)

Bugünkü bütün ayrılma analizi **koridora** karşı yapılmıştı, ve koridor rota dosyasının bütün
hatlarının birleşimi — yani cömert. `çıkarken` satırına yarış hattına uzaklık eklendi ve ölçü
kendi kusurunu gösterdi:

> 49 ayrılma · koridordan çıkarken yarış hattına **medyan 28 m** · en yakın 10, en uzak 152 m ·
> yalnız **2'si** koridorun kendi yarı genişliği (12 m) içinde · 8'i 100 m'den uzakta

Yani koridor çıkışı işin ters gittiği an değil, **görünür olduğu an**. Aynı üç saniyelik kural
hatta karşı soruldu (`OFF_LINE = 25 m`, çünkü waypoint'ler ~30 m aralıklı ve tam ortadaki araba
zaten 15 m uzakta) ve harita yerinden oynadı:

| yer | koridor haritası | **gerçek (hat) haritası** | fark |
|---|---|---|---|
| 4081 | (−352,−159) · 47 km/h | (−347,−129) · 52 km/h | 30 m önce |
| 4121 | (−249,1423) · 64 km/h | **(−371,1600) · 68 km/h** | **215 m ve ~10 sn önce** |
| 4061 | (−2358,1838) · 73 km/h | (−2391,1790) · 77 km/h | 58 m önce |
| 4102 | (31,−53) · 65 km/h | (−13,−97) · **93 km/h** | 62 m önce, çok daha hızlı |
| 4041 | — | (1982,−64) · 26 km/h | yeni |

40 araba hattı bırakıyor, 11 yerde, ilk beşi 31 araba.

**Ve 4121'in gerçek yeri bir şey söylüyordu:** araba tam bir düğümün üstünde (0 m) ve koridorun
**14 cm** içinde — kusursuz sürüyor. Ama halkanın oradaki yarıçapı **13 m**, yani `v = sqrt(a·r)`
ile ölçülmüş 5,2 m/s²'de **30 km/h**'lik bir viraj, ve arabalar 68 km/h ile geliyor. Gereken yanal
ivme 27 m/s².

**Bunun üzerine halkanın kendi virajı için fren yazıldı — ve çürüdü.** Çürütülmüş eski sürüm
**ağın** düğüm poligonuna göre freniliyordu; bu halkanın kendisine göre frenliyor, ki halka artık
koridora çekilmiş yani dürüst. Yine de:

| | waypoint | kursta süre | yan yatarak | furthest | hattı bırakan |
|---|---|---|---|---|---|
| **kapalı** (kalan) | — | **%73,8** | **%0,8** | **5.951 m** | 40 |
| 5,2 m/s² | **−153** | %69,7 | %1,6 | 5.667 m | **44** |
| 8 m/s² | −28 | %72,0 | %1,0 | 5.790 m | 39 |

**İki ayarda da kaybediyor, hattı bırakanı azaltmıyor, ve motive eden rotada en çok kaybediyor**
(4121: −55). Demek ki halkanın tarif ettiği viraj yolun sahip olduğu viraj değil: çekme
waypoint'leri tek tek taşıyor ve kırık bırakıyor, ve bir şehir sokağında 13 m yarıçap bir firkete
değil bir kırıktır. O kırıkları yumuşatmak ayrıca ölçüldü ve o da nötr — **halkanın yarıçapları
frenlenecek kadar güvenilir değil, ve yumuşatmak onları doğru yapmıyor.**

Günün dördüncü "daha az yap" çürütmesi, ve en keskini: aritmetiğin "bu viraj imkânsız" dediği yere
nişan alındı ve yavaşlamak yine kaybetti.

### Kırık bırakmayan çekme de çürüdü — ve üç çürütme birlikte kapıyı kapatıyor (2026-08-20)

Bir önceki bölümün sonu "halkanın yarıçaplarını güvenilir yapmak" diyordu. Üçüncü ve en dikkatli
deneme yazıldı: **çekmeyi tek noktaya değil, komşularına doğrusal sönümle yayan** bir yer
değiştirme alanı (`NFS_PULLSPREAD=<k>`). Ortalama almıyor — yani virajı kesmiyor — yalnız bir
komşuluğu birlikte ötelüyor, böylece kısıt sağlanırken yeni köşe icat edilmiyor.

**Kırıkları gerçekten alıyor:** `Paths4121`'de 40 km/h altı waypoint **3 → 1**, 60 altı **16 → 11**.

**Alan yine kaybediyor:**

| | waypoint | kursta süre | yan yatarak | furthest | hattı bırakan |
|---|---|---|---|---|---|
| **yayılım yok** (kalan) | — | **%73,8** | **%0,8** | **5.951 m** | 40 |
| 2 | **−179** | %72,9 | %0,8 | 5.611 m | **36** |
| 4 | **−262** | %63,3 | %2,6 | 4.910 m | 51 |

Tek iyileşen sütun hattı bırakan araba (40 → 36), gerisi kötü.

**Ve üç çürütme birlikte bir kapıyı kapatıyor.** Düz çekmeden *sonra* halkanın geometrisini daha
iyi yapmak sürüşü daha iyi yapmıyor:

1. halkanın kendi yarıçapı için fren — **çürük** (−153)
2. kırıkları ortalamayla yumuşatmak — **nötr** (−39, gürültü kıyısında)
3. kırık bırakmayan çekme — **çürük** (−179)

Çekme alınacak olanı almış. Bugün kurs tarafında beş şey denendi (ağda yürüme, kırpma, çekme,
yumuşatma, yayılımlı çekme) ve yalnız biri kazandı; pilot tarafında yedi kaldıraç çürüdü. Kolay
olan bitti, ve bu bir sonuç: **kalan kayıp halkanın şeklinde ya da pilotun tepkisinde değil.**

Ölçü artık doğru yere bakıyor (hattı bırakma, koridoru değil), 11 yerin beşi 31 arabayı açıklıyor,
ve o beş yerin her biri ayrı bir soru. Sıradaki iş bir kural denemek değil, o yerlerden birinin
izini baştan sona sürmek.

### 4081'in izi baştan sona sürüldü — teşhis doğru, çıkarılan sonuç yanlış (2026-08-20)

Kural denemeyi bırakıp bir arabanın izini sürme sırasıydı. `Paths4081`, sekiz araba, hattı
bıraktıkları yer `(−347, −128)`:

```
t= 33.8 (-326, -82) 81 km/h · koridora  1.3 m · direksiyon  0.02 · nişan 41 m   -2° · düğüm 239
t= 35.2 (-339,-109) 70 km/h · koridora  0.2 m · direksiyon -0.36 · nişan 35 m   38° · düğüm 205  ← fren 1.00
t= 36.6 (-348,-131) 52 km/h · koridora  2.9 m · direksiyon -0.62 · nişan 16 m   63°
t= 38.0 (-352,-149) 47 km/h · koridora  7.1 m · direksiyon -0.64 · nişan 56 m   68° · yanal 4.6
t= 40.1 (-352,-171) 29 km/h · koridora 17.6 m · direksiyon -0.84 · nişan 30 m   89° · yanal 21.0
t= 42.9 (-355,-180)  3 km/h · koridora 18.7 m · sıkışmış
```

**Kırılma anı t=35,2:** araba **80 km/h**'de, koridorun **0,2 m** içinde, direksiyon düz — ve
**nişan −1°'den 38°'ye tek adımda zıplıyor**, düğüm 239'dan 205'e geçerken. Tam fren ve −0,36
kilit onu izliyor, kırk metre sonra araba dışarıda.

Sebep yapısal ve bugüne kadar yazılı değildi: **nişan bir düğüme yapışık**, ve düğümler ~30 m
arayla. Yürüyüş bir adım atınca nişan ışınlanıyor.

**Düzeltme yazıldı — ve bu pilotta ölçülen her şeyden sert biçimde çürüdü.** `NFS_AIMLERP=1`
nişanı son bacak üzerinde interpole edip tam `look` metreye yerleştiriyor:

| | waypoint | kursta süre | furthest | hattı bırakan |
|---|---|---|---|---|
| **düğüme yapışık** (kalan) | — | **%73,8** | **5.951 m** | 40 |
| ön-takibe yerleştirilmiş | **−664** | %72,5 | **3.981 m** | 37 |

Sekiz rotanın **sekizinde** geride, `furthest` üçte bir eksik.

**Teşhis doğruydu, sonuç yanlış: zıplama gerçek ama kusur değil.** Sayıların söylediği şu —
**kesiklik taşıyıcı.** Düğüm, yolun gerçekten gittiği bir yer; iki düğüm arasında interpole edilen
nokta ise yol orada düzse yolun gittiği bir yer. Daha kötüsü, interpole edilmiş nişan sonsuza dek
tam `look` metrede duruyor: araba yaklaştıkça geri çekiliyor, yani araba **hiçbir yere varmıyor**.
Bu pilot bir hattı takip etmiyor, bir noktayı **kovalıyor** — direksiyon yasasının kendi belgesi
bunu yazıyor — ve yakalanamayan bir nokta hedef değildir. Virajdaki ışınlanma, kesikli nokta
kovalamanın bedeli, ve alternatifinden **on kat** ucuz.

### 4102'nin izi: çekmenin kendi yan etkisi, düzeltmek istediği kusuru geri getiriyor (2026-08-20)

İkinci iz `Paths4102`'den, hattı **93 km/h**'de bırakan gruptan:

```
t= 25.1 ( -38,-134) 91 km/h · koridora 2.7 m · direksiyon 0.02 · hedef 11   20 m   -6°
t= 25.7 ( -29,-121) 92 km/h · koridora 2.0 m · direksiyon 0.01 · hedef 12  103 m  -39°   ← hedef atladı
t= 27.5 (  -2, -84) 93 km/h · koridora 0.8 m · direksiyon 0.34 · hedef 12   71 m  -52°   ← fren 1.00
t= 30.5 (  44, -43) 54 km/h · koridora 25.9 m · direksiyon 0.85 · hedef 12  55 m  -97°
```

Araba **kusursuz sürüyor** (koridorun 2 m içinde, direksiyon 0,02) ve waypoint 11'i geçince hedefi
waypoint 12 oluyor — **103 m ötede, 39° yanda**. Adım 40 m olduğu için bu bir aralık değil,
bir **boşluk**.

**Sebep bugünün en büyük kazancının kendi yan etkisi.** Çekme noktaları tek tek taşıyor, yani
halkayı **esnetiyor**: 40 m'lik adımla en geniş aralık 4102'de **106 m**, 4081'de 97, 4121'de
**228 m**, çünkü 102 m'ye kadar yana çekilen bir waypoint komşusunda olmayan bir delik bırakıyor.
Ve pilotun hedefi bir waypoint olduğu için, bu tam olarak çekmenin düzeltmek için yazıldığı
"hedef yanda kalıyor" kusurunun geri dönmesi.

**Aralığı geri koymak yazıldı** (`NFS_REDENSIFY=1`): `step`'te yeniden örnekle, yeni noktaları
koridora çek, aralık yarım adımdan fazla kalmayana kadar tekrarla (4-6 tur; 106 → 50 m, 228 → 101,
97 → 61). İki işlem birbiriyle çekişiyor ama yakınsıyor.

**İzi sürülen rotayı düzeltiyor, iki rotayı bozuyor:**

| | kursta süre | yan yatarak | furthest | hattı bırakan | hiç bırakmayan |
|---|---|---|---|---|---|
| **esnek** (kalan) | %73,8 | **%0,8** | **5.951 m** | 40 | 15 |
| yeniden sıklaştırılmış | **%78,4** | %1,3 | 5.496 m | **33** | **19** |

`Paths4081` kursta geçen süreyi **%51,0 → %90,3** yapıyor — alan ortalamasındaki +4,6 puanın
tamamı ve fazlası, yani kurala göre kazanç **taşınıyor**. Kayıp da taşınıyor: `furthest`'ın
−455 m'si `Paths4061` (−214) ve `Paths4121`'den (−200). İki sütun ters yöne bakıyor ve ikisini de
birer rota belirliyor — değişiklik yok. Geçilen waypoint burada hakem olamaz, çünkü yeniden
örnekleme waypoint **sayısını** değiştiriyor.

**Kayda değer olan:** çekmenin esnetmesi gerçek, ölçülmüş ve yeri belli. Bir sonraki deneyen
kişinin elinde hem sebep hem tur sayısı hem de hangi rotaların ne yönde tepki verdiği var.

### Çekmenin tam bilançosu, ve itiraz eden tek sütunun açıklaması (2026-08-20)

Boşluk doldurma cerrahi biçimde denendi ve **nötr** çıktı (kursta süre −0,2, `furthest` −311) —
üstelik izi sürülen rota `Paths4081` **hiç değişmedi** (%51,0 → %51,0), oysa tam sıklaştırma onu
%90,3 yapıyordu. Demek ki 4081'i düzelten şey boşluğun kapanması değil.

Waypoint adımı da süpürüldü (20 · 30 · 40 · 60 m) ve kararsız çıktı — 30 m hem 20'den hem 40'tan
kötü, yani o sütun bu örneklem büyüklüğünde gürültülü. 40 kalıyor.

**Sonra doğru soru soruldu: çekme nerede zararlı?** Çekme kapatılıp rota rota bakıldı:

| rota | çekmesiz | çekmeli | fark |
|---|---|---|---|
| 4021 | %68,5 | %94,2 | **+25,7** |
| 4041 | %82,3 | %91,7 | +9,4 |
| 4061 | %58,8 | %62,0 | +3,2 |
| 4001 | %92,7 | %92,8 | +0,1 |
| 4002 | %90,2 | %89,4 | −0,8 |
| 4102 | %52,0 | %43,4 | −8,6 |
| 4121 | %80,1 | %66,2 | −13,9 |
| **4081** | **%88,6** | **%51,0** | **−37,6** |

**`Paths4081`'de çekme zararlı, ve fena hâlde.** O rotanın halkasını 97 m'lik boşluklarla esnetiyor
(adım 40 m), ve tam sıklaştırma onu geri getiriyor (%90,3) — ama iki rotayı bozarak.

**Tam bilanço, bugün eklenen her sütunla:**

| | waypoint | kursta süre | yan yatarak | düşen | furthest | hattı bırakan |
|---|---|---|---|---|---|---|
| çekmesiz (kiriş) | 1.026 | **%76,7** | %1,1 | 0 | 5.805 m | 48 |
| **çekmeli** (kalan) | **1.476** | %73,8 | **%0,8** | 0 | **5.951 m** | **40** |

**+450 waypoint**, sekiz rotanın yedisinde önde, kurala göre sağlam. Ve bugün kurulan daha dürüst
ölçüde de önde: **hattı bırakan 48 → 40**.

**İtiraz eden tek sütun koridor süresi, ve artık sebebi belli.** Koridor rota dosyasının bütün
hatlarının **birleşimi** — paralel şeritler dahil — yani cömert. Köşeleri kesen kiriş halkası
arabayı o birleşimin içinde tutuyor; çekilmiş halka onları **yarışın kendi yoluna** koyuyor, ve
oradan çıktıklarında gerçekten çıkmış oluyorlar. "Hattı bırakan 48 → 40" ile "koridorda kalan
%76,7 → %73,8" aynı şeyin iki yüzü: cömert ölçü kötüleşirken dürüst ölçü iyileşiyor.

Çekme kalıyor. `Paths4081`'in −37,6'sı gerçek, sebebi bilinen (esnetme) ve çaresi ölçülmüş ama
alan düzeyinde bedelli — bir sonraki oturumun elinde tam olarak bu duruyor.

### Çekmenin esnetmesi düzeltildi: eşik ölçüldü, keskin çıktı, ve bir varsayımı geri koyuyor (2026-08-20)

Cerrahi boşluk doldurmanın eşiği süpürüldü ve **keskin bir sınır** çıktı. `Paths4081`, çekmenin
37,6 puan kaybettirdiği rota:

| eşik (adımın katı) | en geniş aralık | kursta geçen süre |
|---|---|---|
| 1,05 × | 43 m | **%90,3** |
| **1,1 ×** | 44 m | **%90,2** |
| 1,2 × | 47 m | %48,0 |
| 1,3 × | 51 m | %48,0 |
| 1,5 × | 60 m | %51,0 |

Yaklaşık **45 m'nin üstünde o rota yarıya iniyor, altında duruyor.** Bu bir çatallanma değil —
1,05 ile 1,1 aynı, 1,2/1,3/1,5 aynı — gerçek ve tekrarlanabilir bir eşik.

**Ve neden orada olduğu belli: pilot zaten bu varsayımı yapıyor.** `PASSED_NEAR` geçilen bir
waypoint'i **60 m içindeyse** bırakıyor, ilerleme kuralı da bir sonraki waypoint tutulandan yakınsa
adım atıyor; ikisi de waypoint'lerin **bir adım aralıklı** olduğunu varsayıyor. Çekme bunu
bozuyordu (adım 40 m iken aralık 106 · 97 · 228 m), ve bırakma yarıçapının ötesindeki hedef
mahsur kalıyordu — 4102'de 92 km/h'de izlendiği gibi.

**Alan sonucu:**

| | kursta süre | yan yatarak | furthest | hattı bırakan | hiç bırakmayan |
|---|---|---|---|---|---|
| esnek (eski) | %73,8 | **%0,8** | 5.951 m | 40 | 15 |
| tam sıklaştırma | **%78,4** | %1,3 | 5.496 m | **33** | **19** |
| **boşluk eşiği 1,1×** (yeni) | %77,4 | %1,2 | **5.928 m** | 34 | 17 |

1,1× tam sıklaştırmanın kazancının neredeyse tamamını veriyor ve **bedelini vermiyor**:
`furthest` 5.928 m, tam sıklaştırmanın 5.496'sına karşı, yani eski hâlin 5.951'iyle aynı.
Hattı bırakan 40 → **34**, kursu hiç bırakmayan 15 → **17**.

Koridor süresindeki +3,6 puan kurala göre 4081'in +39,2'siyle **taşınıyor**, ve bu kabul için
gerekçe değil; gerekçe şu: **bu bir ayar değil, kırılmış bir ön koşulun onarımı.** Çekmenin
esnetmediği bir halka bayt-birebir aynı çıkıyor, çünkü yalnız geniş bacaklar bölünüyor.

**Varsayılan yapıldı** (`NFS_FILLGAPS=0` kapatır). 4081 %51,0 → **%90,2**, 4001 değişmedi.

### Halka düzelince iki sabit yeniden soruldu — biri yerinde kaldı, biri **atıl** çıktı (2026-08-20)

Günün kendi dersi uygulandı: kurs değişince sabitler yeniden süpürülür.

**Ön-takip (`LOOKAHEAD_PER_SPEED = 1.8`) yerinde kaldı.** 1,3 s karar ölçüsünde +76 waypoint
kazanıyor (sağlam) ama dört sütunda birden kaybediyor — koridor %77,4 → %76,7, `furthest`
5.928 → 5.495, hattı bırakan 34 → 40, yan yatma %1,2 → %1,5. 2,4 s'nin +81'i ise taşınıyor
(en büyük tek rota +98). 0,9 s −59 ile sağlam biçimde geride.

**`PASSED_NEAR` ise artık hiçbir şey yapmıyor:**

| bırakma yarıçapı | alan farkı | kursta süre | furthest | hattı bırakan |
|---|---|---|---|---|
| 40 m | +2 | %77,6 | 5.934 m | 34 |
| **60 m** (bugünkü) | — | %77,4 | 5.928 m | 34 |
| 90 m | **±0** | %77,4 | 5.928 m | 34 |

90 m sekiz rotanın sekizinde **bayt-birebir aynı**; 40 m tek rotada iki waypoint oynatıyor.

**Ve bu, boşluk teorisinin en temiz doğrulaması.** Bırakma yarıçapı yalnızca halkada ondan
büyük aralıklar olduğu için önemliydi. `GAP_AT` her aralığı ~1,1 adımın (40 m'lik adımda 44 m)
altında tuttuğu için hiçbir waypoint artık 60 m'den uzak değil, ve yarıçap hiç bağlamıyor.

Bu, sabahki denetimi çürütmüyor, **açıklıyor**: `PASSED_NEAR` keyfi görünüyordu çünkü halka
düzensizdi; halka düzenlenince kanıtlanabilir biçimde atıl. **Değeri ölçülemeyen bir sabit
genellikle başka bir şeyin yerine duruyordur** — burada durduğu şey kursun kendi aralığıydı.

### İkinci sabit de atıl çıktı: kursu onarmak pilotu sadeleştiriyor (2026-08-21)

`PASSED_NEAR`'ın atıl çıkması bir tahmin doğurmuştu: **"hedef arkada kaldı" hâli için yazılmış
kurallar da atıl olmalı**, çünkü o hâlin sebebi halkadaki boşluklardı. `BEHIND` kapatılıp
süpürüldü:

| | alan farkı | kursta süre | furthest | hattı bırakan | bayt-birebir aynı rota |
|---|---|---|---|---|---|
| **BEHIND açık** (bugünkü) | — | %77,4 | 5.928 m | 34 | — |
| BEHIND kapalı | +11 | %77,4 | **6.049 m** | 35 | **5 / 8** |

Alan farkı **+11**, yani sekiz rotalık süpürmenin kendi ölçülmüş gürültü tabanının (±37) içinde;
koridor süresi virgülüne kadar aynı; `furthest` kapalıyken **daha yüksek**. Ve sekiz rotanın
**beşi bayt-birebir aynı** — kural o rotalarda hiç ateşlemiyor.

`GRIP` de yeniden soruldu: 5 ile 8 arasında **+30**, yine gürültünün içinde, yine ayırt edilemez —
kiriş halkasında ~100 waypoint olan fark, çekilmiş halkada sıfıra, onarılmış halkada gürültüye
inmiş durumda.

**İki günün en derin dersi bu:** dün "ölçülmüş değil, makul birer ayar" diye kaydedilen dört
sabitten **ikisi**, altlarındaki gerçek kusur (kursun düzensiz aralığı) bulunup düzeltilince
kanıtlanabilir biçimde **gereksiz** hâle geldi. Üçüncüsü (`GRIP`) zaten ölçülemiyordu.

Yani o sabitler pilotun ayarları değil, **kursun kusurlarının telafisiydi**. Bu, ileriye dönük bir
okuma kuralı da veriyor: *bir pilot sabiti ölçülemez hâle geldiyse iyi haberdir — altındaki kusur
düzelmiş demektir; yeniden ölçülebilir hâle geldiyse kursun kaydığından şüphelen.*

İkisi de kaldırılmadı: nadiren ve doğru ateşleyen bir kuralı silmek için sebep yok, ve maliyeti
sıfır. Ama alanda hiçbir şey artık onlara dayanmıyor.

### DÜZELTME — boşluk doldurma geri alındı: kazandığı her sütun **duran arabayla** şişiyor (2026-08-21)

Bir önceki bölüm `GAP_AT = 1.1`'i varsayılan yaptı. **Yanlıştı, ve daha derin bir ölçüm yakaladı.**

Kabul, koridorda geçen süre (+3,6 puan) ve hattı bırakan arabanın 40 → 34 olmasına dayanıyordu.
İkisi de **yolda duran** arabayla şişen sütunlar: duran araba koridorun içindedir, yani orada
geçen süreyi bedavaya yazar, ve hattı da hiç bırakmaz. İlerleme sorulunca tablo tersine dönüyor:

| | kursta | kursta kalanın düğümü | alan kavşağı | ayrık düğüm | ilerlemesi duran |
|---|---|---|---|---|---|
| **kapalı** (geri alındı) | 15 | **19,0** | **1.641** | **1.594** | **5**, duruş %21 |
| 1,1× | **17** | 11,3 | 1.562 | 1.490 | 8, duruş **%45** |

`Paths4081` bütün hikâyeyi tek rotada anlatıyor: koridorda geçen süresi %51,0 → **%90,2** —
kabulün dayandığı sayı — ama aynı koşuda geçilen waypoint 156 → **128** ve kavşak 219 → **155**.
Arabalar kursta değil, kursun üstüne **park etmiş**. `Paths4121` gerçekten kazanıyor (+57 waypoint)
ve `Paths4021` gerçekten kaybediyor (0 → 4 takılan): değişiklik takılmayı ortadan kaldırmıyor,
rotalar arasında **taşıyor**.

Geri alındı ve doğrulandı (4081 219 kavşak, 4021 235 — çekme sonrası taban).

**Ders, ve bu sefer bir yanlış kabule mal oldu:** *koridorda geçen süre* ve *hattı bırakma*
ölçülerinin ikisi de **duran arabayı başarı sayıyor**. Devrilme ağında da aynı tuzağa değmiştik
(yan yatmış araba koridorda sayılıyor). Arabaları durdurabilecek her değişiklik, kavşak · ayrık
düğüm · takılma sayısıyla **birlikte** yargılanmalı.

### Ve iki sabitin atıllığı ayakta — ama sebebi boşluk düzeltmesi değil, **çekmenin kendisi**

Geri almadan sonra ikisi de yeniden soruldu, bu kez sevk edilen halkada:

| | alan farkı | bayt-birebir aynı rota |
|---|---|---|
| `PASSED_NEAR` 60 → 90 | **±0** | **8 / 8** |
| `BEHIND` açık → kapalı | +6 | **6 / 8** |

Yani dünkü sonuç doğru, atfı yanlıştı. Bu iki sabiti gereksiz kılan şey halkanın **aralığı** değil,
koridora **çekilmiş** olması: koridordaki bir waypoint arabanın kendi yoluna yeterince yakın
olduğu için bırakma yarıçapı hiç bağlamıyor ve "hedef arkada" hâli neredeyse hiç doğmuyor.

### Ders geriye dönük uygulandı: bugünkü iki kabul de ilerleme ölçüsünden geçti (2026-08-21)

"Duran araba başarı sayılıyor" dersi, aynı gün kabul edilen iki değişikliğe de uygulandı — çünkü
biri (doğrultma ağı) arabayı **durmuş hâlde** bırakıyor, yani şüpheliydi:

| | ilerlemesi duran | duruş ort. | alan kavşağı | ayrık düğüm |
|---|---|---|---|---|
| ağ yok | 11 / 64 | %49 | 1.590 | 1.513 |
| **rakiplere ağ** (kabul) | **5 / 64** | **%21** | **1.641** | **1.594** |
| çekmesiz | 5 / 64 | %29 | 1.505 | 1.444 |
| **çekmeli** (kabul) | 5 / 64 | **%21** | **1.641** | **1.594** |

İkisi de temiz geçti: ağ takılan arabayı **yarıdan aza** indiriyor ve kavşağı artırıyor; çekme
takılan sayısını değiştirmeden duruş oranını düşürüyor ve kavşağı 1.505 → 1.641 yapıyor.

Yani günün yanlış kabulü yalnız boşluk doldurmaydı, ve o geri alındı. `BASELINE-SEKIZ-ROTA.md`'ye
dördüncü tablo, uyarısıyla birlikte yazıldı.

### Beş yer paralel olarak izlendi — ve ölçüm aletim haritayı kendisi çiziyormuş (2026-08-21)

Beş yerin her biri için bir izci ve arkasından o izciyi **çürütmekle görevli** bir şüpheci
çalıştırıldı (on ajan). Üç mekanizma doğrulamayı geçti, ikisi düştü ve yerine düzeltilmiş
mekanizma kondu. İki gerçek kusur çıktı, biri benim aletimde.

**1. `OFF_LINE` mesafeyi halkanın KÖŞELERİNE ölçüyordu, kenarlarına değil.**

Çekme halkayı esnetiyor (4121'de iki waypoint arası **231 m**, 4061'de 97 m, adım 40 m). 231 m'lik
bir bacağın tam ortasından kusursuz süren araba iki ucundan da 115 m uzaktır — ve bunun hiçbiri
sapma değildir. Sonuç: dünkü "beş yer" haritasının üçü, bir waypoint'in çevresindeki **25 m'lik
çember**ti. `Paths4121`'de sekiz arabanın sekizi aynı metrede kaydedilmişti — 60, 60, 61, 63, 66,
76, 80 ve 81 km/h'de ve 11,7 saniyeye yayılmış hâlde. Aynı yerde olan tek şey daireydi.

Düzeltildi: mesafe artık halkanın **kenar parçalarına** ölçülüyor. Harita hemen oynadı —
`Paths4061` 6 arabadan 3'e indi ve yeri **50 m öteye, ajanın işaret ettiği dirseğe** taşındı
(düğüm 40'ta 52,7°'lik bir dirsek, 52 m yarıçapında hiç waypoint yok). `Paths4102` 13 m,
`Paths4081` 7 m oynadı. `Paths4121` yerinde kaldı, ve bu da bir cevap: orada araba gerçekten
hattan 25 m uzakta, çünkü 231 m'lik kiriş yolu takip etmiyor.

**2. `Network::of` düğüm kotlarını HAT HAT çözüyor, ve hatlar arası bağlantı iki güverteyi
sessizce birleştirebiliyor.**

`Paths4081`'de arabalar **bir güverte yukarıda** sürüyor, hem de 1,5 km boyunca. Kendi koşumla
doğrulandı (`NFS_WATCH=0 NFS_TRACE=2`), arabanın kotu ile tuttuğu düğümün kotu:

| t | araba y | tutulan düğüm | düğüm y | fark |
|---|---|---|---|---|
| 18,0 | 25,78 | 53 | 25,77 | **+0,01** |
| 22,0 | 17,54 | 233 | 14,26 | +3,28 |
| 28,0 | 9,18 | **55** | 5,71 | **+3,47** |
| 34,0 | 12,71 | 239 | 5,23 | +7,48 |
| 36,0 | 15,25 | 205 | 5,38 | **+9,87** |
| 42,0 | 17,16 | 241 | 5,39 | **+11,77** |

`under [5,36 · 11,22 · 14,60]` — üç güverte, araba en üstünde, düğüm en altında. Sapma **hat
sınırlarında** başlıyor (düğüm 55, +3,47) ve bir daha kapanmıyor; koşu boyunca arabanın bastığı
yüzeyle altındaki yüzey arasındaki en küçük mesafe **2,66 m**, yani inecek rampa yok.

**Ve hiçbir şey fark etmiyor, çünkü pilotun kullandığı her test plan görünümünde:**
`step_avoiding`'in maliyeti `(x,z)` farkı, düğüm ilerlemesi `flat(...)`, `Corridor::locate`
bilerek kotsuz. Araba bir üst geçitte, koridorun *üstünde* uçarken iz "koridora 2,6 m" basıyor.
Kavşak 205'in dört kolu da y = 3,8-5,6'da; araba 15,25'te.

Bu, yarışın kendi hattının **hangi güvertede** olduğu sorusunu açıyor: `route::follow` her hat
için "en az tırmanan diziyi" seçiyor, ama hatlar arası bağlantıya kimse bakmıyor.

### Güverte sapması sekiz rotada ölçüldü, sebebi daraltıldı, ve bir düzeltme çürütüldü (2026-08-21)

**Yaygınlığı:** araba, pilotunun tuttuğu düğümün ne kadar üstünde sürüyor —

| rota | en fazla | adımların yüzdesi (>3 m) |
|---|---|---|
| 4001 | 15,3 m | %31,4 |
| 4002 | **22,5 m** | %15,5 |
| 4021 | 12,9 m | %7,5 |
| 4041 | 6,1 m | %2,6 |
| 4061 | 19,7 m | %11,8 |
| **4081** | 14,2 m | **%70,6** |
| **4102** | **2,7 m** | **%0,0** |
| 4121 | 8,6 m | %22,5 |

Sekiz rotanın yedisinde arabalar yarışın hattından bir güverte yukarıda sürüyor.

**Bağlantı eğimleri sayıldı ve korelasyon kusursuz göründü:** `%25`'ten dik bağlantı sayısı 4001'de
82, 4081'de 13, ve **4102'de sıfır** — yani dik bağlantısı olmayan tek rota, güverte sapması
olmayan tek rota. En dikler akıl almaz: **%1084** (12,2 m'yi 1 m'de), **%941** (14,8 m'yi 2 m'de),
ve **hepsi hat değiştiriyor**.

**Ama dik bağlantıları atmak çürüdü.** `drop_climbing` yazıldı ve süpürüldü:

| | waypoint | kursta süre | kavşak | furthest | 4081'in sapması |
|---|---|---|---|---|---|
| **filtre yok** (kalan) | **1.476** | **%73,8** | **1.641** | **5.951 m** | %70,6 |
| dik bağlantı atıldı | 1.259 | %71,7 | 1.470 | 5.445 m | %69,7 |

Sapmayı düzeltmiyor (%70,6 → %69,7, ve 4001'de **kötüleşiyor**: %31,4 → %53,7) ve 217 waypoint,
171 kavşak, 506 m götürüyor. Korelasyon gerçekti ama nedensellik değil.

**Ve çürütme asıl mekanizmayı buldu.** Sapma **yumuşak** bir bağlantıdan geliyor: düğüm 237
(y=10,68) → düğüm 55 (y=5,71), 30,5 m'de 4,97 m, yani **%16** — hiçbir filtrenin yakalamayacağı,
her sokakta olabilecek bir eğim. Yanlış olan o bağlantının eğimi değil, **iki ucunun farklı
güvertelerde çözülmüş olması**.

**Son ölçü meseleyi kapatıyor:** güverte sapması olan adımlarda, düğümün *kendi XZ'sinde*
arabanın kotunda bir yüzey var mı?

| rota | var |
|---|---|
| 4001 | **%88,3** |
| 4081 | **%68,0** |
| 4002 | %8,1 |

Yani 4001 ve 4081'de yüzey **eksik değil, seçim yanlış**: `route::follow` her hattı ayrı ayrı
"en az tırmanan dizi" diye çözüyor, ve hat sınırının iki yanını hiçbir şey karşılaştırmıyor.
Doğru güverte orada duruyor, seçilmiyor. (4002 ayrı bir durum: orada yüzey gerçekten yok.)

**Sıradaki iş belli ve dar:** kotları hat hat değil **graf genelinde** çözmek — ya da en azından
hat sınırlarındaki bağlantılarda iki ucun aynı güvertede olmasını istemek. Bu `route::follow`'un
kapsamını değiştirmek demek, yani ölçülerek yapılacak bir iş; ama artık hangi sayının düzelmesi
gerektiği belli: 4001'in %31,4'ü ve 4081'in %70,6'sı.

### Kotlar artık graf üzerinde çözülüyor: imkânsız eğimler bir mertebe azaldı (2026-08-21)

`route::follow` bir zincir üzerinde Viterbi'dir ve bir hat zincirdir — bu yüzden **kavşakta** ters
gideni göremez. Aynı en-az-tırmanma kuralı grafın **kapsayan ağacı** üzerine genelleştirildi
(`route::follow_graph`): yapraktan köke bir DP, ağaçta tam çözüm, tıpkı `follow`'un zincirde tam
olması gibi. Çevrim kapatan bağlantılar kısıtlanmıyor; onlar için döngülü inanç yayılımı gerekir
ve ağaç zaten eksik olan kısmı — her hattın komşusuna bağlanmasını — sağlıyor.

**Grafın kendi kalitesi bir mertebe düzeldi.** Hiçbir yolun tırmanamayacağı bağlantı sayısı:

| rota | %25'ten dik | %50'den dik | %100'den dik |
|---|---|---|---|
| 4001 hat hat | 82 | 31 | **21** |
| **4001 graf** | **33** | **4** | **2** |
| 4002 hat hat | 69 | 27 | **17** |
| **4002 graf** | **25** | **6** | **4** |
| 4081 hat hat | 13 | 11 | **7** |
| **4081 graf** | **7** | **6** | **3** |

**Güverte sapmasının en kötüleri yarıya indi:** 4002'de araba tuttuğu düğümün 22,5 m üstüne
çıkarken artık 12,5 m; 4001'de 15,3 → 9,9 m; 4081'de yarışın %70,6'sı → %63,6'sı.

**Sürüş nötr:** waypoint 1.476 → 1.449 (gürültü tabanının içinde), kursta süre %73,8 → %73,9,
kavşak 1.641 → 1.649, `furthest` −33 m, hattı bırakan 39 → 40.

**Yine de kalıyor, ve gerekçesi süpürme değil.** Eski çözüm *bilinen biçimde yanlıştı*: bitişik iki
düğüm arasında 12,2 m'yi 1 m'de tırmanan bir bağlantı üretiyordu, ki o bir yol değil. Yeni çözüm
aynı yolları ölçülebilir biçimde daha tutarlı tarif ediyor ve hiçbir sütunda bedeli yok. Bir
tarifin doğruluğu, onu kullanan pilotun bugün ondan yararlanamamasıyla ölçülmez.

**Ve kalan sapma bir sonraki sorunun yerini gösteriyor.** 4081 hâlâ %63,6'da: graf çözümü *tutarlı*
bir güverte seçiyor ama **hangi güvertenin yarışın kendisi olduğunu bilmiyor** — iki güverte de
kendi içinde pürüzsüz, ve en-az-tırnama kriteri alttakini seçebiliyor. Eksik olan bir çıpa: çıkış
gridi gerçek zemine yerleştiriliyor (`ground at (x,z) is y=… — asked, not guessed`), yani yarışın
hangi kotta başladığı biliniyor. Çözümü oradan çıpalamak bir sonraki adım.

### Çıpa yazılmadan çürüdü; asıl sebep grafın sorduğu zeminmiş (2026-08-21)

Bir önceki kaydın "sıradaki adım" dediği şey **çıkış gridinden çıpalamak**tı. Yazılmadı, çünkü
önce ölçüldü ve ölçüm onu öldürdü.

**Çıpanın düzeltecek bir şeyi yok.** Sekiz rotanın her birinde, gridin durduğu zemin ile grafın o
gridin en yakın düğümüne verdiği kot zaten uyuşuyor:

| rota | grid'in zemini | tuttuğu düğümün kotu | fark | gridin XZ'sindeki yüzeyler |
|---|---|---|---|---|
| 4001 | 9,98 | 10,1 | +0,1 | [7,3 · 10,0] |
| 4002 | 30,74 | 31,0 | +0,3 | [17,9 · 28,7 · 30,7] |
| 4021 | 12,63 | 12,3 | −0,3 | **[12,6]** |
| 4041 | 8,62 | 8,6 | 0,0 | **[8,6]** |
| 4061 | 323,61 | 324,3 | +0,7 | [323,5 · 323,6] |
| 4081 | 30,44 | 29,0 | −1,4 | **[30,4]** |
| 4102 | 17,18 | 17,0 | −0,2 | **[17,1]** |
| 4121 | 3,80 | 3,8 | 0,0 | **[3,8]** |

Güverte ayrımı ~10 m; en büyük hata 1,4 m. Yani seçili aday, çıpanın `|aday − çıpa|` terimini zaten
minimize eden aday — çıpa hiçbir ağırlıkta hiçbir şeyi kıpırdatmaz. Sekiz gridin **beşi** üstelik
tek yüzeyin üstünde duruyor, ve tek adaylı bir düğümde tekil terim `cost` vektörünün her girdisini
aynı miktarda kaydırır: hiçbir `min_by`, hiçbir `chosen` yer değiştirmez. Aritmetik olarak atıl.

**Ve `Paths4081`'in izi sapmanın nerede doğduğunu gösterdi** (`NFS_TRACE=2`, tek araba): sapma
başlangıçta değil, 30. saniyeden sonra açılıyor —

| t | araba y | tutulan düğüm | düğüm y | arabanın altındaki yüzeyler |
|---|---|---|---|---|
| 16,0 | 26,49 | 53 | 25,77 | [25,8] |
| 28,0 | 7,98 | 57 | 7,27 | [3,5 · 4,7 · 7,3] |
| 32,0 | 12,58 | 239 | **5,23** | [5,1 · 9,2 · **11,9**] |
| 34,0 | 15,20 | 205 | **5,38** | [5,4 · 11,2 · **14,5**] |
| 38,0 | 16,89 | 241 | **5,39** | [4,7 · 13,2 · **16,2**] |

Araba üç güvertenin en üstünde, düğüm en altında. Çıpa 700 m geride ve doğru; hata burada.

## Graf, dokuz gündür yanlış zemini soruyormuş

`route::road_ground`'un kendi doküman satırı bunu zaten yazmış: *"Least-climb is only safe because
of the road filter: over all drivable triangles it is degenerate, since the flat shelf beneath the
city climbs by nothing at all and wins everywhere."* `follow_graph` tam olarak o en-az-tırmanma
kuralı, ve **filtresiz zeminle besleniyordu**.

Bu bir karar değil, bir kayma: `road_ground` 2026-08-11'de (`6c7658e`) geldi, `Network::of` ertesi
gün bir binary'ye (`9ebf456`) girdi, ve o commit'in mesajı *"Every node stands on the same `Ground`
the drawn line does, so a driver and a ribbon cannot disagree about where the road is"* diyordu —
yazıldığı anda yanlıştı. Çizilen hat (`build_route`) baştan beri `road_ground` kullanıyor; grafın
kendisi hiç kullanmadı. Hiçbir commit mesajı filtresiz zemini savunmuyor.

### Yol zemini tek başına: bir rotayı düzeltiyor, bir rotayı öldürüyor

`NFS_ROADHEIGHT=strict` — yol yoksa yok:

| rota | yolsuz düğüm | duvarlı kesik | çıkışsız düğüm | güverte %adım |
|---|---|---|---|---|
| 4001 | **44** / 341 | 86 → **152** | 1 → **18** | 33,7 → **100,0** |
| 4041 | 35 / 341 | 18 → **150** | 0 → **31** | 2,6 → 1,9 |
| 4081 | 7 / 262 | 4 → 18 | 0 → 0 | **63,6 → 1,3** |
| 4002 | 6 / 360 | 37 → 64 | 0 → 0 | 6,9 → 6,4 |

4081'de aradığımız düzelme tam olarak geliyor; 4001'de alan duruyor — 31 waypoint, **sıfır kavşak**,
32 m. Mekanizma zincirin tamamı görünüyor: altında yol olmayan düğüm `fill`'in ara değerini alıyor
(hiçbir yüzey olmayan bir kot), `drop_walled` o düğümün bağlantılarını "yol devam etmiyor" diye
kesiyor, ve 18 düğüm çıkışsız kalınca arabaların gidecek yeri kalmıyor.

**Kontrol kolu mekanizmayı doğruluyor:** `strict` + `NFS_WALLED=0` (duvar filtresi kapalı) 4001'i
31 → 350 waypoint'e geri getiriyor. Yani çöküş uydurulmuş kotların duvar filtresine çarpmasından.

### Geri düşüş: yola sor, yol yoksa şehre sor

Varsayılan artık bu. Yol yüzeyi varsa aday listesi ondan; yoksa o düğüm için sürülebilir zeminden.
`road_ground`'un uyardığı yozlaşma böylece yerel ve sınırlı kalıyor — raf ancak yenecek yolun
olmadığı yerde kazanabiliyor, ve yolla çözülmüş komşuları onu hâlâ çekiyor.

| | waypoint | kursta süre | güverte %adım | 1:1'den dik | kavşak | furthest | hiç bırakmayan | fallen |
|---|---|---|---|---|---|---|---|---|
| **filtresiz** (eski) | 1.449 | %73,9 | **%18,6** | 11 | 1.649 | 5.918 m | 14 | 0 |
| `strict` | 1.114 | %71,5 | %18,9 | 36 | 1.334 | 4.784 m | 19 | 0 |
| `strict`+duvarsız | 1.414 | %74,9 | %11,7 | 44 | 1.651 | 5.746 m | 19 | 0 |
| **geri düşüşlü** (kalan) | **1.451** | **%74,3** | **%7,9** | **6** | **1.665** | **5.923 m** | 14 | 0 |

Bu tabloların hepsi `tools/sweep-columns.py <log-dizini> <temel-kol> <kol>...` çıktısı —
`sweep-table.py` yalnız waypoint sütununu okuyor, bu ise sürüş ve kot sütunlarının tamamını, her
satırda alan farkının yanına en büyük tek-rota farkını koyarak.

Rota rota güverte sapması (adımların yüzdesi, >3 m):

| rota | 4001 | 4002 | 4021 | 4041 | 4061 | 4081 | 4102 | 4121 |
|---|---|---|---|---|---|---|---|---|
| filtresiz | 33,7 | 6,9 | 7,5 | 2,6 | 11,8 | **63,6** | 0,0 | 22,6 |
| geri düşüşlü | **7,3** | 6,4 | 7,5 | **5,9** | 11,8 | **1,7** | 0,0 | 22,5 |

Sürüş nötr — waypoint +2 (gürültü tabanı ±37), kavşak +16, `furthest` +5 m, `fallen` 0, 64/64 araba
hâlâ kavşak alıyor — ve grafın kendi kalitesi düzeliyor: hiçbir yolun tırmanamayacağı bağlantı
11 → 6.

**Dürüstçe: iki rotada en kötü tek adım büyüyor** (4001 9,9 → 15,0 m; 4041 6,1 → 13,0 m) ve 4041'in
sapan adım oranı %2,6 → %5,9. Karşılığında 4081 %63,6 → %1,7 ve 4001 %33,7 → %7,3. Alan ölçüsü
adımların oranı; en kötü tek adım tek bir örnek ve öyle okunmalı.

**4081'de kalan sapma artık başka bir sınıf:** kalan %1,7'lik adımların **%0**'ında düğümün kendi
XZ'sinde arabanın kotunda bir yüzey var. Yani "seçim yanlış" sınıfı o rotada bitti; kalan "yüzey
gerçekten yok".

### Yol zemininin ortaya çıkardığı iki kusur — dünkü graf çözümünde

Filtresiz zeminde her düğümün en az iki adayı vardı, yani ikisi de hiç tetiklenmiyordu.

1. **Adaysız düğüm grafı ikiye bölüyordu.** `follow_graph`'ın BFS'i yalnız canlı düğümleri
   geziyordu, dolayısıyla adaysız her düğüm bir kesme noktasıydı — ve yol filtresiyle o düğümler
   tam olarak **hat sınırlarına** düşüyor, yani bu fonksiyonun var olma sebebi olan yeri kesiyordu.
   Artık köprüleniyor: yürüyüş en yakın canlı atayı deliğin içinden geçiriyor.
2. **`fill` bütün tabloyu tek dizi sayıyordu.** Düğüm indeksine göre ara değer biçiyor ve uçlarda
   düz tutuyor — hat içinde dürüst (medyan aralık 29 m), hat sınırında anlamsız: komşu, şehrin
   başka bir yerindeki başka bir yol. Hat hat çalışan koldaki davranış buydu zaten; graf kolu
   almamıştı. Düzeltildi.

**İkisi de eski zeminde ispatlı biçimde atıl.** Yalnız bu iki düzeltmeyi içeren kol (`NFS_ROADHEIGHT=0`)
sürüşün her sütununda temel ölçümle **birebir aynı** çıkıyor; tek fark 4081'de 1:1'den dik sayılan
bağlantının 3 → 2 olması, o da tek bir doldurulmuş düğümün kotunun hat içinde kalmasından.

`follow_graph`'ın hiç testi yoktu; dört tane yazıldı — zincirde `follow` ile aynı cevabı vermesi,
deliğin komşuları ayırmak yerine birleştirmesi, birleşmeyen parçaların sayılması, ve iki düz
güvertenin berabere kalıp alttakinin seçilmesi (yani bu modülün bütün derdinin tek testte yazılı
hâli).

### Sıradaki iş

- **Çizilen hat hâlâ katı yol zemininde.** `build_route` yol yoksa `fill`'in ara değerini alıyor,
  graf ise şehre soruyor — yani ikisi artık yalnız o 4-44 düğümde ayrışıyor (eskiden her yığında
  ayrışıyorlardı). Aynı geri düşüşü `build_route`'a vermek koridoru değiştirir, ve koridor her
  ölçümün "kursta mı" testi; ayrı bir süpürme işi.
- **`deck_had` ve gridin durduğu kot hâlâ filtresiz zemine soruyor** (`nfs_sim`). "Yüzey vardı"
  sayısı bu yüzden *normale göre sürülebilir bir katman* diyor, *yol* demiyor.
- **4001'in kalan %7,3'ü**, ve o adımların %95'inde düğümün kotunda yüzey var — yani orada hâlâ
  bir seçim yanlış. Bir sonraki bakılacak yer burası.
- **44 düğümün altında neden hiç yol yok?** Ölçüldü — aşağıdaki kayda bakın: filtre dar değil.

### `is_road` ölçüldü: filtre dar değil, eksik olan kapsam (2026-08-21)

Bir ad filtresi ancak adlara karşı sınanabilir, ve `Ground` adları kuruluş gereği atıyor — o bir
yükseklik alanı. Bu yüzden `world::surfaces_by_object` yazıldı: şehrin bütün üçgenlerini tek geçişte
tarayıp bir XZ'nin altında **hangi adlı nesnenin** hangi kotta, hangi yüzey sınıfıyla yüzey verdiğini
söylüyor. `nfs_sim`'de `NFS_ROADNAMES=<n>` onu sürüyor.

**Şehrin adlandırması.** `TRN_<bölge>_<sınıf>_..._CHOP_<hücre>_<lod>`. Adlar 27 karakterde
kırpılıyor, ama sınıf alanı 8. karakterde başlıyor, yani `ROAD` jetonu asla kırpılmıyor. Tüm
bundle'lar yüklüyken 13.986 nesne, 4.699 ad ailesi. En kalabalık sınıflar:

| sınıf | nesne | | sınıf | nesne |
|---|---|---|---|---|
| TERRAINA | 2.296 | | FOUNDATION | 121 |
| **ROADA** | **1.472** | | CEILINGSA | 112 |
| GRASS | 939 | | GRASSDRAG | 110 |
| TERRAIN | 700 | | TRAINTRACKS | 96 |
| **RDP** | **699** | | ROADDRAG | 88 |
| CONCRETE | 414 | | DRIFTSZ# | 70 |
| PROPSA / PROPSB | 327 / 195 | | ROAD# | 53 |

`is_road` **1.928** nesne yakalıyor — fonksiyonun kendi dokümanındaki sayı birebir doğrulandı — ve
yakaladığı yalnız `ROADA` değil: ROADA 1.537, ROAD# 184, ROADDRAG 88, ROAD 39, ROADB 27, artı
`ROADPIECE*` ailesi. **Aralarında 15 tane yüzey olmayan da var** (`ROADSIGNB` 3, `ROADBARRIERB` 3,
`ROADSKID*` 9); tabela ve bariyer geometrisi dik olduğu için `surface_of`'un onları eleyip elemediği
ayrıca ölçülmedi.

**Ve dokümanın `RDP_*` iddiası yanlış.** `RDP` bir yol sınıfı değil, bir **yer** öneki — havaalanı:
699 nesnenin 630'u `TRN_RDP_RUNWAY_*`, 44'ü `TRN_RDP_DRAG#_*`, 25'i `TRN_RDP_RUNWAYSKID_*`. Hiçbirinin
adında `ROAD` geçmiyor, yani `is_road` onların **sıfırını** yakalıyor. O cümle düzeltildi.

**Genişletmek hiçbir şey kurtarmıyor.** Sekiz rotanın 2.052 düğümünün 131'inin altında yol nesnesi
yok (bölge bundle'ı tek başına yüklüyken 190). Jetonu `is_road`'a eklemenin kurtardığı düğüm sayısı,
sekiz rota toplamı:

| jeton | TUNNEL | TUNNNEL | BRIDGE | MERIDIAN | RUNWAY | DRIFT | PUDDLE | PROPS | CEILING | TRAINTRACK | TERRAIN |
|---|---|---|---|---|---|---|---|---|---|---|---|
| kurtardığı | **0** | **0** | **0** | **0** | **0** | **0** | **0** | 4 | 7 | 34 | 131 |

Yedi adayın hepsi sıfır. Kurtaran üçü de eklenmemeli: `TERRAIN` bu filtrenin dışarıda tutmak için var
olduğu düz rafın ta kendisi, `CEILING` bir üst geçidin alt yüzeyi, ve `TRAINTRACK` bir ray —
`Paths4041`'de raylar y = −1'de, yarışın sürdüğü zemin ise 9,7 m yukarıda (`TRN_IP_TERRAINA_NR_CHOP_J*`),
yani eklemek arabaları garın tabanına indirirdi.

**Yüzey testi de saklamıyor.** O düğümlerin altında `surface_of`'un duvar saydığı bir yol nesnesi
bulunan düğüm sayısı sekiz rotada da **sıfır**. Filtre "yol yok" dediğinde gerçekten yol nesnesi yok.

**Asıl boşluk kapsam, ve yalnız üç rotada.** Bölge bundle'ı yerine bütün şehir yüklendiğinde:

| rota | 4001 | 4002 | 4021 | 4041 | 4061 | 4081 | 4102 | 4121 | toplam |
|---|---|---|---|---|---|---|---|---|---|---|
| bölge bundle'ı | 53 | 9 | 14 | 86 | 6 | 10 | 8 | 4 | **190** |
| tüm bundle'lar | **10** | **0** | 14 | 86 | 6 | **3** | 8 | 4 | **131** |

4001, 4002 ve 4081'de yolun bir kısmı gerçekten başka bir bölgenin bundle'ında; kalan beş rotada
şehir orada yolu hiç modellememiş. Bu `NFS_BUNDLE=all`'ı bir düzeltme yapmaz — kayıtta duruyor ki o
düğme delikleri bir arenanın tabanıyla kapatıyor — ama bu sefer kapanan şey *yol adlı* nesne, ki
farklı bir iddia ve ayrıca ölçülmeye değer.

**Sonuç: `is_road` olduğu gibi kalıyor.** Ölçülen bir refütasyon, bir düzeltme değil.

### Güverte ölçüsü iki ayrı şeyi karıştırıyormuş; ayrılınca sekizin beşi sıfır (2026-08-21)

Güverte sapması "araba, pilotunun tuttuğu düğümün kaç metre üstünde" diye ölçülüyordu ve **ikisinin
planda ne kadar uzak olduğunu hiç sormuyordu**. Pilot düzenli olarak onlarca metre ötedeki bir
düğümü tutuyor — grafın baştan beri bilinen paralel-şerit sorunu — ve o zaman ikisi ayrı yollarda
oluyor, yani aralarındaki kot farkı *rota seçimi* hakkında bir olgu, kot çözümü hakkında değil.

`DECK_NEAR = 15 m` eklendi (düğüm aralığı medyan 29 m, yani yolun üstünde duran bir araba tuttuğu
düğüme bundan yakındır) ve ölçü ikiye ayrıldı:

| rota | tüm adım >3 m | **araba düğümündeyken >3 m** | düğüme ort. plan mesafesi | >30 m olan adım |
|---|---|---|---|---|
| 4001 | %7,3 | **%9,6** | 14,5 m | %9,5 |
| 4002 | %6,4 | **%0,0** | 54,5 m | %43,0 |
| 4021 | %7,5 | **%0,0** | 30,7 m | %24,9 |
| 4041 | %5,9 | **%3,9** | 15,5 m | %12,1 |
| 4061 | %11,8 | **%2,1** | 32,0 m | %28,6 |
| 4081 | %1,7 | **%0,0** | 33,5 m | %42,6 |
| 4102 | %0,0 | **%0,0** | 47,1 m | %50,6 |
| 4121 | %22,5 | **%0,0** | 81,8 m | %49,0 |
| **ALAN** | | **%2,6** (579.625 adımın 15.321'i) | | |

Sekiz rotanın **beşi tam sıfır**. 4121'in iki gündür konuşulan %22,5'i tamamen bu: arabalar tuttukları
düğümden ortalama **81,8 m** uzakta.

**Ve kot çözümünü suçlayan sayı artık sıfır.** `deck_had` sürülebilir zemine soruyordu, yani "arabanın
kotunda bir katman var" diyordu, "bir yol var" demiyordu — ikincisi eklendi ve **sekiz rotanın
hepsinde %0,0**: sapan hiçbir adımda düğümün kendi XZ'sinde arabanın kotunda bir *yol* yok. Yol
zemini girdiğinden beri "seçim yanlış" sınıfı bitmiş durumda.

**Kalan sapmanın nerede olduğu da artık adlı adınca belli.** 4001'in %9,6'sının neredeyse tamamı dört
düğümde, ve dördünün de altında yol nesnesi yok:

| düğüm | çözülen kot | yol adayları | sürülebilir adaylar |
|---|---|---|---|
| 292 | 11,54 | — | [11,5 · 11,8 · 13,9] |
| 293 | 4,31 | — | [4,3 · 12,9 · 15,0] |
| 294 | 3,86 | — | [3,9 · 14,0 · 16,1] |
| 295 | 3,38 | [3,4] | [3,4 · 15,3 · 17,4] |

295'te şehrin verdiği tek yol 3,4'te ve çözüm doğru olarak onu alıyor; arabalar 18'de sürüyor. Yani
oradaki üst güvertenin `ROADA` nesnesi yok — `is_road` kaydındaki kapsam boşluğunun ta kendisi, bu
sefer sürüşün içinde görünüyor.

### Sıradaki iş, ve artık kot değil

Ayrım asıl işi ortaya çıkardı: **pilot, üstünde olmadığı bir düğümü tutuyor.** Arabanın tuttuğu
düğüme plan mesafesi rota ortalaması 14,5 m ile 81,8 m arasında, ve adımların %9,5 ile %50,6'sı
**bir düğüm aralığından (30 m) daha uzakta**. Bu, `Network::step_avoiding`'in kendi dokümanının
anlattığı paralel-şerit sorununun ilk kez sayıya dökülmüş hâli, ve ölçülen her şeyin üstünde
duruyor: kavşak sayımı, `strayed`, güverte, hepsi "tutulan düğüm" üzerinden tanımlı.

Bir sonraki ölçüm bu olmalı — ve dikkat: mesafe büyük olduğunda arabanın *daha yakın* bir düğüm
olup olmadığı ayrıca sorulmalı, çünkü "yanlış düğümü tutuyor" ile "orada düğüm yok" farklı
şeyler ve bu tablo ikisini ayırmıyor.

### Ölçüldü: pilot yanlış düğümü tutuyor, ve daha yakını genelde yarış hattında (2026-08-21)

Bir önceki kaydın sorduğu ayrım — "yanlış düğümü tutuyor" mu, "orada düğüm yok" mu — `NFS_HELD=1`
ile ölçüldü. Her adımda grafın planda en yakın düğümü aranıyor ve tutulanla karşılaştırılıyor:

| rota | tutulan = en yakın | 5 m'den daha yakını VARDI | onun yarış hattında olma oranı | tutulana ort. | en yakına ort. |
|---|---|---|---|---|---|
| 4001 | %88,0 | %6,9 | **%100,0** | 14,5 m | 13,3 m |
| 4002 | %31,7 | **%55,7** | %77,9 | **54,5 m** | **12,6 m** |
| 4021 | %57,5 | %34,3 | %88,1 | 30,7 m | 10,9 m |
| 4041 | %65,0 | %22,9 | %87,9 | 15,5 m | 10,3 m |
| 4061 | %77,4 | %17,2 | %93,5 | 32,0 m | 19,0 m |
| 4081 | %46,3 | **%46,3** | %20,6 | 33,5 m | 16,5 m |
| 4102 | %67,4 | %28,6 | **%100,0** | 47,1 m | 36,5 m |
| 4121 | %44,2 | **%49,0** | %24,9 | **81,8 m** | 16,2 m |

**Cevap birinci okuma.** Graf orada düğüm sunuyor: 4002'de araba tuttuğu düğümden ortalama 54,5 m
uzaktayken en yakın düğüm 12,6 m'de, 4121'de 81,8 m'ye karşı 16,2 m. Ve adımların yarıya yakınında
5 m'den daha yakın bir düğüm var.

**Üstelik o düğüm genelde doğru olanı.** 4001, 4021, 4041, 4061 ve 4102'de daha yakın düğümün
**%88-100'ü yarış hattında** (`mark_line`, 40 m). Yani mesele paralel şeride kayma değil; pilot
hattın üstündeki daha yakın bir düğümü görmezden geliyor ve geride kalmış birini tutmaya devam
ediyor.

4081 ve 4121 ayrı duruyor: orada daha yakın düğümün yalnız %21-25'i hatta, yani o iki rotada araba
gerçekten yarışın hattından çıkmış — ki 4121 için bu zaten biliniyordu (düğüm 110→111, 21 sapmanın
21'inde hatta kalan bir kol vardı ve alınmadı).

**Neden önemli:** ölçülen hemen her şey "tutulan düğüm" üzerinden tanımlı — kavşak sayısı,
`strayed`, güverte sapması, `step_avoiding`'in maliyeti. Pilot yarışın ortalama yarım şehir bloğu
gerisindeki bir düğümü tutuyorsa, o sayıların hepsi kaymış bir referansa göre okunuyor.

**Sıradaki iş:** pilotun düğüm ilerletme kuralı. Dikkat: "en yakın düğüme atla" diye bir kural
denenmemeli — grafın kendi başlığı paralel şeritleri birbirine bağladığını söylüyor, ve `guide_to`
ile `along_roads` tam olarak o yüzden kaybetti. Ölçülecek şey, ilerletmenin neden geride kaldığı.

### İlerletme kuralı okundu, iki aday sınandı, ikisi de çürüdü (2026-08-21)

**Mekanizma tek bir satırda.** `self.at`'i üç yer yazıyor: `Pilot::place` (araba başına bir kez),
tıkanma çıkışı, ve yürüyüş — yani bir düğüm ancak *üstünden yürünerek* bırakılabiliyor,
yeniden-edinme yolu yok. Yürüyüşün tamamı:

```rust
for _ in 0..3 {
    let Some(next) = net.step_avoiding(self.at?, self.from, toward, &self.blocked) else { break };
    if dist(next) >= dist(self.at?) { break; }
```

`step_avoiding` **tek** bir kol döndürüyor ve onu `toward`'a — hedef waypoint'e — yakınlığa göre
seçiyor; kapı ise **arabaya** yakınlığa bakıyor. İki ayrı amaç, ve döngü düğümün diğer kollarını hiç
görmüyor. Üstelik kapı araba uzaklaştıkça *kolaylaşıyor* (`dist(held)` büyüyor), yani 54,5 m ve
81,8 m'lik ortalamalar kapının muhafazakârlığı değil: sunulan tek kolun kendisi arabadan o kadar
uzak.

**İki aday sınandı, ikisi de düştü.**

| | waypoint | kursta süre | hiç bırakmayan | kavşak | furthest | tutulan=en yakın | tutulana ort. |
|---|---|---|---|---|---|---|---|
| **kalan** | **1.451** | **%74,3** | **14** | 1.665 | **5.923 m** | %59,7 | 38,7 m |
| `NFS_WALKCAP=12` | 1.446 | %74,2 | 13 | 1.665 | 5.923 m | %59,4 | 39,0 m |
| `NFS_ADVANCE=arms` | 1.359 | %68,9 | 11 | 1.900 | 5.793 m | **%69,7** | **32,9 m** |

**Derinlik bağlayıcı değil.** Sıçrama üst sınırını 3'ten 12'ye çıkarmak 1.451 waypoint'in 5'ini,
tek rotada oynatıyor — gürültü tabanının altıda biri. Yürüyüş sıçramadan bitmiyor, kapıda ilk
adımda duruyor.

**Ve "diğer kollara da bak" kuralı tam olarak yapması gerekeni yapıyor, araba yine de daha kötü
sürüyor.** İşaretçi yakalanıyor: tutulan düğümün en yakın olma oranı %59,7 → %69,7, arabanın
tuttuğu düğüme ortalama mesafesi 38,7 → 32,9 m, 4121'de 81,8 → 46,3 m ve 4002'de 54,5 → 31,9 m.
Buna karşılık alan 92 waypoint, kursta süre 5,4 puan ve 3 araba kaybediyor — ve en çok gecikmenin
en büyük olduğu yerde: **4121 158 → 72 waypoint**, furthest 787 → 380 m, gecikmesi yarıya inerken.
Kavşak ve ayrık düğüm sayısındaki artış (1.665 → 1.900, 1.588 → 1.780) arabanın değil işaretçinin
hareketi.

**Yani işaretçinin geride kalması bir belirti, kusur değil.** Onu en yakın düğüme çekmek, *en yakın
hangi yolsa* ona çekiyor — ki bu grafta düzenli olarak yarışılanın yanındaki şerit. `guide_to`'yu ve
yürünmüş halkayı öldüren cümlenin aynısı: taahhüt edilmiş bir yakınlık, arabanın süremeyeceği
bağlantılardan geçiyor.

**Bulgudan geriye kalan, ve sınanmamış olan, aşağı akışta.** Nişan yürüyüşünün sayacı arabanın
tuttuğu düğüme olan mesafesiyle tohumlanıyor ve `look` en fazla `LOOKAHEAD_MAX = 40 m`. Gecikme
54,5 m veya 81,8 m olunca tohum tek başına `walked >= look`'u sağlıyor, yani o rotalarda
**lookahead sabiti tamamen atıl** ve yürüyüşün tek kalan çıkışı "arabanın önündeki ilk düğüm" —
kontrolsüz bir mesafede. Ölçülmesi gereken bir sonraki şey nişan mesafesinin kendi dağılımı.

### Nişan mesafesini hiçbir şey denetlemiyormuş; denetleyen kural kalıcı (2026-08-21)

Bir önceki kaydın işaret ettiği yer ölçüldü. Nişan yürüyüşünün sayacı arabanın **tuttuğu düğüme**
olan mesafesiyle tohumlanıyor, `look` ise `hız × 1,8 s` ve 12-40 m'ye kırpılı. Gecikme 54,5 m veya
81,8 m olunca tohum tek başına `walked >= look`'u sağlıyor, yani yürüyüşün tek kalan çıkışı
"nişan arabanın önünde" — mesafeyi hiçbir şey belirlemiyor. Ölçüldü:

| rota | nişan ort. | %40 m'den uzak | %100'den | nişan tutulan düğümün üstünde | nişan arabanın ARKASINDA |
|---|---|---|---|---|---|
| 4001 | 45,4 m | %55,9 | %2,9 | %6,3 | %0,3 |
| 4002 | 33,2 m | %19,5 | %7,6 | %9,4 | **%29,7** |
| 4021 | 40,0 m | %37,0 | %2,2 | %5,5 | %12,0 |
| 4041 | 36,5 m | %37,0 | %0,1 | %4,4 | %5,8 |
| 4061 | 49,4 m | %55,5 | %5,0 | %13,4 | %10,6 |
| 4081 | 33,7 m | %28,5 | %0,8 | %6,9 | %19,0 |
| 4102 | **56,6 m** | %46,4 | **%15,8** | %14,1 | %22,7 |
| 4121 | 54,0 m | %43,4 | %10,8 | %9,0 | %9,3 |

40 m'lik tavan hiçbir rotada ortalamayı tutmuyor, ve 4002'de adımların **%29,7'sinde nişan arabanın
arkasında** — modülün kendi dokümanının "tam kilit ve bir daire" dediği durum.

### `NFS_AIMREACH` — yürüyüşü arabadan uzaklığa göre durdur (kalıcı)

Saf takip (pure pursuit) yolun üstünde **araçtan** `look` metre uzaktaki noktayı ister. Durdurmayı
öyle ölçmek, yol-mesafesi sayacının var oluş sebebini bozmuyor — nişan hâlâ yürünen yolun üstündeki
bir düğüm, yani hâlâ köşeyi kesmek yerine yolu dönüyor — sadece yürüyüşün nerede durduğunu
değiştiriyor.

| | waypoint | kursta süre | hiç bırakmayan | away | fallen | kavşak | furthest |
|---|---|---|---|---|---|---|---|
| eski | 1.451 | %74,3 | 14 | 64 | 0 | 1.665 | 5.923 m |
| **`reach` (kalan)** | 1.355 | **%80,0** | **33** | 64 | 0 | 1.454 | 5.432 m |
| `lerp` (tam çember) | 1.306 | %77,4 | 22 | 64 | 0 | — | — |
| yalnız tavan 60/100 | 1.462 | %74,2 | **10** | 64 | 0 | — | — |

**Alan waypoint'i düşüyor ve bu bir kayıp değil** — deponun kendi okuma tuzağı, tersinden. Metrikler
kursu bırakan/bırakmayan diye ayrıldığında:

| | araba | waypoint | araba başına |
|---|---|---|---|
| eski · hiç bırakmayan | 14 | 262 | 18,7 |
| eski · bırakan | 50 | 1.189 | 23,8 |
| **`reach` · hiç bırakmayan** | **33** | **768** | **23,3** |
| `reach` · bırakan | 31 | 587 | 18,9 |

Yani kursta kalan nüfus ikiye katlanıyor **ve** o nüfusun araba başına ilerlemesi %25 artıyor
(18,7 → 23,3); alan toplamının düşmesi, 23,8 puan toplayarak dolaşan kayıp nüfusun 50'den 31'e
inmesinden. Rota rota, kursu hiç bırakmayan arabalar (sayı / waypoint):

| rota | 4001 | 4002 | 4021 | 4041 | 4061 | 4081 | 4102 | 4121 |
|---|---|---|---|---|---|---|---|---|
| eski | 1 / 42 | 5 / 31 | 6 / 138 | 2 / 51 | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| `reach` | **6 / 213** | **6 / 44** | 8 / 116 | **4 / 129** | **5 / 222** | **4 / 44** | 0 / 0 | 0 / 0 |

Beş rotada kazanıyor, 4021'de araba sayısı artarken toplam düşüyor, iki rotada ikisi de sıfır.

**İki rakip açıklama ölçülerek elendi.**

1. **"Tam denetim daha iyi olmalı" — hayır.** `NFS_AIMREACH=lerp` nişanı lookahead çemberiyle
   kesişime koyuyor, yani mesafeyi *tam* olarak `look`'a oturtuyor (4061'de nişan ortalaması
   55,1 → 29,0 m, tavanın ötesi %85,1 → **%0,0**). Alan: 1.306 waypoint, %77,4, 22 araba — üç
   sütunda da `reach`'in altında.
2. **"O hâlde tavan küçük" — hayır, ve bu ayırt edici deney.** `NFS_LOOKMAX=60` ve `=100` (kural
   yok, yalnız tavan) birbirinin aynı çıkıyor ve **kaybettiriyor**: kursta süre %74,2, hiç
   bırakmayan 14 → **10**, o nüfusun araba başına ilerlemesi 18,7 → **16,2**. Mekanizma mesafe
   değil; yürüyüşü *arabanın nerede olduğuna* göre durdurmak.

**Bedeli 4121, ve yazılı duruyor.** O rotada kursta süre %66,2 → %48,0, waypoint 158 → 72, furthest
787 → 360 m. İki kolda da kursu hiç bırakmayan arabası yok, yani sayılarının tamamı dolaşan
nüfusun; ama kursta süre deponun tercih ettiği ölçü ve orada gerçek bir kayıp. Muhtemel sebep
ölçülü: `reach` yürüyüşü uzattığı için sekiz-sıçrama sınırı daha sık tükeniyor ve nişan arkada
kalıyor — 4121'de arkada kalma **%9,3 → %26,8**, 4001'de %0,3 → %8,4.

Tabloların hepsi `tools/sweep-columns.py` çıktısı; nüfus ayrımı da artık orada, çünkü bu değişikliği
"kayıp" gibi gösteren tek şey onu yapmamaktı.

**Sıradaki iş:** yürüyüş sıçrama sınırını tüketince nişanın arkada bırakılması. Sınırı büyütmek
veya tükenince önde görülmüş son düğüme dönmek — ikisi de ölçülmedi, ve 4121 ikisinin de sınavı.

### Sıçrama sınırı de çürüdü, ama yarısını düzeltmesi asıl bilgiyi verdi (2026-08-21)

Bir önceki kaydın bıraktığı iki aday sınandı. Karar ölçüsü yine nüfus ayrımı:

| | hiç bırakmayan araba | onların waypoint'i | araba başına | kursta süre |
|---|---|---|---|---|
| **kalan** | **33** | **768** | **23,3** | **%80,0** |
| `NFS_AIMKEEP=1` | 31 | 686 | 22,1 | %79,8 |
| `NFS_AIMHOPS=16` | 27 | 504 | 18,7 | %78,2 |

**Sıçrama sınırını büyütmek düpedüz kaybettiriyor** — 8'den 16'ya çıkmak kursta kalan arabayı 33'ten
27'ye, onların ilerlemesini 23,3'ten 18,7'ye indiriyor. Yürüyüş sıçramadan yoksun değil.

**"Tükenirse önde görülmüş en iyi noktaya dön" kuralı hedefini kısmen tutuyor ve alanı yine de
kaybediyor.** 4121'de waypoint 72 → 102 ve kursta süre %48,0 → %52,1 — yani yazıldığı rotada
çalışıyor — ama 4061'de tersi oluyor (318 → 302 waypoint, %90,6 → %84,7, kursta kalan 5 → 3) ve
alanda kursta kalan nüfus 33 → 31, ilerlemesi 23,3 → 22,1. Deponun kuralı gereği silinmiyor ama
varsayılan da olmuyor.

**Asıl bilgi kısmî olmasında.** Kural, yürüyüş önde bir düğüm *görmüşse* ateşleniyor. 4121'de nişanın
arkada kalması %26,8 → **%13,7**'ye iniyor, sıfıra değil — ve kalan %13,7 tam olarak
`front == None` hâli, yani:

> `Paths4121`'de nişanın arkada kaldığı adımların **yarısında**, pilotun tuttuğu düğümden sekiz
> sıçrama boyunca yürünen yol üzerinde **arabanın önünde tek bir düğüm yok**.

Bu "yürüyüş erken pes etti" değil; nişan alınacak bir şey yok demek. Araba, üstünde yürüdüğü grafın
gittiği yönün tersine bakıyor. 4121'in uzun süredir bilinen düğüm 110 → 111 sapması tam olarak bu
şeklin adı, ve buradaki sayı ona ilk kez bir büyüklük veriyor.

**Sıradaki iş:** o adımlarda pilotun tuttuğu düğüm nerede, arabanın burnu nereye bakıyor, ve grafın o
noktadaki kolları nereye gidiyor. `NFS_ARM=<düğüm>` bunu tek bir kavşak için zaten yazdırıyor;
eksik olan, bu adımların hangi düğümlerde yoğunlaştığı.

### 4121'in sapması tek bir kenar, ve hattaki alternatifi geriye gidiyor (2026-08-21)

Nişanın arkada kaldığı adımlar, pilotun o sırada tuttuğu düğüme göre sayıldı. Dağılmıyorlar:

| rota | en çok tutulan düğüm | adım | rotanın tüm adımlarının | yarış hattına |
|---|---|---|---|---|
| **4121** | **274** | 33.377 | **%20,0** | **156 m** |
| 4102 | 43 | 22.418 | %13,4 | hatta |
| 4002 | 16 | 12.120 | %7,3 | hatta |

4121'de tek bir düğüm rotanın **bütün adımlarının beşte birini** ve nişanın arkada kaldığı adımların
**%75'ini** taşıyor. `NFS_ARM=274`: düğüm hattan 156 m uzakta ve beş kolunun hepsi de 72-167 m
uzakta — grafın oradan yarışa dönen bir kolu yok. Araba oraya vardıktan sonra nişan alacak bir şey
kalmıyor; bu bir nişan kuralı sorunu değil, varış yeri sorunu.

**Oraya nasıl gidiliyor, kenar kenar:**

| düğüm | hatta uzaklık | kolları (hatta uzaklık) |
|---|---|---|
| 110 | 10 m | 109 (4 m, HATTA) · 111 (26 m, HATTA) · 294 (2 m, HATTA) |
| **111** | **26 m** | 110 (10 m, HATTA) · **112 (72 m)** · 294 (2 m, HATTA) |
| 112 | 72 m | 111 (26 m, HATTA) · 113 (120 m) · 274 (156 m) |

**Ve bu, iki gün önceki refütasyonu açıklıyor.** Kayıtta "sapma 110 → 111'de, ve 21 sapmanın
21'inde hatta kalan bir kol vardı ve alınmadı" yazıyor. Kenar aslında **111 → 112**, ve 111'de
hatta kalan kol gerçekten var: 294, hattan yalnız 2 m. Ama 294 **118 m uzakta ve geriye gidiyor** —
111 (−371) ile 294 (−488) arasında 110 (−424) duruyor, yani 294 arabanın az önce geçtiği yerin
batısında, üstelik farklı bir hatta (hat 1, 111 ise hat 2).

Yani "hattaki kolu tercih et" kuralı arabaları geri gönderiyordu. `mark_line` ölçümü doğruydu,
kuraldan çıkarılan sonuç yanlıştı, ve neden yanlış olduğu artık tek bir kavşakta yazılı:
**111'in ileri giden tek kolu hattı terk ediyor.**

**Sıradaki iş buradan iki yöne gidiyor, ve ikisi de sürücü tarafında değil:**

- Yarış hattı 111'den sonra nereye gidiyor? Halka koridora çekildiğinden beri hattın kendisi
  yolların üstünde; eğer 111'den sonra hat, grafın 112'ye giden kenarından 72 m ötede devam
  ediyorsa, orada yol var ve graf onu bilmiyor demektir — `is_road` kaydındaki kapsam sorusunun
  sürüşteki karşılığı.
- 274 ve 112/113, `Paths4121`'in kendi dosyasında hangi hatta ait? 274 hat 3, 112-113 hat 2, 294
  hat 1. Rota dosyası bu üç hattı birbirine bağlıyor ve yarış yalnız birinden geçiyor; hangisi
  olduğunu söyleyen tek şey outline, ve outline 40 m'lik bir bantla soruluyor.

### DÜZELTME — `NFS_AIMREACH` varsayılanı geri alındı: aynı tuzağa aynı gün ikinci kez düşüldü (2026-08-21)

Yukarıdaki kabul **koridorda geçen süre** (%74,3 → %80,0) ve **kursu hiç bırakmayan araba**
(14 → 33) sütunlarına dayanıyordu. Bu dosyada, aynı gün, birkaç bölüm yukarıda yazıyor:

> *koridorda geçen süre* ve *hattı bırakma* ölçülerinin ikisi de **duran arabayı başarı sayıyor**.
> Arabaları durdurabilecek her değişiklik, kavşak · ayrık düğüm · takılma sayısıyla **birlikte**
> yargılanmalı.

Ders kendi kabulüme uygulandı ve kabul geçmiyor. İlerleme ölçüsü — son üçte birinde tek waypoint
kazanmayan araba:

| rota | 4001 | 4002 | 4021 | 4041 | 4061 | 4081 | 4102 | 4121 | ALAN |
|---|---|---|---|---|---|---|---|---|---|
| eski | 0 | 5 | 3 | 4 | 3 | 5 | 6 | 4 | **30 / 64** |
| `reach` | 2 | 5 | **6** | **0** | **1** | 6 | 6 | **8** | **34 / 64** |

Dört rotada kötüleşiyor, ikisinde düzeliyor, ikisinde aynı — ve `Paths4121`'de **sekiz arabanın
sekizi** duruyor. Aynı yöne bakan diğer sütunlar: kavşak 1.665 → 1.454, ayrık düğüm 1.588 → 1.406,
`furthest` 5.923 → 5.432 m.

**Ne ayakta kalıyor, ne kalmıyor.** Nişan sayımı bir olgu ve mekanizma gerçek: nişanın mesafesini
hiçbir şey belirlemiyordu, bu kural belirliyor. Nüfus okuması da gerçek: kursta kalan araba ikiye
katlanıyor **ve** araba başına 18,7 → 23,3 waypoint kat ediyor. Ama üç ilerleme sütunu ve durma
sayısı ters yöne bakıyor, ve bir rota hepsinde birden çöküyor. Bu bir varsayılan değil, `Paths4121`'de
çözülecek bir takas.

`NFS_AIMREACH=1` artık opt-in; varsayılan, 2026-08-21 öncesi her sayının alındığı yol-sayacı.

**Ve alet düzeltildi ki bu bir daha unutulmasın:** `tools/sweep-columns.py` artık *ilerlemesi duran
araba* ve *duruş payı* sütunlarını da basıyor. Bu ders bu dosyada iki kez öğrenildi — ikisi de bir
kabulü geri almaya mal oldu — ve ikisinde de eksik olan şey ölçünün elde olmamasıydı.

### Halkanın delikleri gerçekten yol, ama yürüyerek doldurmak çürüdü — hem de kendi rotasında (2026-08-21)

**Önce olgu.** `NFS_RING` artık her geniş aralık için grafın iki ucu bağlayıp bağlamadığını da
soruyor. Bağlıyor, ve ucuz bağlıyor:

| rota | geniş aralık | yolla / kiriş oranları |
|---|---|---|
| 4102 | 7 | 0,9 · 1,0 · 1,2 · 1,2 · 1,3 · 1,4 · 1,5 — **hepsi ≤1,5** |
| 4002 | 11 | 0,8 · 1,0 · 1,0 · 1,1 · 1,2 · 1,2 · 1,2 · 1,2 · **3,9 · 5,8** |
| 4121 | 8 | 0,9 · 1,2 · **1,6** · 2,0 · 2,1 · 2,3 · 4,3 · 4,6 |

4121'in kritik deliği — rotanın adımlarının beşte birinin harcandığı yer — **228 m kiriş, 373 m
yolla, ×1,6, sekiz düğüm**. Yani halkanın delikleri, yolun gitmediği yerler değil; halkanın tarif
etmediği yollar. Bu, çekmenin bir yan etkisi olarak zaten biliniyordu; şimdi grafın o yolları
bildiği de ölçülü.

**Sonra çürütme.** `NFS_WALKGAPS=<oran>`: yalnız geniş aralıkları, grafı yürüyerek doldur; oranı
aşan aralığı olduğu gibi bırak. Bu ne `along_roads` (bütün halkayı yürüyordu, çürüdü) ne
`NFS_FILLGAPS` (aralıkları düz lerp'liyordu, geri alındı) — üçüncü biçim, ve denenmemişti.

**Ve tam da yazıldığı rotada kaybediyor:**

| rota | 4001 | 4002 | 4021 | 4041 | 4061 | 4081 | 4102 | **4121** |
|---|---|---|---|---|---|---|---|---|
| kursta süre | = | = | −9,0 | −3,3 | **+10,9** | = | −0,2 | **−10,5** |
| furthest | +2 | 0 | −1 | −51 | +8 | 0 | +29 | **−214 m** |

4061 ve 4102'de kazanıyor, 4021/4041/4121'de kaybediyor, ve 4121'in kursta süresi %66,2 → %55,7.
Deliği kapatmak o rotayı düzeltmiyor, kötüleştiriyor. `×2` ile `×3` birbirinin aynı çıkıyor, yani
eşik de bir kaldıraç değil.

**Ve bir okuma tuzağı daha, bu sefer benim yeni eklediğim sütunu da vuruyor.** Bu kol halkanın
**boyunu** değiştiriyor (4021: 65 → 92, 4102: 104 → 138, 4121: 130 → 170 waypoint). Hem "geçilen
waypoint" hem de "ilerlemesi duran araba" halkaya karşı sayılıyor — daha çok waypoint, hem geçilecek
daha çok şey hem de kazanma fırsatı demek. Yani bu kolun +82 waypoint'i ve −5 duran arabası
**kanıt değil**. ROADMAP bunu `NFS_REDENSIFY` için zaten yazmıştı ("waypoint sayısı burada hakemlik
edemez"); yeni durma sütunu aynı kusuru miras alıyor ve `tools/sweep-columns.py` artık ikisini de
söylüyor.

Halkanın boyunu değiştiren bir kol yalnız şunlarla yargılanır: `furthest`, kavşak, ayrık düğüm,
kursta süre (koridor `build_route`'tan gelir, halkadan değil), `away`, `fallen`. Hepsi bu kolda ya
düz ya aşağı.

## Günün sonunda nerede duruyoruz (2026-08-21)

Bugün on sekiz commit girdi ve bir tek varsayılan taşındı. Bu bölüm bir özet, yeni bir iddia değil;
her satırın dayanağı yukarıda kendi kaydında duruyor.

### Taşınan tek varsayılan

**Grafın düğüm kotları artık yol-filtreli zeminden geliyor**, ve şehrin hiç yolu olmayan düğümlerde
sürülebilir zemine düşüyor. Güverte uyuşmazlığı adımların %18,6'sından %7,9'una indi, hiçbir yolun
tırmanamayacağı bağlantı 11 → 6, ve sürüş nötr kaldı (1.449 → 1.451 waypoint). Bunun ortaya
çıkardığı iki kusur — adaysız düğümün grafı bölmesi ve `fill`'in hat sınırını aşması — düzeltildi ve
eski zeminde ispatlı biçimde atıl.

### Kapanan sorular

| soru | cevap |
|---|---|
| Çözümü çıkış gridinden çıpalamak? | **Hayır** — sekiz rotanın hepsinde graf gridle zaten ≤1,4 m uyuşuyor, çıpanın düzeltecek şeyi yok |
| `is_road` çok mu dar? | **Hayır** — TUNNEL/BRIDGE/MERIDIAN/RUNWAY/DRIFT/PUDDLE jetonlarının her biri sıfır düğüm kurtarıyor; kurtaran üçü (TERRAIN/CEILING/TRAINTRACK) eklenmemeli |
| Dokümanın `RDP_*` iddiası? | **Yanlış** — RDP bir yol sınıfı değil, havaalanı bölgesi öneki; 699 nesnesinin hiçbirinde `ROAD` geçmiyor |
| Güverte sapması bir kot hatası mı? | **Artık değil** — araba gerçekten tuttuğu düğümdeyken alan %2,6, sekiz rotanın beşi tam sıfır, ve sapan hiçbir adımda düğümün kotunda bir *yol* yok |
| Pilot yanlış düğümü mü tutuyor? | **Evet, ölçüldü** — 4121'de ortalama 81,8 m'ye karşı en yakın düğüm 16,2 m'de; ve **düzeltmek kaybettiriyor** |
| Halkanın delikleri yol mu? | **Evet** — grafın köprülediği yol medyan ×1,2; ama **doldurmak kendi rotasında kaybediyor** |
| Arabalar daire mi çiziyor? | **Hayır** — alan kavşak/ayrık düğüm oranı 1,05 |
| 4102'de yan yatmak durmanın sebebi mi? | **Hayır** — yan yatan 4 araba 177 kavşağın 73'ünü alıyor |

### Bugün çürüyenler (hepsi düğme olarak, ölçüleriyle duruyor)

`drop_climbing` · çıkış-gridi çıpası · `NFS_ADVANCE=arms` · `NFS_WALKCAP` · `NFS_AIMHOPS` ·
`NFS_AIMKEEP` · `NFS_AIMREACH` (kabul edildi, aynı gün geri alındı) · `NFS_AIMREACH=lerp` ·
`NFS_LOOKMAX` · `NFS_REDENSIFY` (yeni tabana karşı) · `NFS_WALKGAPS`.

### Öğrenilen okuma kuralları — ikisi bir kabule mal oldu

1. **Duran araba başarı sayılıyor.** *Koridorda geçen süre* ve *hattı bırakmama* bir kabulün tek
   dayanağı olamaz; kavşak · ayrık düğüm · `furthest` · duran araba ile birlikte okunmalı.
2. **Halkanın boyunu değiştiren kol iki sütunu diskalifiye eder.** *Geçilen waypoint* ve *ilerlemesi
   duran araba* halkaya karşı sayılır; böyle bir kol yalnız `furthest`, kavşak, ayrık düğüm, kursta
   süre, `away` ve `fallen` ile yargılanır.
3. **Pilotun tuttuğu düğüme göre tanımlı her sütun,** ilerletme kuralını değiştiren bir kolda
   kendiliğinden hareket eder.

`tools/sweep-columns.py` üçünü de kendi dokümanında söylüyor ve gereken sütunları basıyor.

### Açık cephe

- **4121 ve 4102'de kursu hiç bırakmayan araba sıfır**, her kolda. 4121'in sebebi bilinen: düğüm
  111'in ileri giden tek kolu hattı terk ediyor, ve varılan yer (274, hattan 156 m) bir çukur —
  grafın oradan dönen kolu yok.
- **4001'in kalan güverte sapması** dört düğümde (292-295) ve dördünün de altında yol nesnesi yok.
- **Nişan mesafesini denetleyen kural** (`NFS_AIMREACH`) gerçek bir mekanizmayı düzeltiyor ama
  ilerleme sütunlarında bedelli. Takas 4121'de çözülür.

### Ve 4102'nin 56°'si daraltıldı: açı gecikmenin kendisi (2026-08-21)

Nişan açısı direksiyon komutudur, ve `Paths4102`'de ortalaması **56°** — pilot yarışın yarısında
yana bakıyor. Açı, arabanın *yürüyüşün başladığı düğüme* uzaklığına göre ayrıldı:

| rota | genel | araba düğümünde (≤15 m) | düğümünden uzakta (>15 m) |
|---|---|---|---|
| **4001** | 10° | **9°** | **10°** |
| 4002 | 59° | 44° | 65° |
| 4021 | 35° | **18°** | 50° |
| 4041 | 26° | **15°** | 42° |
| 4061 | 35° | **16°** | 44° |
| 4081 | 47° | **12°** | 63° |
| 4102 | 56° | **17°** | 76° |
| 4121 | 32° | **9°** | 43° |

**Sekiz rotanın yedisinde, araba tuttuğu düğümün üstündeyken nişan 9-18°'dir.** Aynı arabalar,
aynı pilot, aynı sabitler. On beş metreden uzaklaştığında açı 42-76°'ye çıkıyor. 4001 kontrol
grubu: gecikmesi en küçük olan rota (ortalama 14,5 m) ve tek fark etmeyen rota (9° / 10°).

**Zincir kapandı:** işaretçi geride kalıyor → nişan yana kayıyor → pilot tam kilit istiyor. Bugün
"pilot çok sert direksiyon kırıyor" diye bakılan her şey — ve dünlerde direksiyon tepkisini
yumuşatan her refütasyon — bu tek olgunun aşağı akışı. Direksiyon kuralında yanlış bir şey yok;
ona söylenen şey yanlış.

**Ve bu, bugünün iki refütasyonunu birlikte okumayı gerektiriyor.** Gecikmeyi kapatan iki kural da
(`NFS_ADVANCE=arms`, `NFS_AIMREACH`) alanı kaybetti — ama ikisi de işaretçiyi *en yakın yola*
çekerek kapatıyordu, ki bu grafta düzenli olarak yarışılanın yanındaki şerit. Bir sonraki soru
artık dar ve doğru biçimde sorulmuş durumda: **işaretçi, paralel şeride atlamadan arabayla nasıl
birlikte tutulur.** `mark_line` hangi düğümlerin yarışın hattında olduğunu zaten biliyor ve
ilerletme döngüsü ona hiç bakmıyor — hattı *kol seçiminde* kullanmak çürüdü, *işaretçiyi
sınırlamakta* kullanmak denenmedi.

### İşaretçiyi hatta sınırlamak da çürüdü — ve kapatması gereken şeyi kapatmıyor (2026-08-21)

Bir önceki kaydın adını koyduğu denenmemiş kural yazıldı. `NFS_ADVANCE=line`: `arms`'ın aynısı, ama
geri düşülen kol yalnız `mark_line`'ın yarışın hattında saydığı düğümlere. Gerekçe, bugünün iki
sonucunu birleştirmekti — `arms` gecikmeyi kapatıyordu ama en yakın *yola* atlıyordu, ve hangi
düğümün yarışın hattında olduğunu graf zaten biliyor.

| | waypoint | kursta süre | hiç bırakmayan | **ilerlemesi duran** | furthest | tutulana ort. |
|---|---|---|---|---|---|---|
| **kalan** | **1.451** | **%74,3** | **14** | **30 / 64** | **5.923 m** | 38,7 m |
| `line` | 1.368 | %66,6 | 11 | **44 / 64** | 5.372 m | **38,0 m** |

Her sütunda kaybediyor, ilerleme sütunlarında da: duran araba 30 → **44**, duruş payı %31,6 → %40,9.
Kavşak ve ayrık düğümün yükselmesi (1.665 → 1.808, 1.588 → 1.719) daha çok kaybolan arabanın grafta
dolaşması. Nüfus ayrımı da aynı yöne: kursta kalan 14 → 11 ve araba başına 18,7 → 16,8.

**Ama asıl bilgi son sütunda.** Kural gecikmeyi **kapatmıyor**: arabanın tuttuğu düğüme ortalama
mesafesi 38,7 → 38,0 m, yani hiç. Sebebi de belli: gecikmenin en büyük olduğu yerlerde tutulan
düğümün kendisi hattın dışında (4121'in 274'ü hattan 156 m), orada geri düşme hiç ateşlenmiyor; ve
ateşlendiği yerlerde hat kısıtı çoğu zaman elinde aday bırakmıyor.

**Bu, kol seçimi kapısını kapatıyor.** İşaretçinin gecikmesi *hangi kol* sorusuyla kapanmıyor: bütün
kollara bakmak (`arms`) kapatıyor ama paralel şeride atlıyor, hatta sınırlamak (`line`) ne kapatıyor
ne de kazandırıyor. Geriye sentezin çözülmemiş bıraktığı çatal kalıyor: ilerletme kapısı tıkandığında
sebep **bayat `toward`** mu (waypoint sayacı donmuş, `step_avoiding` de ona göre kol seçiyor), yoksa
kara liste / `came_from` arabaya doğru olan kolu elemiş mi? İkisi farklı işler, ve ayrımı ölçülmedi.

### İşaretçi neden takılıyor: üç yönlü ayrım, ve iki rotanın iki ayrı sebebi (2026-08-21)

İşaretçinin bir düğüm aralığından (30 m) fazla geride kaldığı her adımda grafın ne sunduğu soruldu.
Dört şık: arabaya daha yakın **uygun** bir kol vardı (yani kapı açılabilirdi ve `step_avoiding`'in
hedefe göre seçtiği tek kol o değildi), yalnız **kara listedeki** bir kol daha yakındı, yalnız
**geldiği** kol daha yakındı, ya da hiçbir kol daha yakın değildi.

| rota | uygun kol | **kara liste** | **geldiği kol** | hiçbiri | hedef waypoint kaç sn'dir donmuş |
|---|---|---|---|---|---|
| 4001 | %6,3 | %0,0 | %26,9 | **%66,8** | **2,7 s** |
| 4002 | %20,4 | **%76,0** | %3,4 | %0,2 | 23,1 s |
| 4021 | %24,8 | %31,9 | %26,6 | %16,6 | 15,1 s |
| 4041 | %42,0 | %32,9 | %0,0 | %25,1 | 13,9 s |
| 4061 | %0,7 | %29,0 | %25,2 | %45,1 | 15,3 s |
| 4081 | %3,4 | **%64,9** | %1,6 | %30,1 | 17,2 s |
| 4102 | %0,0 | %0,0 | **%49,0** | %51,0 | 16,0 s |
| 4121 | %9,5 | %0,0 | **%84,2** | %6,3 | 24,4 s |

**Sebep tek değil, ve benim de keşif turunun da favorisi değildi.** "Amaç uyuşmazlığı" — yani
`step_avoiding`'in hedefe göre seçtiği tek kolun arabaya göre yanlış olması — yalnız 4041'de baskın.
Başarısız iki rotanın sebepleri birbirinden farklı: **4002 ve 4081'de kara liste** (%76 ve %65),
**4121 ve 4102'de geldiği kol** (%84 ve %49). Ve 4001, hiçbir şeyin bozuk olmadığı rota, tek düşük
donma süresine sahip olan: **2,7 s'ye karşı diğerlerinde 13,9-24,4 s.**

**Üçüncü aday da çürüdü.** `NFS_ADVANCE=free` — işaretçinin yürüyüşü kara listeyi yok sayar,
`came_from` durur; sürüşün kendisi ve kaçış listeyi görmeye devam eder.

| | waypoint | kursta süre | hiç bırakmayan | ilerlemesi duran | kavşak | ayrık düğüm | oran |
|---|---|---|---|---|---|---|---|
| **kalan** | **1.451** | **%74,3** | **14** | **30** | 1.665 | 1.588 | **1,05** |
| `free` | 1.369 | %68,8 | 12 | 40 | **2.473** | 1.805 | **1,37** |

Kavşak sayısının 808 artması ve oranın 1,05'ten 1,37'ye çıkması bu deponun salınım imzası:
listeyi kaldırınca işaretçi vazgeçilmiş düğümle normal düğüm arasında gidip geliyor.

**Kol seçimi kapısı üç ölçülmüş varyantla kapandı:** bütün kollar (`arms`) gecikmeyi kapatır ama
paralel şeride atlar; hat-üstü kollar (`line`) ne kapatır ne kazandırır; listesiz (`free`) salınıma
sokar. Üçü de alanı kaybediyor ve üçünün de ilerleme sütunları kötüleşiyor.

**Geriye kalan ve bu oturumda hiç dokunulmayan şey yukarı akışta:** hedef waypoint sayacı. İşaretçi
takıldığında hedef **ortalama 13,9-24,4 saniyedir** aynı — 4001'de 2,7. Sayacın donması bilinen bir
sorun (`pilot.rs`, "İlerletme kuralının iki kolu da ölü") ve bir çaresi (nearest'a resync) ölçülüp
çürütülmüş; ama donmanın **ne kadar sürdüğü** ilk kez burada bir sayı. Zincirin başı orası:
hedef donuyor → `toward` bayatlıyor → işaretçi geride kalıyor → nişan yana kayıyor → tam kilit.

### DÜZELTME + başlatıcı sebep: donmuş hedef zincirin başı değil, sonucu (2026-08-21)

Bir önceki kayıt zinciri "hedef donuyor → `toward` bayatlıyor → işaretçi geride kalıyor" diye
sıralamıştı. **Sıra ölçüldü ve üç rotada tersi çıktı.**

**Önce: hedef neden donuyor.** Donmuş adımların %71-99,9'unda hedef bırakma yarıçapının (60 m)
*içinde* ve araba onu geçmemiş — yani kural doğru olanı yapıyor. Ve o adımlarda araba çoğu zaman
**duruyor**:

| rota | donmuş adım | hedefe / yanal | ortalama hız | araba duruyor |
|---|---|---|---|---|
| 4001 | 6.063 | 33 m / 17 m | 22 km/h | %17,4 |
| 4002 | 128.871 | 58 m / 20 m | **7 km/h** | **%63,2** |
| 4081 | 76.262 | 44 m / 33 m | **3 km/h** | **%64,8** |
| 4102 | 98.811 | **90 m / 48 m** | 22 km/h | %40,2 |

Yani donmuş hedef, duran arabanın sonucu. (4102 ayrı bir durum: araba hareket hâlinde ama hedef
90 m ötede ve 48 m yanda — halka başka bir yolun üstünde.)

**Sonra: sıra.** Her arabanın işaretçisinin **ilk kez** 30 m geride kaldığı an kaydedildi:

| rota | o an hız | hareket hâlinde | hedef o an kaç sn'dir donmuş | **o an sebep** |
|---|---|---|---|---|
| 4001 | 57 km/h | %75 | 1,7 s | hiçbiri 6 · geldiği 2 |
| **4002** | **−0 km/h** | **%0** | **14,7 s** | uygun 4 · kara liste 4 |
| 4021 | 82 km/h | %100 | 3,6 s | **uygun 8** |
| 4041 | 81 km/h | %100 | 1,1 s | **uygun 8** |
| 4061 | 32 km/h | %100 | 4,1 s | hiçbiri 8 |
| 4081 | 53 km/h | %100 | 1,2 s | **uygun 8** |
| 4102 | 42 km/h | %100 | 7,1 s | hiçbiri 6 · geldiği 2 |
| 4121 | 47 km/h | %100 | 1,5 s | **uygun 8** |

**Düzeltme:** 4002 dışında hiçbir rotada hedef, işaretçiden önce donmuyor — arabalar 47-82 km/h ile
giderken ve hedef **1,1-4,1 saniyelik tazeyken** işaretçi geride kalıyor. Zincirin başı hedef değil.
4002 tersi: araba işaretçi geride kalmadan önce zaten durmuş, hedef 14,7 saniyedir donuk. O rota bir
seyir sorunu değil.

**Ve başlatıcı sebep, bir önceki kaydın sebebi değil.** Dört rotada (4021, 4041, 4081, 4121) **sekiz
arabanın sekizinde** o an arabaya daha yakın **uygun** bir kol var — yani kapı açılabilirdi ve
`step_avoiding`'in hedefe göre seçtiği tek kol o değildi. Amaç uyuşmazlığı. Üç rotada (4001, 4061,
4102) sebep "hiçbir kol daha yakın değil": işaretçi yerel bir minimumda, araba grafın izlemediği bir
yere gidiyor.

Kara liste onset'te yalnız 4002'de görünüyor — yani bir önceki kaydın "kara liste %76" tablosu
**sonrasını** ölçüyor, sebebi değil: araba kaybolduktan sonra liste doluyor ve steady-state'i o
yönetiyor. İkisi farklı sorular ve ben ilkini ikincisiyle cevaplamıştım.

**Sıradaki iş, ve neden bugünkü çürütmeler onu kapatmıyor.** Amaç uyuşmazlığını düzelten kural
`NFS_ADVANCE=arms` ve o alanı kaybetti — ama *her* adımda ateşleniyor, yani araba çoktan kaybolduktan
sonra da, ki orada en yakın kol düzenli olarak paralel şerit. Aranan şey aynı düzeltmenin yalnız
**onset'te** — araba hâlâ kursun üstündeyken — geçerli olan biçimi. `line` bunu `mark_line` ile
denedi ve kaybetti çünkü en kötü yerlerde tutulan düğüm zaten hattın dışında; koridorun kendisi
(`Corridor`) pilota hiç verilmiyor, ve "araba koridordayken" bu ayrımın doğru tarafı olabilir.

### İşaretçi ailesi tükendi: dört varyant, dördü de aynı yerde (2026-08-21)

Bir önceki kayıt, `arms`'ın neden kaybettiğini "araba kaybolduktan sonra da ateşlendiği için"
diye açıklamış ve düzeltmesini adlandırmıştı: aynı geri düşme, ama yalnız araba hâlâ kursun
üstündeyken. Yazıldı (`NFS_ADVANCE=near`, eşik en yakın **waypoint**'e mesafe — koridor değil,
çünkü koridor yandaki şeride sıfır der) ve iki genişlikte süpürüldü.

| | waypoint | kursta süre | hiç bırakmayan | ilerlemesi duran | kursta kalanın araba başına ilerlemesi |
|---|---|---|---|---|---|
| **kalan** | **1.451** | **%74,3** | **14** | **30** | **18,7** |
| `arms` (kapısız) | 1.359 | %68,9 | 11 | — | — |
| `line` (hat kısıtlı) | 1.368 | %66,6 | 11 | 44 | 16,8 |
| `free` (listesiz) | 1.369 | %68,8 | 12 | 40 | 17,2 |
| `near` 40 m | 1.369 | %67,9 | 11 | 42 | 17,0 |
| `near` 25 m | 1.371 | %66,8 | 10 | 41 | 14,3 |

**Dördü de aynı yere düşüyor, ve bu tek başına bir bulgu.** Kapıyı kursun üstüne kısıtlamak `arms`'ı
neredeyse hiç değiştirmiyor (1.359 → 1.369) — yani zarar araba kaybolduktan *sonra* verilmiyor,
araba hâlâ kursun üstündeyken veriliyor. Bir önceki kaydın gerekçesi yanlıştı.

**Ailenin tamamı kapandı.** İşaretçinin gecikmesi bir kol seçme kuralıyla kapanmıyor: bütün kollar,
hat-üstü kollar, listesiz kollar, ve kurs-üstü kapılı kollar — dördü de alanı ve ilerleme
sütunlarını kaybediyor. Onset ölçümü (dört rotada sekiz arabanın sekizinde arabaya daha yakın uygun
bir kol var) gerçek ve duruyor; ona **işaretçi tarafından** müdahale etmek çürüdü.

Geriye tek yön kalıyor ve o da kapalı: amaca müdahale etmek, yani `step_avoiding`'in `min_by`'ını
araba mesafesiyle harmanlamak. Deponun kendi kaydı onu açıkça yasaklıyor — *"Whatever fixes it
starts by asking where node 110's on-line arms go and what is there, not by weighting this `min_by`
again."* Ve bugün o soru soruldu: 111'in ileri giden tek kolu hattı terk ediyor, hat-üstü alternatifi
118 m geriye gidiyor. Yani ağırlıklandırmanın seçeceği daha iyi bir kol orada **yok**.

**Bunun anlamı:** işaretçinin gecikmesi bir pilot kusuru değil, **kursun tarifi ile grafın
uyuşmazlığı**. Aynı sonuca bugün üç ayrı yoldan varıldı — halkanın 228 m'lik deliği, 111'in kolları,
ve şimdi dört kol kuralının hep birlikte düşmesi. Bir sonraki oturumun sürücüde arayacak bir şeyi
kalmadı; aranacak yer rota dosyasının kendisi.
