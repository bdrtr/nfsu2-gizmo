//! Motor ve aktarma, `SPEED2.EXE`'nin kendi kurallarıyla — motorun kilitli debriyajının üstünde.
//!
//! Motorun `VehicleController`'ı bir aktarma organını en basit hâliyle kuruyor: debriyaj hep
//! kilitli, vites anında değişiyor, devir limitöre gelince yalnız **kırpılıyor** (tork kesilmiyor),
//! gaz bırakılınca motor tork vermeyi bırakıyor ama frenlemiyor, ve sürüklenme her arabada aynı
//! `C_d 0,32 · 2,2 m²`. Oyunun kodu bunların dördünü de başka yapıyor, ve hepsi kayıtta:
//!
//! - **Vites değişimi zaman alıyor** (`0x5A1400` durum makinesi, `0x5949B0` ilerleme hızı):
//!   debriyaj `3 × hız` ile açılıyor — bu sürede tekerleğe tork yok, motor gazı sıfır — sonra
//!   `1,5 × hız` ile kapanıyor, gaz %70. Hız vites kutusunun `+0x1C` şeridi
//!   ([`gizmo_nfs::Gearbox::shift_rate`]): stok 2 → 0,5 s, yükseltmeler 8'e kadar → 0,125 s.
//! - **Kesici** (`0x5AA760`): limitörün 25 devir altında yakıt kesiliyor, %98,75'in altına inene
//!   kadar kesik kalıyor. Bu, oyunda son viteste bir **vites sınırı** demek; motorda yoktu.
//! - **Motor freni** (`0x594300`): krank torku `T(rpm)·gaz − (1 − gaz)·Tfren(rpm)`, net eksi ise
//!   ×0,85, kesikken fren ×2. `Tfren` kaydın üç noktalı eğrisi ([`gizmo_nfs::Engine::brake_nm`]).
//! - **Sürüklenme** (`0x59E620`): `(2 − gaz) · k · |v|·v`, `k` kaydın `+0x370` şeridi
//!   ([`gizmo_nfs::Chassis::drag`]) — tam gazda kayıttaki değer, gaz kesilince iki katı.
//!
//! - **Nitro** (`0x594300`, depo `0x592480`/`0x5925B0`/`0x592640`): krankta
//!   `gaz · şekil(devir) · tork · kademe` kadar ek tork ([`Nitro`]).
//!
//! Hepsi motorun adımının **etrafında** uygulanıyor ([`super::vehicle_step`]): adımdan önce gaz
//! kısılıyor ve sürüklenme katsayısı yazılıyor, adımdan sonra sürücünün gazı geri konuyor ve motor
//! freni tahrik tekerleklerinin dönüşünden düşülüyor. Motor bunlardan hiçbirini görmüyor; `MOTOR-NOTLARI.md` #21
//! motordan istenen karşılıkları yazıyor.
//!
//! `NFS_POWERTRAIN=off` hepsini kapatıyor, `NFS_POWERTRAIN_SKIP=shift,cut,brake,drag,points` tek
//! tek. Açılınca pilot süpürmesi önce 1 601 → 1 351 geriledi — pilotun viraj gaz kesmesi süzülen bir
//! arabaya göre seçilmişti, motor freni yarım gazda bile frenliyor — ve `pilot::CORNER_LIFT`
//! 0,75 → 0,3 ile 1 587'ye döndü (ROADMAP "Motor ve aktarma").

use gizmo::core::component::Component;
use gizmo::physics::vehicle::VehicleController;
use gizmo::prelude::*;

