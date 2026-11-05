//! Oyunun klavye direksiyonu, `SPEED2.EXE`'den (`0x416BB0`) — `NFS_STEER=game` ile seçiliyor.
//!
//! Oyunun üç direksiyon yolu var (`0x41DFC0`): analog bir aygıt için `0x4061C0`, bir geliştirici
//! bayrağının (`0x81CB8C`, dosyada 0 ve hiçbir yer yazmıyor) arkasında `0x406A40`, ve **varsayılan**
//! `0x416BB0`. Bu modül sonuncusu. Ön tekerin açısı derece cinsinden:
//!
//! `açı = eğri(girdi) · kilit · kaydın direksiyon çarpanı`, ±45°'de kırpılmış, ve
//!
//! `kilit = T_girdi(|girdi|) · T_kilit(v) · (1 + T_oran(v) · (1 − (gaz + 1 − fren/2)/2))` (`0x405DC0`).
//!
//! - `T_kilit` (`0x7E9ED8`) ve `T_oran` (`0x7E9F14`) hızla düşen on noktalı tablolar, eksenleri
//!   0–160 **m/s** (alan `+0x42C`; aynı alan başka yerlerde mph'ye ×2,237 ile çevriliyor ve bir
//!   mesafeyi bölüp süre veriyor). Yani sürülen aralıkta yalnız ilk dört-beş noktası çalışıyor:
//!   0, 64, 128, 192 km/h'de 40°, 20°, 10°, 5°.
//! - Gaz kilidi daraltıyor: tam gazda `T_kilit`, gaz bırakılınca `× (1 + T_oran/2)`, tam frende
//!   `× (1 + 0,75 · T_oran)` — düşük hızda neredeyse bir buçuk katı.
//! - `T_girdi` (`0x7E9F00`) tam girdiye 1,35 veriyor; `eğri` (`0x7E9E60`) yumuşak bir orta.
//! - Direksiyon çarpanı kaydın `+0x380`'i (çoğu arabada 1,1; [`gizmo_nfs::CarHandling::steer_ratio`]).
//!
//! **Karşı direksiyon** (`0x405DC0`'ın ikinci yarısı, [`catch_lock_deg`]): ölçtüğü açı yaw hızı
//! değil, **kayma açısı** — burnun gösterdiği yönle hız vektörü arasındaki açı (`0x59FA10` →
//! `0x591D90`: hızın `atan2`'si eksi gövdenin yönü, hız 1 m/s'nin altındayken 0). Kayma 15°'yi ve
//! hız 13,4 m/s'yi (30 mph) geçince bir drift bayrağı kalkıyor, kayma 5°'nin altına inince iniyor
//! ([`drifting`]). Tuş kaymanın tarafına basılıyken — tekerleri hızın gittiği yöne çeviren tarafa —
//! ve araba ya drift'teyse ya da kayma kilidi aşmışsa, kilit `|kayma| × T(|girdi|)` oluyor
//! (`T` = `0x7E9EB0`, 0,5 → 0,95; en çok 45°). Tam tuşta tekerler neredeyse tam hızın yönüne
//! bakıyor: araba kendi kaymasını yakalıyor. Bu, iki direksiyon yolunun da üstünde çalışıyor
//! ([`super::CarRig::catch_slide`]); `NFS_CATCH=0` kapatıyor.
//!
//! **Burada olmayanlar**, ölçülmüş ama alınmamış: (a) dönüş hızı (`0x4060E0`: 180°/s × hızla
//! 1 → 0,32 × girdinin değişim hızıyla 1 → 3,1) — tuşun kendi rampası [`super::Driver`]'da;
//! (b) girdi ≥ 0,5 tutulurken kilidin hızla daralmaması (önceki kilit korunuyor);
//! (c) kalkışın ilk saniyesindeki daraltma; (d) bazı yarış kiplerinde karşı direksiyonun daha hafif
//! tablosu (`0x7E9EC4`, 0,2 → 1,0; bayraklar `0x89E7D9`, `0x890104`, `0x890114`).
//!
//! **Büyük açılarda önü kaydırması lastikten değil.** Oyunun yanal eğrisi tepeden sonra düşmüyor,
//! motorunki biraz düşüyor (30°'de %96); ama oyunun eğrisine uydurulmuş lastikle
//! (`NFS_TYRE=game`) 100 km/h'deki fark aynı kaldı (11,1 m/s² — yardımla 12,3). Ölçüm
//! `ROADMAP.md`'de.

