# Sekiz-rota baseline — pin yükseltmesinin iki yanı

Bu dosya bir ölçüm kaydı, bir plan değil. **İki tablo var:** yükseltmeden önceki (aşağıda) ve
2026-08-20'de yükseltmeden sonra alınan (dosyanın sonunda, karşılaştırmasıyla). Sekiz-rota
süpürmesi `nfsu2-gizmo`'da her sürücü/dünya değişikliğini yargılayan ölçüm; hemen aşağıdaki
sayılar **motor pini taşınmadan önceki** hâli.

Neden yazıldı: motorun kendi determinizm kapısı (`headless_stress_test`, hash
`A462C9EB8A09D5CA`) **kendi 200 kutuluk yıkım sahnesini** kilitliyor, bu şehri değil. Motor
yükseltmesinin sürüşü kaydırıp kaydırmadığını yalnız bu tablo söyleyebilir. Yükseltmeden sonra
alınan bir ölçümün karşılaştıracak bir şeyi kalmaz — o yüzden önce alındı.

## Koşum

| | |
|---|---|
| tarih | 2026-08-20 |
| motor pini | `09c948a9848d481fc9a57b17ea7fd5bf519e4841` (ağacı `3433aefe` ile bayt-birebir aynı) |
| oyun commit'i | `ff0b6a9` (`roadmap`) |
| binary | `target/release/nfs_sim`, 02:10'da yeniden derlendi — bayat değil, derleme çıktısı doğrulandı |
| araç | 240SX, rota başına 8 araba |
| süre | rota başına 90 simüle saniye |
| bölge | `ROUTESL4RA` |

```sh
export NFSU2_ROOT="…/Need for Speed Underground 2"
cargo build --release --bin nfs_sim      # önce derle, çıktıyı DOĞRULA
for r in 4001 4002 4021 4041 4061 4081 4102 4121; do
  NFS_ROUTE="$NFSU2_ROOT/TRACKS/ROUTESL4RA/Paths$r.bin" NFS_SECONDS=90 \
    target/release/nfs_sim "$NFSU2_ROOT/TRACKS" "$NFSU2_ROOT/CARS/240SX/GEOMETRY.BIN"
done
```

## Sayılar

| rota | away | fallen | furthest | junctions | held | waypoint | distinct nodes |
|---|---|---|---|---|---|---|---|
| 4001 | 8 | 0 | 891 | 230 | 102 | 25 | 222 |
| 4002 | 8 | 0 | 208 | 47 | 419 | 93 | 38 |
| 4021 | 8 | 1 | 609 | 181 | 45 | 44 | 174 |
| 4041 | 8 | 1 | 966 | 129 | 46 | 28 | 119 |
| 4061 | 8 | 0 | 890 | 188 | 91 | 73 | 171 |
| 4081 | 8 | 0 | 578 | 198 | 55 | 125 | 186 |
| 4102 | 8 | 0 | 468 | 232 | 8 | 16 | 202 |
| 4121 | 8 | 0 | 740 | 247 | 56 | 31 | 242 |
| **TOPLAM** | **64** | **2** | **5350** | **1452** | **822** | **435** | **1354** |

Her rotada `edge`, `through` ve `nowhere` sıfır; `fallen` yalnız 4021 ve 4041'de birer tane ve
ikisi de `edge=1` ile eşleşiyor.

## Nasıl karşılaştırılır

Ritüelin kıyasladığı sayılar: **`away`, `fallen`, rotalar toplamı `furthest`, `junctions`**, ve
gerçek ilerleme ile daire çizmeyi ayıran **distinct nodes**. Tek rotalık sonuç kanıt değildir —
bir rotada kazanıp üçünde kaybeden bir değişiklik silinir.

Yükseltmeden sonra aynı komutu koştur, tabloyu yan yana koy ve şuna bak:

- **`fallen` artıyorsa** temas/sürtünme tarafı kaymıştır — `pin..main`'de statik/dinamik
  (stick-slip) sürtünme modeli geldi, ilk şüpheli orası.
- **`away` 64'ün altına düşüyorsa** kavşak seçimi bozulmuştur; sürüş girdisi ya da ağ tarafı.
- **`furthest` toplamı belirgin düşüyorsa** araba ya yavaşlamış ya takılıyor. Araç fiziği bu
  aralıkta **hiç değişmedi** (`crates/gizmo-physics-dynamics/src/vehicle/` diff'inde yorum dışı tek
  kod satırı yok — ölçüldü), o yüzden ilk bakılacak yer araç modeli değil, temas sürtünmesi ve
  wgpu 30 geçişi.
- **distinct nodes düşerken junctions sabit kalıyorsa** arabalar aynı kavşakları tekrar tekrar
  geçiyordur — daire çiziyorlar.

Kare süresi bu tabloda yok: `nfs_sim` başsız koşuyor. Render tarafının maliyeti ayrıca
`nfs_cruise`/`nfs_city` ile ölçülmeli.

