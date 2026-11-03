//! Motor sesi: devirden sentezlenmiş iki döngü, motorun anlık devriyle çalınıyor.
//!
//! NFSU2'nin kendi motor sesleri (`.gin` granüler örnekler) M6'nın işi; bu, o gelene kadar
//! arabanın **duyulmasını** sağlayan şey. Oyunlarda yaygın olan yapı: iki döngü kaydı, biri düşük
//! devirde ([`LOW_RPM`]) biri yüksekte ([`HIGH_RPM`]); her biri `rpm / kayıt_devri` hızında
//! çalınıyor (perde devirle birlikte yükseliyor), ve aradaki bantta eşit güçle birbirine geçiyor
//! ([`layer_gains`]). Tek bir kaydı 800'den 7000 devire çekmek 9 kat hız demek — ses bir motor
//! olmaktan çıkıp vızıltıya dönüyor; iki kayıtla hiçbiri 0,5–2 katının dışına çıkmıyor.
//!
//! Kayıtlar diskten değil, burada üretiliyor ([`synth`]): dört zamanlı dört silindirin ateşleme
//! frekansı (`rpm / 30` Hz) ve harmonikleri, krank frekansında bir gurultu (silindirler eşit
//! değil), ve her ateşlemede sönen bir gürültü patlaması. Uzunluk tam sayıda krank periyodu, yani
//! döngünün dikişi duyulmuyor. Hepsi saf fonksiyon ve test ediliyor; cihaz yalnız
//! [`EngineSound`]'da.

use gizmo::audio::AudioManager;

/// Üretilen kayıtların örnekleme hızı.
pub const RATE: u32 = 44_100;
/// Düşük ve yüksek kaydın devri. İkisi de krank frekansını tam sayı örnek yapıyor (25 Hz = 1764,
/// 75 Hz = 588 örnek), döngü bu yüzden dikişsiz.
pub const LOW_RPM: f32 = 1500.0;
pub const HIGH_RPM: f32 = 4500.0;
/// Geçiş bandı: bunun altında yalnız düşük kayıt, üstünde yalnız yüksek kayıt.
const BLEND_FROM: f32 = 2200.0;
const BLEND_TO: f32 = 3800.0;
/// Her şeyin üstündeki genel ses düzeyi; 1.0 tam ölçek.
const MASTER: f32 = 0.55;

/// `rpm` devrinde bir saniyelik motor döngüsü, −1..1.
///
/// `bright` 0..1: yüksek kayıt daha çok üst harmonik ve daha çok gürültü taşıyor — yüksek devirde
/// bir motor daha sert duyuluyor, perdesi daha yüksek olduğu için değil yalnızca.
#[must_use]
pub fn synth(rpm: f32, bright: f32) -> Vec<f32> {
    let crank = rpm / 60.0;
    let firing = crank * 2.0; // dört silindir, dört zaman: krank turu başına iki ateşleme
    let n = RATE as usize; // bir saniye; 25 ve 75 Hz'te tam sayıda periyot
    let mut seed = 0x2545_f491_u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let harmonics = 6 + (bright * 6.0) as usize;
    // Sabit, rastgele görünen fazlar: hepsi sıfırdan başlarsa dalga bir iğne dizisi olur.
    let phases: Vec<f32> = (0..harmonics).map(|k| (k as f32 * 2.399).fract() * std::f32::consts::TAU).collect();
    let tau = std::f32::consts::TAU;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let mut s = 0.0;
        for (k, ph) in phases.iter().enumerate() {
            let k1 = (k + 1) as f32;
            s += (tau * firing * k1 * t + ph).sin() / k1.powf(1.1 - 0.4 * bright);
        }
        // Krank frekansı: silindirlerin eşit olmayışı, rölantideki o tok gurultu.
        s += 0.45 * (tau * crank * t).sin();
        // Ateşleme anında sönen gürültü: yanmanın kendisi.
        let since = (t * firing).fract();
        s += (0.25 + 0.35 * bright) * noise() * (-since * 6.0).exp();
        out.push(s);
    }
    // Yumuşak kırpma ve normalize.
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    for v in &mut out {
        *v = (1.6 * *v / peak).tanh() * 0.9;
    }
    out
}

/// Örnekleri 16 bit tek kanallı bir WAV dosyasına yazar — `AudioManager::load_sound_bytes` tam bir
/// dosya istiyor.
#[must_use]
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