/// Hız ekseni, m/s: on nokta 0..160.
const SPEED_MAX: f32 = 160.0;
/// `0x7E9ED8`: tam gazdaki kilit, derece.
const LOCK_DEG: [f32; 10] = [40.0, 20.0, 10.0, 5.0, 3.5, 3.1, 2.9, 2.9, 2.9, 2.9];
/// `0x7E9F14`: gaz bırakılınca kilidin açılma payı.
const LIFT_SHARE: [f32; 10] = [1.0, 0.92, 0.78, 0.64, 0.54, 0.48, 0.42, 0.38, 0.34, 0.32];
/// `0x7E9F00`: girdinin büyüklüğüyle kilit çarpanı, 0..1'de altı nokta.
const INPUT_GAIN: [f32; 6] = [1.0, 1.0, 1.1, 1.2, 1.25, 1.35];
/// `0x7E9E60`: girdi eğrisi, −1..1'de 21 nokta (tek), burada yarısı: 0, 0,1, … 1.
const INPUT_CURVE: [f32; 11] = [0.0, 0.034, 0.061, 0.08, 0.116, 0.148, 0.216, 0.303, 0.453, 0.712, 1.0];
/// `0x7844D4`: açının mutlak sınırı.
pub const MAX_DEG: f32 = 45.0;
/// `0x7E9EB0`: karşı direksiyonda kilit kaymanın kaçı, girdinin büyüklüğüyle (0..1'de beş nokta).
const CATCH: [f32; 5] = [0.5, 0.6, 0.7, 0.8, 0.95];
/// `0x7E9130`: drift bayrağının kalktığı kayma, derece.
const DRIFT_ON_DEG: f32 = 15.0;
/// `0x7E9134`: …ve indiği.
const DRIFT_OFF_DEG: f32 = 5.0;
/// `0x7E9138`: bayrağın kalkması için en düşük hız, m/s (30 mph).
const DRIFT_MIN_SPEED: f32 = 13.410_818;

/// `table`'ı `0..=max` üzerinde eşit aralıklı noktalardan, uçlarda kırpılmış okur.
fn lookup(table: &[f32], x: f32, max: f32) -> f32 {
    let last = (table.len() - 1) as f32;
    let at = (x / max * last).clamp(0.0, last);
    let i = (at as usize).min(table.len() - 2);
    let f = at - i as f32;
    table[i] + (table[i + 1] - table[i]) * f
}

/// Oyunun bu hızdaki, bu girdi büyüklüğündeki ve bu pedallardaki kilidi, derece (`0x405DC0`).
/// `brake` fren ile el freninin toplamı, `0..=2` — oyun ikisini toplayıp yarılıyor.
#[must_use]
pub fn lock_deg(speed_mps: f32, input_abs: f32, throttle: f32, brake: f32) -> f32 {
    let v = speed_mps.abs();
    let pedals = 1.0 - (throttle.clamp(0.0, 1.0) + 1.0 - brake.clamp(0.0, 2.0) * 0.5) * 0.5;
    lookup(&INPUT_GAIN, input_abs.abs(), 1.0)
        * lookup(&LOCK_DEG, v, SPEED_MAX)
        * (1.0 + lookup(&LIFT_SHARE, v, SPEED_MAX) * pedals)
}

/// Ön tekerin açısı, derece, işaretli: girdi eğriden geçiyor, kilit ve kaydın çarpanıyla
/// çarpılıyor, ±45°'de kırpılıyor (`0x416BB0`).
#[must_use]
pub fn angle_deg(input: f32, speed_mps: f32, throttle: f32, brake: f32, steer_mult: f32) -> f32 {
    shaped_deg(input, lock_deg(speed_mps, input, throttle, brake), steer_mult)
}

/// Bir kilidin bu girdideki açısı, derece, işaretli: `eğri(girdi) · kilit · çarpan`, ±45°.
#[must_use]
pub fn shaped_deg(input: f32, lock_deg: f32, steer_mult: f32) -> f32 {
    let shaped = lookup(&INPUT_CURVE, input.abs(), 1.0).copysign(input);
    (shaped * lock_deg * steer_mult).clamp(-MAX_DEG, MAX_DEG)
}

/// Drift bayrağı bu karede: kalkmak için kayma > 15° ve hız ≥ 13,4 m/s, inmek için kayma < 5°
/// (`0x405F9C..0x405FE9`; inişin hız eşiği `0x81CB90` = 0, yani hız indirmiyor).
#[must_use]
pub fn drifting(was: bool, slip_deg: f32, speed_mps: f32) -> bool {
    let s = slip_deg.abs();
    if was {
        s >= DRIFT_OFF_DEG
    } else {
        s > DRIFT_ON_DEG && speed_mps >= DRIFT_MIN_SPEED
    }
}