## Yükseltmeden sonra — 2026-08-20, pin `58dc2623`

Aynı komut, aynı araç, aynı süre; tek değişen motor. Oyun commit'i `2b58c74`, binary yeniden
derlendi ve çıktısı doğrulandı.

| rota | away | fallen | furthest | junctions | held | waypoint | distinct nodes |
|---|---|---|---|---|---|---|---|
| 4001 | 8 | 0 | 995 | 236 | 105 | 39 | 229 |
| 4002 | 8 | 0 | 215 | 32 | 350 | 93 | 28 |
| 4021 | 8 | 1 | 618 | 200 | 19 | 44 | 193 |
| 4041 | 8 | 2 | 938 | 133 | 111 | 28 | 123 |
| 4061 | 8 | 0 | 708 | 168 | 3 | 67 | 154 |
| 4081 | 8 | 0 | 578 | 198 | 55 | 125 | 186 |
| 4102 | 8 | 0 | 413 | 248 | 2 | 25 | 217 |
| 4121 | 8 | 0 | 705 | 230 | 27 | 29 | 222 |
| **TOPLAM** | **64** | **3** | **5170** | **1445** | **672** | **450** | **1352** |

### Okuma: sürüş kaymadı

| ölçü | önce | sonra | fark |
|---|---|---|---|
| away | 64 | 64 | **0** |
| distinct nodes | 1.354 | 1.352 | −0,1 % |
| waypoint | 435 | 450 | **+3,4 %** |
| junctions | 1.452 | 1.445 | −0,5 % |
| furthest | 5.350 | 5.170 | −3,4 % |
| fallen | 2 | 3 | +1 araba |
| held | 822 | 672 | −18 % |

Dosyanın kendi kriterleri sırayla: **`away` 64'ün altına düşmedi**, yani kavşak seçimi
bozulmadı. **Distinct nodes sabit** (1.354 → 1.352) ve **waypoint arttı**, yani gerçek ilerleme
aynı ya da biraz iyi — daire çizmeye kayma yok. `furthest`'in %3,4 düşmesi tek başına
"yavaşladı" demek değil: aynı ilerleme için daha az metre, çırpınmanın azalması gibi de okunur, ve
düşüşün yarısı tek rotadan geliyor (4061, −182; buna karşılık 4001 +104).

