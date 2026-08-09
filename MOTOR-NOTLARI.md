# Motor notları — sabitlenmiş Gizmo sürümü

Oyun **donmuş bir motor sürümüne** karşı geliştiriliyor. `Gizmo` kendi yolunda ilerlemeye devam
ediyor; oyun onu takip etmiyor. Bu dosya iki şeyi tutar: **pinin ne olduğunu**, ve pinlenmiş
motorda **eksik ya da yanlış bulduğumuz her şeyi** — böylece pini yükselttiğimiz gün elimizde
tahmin değil bir liste olur.

## Pin

| | |
|---|---|
| paket | `gizmo-engine` (kütüphane adı `gizmo`), sürüm `0.9.0` |
| kaynak | `https://github.com/bdrtr/Gizmo` |
| commit | `4d1a8cb7dab9df9e97b9e4c08255cbd56cef568f` — `main`, *"physics: the joint solver stops writing into sleeping mechanisms"* |
| sabitlendi | 2026-08-09 |
| nerede yazılı | `game/Cargo.toml` → `rev = ...`; `Cargo.lock` aynı commit'i ayrıca kaydeder |

Bu commit'te oyunun bugün dayandığı her şey var — doğrulandı, varsayılmadı:
`Collider::trimesh` ve `TriMeshShape::local_aabb` (önbelleklenmiş trimesh AABB,
`gizmo-physics-core/src/components/collider.rs:648`), `MaterialType::BakedLit`,
`VehicleTuning::torque_curve`. `trimesh-aabb` ve `shadow-gate` dallarının ikisi de `main`'e
girmiş durumda, yani ROADMAP'in "push edilmedi" notu artık geçerli değil.

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
`git -C ../Gizmo log --oneline 4d1a8cb..main`.

## Pinin neyi dondurduğu — ve neyi dondurmadığı

Üç sınır, üçü de sonradan sürpriz olmasın diye yazılı.

**Motorun kaynağı donmuştur, motorun bağımlılıkları donmamıştır.** Git bağımlılığı kendi
`Cargo.lock`'unu getirmez; motorun caret aralıklarını bu repo çözer. Yani `cargo update` bir gün
donmuş motorun altındaki `wgpu`/`naga`'yı oynatabilir ve **hiçbir motor commit'inin açıklamadığı**
bir render farkı üretebilir. Bugün oyun `wgpu`/`naga` 29.0.4 çözüyor, Gizmo'nun kendi kilidi
29.0.3'te. Renderer'da açıklanamayan bir değişiklik görürsen ilk bakılacak yer motor değil,
`Cargo.lock` diff'idir.

**Pini yalnız `origin/main` tutuyor.** `4d1a8cb`'yi işaret eden bir etiket yok. `main` force-push
ile bu commit'in gerisine alınırsa GitHub nesneyi eninde sonunda toplar ve bağımlılık, deposu
`~/.cargo/git`'te sıcak olmayan her makinede çözülemez olur — bu makine fark etmez, kırılma önce
temiz bir checkout'ta ya da CI'da görünür. Kalıcı çözüm `Gizmo`'da `4d1a8cb`'ye bir etiket atıp
push etmek (ör. `nfsu2-pin-2026-08-09`); bu, motor deposuna yazmak demek olduğu için bilerek
yapılmadı.

**Kök `[patch]` donmuş motorun içine de uzanır.** Bir `[patch]` git bağımlılığının *geçişli*
bağımlılıklarını da yeniden yazar. Bugün zararsız — Gizmo `gizmo-nfs`'e bağlı değil — ama motor bir
gün ona bağlanırsa, pin duruyor olmasına rağmen parser sessizce yerel `../PryHUB`'dan gelir. Freeze'in
tek gerçek deliği bu.

## Eksikler ve düzeltmeler

Geliştirirken pinlenmiş motorda ne bulduysak buraya. Kural: madde **motor-genel** dille yazılır.
NFSU2 kelime dağarcığı (chunk id, dosya adı, "Bayview") bir motor maddesine giremez — bkz.
ROADMAP §6. Bir eksiği oyun tarafında geçici çözümle atlattıysak, o çözüm de yazılır: pin
yükselince geri alınacak olan şey odur.

Durumlar: `açık` · `motorda düzeldi` (commit ile) · `pin yükselince doğrula` · `kapandı`