/// Hava yoğunluğu; motorun aero kodundakiyle aynı sayı, çünkü sürüklenme oraya `C_d` olarak
/// yazılıyor ve motor onu bununla çarpıyor.
const AIR_DENSITY: f32 = 1.225;
/// Debriyaj açılırken ve kapanırken ilerlemenin hız çarpanı (`0x7FC298`: `[−3, 0, 1,5, 0]`).
const OPEN_PER_RATE: f32 = 3.0;
const CLOSE_PER_RATE: f32 = 1.5;
/// Debriyaj kapanırken motora verilen gaz payı (`0x7844D8`).
const ENGAGING_THROTTLE: f32 = 0.7;
/// Kesici: limitörün bu kadar altında keser (`0x7A18DC`), limitörün bu payının altında açar
/// (`1 − 0,0125`, `0x7A18E0`).
const CUT_BELOW_RPM: f32 = 25.0;
const RESUME_SHARE: f32 = 1.0 - 0.0125;
/// Net krank torku eksiyken ve debriyaj bağlıyken çarpan (`0x7A191C`); kesikken motor freni ×2
/// (`0x7A18D8`).
const OVERRUN_SHARE: f32 = 0.85;
const CUT_BRAKE: f32 = 2.0;
/// Otomatik kutunun yukarı geçişi: kırmızı çizginin 100 devir altı (`0x784258`; `0x5AC7D0`'daki
/// "en iyi geçiş noktası" araması `0x86B33C` bayrağına bağlı ve o bayrağa hiçbir yer yazmıyor),
/// üstüne `0..100` devirlik rastgele bir erken geçiş (`0x594710`). Ortalaması alınıyor.
const UPSHIFT_BELOW_RED: f32 = 150.0;
/// Aşağı geçiş (`0x5AA960`): bir alt vitesin geçiş noktası kırmızı çizginin 200 altı sayılıyor
/// (100 sabit, 100 de `+0xCC`), ve o devre bu vitesin oranıyla düşülen devrin 200 altında iniliyor.
const DOWNSHIFT_LOWER_BELOW_RED: f32 = 200.0;
const DOWNSHIFT_MARGIN: f32 = 200.0;
/// Motor rölantiye bu kadar yakınken debriyaj zaten ayrılmış sayılıyor (`0x594E90`: tekerlek tarafı
/// rölantinin altına inince ayırır) — motor freni burada bırakıyor ki duran araba geri itilmesin.
const CLUTCH_OPENS_ABOVE_IDLE: f32 = 50.0;
/// Nitro torkunun devirle şekli, rölantiden kırmızı çizgiye eşit aralıklı dokuz nokta
/// (`0x7FBF38`; motorun kurucusu `0x5AA5A0` bu tabloyu her arabaya aynı veriyor).
const NITRO_SHAPE: [f32; 9] = [0.3, 0.4, 0.45, 0.5, 0.55, 0.6, 0.7, 1.0, 0.7];
/// Deponun doluluğu bu eşikleri geçince nitro kademe atlıyor (`0x59FA30`, eşikler `0x7A1790`,
/// çarpanlar `0x7A1798`): dolu bir depo ×1, taşırılmış bir depo ×1,4 ya da ×1,7.
const NITRO_STAGES: [(f32, f32); 2] = [(1.7, 1.7), (1.4, 1.4)];
/// Depo dolunun iki katına kadar taşırılabiliyor (`0x592640`: 1'e kadar olağan, üstü `2 − doluluk`
/// sınırıyla).
const NITRO_OVERFILL: f32 = 2.0;
/// Bizim dolum kuralımız, oyununki değil. Oyun depoyu iki yoldan dolduruyor (`0x6016A0`): stil
/// puanı (1 500 puan bir depo) ve yarışta tur ilerlemesi (bir tur yarım depo). İkisi de burada yok;
/// yerine sürülen yol sayılıyor, 2 km'de yarım depo — kısa bir turun boyu.
const NITRO_REFILL_PER_M: f32 = 0.5 / 2000.0;

/// Kaydın aktarmayla ilgili kısmı, motorun tuning'inin dışında kalan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub idle_rpm: f32,
    pub red_line_rpm: f32,
    pub limiter_rpm: f32,
    /// Motor freni, N·m, rölantiden limitöre eşit aralıklı üç nokta.
    pub brake_nm: [f32; 3],
    /// Debriyajın saniyede katettiği yol ([`gizmo_nfs::Gearbox::shift_rate`]).
    pub shift_rate: f32,
    /// Tam gazdaki sürüklenme, N/(m/s)².
    pub drag_n: f32,
    /// Nitro deposu, saniye; 0 ise nitro takılı değil ([`gizmo_nfs::Chassis::nitro_full`]).
    pub nitro_tank_s: f32,
    /// Nitronun en yüksek ek torku, N·m, şekil ve kademe çarpanlarından önce.
    pub nitro_nm: f32,
}

/// Nitro, çalışırken.
///
/// Oyunda depo saniye cinsinden tutuluyor ve düğme basılı tutuldukça gerçek zamanla boşalıyor —
/// gazdan bağımsız; gaz yalnız torku ölçekliyor. Burada doluluk depo payı olarak tutuluyor:
/// 1 dolu, 2'ye kadar taşırılmış.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Nitro {
    /// Depo payı, `0..=2`.
    pub fill: f32,
    /// Sürücü düğmeye basıyor mu.
    pub request: bool,
    /// Bu adımda yanıyor mu.
    pub firing: bool,
}