/// Karşı direksiyonun kilidi, derece — ya da tuş kaymanın tarafında değilse, ya da araba drift'te
/// değil ve kayma `lock_deg`'i aşmıyorsa `None` (`0x405F81..0x406075`).
///
/// `slip_left_deg`: hızın burnun neresinde olduğu, derece, **sola artı** — girdiyle aynı işaret
/// düzeni, yani ikisinin çarpımı artıyken tuş tekerleri hızın yönüne çeviriyor.
#[must_use]
pub fn catch_lock_deg(input: f32, slip_left_deg: f32, drifting: bool, lock_deg: f32) -> Option<f32> {
    if input * slip_left_deg <= 0.0 {
        return None;
    }
    let s = slip_left_deg.abs();
    if !drifting && s <= lock_deg {
        return None;
    }
    Some(s.min(MAX_DEG) * lookup(&CATCH, input.abs(), 1.0))
}

/// `NFS_CATCH=0` değilse açık.
#[must_use]
pub fn catch_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !std::env::var("NFS_CATCH").is_ok_and(|v| v == "0"))
}

/// `NFS_STEER=game` seçili mi.
#[must_use]
pub fn chosen() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NFS_STEER").is_ok_and(|v| v == "game"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_throttle_reads_the_lock_table_and_lifting_opens_it() {
        // Standing, full key, full throttle: the table's 40° times the full-input gain.
        assert!((lock_deg(0.0, 1.0, 1.0, 0.0) - 40.0 * 1.35).abs() < 1e-3);
        // 64 km/h is the second point.
        assert!((lock_deg(160.0 / 9.0, 1.0, 1.0, 0.0) - 20.0 * 1.35).abs() < 1e-3);
        // Lifting off widens by half the lift share, full brake by three quarters of it.
        let v = 160.0 / 9.0;
        assert!((lock_deg(v, 1.0, 0.0, 0.0) / lock_deg(v, 1.0, 1.0, 0.0) - (1.0 + 0.92 * 0.5)).abs() < 1e-4);
        assert!((lock_deg(v, 1.0, 0.0, 1.0) / lock_deg(v, 1.0, 1.0, 0.0) - (1.0 + 0.92 * 0.75)).abs() < 1e-4);
    }

    #[test]
    fn the_angle_is_soft_round_the_middle_and_capped_at_45() {
        assert_eq!(angle_deg(0.0, 20.0, 1.0, 0.0, 1.1), 0.0);
        let half = angle_deg(0.5, 30.0, 1.0, 0.0, 1.0);
        let full = angle_deg(1.0, 30.0, 1.0, 0.0, 1.0);
        assert!(half > 0.0 && half < 0.2 * full, "half a key is a seventh of the angle: {half} vs {full}");
        assert_eq!(angle_deg(-1.0, 0.0, 0.0, 1.0, 1.25), -MAX_DEG);
    }

    #[test]
    fn the_drift_flag_rises_at_15_degrees_and_30_mph_and_falls_below_5() {
        assert!(!drifting(false, 14.0, 30.0));
        assert!(!drifting(false, 20.0, 13.0), "too slow to start a drift");
        assert!(drifting(false, -16.0, 14.0));
        assert!(drifting(true, 6.0, 0.5), "once up, speed does not bring it down");
        assert!(!drifting(true, 4.9, 30.0));
    }

    #[test]
    fn steering_into_the_slide_opens_the_lock_to_the_slip() {
        // Full key towards a 30° slide: 0.95 of it.
        assert!((catch_lock_deg(1.0, 30.0, true, 5.0).unwrap() - 28.5).abs() < 1e-4);
        // The other way is not a catch.
        assert_eq!(catch_lock_deg(-1.0, 30.0, true, 5.0), None);
        // Not drifting and inside the lock: the ordinary steering stands.
        assert_eq!(catch_lock_deg(1.0, 4.0, false, 5.0), None);
        // Not drifting but past the lock: caught all the same.
        assert!(catch_lock_deg(1.0, 8.0, false, 5.0).is_some());
        // Capped at 45° of slip, and a light key catches half.
        assert!((catch_lock_deg(0.0001, 80.0, true, 5.0).unwrap() - 22.5).abs() < 0.01);
    }
}
