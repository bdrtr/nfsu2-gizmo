# Sekiz-rota baseline — pin yükseltmesinden önce

Bu dosya bir ölçüm kaydı, bir plan değil. Sekiz-rota süpürmesi `nfsu2-gizmo`'da her sürücü/dünya
değişikliğini yargılayan ölçüm; burada yazan sayılar **motor pini taşınmadan önceki** hâli.

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