/// Bir arabanın aktarması, çalışırken. Şasi varlığına bileşen olarak takılıyor.
#[derive(Clone, Debug)]
pub struct Powertrain {
    pub spec: Spec,
    /// Son görülen vites; değiştiyse bir geçiş başlıyor.
    gear: usize,
    /// Geçiş başladıysa üstünden geçen süre.
    shifting: Option<f32>,
    /// Kesici yakıtı kesti mi.
    pub cut: bool,
    /// Bu adımda motora verilen gaz, sürücünün istediğinden sonra.
    applied: f32,
    /// Sürücünün istediği gaz, adım boyunca saklanan.
    driver: f32,
    pub nitro: Nitro,
    /// Bir insan mı sürüyor; gazın yanal tutuşu yemesi ([`power_grip`]) yalnız onun arabasında.
    pub human: bool,
    /// Rüzgâr arkası, `0..=0,9` ([`draft`]): sürükleme `(1 − draft)` ile çarpılıyor (`0x599170`).
    /// Dışarıdan yazılıyor — öndeki arabaları bilen taraf.
    pub draft: f32,
    /// Tahrikli tekerlerin yanal tepe tutuşu, kurulduğu hâliyle — [`power_grip`] her adım bunun
    /// kesrini yazıyor. İlk adımda okunuyor.
    lat_base: Vec<f32>,
}

impl Component for Powertrain {}

impl Powertrain {
    #[must_use]
    pub fn new(spec: Spec) -> Self {
        let nitro = Nitro { fill: if spec.nitro_tank_s > 0.0 { 1.0 } else { 0.0 }, ..Nitro::default() };
        Self { spec, gear: 1, shifting: None, cut: false, applied: 0.0, driver: 0.0, nitro, human: false, draft: 0.0, lat_base: Vec::new() }
    }

    /// Geçişin şu anki evresi, varsa.
    #[must_use]
    pub fn shift_phase(&self) -> Option<f32> {
        self.shifting.and_then(|t| shift_throttle(t, self.spec.shift_rate))
    }
}

/// Bir geçişin `t` saniyesinde motora verilen gaz payı: debriyaj açılırken 0, kapanırken 0,7.
/// Geçiş bittiyse `None`.
#[must_use]
pub fn shift_throttle(t: f32, rate: f32) -> Option<f32> {
    if rate <= 0.0 {
        return None;
    }
    let open = 1.0 / (OPEN_PER_RATE * rate);
    let close = 1.0 / (CLOSE_PER_RATE * rate);
    if t < open {
        Some(0.0)
    } else if t < open + close {
        Some(ENGAGING_THROTTLE)
    } else {
        None
    }
}

/// Kesicinin yeni durumu: limitörün 25 devir altında keser, %98,75'in altında açar.
#[must_use]
pub fn limiter_cut(cut: bool, rpm: f32, limiter: f32) -> bool {
    if cut {
        rpm >= limiter * RESUME_SHARE
    } else {
        rpm >= limiter - CUT_BELOW_RPM
    }
}

/// Üç noktalı motor freni eğrisinden `rpm`'deki değer, uçlarda kırpılmış.
#[must_use]
pub fn brake_at(spec: &Spec, rpm: f32) -> f32 {
    let span = (spec.limiter_rpm - spec.idle_rpm).max(1.0);
    let x = ((rpm - spec.idle_rpm) / span * 2.0).clamp(0.0, 2.0);
    let i = (x as usize).min(1);
    let f = x - i as f32;
    spec.brake_nm[i] + (spec.brake_nm[i + 1] - spec.brake_nm[i]) * f
}

/// Oyunun net krank torku (`0x594300`), motorun zaten verdiği `drive = T(rpm)·gaz` bilinirken.
#[must_use]
pub fn net_crank(drive: f32, throttle: f32, engine_brake: f32, cut: bool) -> f32 {
    if cut {
        return -CUT_BRAKE * engine_brake;
    }
    let net = drive - (1.0 - throttle.clamp(0.0, 1.0)) * engine_brake;
    if net < 0.0 { net * OVERRUN_SHARE } else { net }
}

/// Rüzgâr arkasının tavanı (`0x7A55BC`).
pub const DRAFT_MAX: f32 = 0.9;
/// Öndeki arabanın hızı → arkasındaki rüzgâr boyu, metre (`0x8023D0`, `0x89CF30`): 1 m/s'de 0,
/// 5'te 10, 150'de 60, arası doğrusal.
const DRAFT_LENGTH: [(f32, f32); 3] = [(1.0, 0.0), (5.0, 10.0), (150.0, 60.0)];
/// `mesafe / boy` → rüzgâr payı, 0..1'de altı nokta (`0x8023F0`).
const DRAFT_FALLOFF: [f32; 6] = [1.0, 0.9, 0.75, 0.55, 0.3, 0.0];
/// Rüzgâr arkasında sayılmak için en uzak mesafe, m (`0x8023EC`) ve öndekinin burnuyla en çok açının
/// kosinüsü (`0x7A55B8`, ≈ 18°).
const DRAFT_REACH: f32 = 60.0;
const DRAFT_CONE: f32 = 0.95;