| # | tarih | eksik / hata | nerede çarptık | oyundaki geçici çözüm | motor tarafı | durum |
|---|---|---|---|---|---|---|
| 1 | 2026-08-04 | `update_vehicle` için broadphase destekli sorgu tutamacı yok: tekerlek başına tüm collider listesi doğrusal taranıyor, `gather_colliders` her çağrıda hepsini klonluyor. Şehirde adım başına ~14.000 collider kopyası | ROADMAP "Sıradaki adım" §3 — şehir + araba binary'sinin kare hızını bunun belirlemesi bekleniyor | yok (henüz ölçülmedi) | `gizmo-physics-dynamics` × `-rigid` | açık |
| 2 | 2026-08-04 | Motorda hücre/bölge (`Cell`/`Region`) kavramı ve uzamsal indeks yok; `Frustum::test_aabb_masked` pinlenmiş commit'te hâlâ **sıfır çağıranlı** (yalnız kendi testleri çağırıyor) | 8.119 mesh her kare gönderiliyor, culling yok | culling oyun tarafında yok; `world/` kendi hücrelerini kuruyor ama render onları kullanmıyor | yeni `gizmo-world` ya da `gizmo-scene` + `gizmo-renderer` | açık |
| 3 | 2026-08-09 | `BakedLit` **çıplak bir çarpım zinciri**: `vertexRengi × instanceAlbedo × doku`, üstüne gölge terimi. Kazanç yok, emissive yok, ambient taban yok, sis yok — karanlık içeriği kaldıracak hiçbir kolu yok. Üstüne ACES toe'su (`post_process.wgsl:108`, `x→0`'da `aces(x) ≈ 0,214·x`) karanlıkta 4,67× kısıyor ve `exposure = 1,15` (`post_process.rs:151`) bunu telafi etmiyor. **Dikkat:** orta tonlarda boru hattı düz gamma modulate'e neredeyse eşit (128×128 → 62,8'e karşı 64,3), yani sorun tonemap'in yanlışlığı değil, **ayarlanabilir olmaması** | Bayview: pencerenin medyanı **1/255**, ortalama 8,5/255. Yol ekranda 2–14/255. Uçtan uca: vcol 128 × doku 128 → 63; 96×96 → 21; **64×64 → 2,6**; vcol 128 × doku 255 → 164 (parlayan pencereler) | yok | `gizmo-renderer` | açık |
| 4 | 2026-08-09 | `SHADOW_DISTANCE = 100` m ve son cascade'in ötesinde gölge fonksiyonu `1.0` (tam aydınlık) döndürüyor. Gölge terimi 0,55'te tabanlandığı için bu, 100 m'de **sert 1,82× parlaklık basamağı** demek — mesafeye göre yumuşayan bir geçiş yok | Bayview'da ufkun hemen altında ölçülen parlak şerit; takip kamerasından ~2,3° aşağı düşüyor | yok | `gizmo-renderer` | açık |
| 5 | 2026-08-09 | `BakedLit`, "vertex rengi yok" ile "vertex rengi siyah"ı ayırt edemiyor: `baked_lit.wgsl:143-146`, `length(baked) < 0.0001` ise rengi **beyaza** çeviriyor. Gerekçe doğru (rengi set etmeyen importer modeli karartmasın) ama proxy yanlış — niteliğin varlığı vertex layout'undan bilinmeli, piksel değerinden tahmin edilmemeli. **Latent:** doğuş noktası çevresinde yalnız bir ışık-konisi decal'inin 12 vertex'i sıfır, yani bugün görünür bir bozulma yapmıyor | Kod okumasıyla bulundu, gözle değil — ufuktaki parlak şeridin sebebi bu **değil** (o 4 numara) | yok | `gizmo-renderer` (`src/shaders/baked_lit.wgsl`) | açık |
| 6 | 2026-08-09 | Vertex **alfası** sessizce düşürülüyor, şehrin fren-izi ve decal katmanları yolun üstüne opak geometri olarak çiziliyor | Bayview yol yüzeyi | yok | `gizmo-renderer` | açık |

<!--
Yeni madde eklerken şablon — boş sütun bırakma, bilmiyorsan "ölçülmedi" yaz:
| n | YYYY-AA-GG | ne eksik/yanlış (motor-genel dille) | hangi iş sırasında çarptık | oyunda ne yaptık | crate | açık |
-->

## Pini yükseltme kontrol listesi

1. **Ne değişmiş, gör:** `git -C ../Gizmo log --oneline 4d1a8cb..main`
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