**Tek gerçek işaret `fallen` 2 → 3** — 4041'de bir araba daha düştü. Dosyanın uyardığı yer burası
(`pin..main`'de statik/dinamik sürtünme modeli geldi). 64 arabada bir araba, tek başına bir karar
verdirmez; sonraki süpürmelerde 4041 izlenmeli, üç olur ve orada kalırsa temas tarafına bakılır.

**Karar: yükseltme sürüşü bozmadı, geri alınacak bir şey yok.** Bu tablo bundan sonraki
karşılaştırmaların tabanıdır; yukarıdaki yükseltme öncesi tablo tarihsel.

**Not, aynı gün:** bu tablo alındıktan sonra pilota `GRIP` freni eklendi (`ROADMAP.md`,
2026-08-20) ve alanı 883 → 927 waypoint, 1.352 → 1.280 ayrık düğüm, kursta kalan araba 9 → 14
taşıdı. Yani buradaki sayılar **motorun** yükseltmesini yargılar; sürücü tarafındaki bir sonraki
değişiklik `GRIP` açık hâliyle kıyaslanmalı.

## Üçüncü tablo — 2026-08-20 akşamı, günün bütün değişikliklerinden sonra

Aynı ölçüm, aynı sekiz rota, aynı 8 araba ve 90 saniye. Aradaki fark **yalnız** oyun tarafı; motor
pini değişmedi.

| | |
|---|---|
| oyun commit'i | `390c322` (`roadmap`) |
| halka | **koridora çekilmiş** (`PULL_TO = 0`) — kirişin koridor dışı waypoint'leri içeri taşınıyor |
| pilot | `LOOKAHEAD_PER_SPEED = 1.8` (0,9'du), `GRIP = 8`, `PASSED_NEAR = 60`, `BEHIND = 170°`, `ESCAPE_FULL = 2` |
| çit | kot değiştiren yolda ateşlemiyor |
| gürültü tabanı | ±37 waypoint (%2,8) — ölçülmüş, aynı gün |

| rota | away | düşen | furthest | junctions | held | geçilen waypoint | kursta süre | ayrık düğüm |
|---|---|---|---|---|---|---|---|---|
| 4001 | 8 | 0 | 1.136 | 318 | 2 | 360 | %92,8 | 318 |
| 4002 | 8 | 0 | 415 | 82 | 153 | 74 | %90,9 | 73 |
| 4021 | 8 | 0 | 609 | 235 | 10 | 186 | %94,2 | 225 |
| 4041 | 8 | 1 | 1.078 | 261 | 0 | 240 | %90,8 | 261 |
| 4061 | 8 | 0 | 1.103 | 169 | 2 | 181 | %64,7 | 161 |
| 4081 | 8 | 0 | 583 | 221 | 0 | 156 | %53,6 | 209 |
| 4102 | 8 | 0 | 312 | 126 | 9 | 62 | %68,4 | 96 |
| 4121 | 8 | 0 | 787 | 178 | 10 | 158 | %66,2 | 170 |
| **toplam** | **64 / 64** | **1** | **6.023 m** | **1.590** | **186** | **1.417** | **%77,7** | |

**Günün başına göre:** geçilen waypoint 1.035 → **1.417**, düşen 6 → **1**, `furthest` 5.305 →
**6.023 m**, çitin müdahalesi 1.121 → **186**, ve sekiz rotanın sekizinde **bütün arabalar kavşak
alıyor**.

**Yeni sütun — kursta geçen süre.** Gün boyu başlık sayısı "kursu hiç bırakmayan araba"ydı; o gün
içinde çürütüldü: kursu bırakan 34 arabanın 33'ü koridora geri dönüyor, kimi yarışın yarısından
fazlasında. Karşılaştırmalar artık bu sütunla yapılmalı.

**Ve okuma uyarısı:** "geçilen waypoint" iki farklı halka arasında karşılaştırılamaz — halka
değişirse (yürünmüş, kırpılmış) sayının ölçeği değişir. Çekilmiş halka waypoint'leri *taşıdığı*
için sayıları kirişle karşılaştırılabilir, ama o karşılaştırma da halkayı arabaların sürdüğü yere
yaklaştırdığı için hafif yanlıdır; bağımsız sütunlar `furthest`, `düşen` ve `kursta süre`.

## Dördüncü tablo — 2026-08-21, rakip doğrultma ağından sonra

Üçüncü tablodan beri iki şey değişti: rakipler artık oyuncunun `keep_in_world` ağını alıyor, ve
motor pini `550a7df`'e taşındı (süpürme o taşımada bayt-birebir aynı çıktı). Halka tarafında
denenip **geri alınan** üç şey var — tam yeniden sıklaştırma, boşluk doldurma, yumuşatma — üçü de
kayıtta.

| | |
|---|---|
| oyun commit'i | `1a5d664` (`roadmap`) |
| motor pini | `550a7dfd` — *"renderer: an alpha cutoff the baked-lit path can reach"* |
| halka | koridora çekilmiş (`PULL_TO = 0`), boşluk doldurma **kapalı** (`GAP_AT` çürütüldü) |
| pilot | `LOOKAHEAD_PER_SPEED = 1.8` · `GRIP = 8` · `PASSED_NEAR = 60` (**atıl**) · `BEHIND = 170°` (**atıl**) · `ESCAPE_FULL = 2` |
| ağ | rakipler oyuncunun `keep_in_world`'ünü alıyor |

| rota | away | düşen | furthest | junctions | held | geçilen waypoint | kursta süre | yan yatarak |
|---|---|---|---|---|---|---|---|---|
| 4001 | 8 | 0 | 1.136 | 318 | 2 | 360 | %92,8 | %0,0 |
| 4002 | 8 | 0 | 342 | 77 | 268 | 81 | %89,4 | %1,3 |
| 4021 | 8 | 0 | 609 | 235 | 10 | 186 | %94,2 | %0,0 |
| 4041 | 8 | 0 | 1.078 | 260 | 0 | 240 | %91,7 | %0,5 |
| 4061 | 8 | 0 | 1.103 | 177 | 1 | 196 | %62,0 | %0,5 |
| 4081 | 8 | 0 | 583 | 219 | 0 | 156 | %51,0 | %2,6 |
| 4102 | 8 | 0 | 313 | 177 | 48 | 99 | %43,4 | %1,3 |
| 4121 | 8 | 0 | 787 | 178 | 10 | 158 | %66,2 | %0,6 |
| **toplam** | **64 / 64** | **0** | **5.951 m** | **1.641** | **339** | **1.476** | **%73,8** | **%0,8** |

**İki gün öncesine göre:** geçilen waypoint 1.035 → **1.476**, düşen 6 → **0**, `furthest`
5.305 → **5.951 m**, yan yatarak geçen süre %7,0 → **%0,8**, ve sekiz rotanın sekizinde bütün
arabalar kavşak alıyor.

**Okuma uyarısı — 2026-08-21'de bir yanlış kabule mal oldu.** *Kursta geçen süre* ve *hattı
bırakma* sütunlarının ikisi de **duran arabayı başarı sayar**: yolda durmuş bir araba koridorun
içindedir ve hattı hiç bırakmaz. Arabaları durdurabilecek bir değişiklik bu iki sütunla
yargılanamaz; **kavşak · ayrık düğüm · ilerlemesi duran araba** ile birlikte bakılmalı. Bu tabloda
ilerlemesi duran araba **5 / 64**, duranların ortalaması yarışın **%21**'i.