/// Bir arabanın rüzgâr arkası, öndeki arabalardan (`0x5EBD80`, `0x5EBC20`): her biri için — hızı
/// 1 m/s'nin üstündeyse, 60 m içindeyse ve bu araba onun burnunun arkasındaki ~18°'lik konideyse —
/// `boy = DRAFT_LENGTH(hız) · hizalanma`, pay `DRAFT_FALLOFF(mesafe / boy)`; toplam 0,9'da kırpılır.
/// `others`: her birinin konumu, burun yönü (birim) ve hızı, m/s.
#[must_use]
pub fn draft(me: Vec3, others: &[(Vec3, Vec3, f32)]) -> f32 {
    let mut total = 0.0;
    for &(at, nose, speed) in others {
        if speed < 1.0 {
            continue;
        }
        let rel = at - me;
        let d = rel.length();
        if d < 1e-5 || d > DRAFT_REACH {
            continue;
        }
        let align = (rel.dot(nose) / d - DRAFT_CONE) / (1.0 - DRAFT_CONE);
        if align < 0.0 {
            continue;
        }
        let length = interp_pairs(&DRAFT_LENGTH, speed) * align;
        if length < 1e-5 {
            continue;
        }
        let x = (d / length).clamp(0.0, 1.0) * (DRAFT_FALLOFF.len() - 1) as f32;
        let i = (x as usize).min(DRAFT_FALLOFF.len() - 2);
        total += DRAFT_FALLOFF[i] + (DRAFT_FALLOFF[i + 1] - DRAFT_FALLOFF[i]) * (x - i as f32);
        if total >= DRAFT_MAX {
            return DRAFT_MAX;
        }
    }
    total
}

/// `(x, y)` çiftleri arasında doğrusal, uçlarda kırpılmış.
fn interp_pairs(pairs: &[(f32, f32)], x: f32) -> f32 {
    if x <= pairs[0].0 {
        return pairs[0].1;
    }
    for w in pairs.windows(2) {
        if x <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    pairs[pairs.len() - 1].1
}

/// Motorun aero koduna yazılacak `C_d`: `½ρ·C_d·A = (2 − gaz)·k`.
#[must_use]
pub fn drag_coefficient(drag_n: f32, throttle: f32, area: f32) -> f32 {
    (2.0 - throttle.clamp(0.0, 1.0)) * drag_n * 2.0 / (AIR_DENSITY * area.max(1e-3))
}

/// `gear` vitesindeki aşağı geçiş devri (`0x5AA960`), `ratios` motorun `[geri, boş, ileri…]`
/// dizisi. Birinci vitesten aşağısı yok.
#[must_use]
pub fn downshift_rpm(ratios: &[f32], gear: usize, red_line: f32) -> Option<f32> {
    if gear < 3 {
        return None;
    }
    let (here, below) = (*ratios.get(gear)?, *ratios.get(gear - 1)?);
    if below.abs() < 1e-6 {
        return None;
    }
    Some((here / below).abs() * (red_line - DOWNSHIFT_LOWER_BELOW_RED) - DOWNSHIFT_MARGIN)
}

/// Nitronun `rpm`'deki şekil çarpanı ([`NITRO_SHAPE`]), uçlarda kırpılmış.
#[must_use]
pub fn nitro_shape(rpm: f32, idle: f32, red_line: f32) -> f32 {
    let last = (NITRO_SHAPE.len() - 1) as f32;
    let x = ((rpm - idle) / (red_line - idle).max(1.0) * last).clamp(0.0, last);
    let i = (x as usize).min(NITRO_SHAPE.len() - 2);
    let f = x - i as f32;
    NITRO_SHAPE[i] + (NITRO_SHAPE[i + 1] - NITRO_SHAPE[i]) * f
}

/// Deponun bu doluluğundaki kademe çarpanı; boş depoda 0.
#[must_use]
pub fn nitro_stage(fill: f32) -> f32 {
    if fill <= 0.0 {
        return 0.0;
    }
    NITRO_STAGES.iter().find(|&&(above, _)| fill > above).map_or(1.0, |&(_, k)| k)
}

/// Depoya `amount` depo payı ekle, taşma sınırına kadar.
#[must_use]
pub fn nitro_add(fill: f32, amount: f32) -> f32 {
    (fill + amount).clamp(0.0, NITRO_OVERFILL)
}

/// `NFS_POWERTRAIN_SKIP=shift,cut,brake,drag,points`: tek tek kapatmak, hangisinin ne yaptığı
/// ölçülebilsin diye.
fn skips(rule: &str) -> bool {
    static SKIP: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SKIP.get_or_init(|| std::env::var("NFS_POWERTRAIN_SKIP").unwrap_or_default()).split(',').any(|r| r == rule)
}

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("NFS_POWERTRAIN").is_ok_and(|v| v == "off"))
}