/// (düşük, yüksek) kaydın kazancı `rpm`'de: eşit güçte geçiş, toplam güç sabit.
#[must_use]
pub fn layer_gains(rpm: f32) -> (f32, f32) {
    let f = ((rpm - BLEND_FROM) / (BLEND_TO - BLEND_FROM)).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    let a = f * std::f32::consts::FRAC_PI_2;
    (a.cos(), a.sin())
}

/// Motorun ne kadar yüklü çaldığı: gazla belirgin biçimde, devirle biraz yükseliyor. Gaz
/// kesildiğinde ses düşüyor ama susmuyor — dönen bir motor gazsız da duyulur.
#[must_use]
pub fn loudness(rpm: f32, max_rpm: f32, throttle: f32) -> f32 {
    let r = if max_rpm > 0.0 { (rpm / max_rpm).clamp(0.0, 1.0) } else { 0.0 };
    MASTER * (0.30 + 0.45 * throttle.clamp(0.0, 1.0) + 0.25 * r)
}

/// Bir arabanın motor sesi, çalarken.
pub struct EngineSound {
    audio: AudioManager,
    low: u64,
    high: u64,
    /// Ses düzeyinin yumuşatılmış hali: gaz bir karede 0'dan 1'e atlayabilir, ses atlamamalı.
    load: f32,
}

impl EngineSound {
    /// Cihazı açar ve iki döngüyü başlatır. Cihaz yoksa, ya da `NFS_MUTE` verilmişse, `None` — oyun
    /// sessiz çalışır, durmaz.
    #[must_use]
    pub fn start() -> Option<Self> {
        if std::env::var("NFS_MUTE").is_ok_and(|v| v != "0") {
            return None;
        }
        let mut audio = AudioManager::new().ok()?;
        audio.load_sound_bytes("engine_low", wav(&synth(LOW_RPM, 0.0)));
        audio.load_sound_bytes("engine_high", wav(&synth(HIGH_RPM, 1.0)));
        let low = audio.play_looped("engine_low").ok()?;
        let high = audio.play_looped("engine_high").ok()?;
        audio.set_volume(low, 0.0);
        audio.set_volume(high, 0.0);
        Some(Self { audio, low, high, load: 0.0 })
    }

    /// Bu karenin devri ve gazıyla perdeyi ve düzeyi ayarlar.
    pub fn update(&mut self, rpm: f32, max_rpm: f32, throttle: f32, dt: f32) {
        let rpm = if rpm.is_finite() { rpm.max(1.0) } else { LOW_RPM };
        self.load += (throttle - self.load) * (1.0 - (-8.0 * dt).exp());
        let level = loudness(rpm, max_rpm, self.load);
        let (gl, gh) = layer_gains(rpm);
        self.audio.set_pitch(self.low, rpm / LOW_RPM);
        self.audio.set_pitch(self.high, rpm / HIGH_RPM);
        self.audio.set_volume(self.low, level * gl);
        self.audio.set_volume(self.high, level * gh);
        self.audio.update();
    }

    /// Bir `VehicleController`'dan okuyarak [`Self::update`].
    pub fn follow(&mut self, v: &gizmo::physics::vehicle::VehicleController, dt: f32) {
        self.update(v.engine_rpm, v.tuning.max_rpm, v.throttle_input, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loops_are_whole_and_in_range() {
        for (rpm, bright) in [(LOW_RPM, 0.0), (HIGH_RPM, 1.0)] {
            let s = synth(rpm, bright);
            assert_eq!(s.len(), RATE as usize);
            assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 0.9 + 1e-6));
            // Krank periyodu tam sayı örnek: döngünün dikişi yok.
            let period = RATE as f32 * 60.0 / rpm;
            assert_eq!(period.fract(), 0.0, "{rpm} rpm");
        }
    }

    #[test]
    fn the_wav_header_says_what_follows() {
        let b = wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 6);
        assert_eq!(b.len(), 44 + 6);
    }

    #[test]
    fn the_layers_hand_over_at_constant_power() {
        assert_eq!(layer_gains(800.0), (1.0, 0.0));
        let (l, h) = layer_gains(7000.0);
        assert!(l.abs() < 1e-6 && (h - 1.0).abs() < 1e-6);
        for rpm in (800..7000).step_by(100) {
            let (l, h) = layer_gains(rpm as f32);
            assert!((l * l + h * h - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn throttle_is_louder_and_off_throttle_is_not_silent() {
        assert!(loudness(3000.0, 7000.0, 1.0) > loudness(3000.0, 7000.0, 0.0));
        assert!(loudness(800.0, 7000.0, 0.0) > 0.0);
        assert!(loudness(7000.0, 7000.0, 1.0) <= MASTER);
    }
}