/// Motorun adımından önce: geçiş ve kesici gazı kısıyor, sürüklenme ve aşağı geçiş devri yazılıyor.
pub fn before_step(world: &mut World, dt: f32) {
    if !enabled() {
        return;
    }
    let ids: Vec<u32> = world.borrow::<Powertrain>().entities().collect();
    for id in ids {
        let Some(mut p) = world.borrow::<Powertrain>().get(id).cloned() else { continue };
        {
            let mut vehicles = world.borrow_mut::<VehicleController>();
            let Some(mut vc) = vehicles.get_mut(id) else { continue };
            prepare(&mut p, &mut vc, dt);
        }
        if let Some(mut slot) = world.borrow_mut::<Powertrain>().get_mut(id) {
            *slot = p;
        }
    }
}

/// [`before_step`] for one car.
fn prepare(p: &mut Powertrain, vc: &mut VehicleController, dt: f32) {
    if vc.current_gear != p.gear {
        // Yalnız ileri vitesler arası bir geçiş zaman alıyor; boştan kalkış ve geri vites değil.
        p.shifting = (p.gear >= 2 && vc.current_gear >= 2).then_some(0.0);
        p.gear = vc.current_gear;
    }
    if skips("shift") {
        p.shifting = None;
    }
    let mut share = 1.0;
    if let Some(t) = p.shifting {
        match shift_throttle(t, p.spec.shift_rate) {
            Some(s) => {
                share = s;
                p.shifting = Some(t + dt);
            }
            None => p.shifting = None,
        }
    }
    p.cut = !skips("cut") && limiter_cut(p.cut, vc.engine_rpm, p.spec.limiter_rpm);
    if p.cut {
        share = 0.0;
    }
    p.driver = vc.throttle_input;
    p.applied = vc.throttle_input * share;
    vc.throttle_input = p.applied;
    nitro(p, vc, dt);
    power_grip_wheels(p, vc);
    if p.spec.drag_n > 0.0 && !skips("drag") {
        let area = vc.tuning.aero.frontal_area;
        vc.tuning.aero.drag_coefficient =
            drag_coefficient(p.spec.drag_n, p.applied.abs(), area) * (1.0 - p.draft.clamp(0.0, DRAFT_MAX));
    }
    if skips("points") {
        return;
    }
    let red = p.spec.red_line_rpm;
    vc.tuning.upshift_rpm = red - UPSHIFT_BELOW_RED;
    if let Some(down) = downshift_rpm(&vc.tuning.gear_ratios, vc.current_gear, red) {
        vc.tuning.downshift_rpm = down;
    }
}

/// Gaz verirken çekiş kuvvetinin sürtünme çemberinde kapladığı pay, oyunun sayışıyla (`0x59D600`).
pub const POWER_COUNT: f32 = 1.5;
/// Yanal tutuşun gazla inebileceği en düşük kesir: motorun kayan lastik tabanı (0,35).
const POWER_FLOOR: f32 = 0.35;

/// Gaz verirken tahrikli tekerin yanal tutuşunun kesri (`0x59D600`).
///
/// Oyunun sürtünme çemberi `|(Fx, Fy)| ≤ μ·Fz`, ama gaz verilirken `Fx` çembere **1,5 katı**
/// sayılarak giriyor. Böylece sınırdaki tahrikli tekerde gaz, yanal kuvvete kalan payı
/// `√(1 − (1,5·r)²)`'ye indiriyor (`r = Fx / μFz`); motorun çemberi `√(1 − r²)` bırakıyor. Bu,
/// ikisinin oranı: arka itişli bir araba virajda gazla arkasını kaydırıyor (power oversteer).
#[must_use]
pub fn power_grip(share: f32) -> f32 {
    let r = share.abs();
    let game = 1.0 - (POWER_COUNT * r).powi(2);
    if game <= 0.0 {
        return POWER_FLOOR;
    }
    // Motorun çemberi `1,2 · d · Fz` (`pacejka_combined`): yanal tepe `d·Fz`'ye, çekiş onu
    // `√(1,44 − r²)` kesene kadar dokunmuyor.
    let engine = (1.44 - r * r).clamp(1e-3, 1.0);
    (game / engine).sqrt().clamp(POWER_FLOOR, 1.0)
}

/// [`power_grip`]'i insanın arabasının tahrikli tekerlerine yazar: çekiş kuvveti bir önceki adımın
/// tekerlek torkundan (`tork / r`, oyunun tutunma modundaki boyuna kuvveti), yük süspansiyondan.
/// Gaz yokken ya da teker havadayken tutuş kurulduğu değere döner.
///
/// **Varsayılan kapalı, `NFS_POWER=1` açıyor.** Tepe tutuşu kısmak motorun Pacejka eğrisinin
/// tamamını — sertliğini de — kısıyor; oyunun çemberi ise yalnız sınırı aşan bileşkeyi orantılı
/// kesiyor. Tezgâhta (`NFS_BENCH=power`) 240SX ve RX8 sabit virajda, tam gazdan önce döndü: 2. viteste
/// teker torku 950–1460 N·m, çemberin 0,7–1 katı. Doğrusu motorun işi (`MOTOR-NOTLARI.md` #24).
fn power_grip_wheels(p: &mut Powertrain, vc: &mut VehicleController) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("NFS_POWER").is_ok_and(|v| v == "1")) {
        return;
    }
    if p.lat_base.len() != vc.wheels.len() {
        p.lat_base = vc.wheels.iter().map(|w| w.pacejka_lat.d).collect();
    }
    let on = p.human && vc.throttle_input > 0.0 && !vc.reverse_input;
    let drivetrain = vc.drivetrain;
    for (w, &base) in vc.wheels.iter_mut().zip(&p.lat_base) {
        let f = if on && w.is_grounded && drivetrain.drives(&w.axle_type) && w.suspension_force > 1.0 {
            power_grip(w.drive_torque / w.radius.max(0.05) / (base * w.suspension_force))
        } else {
            1.0
        };
        w.pacejka_lat.d = base * f;
    }
}

/// Nitro: yanıyorsa motorun gazı, krankın torku `T(devir) + şekil · tork · kademe` olacak kadar
/// büyütülüyor — motorun torku `eğri · |gaz|` ve oyunun ek torku da gazla çarpılıyor, yani ikisinin
/// oranı gazın çarpanı. Yanmıyorsa depo sürülen yolla doluyor.
fn nitro(p: &mut Powertrain, vc: &mut VehicleController, dt: f32) {
    let tank = p.spec.nitro_tank_s;
    if tank <= 0.0 {
        return;
    }
    let n = &mut p.nitro;
    n.firing = n.request && n.fill > 0.0;
    if !n.firing {
        // İnsanın deposunu stil puanları dolduruyor (`CarRig::style_tick`, oyunun kuralı); mesafe
        // yer tutucusu yalnız puanlayıcısı olmayan arabalar için.
        if !p.human {
            n.fill = nitro_add(n.fill, vc.current_speed_kmh.abs() / 3.6 * dt * NITRO_REFILL_PER_M);
        }
        return;
    }
    let stage = nitro_stage(n.fill);
    n.fill = (n.fill - dt / tank).max(0.0);
    if p.applied <= 0.0 {
        return;
    }
    let extra = nitro_shape(vc.engine_rpm, p.spec.idle_rpm, p.spec.red_line_rpm) * p.spec.nitro_nm * stage;
    vc.throttle_input = 1.0;
    let curve = vc.engine_torque();
    vc.throttle_input = p.applied * (1.0 + extra / curve.max(1.0));
}

/// Motorun adımından sonra: motor freni tahrik tekerleklerine, sürücünün gazı geri.
pub fn after_step(world: &mut World, dt: f32) {
    if !enabled() {
        return;
    }
    let ids: Vec<u32> = world.borrow::<Powertrain>().entities().collect();
    for id in ids {
        let Some(p) = world.borrow::<Powertrain>().get(id).cloned() else { continue };
        let mut vehicles = world.borrow_mut::<VehicleController>();
        let Some(mut vc) = vehicles.get_mut(id) else { continue };
        engine_braking(&p, &mut vc, dt);
        vc.throttle_input = p.driver;
    }
}

/// Motor freni, **tahrik tekerleklerinin dönüşüne**: debriyaj bağlıysa ve motor rölantinin
/// üstündeyse, krankın fazladan eksi torku oranla çarpılıp tekerleklere paylaştırılıyor ve her
/// birinin `ω`'sı kendi etkin ataletiyle (`½·m·r²` ve üstüne yansıyan volan, `I·oran²` — motorun
/// kendi kontrolcüsünün kullandığı) yavaşlatılıyor. Araca lastik üzerinden geçiyor, oyundaki gibi
/// (`0x594CB0` net torku tekerleklere veriyor): yerdeki tekerlek kayınca aracı frenler, havadaki
/// tekerlek yalnız kendini yavaşlatır.
///
/// İlk sürüm bunu gövdenin hızından doğrudan düşüyordu, ve yokuştan düşen bir araba havada boşta
/// yüksek devire çıkmış tekerleklerin motor freniyle **havada** frenlenip saplanıyordu.
fn engine_braking(p: &Powertrain, vc: &mut VehicleController, dt: f32) {
    if p.shifting.is_some() || vc.current_gear == 1 || skips("brake") {
        return;
    }
    if vc.engine_rpm < p.spec.idle_rpm + CLUTCH_OPENS_ABOVE_IDLE {
        return;
    }
    let ratio = vc.tuning.gear_ratios.get(vc.current_gear).copied().unwrap_or(0.0)
        * vc.tuning.final_drive_ratio;
    let driven = vc.wheels.iter().filter(|w| vc.drivetrain.drives(&w.axle_type)).count();
    if ratio == 0.0 || driven == 0 {
        return;
    }
    let drive = vc.engine_torque();
    let net = net_crank(drive, p.applied.abs(), brake_at(&p.spec, vc.engine_rpm), p.cut);
    let extra = net - drive;
    if extra >= 0.0 {
        return;
    }
    let per_wheel = -extra * ratio.abs() / driven as f32;
    let reflected = vc.flywheel_inertia * ratio * ratio;
    let drivetrain = vc.drivetrain;
    for w in vc.wheels.iter_mut().filter(|w| drivetrain.drives(&w.axle_type)) {
        let inertia = 0.5 * w.wheel_mass * w.radius * w.radius + reflected;
        let dw = (per_wheel / inertia.max(1e-3) * dt).min(w.angular_velocity.abs());
        w.angular_velocity -= w.angular_velocity.signum() * dw;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec { idle_rpm: 800.0, red_line_rpm: 6500.0, limiter_rpm: 7000.0, brake_nm: [120.0, 160.0, 120.0], shift_rate: 2.0, drag_n: 0.4, nitro_tank_s: 6.0, nitro_nm: 500.0 }
    }

    #[test]
    fn a_stock_shift_is_half_a_second_and_a_third_of_it_has_no_drive() {
        assert_eq!(shift_throttle(0.0, 2.0), Some(0.0));
        assert_eq!(shift_throttle(0.16, 2.0), Some(0.0));
        assert_eq!(shift_throttle(0.17, 2.0), Some(ENGAGING_THROTTLE));
        assert_eq!(shift_throttle(0.49, 2.0), Some(ENGAGING_THROTTLE));
        assert_eq!(shift_throttle(0.51, 2.0), None);
        // A built box (rate 8) is four times as quick.
        assert_eq!(shift_throttle(0.13, 8.0), None);
    }

    #[test]
    fn the_limiter_cuts_short_of_the_limit_and_holds_the_cut() {
        assert!(!limiter_cut(false, 6974.0, 7000.0));
        assert!(limiter_cut(false, 6975.0, 7000.0));
        assert!(limiter_cut(true, 6950.0, 7000.0), "still above 98.75 %");
        assert!(!limiter_cut(true, 6900.0, 7000.0));
    }

    #[test]
    fn the_brake_curve_is_read_between_its_three_points() {
        let s = spec();
        assert_eq!(brake_at(&s, 0.0), 120.0);
        assert_eq!(brake_at(&s, 3900.0), 160.0);
        assert_eq!(brake_at(&s, 9000.0), 120.0);
        assert!((brake_at(&s, 2350.0) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn lifting_off_brakes_and_full_throttle_does_not() {
        assert_eq!(net_crank(200.0, 1.0, 160.0, false), 200.0);
        assert!((net_crank(0.0, 0.0, 160.0, false) + 136.0).abs() < 1e-3);
        assert_eq!(net_crank(0.0, 0.0, 160.0, true), -320.0);
        // Half throttle on a strong engine still drives, unscaled.
        assert_eq!(net_crank(100.0, 0.5, 160.0, false), 20.0);
    }

    #[test]
    fn drag_is_the_record_at_full_throttle_and_twice_it_lifted() {
        let half_rho_cd_a = |thr| 0.5 * AIR_DENSITY * drag_coefficient(0.4, thr, 2.2) * 2.2;
        assert!((half_rho_cd_a(1.0) - 0.4).abs() < 1e-5);
        assert!((half_rho_cd_a(0.0) - 0.8).abs() < 1e-5);
    }

    #[test]
    fn nitro_shape_rises_to_a_peak_just_short_of_the_red_line() {
        assert!((nitro_shape(800.0, 800.0, 6400.0) - 0.3).abs() < 1e-6);
        assert!((nitro_shape(800.0 + 7.0 * 700.0, 800.0, 6400.0) - 1.0).abs() < 1e-6);
        assert!((nitro_shape(6400.0, 800.0, 6400.0) - 0.7).abs() < 1e-6);
        assert!((nitro_shape(9000.0, 800.0, 6400.0) - 0.7).abs() < 1e-6, "clamped past the red line");
    }

    #[test]
    fn an_overfilled_tank_burns_harder_and_a_full_one_does_not() {
        assert_eq!(nitro_stage(0.0), 0.0);
        assert_eq!(nitro_stage(0.5), 1.0);
        assert_eq!(nitro_stage(1.0), 1.0);
        assert_eq!(nitro_stage(1.5), 1.4);
        assert_eq!(nitro_stage(1.9), 1.7);
        assert_eq!(nitro_add(1.9, 0.5), 2.0, "overfill stops at twice the tank");
    }

    #[test]
    fn a_car_right_behind_another_drafts_and_one_beside_it_does_not() {
        // A rival at 40 m/s driving along −Z, 8 m ahead of us: its wake is 10 + 35/145·50 ≈ 22 m
        // long, we are 0.36 of the way down it.
        let rival = (Vec3::new(0.0, 0.0, -8.0), Vec3::NEG_Z, 40.0);
        let d = draft(Vec3::ZERO, &[rival]);
        assert!(d > 0.6 && d < 0.8, "{d}");
        // The same car 8 m to the side: outside the cone.
        assert_eq!(draft(Vec3::ZERO, &[(Vec3::new(8.0, 0.0, 0.0), Vec3::NEG_Z, 40.0)]), 0.0);
        // Too far, and a parked car: nothing.
        assert_eq!(draft(Vec3::ZERO, &[(Vec3::new(0.0, 0.0, -70.0), Vec3::NEG_Z, 40.0)]), 0.0);
        assert_eq!(draft(Vec3::ZERO, &[(Vec3::new(0.0, 0.0, -3.0), Vec3::NEG_Z, 0.5)]), 0.0);
        // Two cars in a row cap at 0.9.
        let near = (Vec3::new(0.0, 0.0, -2.0), Vec3::NEG_Z, 40.0);
        assert_eq!(draft(Vec3::ZERO, &[near, near]), DRAFT_MAX);
    }

    #[test]
    fn throttle_eats_lateral_grip_only_near_the_limit() {
        assert_eq!(power_grip(0.0), 1.0);
        // A third of the circle on drive: the game leaves √(1 − 0.25); the engine's 1.2 circle
        // does not cut the lateral peak at all yet.
        assert!((power_grip(1.0 / 3.0) - 0.75f32.sqrt()).abs() < 1e-5);
        // Two thirds and beyond: the game's circle is full, the floor holds.
        assert_eq!(power_grip(0.7), POWER_FLOOR);
        assert_eq!(power_grip(-0.7), POWER_FLOOR, "the sign of the torque does not matter");
    }

    #[test]
    fn a_new_car_with_nitrous_starts_with_a_full_tank() {
        assert_eq!(Powertrain::new(spec()).nitro.fill, 1.0);
        let none = Spec { nitro_tank_s: 0.0, ..spec() };
        assert_eq!(Powertrain::new(none).nitro.fill, 0.0);
    }

    #[test]
    fn downshifts_land_below_the_rpm_an_upshift_drops_to() {
        // 240SX, stock: R, N, then five forward gears; red line 6500.
        let r = [-3.657, 0.0, 3.321, 1.902, 1.308, 1.0, 0.9];
        assert_eq!(downshift_rpm(&r, 2, 6500.0), None, "nothing below first");
        for g in 3..r.len() {
            let down = downshift_rpm(&r, g, 6500.0).unwrap();
            let lands = r[g] / r[g - 1] * (6500.0 - UPSHIFT_BELOW_RED);
            assert!(down < lands && down > 0.5 * lands, "gear {g}: {down} vs {lands}");
        }
    }
}
