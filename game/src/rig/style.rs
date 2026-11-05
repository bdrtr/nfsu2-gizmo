//! Stil puanları, `SPEED2.EXE`'nin puanlayıcısından (`0x56C740`, `0x56E920`, `0x56EAB0`) ve
//! tanımları `GLOBALB.BUN`'un `0x00034A07` parçasından ([`gizmo_nfs::globalb::style`]).
//!
//! Oyun her olay için bir canlı nesne tutuyor. Burada iki parça var:
//!
//! - [`Scorer`]: tanımlar üstünde oyunun kuralları — sürekli bir olay (kayma, havada kalma, geri
//!   gitme) beslendiği her karede `oran · girdi · çarpan · dt` biriktiriyor, `settle_s` boyunca
//!   sürüp `threshold`'u geçmeden sayılmıyor, `wait_s` boyunca beslenmeyince bankaya geçiyor; anlık
//!   bir olay (360, J-turn) ateşlendiği anda `oran · girdi · çarpan` ödüyor. Bankaya her geçişte
//!   çarpan `multiplier_step` kadar büyüyor, `max_multiplier`'e kadar; `limit` bir yarışın kaç kez
//!   ödediği.
//! - [`Moves`]: arabanın hareketinden hangi olayın beslendiği, oyunun tetikleyicileri ve eşikleriyle.
//!
//! Bankaya geçen puan nitroyu dolduruyor (`0x6016A0`): `Δpuan / 15 · 0,01` depo, yani 1 500 puan bir
//! depo ([`refill`]).
//!
//! **Burada olmayanlar:** drift kipinin kendi puanlayıcısı (`0x56ED40`, beş kademeli kombo, drift
//! yarışında) — serbest dolaşımda yok; DRAFTING (rakibin rüzgârı), trafik, kısa yol, temiz bölüm, sıra
//! kazanma — bunlar yarışın ve trafiğin bilgisi; BURNOUT'un tetikleyicisi okunmadı. POWER SLIDE'ı duvar
//! vuruşu bozuyor (`0x56F4C0`), burada çarpışma bilgisi yok, bozulmuyor.

use gizmo_nfs::globalb::style::StyleEvent;

/// Olay sıraları, kaydın kendi sırası (`StyleEvent::index`).
pub mod event {
    pub const DRIFT: usize = 0;
    pub const SPIN_360: usize = 22;
    pub const J_TURN: usize = 25;
    pub const POWER_SLIDE: usize = 34;
    pub const HANG_TIME: usize = 35;
    pub const DRAFTING: usize = 36;
    pub const REVERSE: usize = 37;
}

/// Bir olayın bu yarıştaki hâli.
#[derive(Clone, Debug, PartialEq)]
enum Phase {
    /// Beslenmiyor.
    Idle,
    /// Besleniyor, henüz sayılmadı.
    Building { age: f32, pending: f32, since_fed: f32 },
    /// Sayıldı; beslenmesi kesilince bankaya geçecek.
    Counting { pending: f32, since_fed: f32 },
}

#[derive(Clone, Debug)]
struct Live {
    def: StyleEvent,
    name: String,
    phase: Phase,
    multiplier: f32,
    paid: u32,
}

/// Bir yarışın stil puanları.
#[derive(Clone, Debug, Default)]
pub struct Scorer {
    events: Vec<Live>,
    /// Bankadaki toplam.
    pub banked: f32,
    /// Son bankaya geçen olay: adı, kaç puan, kaç saniye önce.
    pub last: Option<(String, f32, f32)>,
    /// Son [`Scorer::tick`]'ten beri bankaya geçen — anlık olaylar dahil.
    fresh: f32,
}

impl Scorer {
    /// `modes` bitlerinden birini taşıyan tanımlarla. `name` bir tanımın adını verir (dil dosyası).
    #[must_use]
    pub fn new(defs: &[StyleEvent], modes: u8, name: impl Fn(&StyleEvent) -> String) -> Self {
        let events = defs
            .iter()
            .map(|d| Live {
                def: *d,
                name: name(d),
                phase: Phase::Idle,
                multiplier: 1.0,
                // Kipi tutmayan olay hiç ödemesin: limiti dolmuş gibi.
                paid: if d.modes & modes == 0 { u32::MAX } else { 0 },
            })
            .collect();
        Self { events, banked: 0.0, last: None, fresh: 0.0 }
    }

    /// Bu olay bu yarışta ödeyebilir mi.
    fn open(&self, i: usize) -> Option<&Live> {
        let e = self.events.get(i)?;
        let limit_left = e.def.limit < 0 || e.paid < e.def.limit as u32;
        (e.paid != u32::MAX && limit_left).then_some(e)
    }

    /// Sürekli bir olayı bu kare besle, `input` girdiyle.
    pub fn feed(&mut self, i: usize, input: f32, dt: f32) {
        if self.open(i).is_none() || input <= 0.0 {
            return;
        }
        let e = &mut self.events[i];
        let gain = e.def.rate * input * e.multiplier * dt;
        let cap = e.def.cap;
        let add = |p: f32| if cap > 0.0 { (p + gain).min(cap) } else { p + gain };
        e.phase = match e.phase {
            Phase::Idle => Phase::Building { age: dt, pending: add(0.0), since_fed: 0.0 },
            Phase::Building { age, pending, .. } => {
                Phase::Building { age: age + dt, pending: add(pending), since_fed: 0.0 }
            }
            Phase::Counting { pending, .. } => Phase::Counting { pending: add(pending), since_fed: 0.0 },
        };
    }

    /// Anlık bir olayı ateşle. Eşiği geçerse hemen bankaya.
    pub fn fire(&mut self, i: usize, input: f32) {
        if self.open(i).is_none() {
            return;
        }
        let e = &self.events[i];
        let pts = e.def.rate * input * e.multiplier;
        let clears = if e.def.rate < 0.0 { pts < e.def.threshold } else { pts > e.def.threshold };
        if clears {
            self.bank(i, pts);
        }
    }

    /// Bir olayı onaylanmadan iptal et (oyunun `0x56D5C0`'ı: kayarken duvara vurmak).
    pub fn spoil(&mut self, i: usize) {
        if let Some(e) = self.events.get_mut(i) {
            if matches!(e.phase, Phase::Building { .. }) {
                e.phase = Phase::Idle;
            }
        }
    }

    /// Zamanı ilerlet; son çağrıdan beri bankaya geçen puanı döndürür (araya ateşlenen anlık olaylar
    /// dahil).
    pub fn tick(&mut self, dt: f32) -> f32 {
        if let Some(last) = &mut self.last {
            last.2 += dt;
        }
        for i in 0..self.events.len() {
            let e = &mut self.events[i];
            let (wait, settle, threshold) = (e.def.wait_s, e.def.settle_s, e.def.threshold);
            match e.phase.clone() {
                Phase::Idle => {}
                Phase::Building { age, pending, since_fed } => {
                    let since = since_fed + dt;
                    let reached = if e.def.rate < 0.0 { pending < threshold } else { pending > threshold };
                    if age >= settle && reached {
                        e.phase = Phase::Counting { pending, since_fed: since };
                    } else if since > wait.max(dt) {
                        e.phase = Phase::Idle;
                    } else {
                        e.phase = Phase::Building { age, pending, since_fed: since };
                    }
                }
                Phase::Counting { pending, since_fed } => {
                    let since = since_fed + dt;
                    if since > wait.max(dt) {
                        self.events[i].phase = Phase::Idle;
                        self.bank(i, pending);
                    } else {
                        self.events[i].phase = Phase::Counting { pending, since_fed: since };
                    }
                }
            }
        }
        std::mem::take(&mut self.fresh)
    }

    /// Şu an biriken ama henüz bankaya geçmemiş en büyük olay: adı ve puanı.
    #[must_use]
    pub fn running(&self) -> Option<(&str, f32)> {
        self.events
            .iter()
            .filter_map(|e| match e.phase {
                Phase::Counting { pending, .. } | Phase::Building { pending, .. } if pending > 0.0 => {
                    Some((e.name.as_str(), pending))
                }
                _ => None,
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn bank(&mut self, i: usize, pts: f32) {
        let e = &mut self.events[i];
        let pts = pts.trunc();
        self.banked += pts;
        self.fresh += pts;
        e.paid = e.paid.saturating_add(1);
        e.multiplier = (e.multiplier + e.def.multiplier_step).min(e.def.max_multiplier.max(1.0));
        self.last = Some((e.name.clone(), pts, 0.0));
    }
}

/// Puanın nitro karşılığı, depo kesri: `Δpuan / 15 · 0,01` (`0x6016A0`).
#[must_use]
pub fn refill(points: f32) -> f32 {
    points / 15.0 * 0.01
}

/// Arabanın bu karedeki hareketi, olayları beslemek için.
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    /// Kayma açısı, radyan (işareti önemsiz).
    pub slip: f32,
    /// Düzlemdeki hız, m/s.
    pub speed: f32,
    /// Burun yönündeki hız, m/s; geri giderken eksi.
    pub forward: f32,
    /// Yerdeki teker sayısı.
    pub wheels_down: usize,
    /// Dikey eksen etrafında dönme hızı, rad/s.
    pub yaw_rate: f32,
    /// Burnun yönü, radyan.
    pub heading: f32,
    /// Rüzgâr arkası, `0..=0,9` ([`super::powertrain::draft`]).
    pub draft: f32,
}

/// POWER SLIDE: kayma > 5°, hız > 40 mph, dört teker yerde (`0x56F4C0`).
const SLIDE_SLIP: f32 = 5.0 * std::f32::consts::PI / 180.0;
const SLIDE_SPEED: f32 = 17.881_09;
/// …girdi `min(1, v·|kayma| / 20)`.
const SLIDE_SCALE: f32 = 20.0;
/// DRAFTING: hız > 20 m/s ve rüzgâr arkası > 0,25, girdi rüzgâr payının kendisi (`0x56F720`).
const DRAFT_SPEED: f32 = 20.0;
const DRAFT_MIN: f32 = 0.25;
/// REVERSE: 40 mph'den hızlı geri (`0x56F720`).
const REVERSE_SPEED: f32 = 17.881_09;
/// J-TURN: 30 mph'den hızlı geri, sonra 20 mph'den hızlı ileri (`0x56FA60`).
const JTURN_BACK: f32 = 13.410_82;
const JTURN_FORWARD: f32 = 8.940_55;
/// 360: dönme hızı 2 rad/s'de başlıyor, 1'in altında bitiyor; 170°'lik üç adım (`0x56F620`).
const SPIN_START: f32 = 2.0;
const SPIN_KEEP: f32 = 1.0;
const SPIN_STEP: f32 = 170.0 * std::f32::consts::PI / 180.0;

/// Olayları hareketten besleyen durum: 360 ve J-turn'ün yarım kalan adımları.
#[derive(Clone, Copy, Debug, Default)]
pub struct Moves {
    spin_steps: u8,
    spin_mark: f32,
    jturn_armed: bool,
}

impl Moves {
    /// Bu karenin hareketiyle olayları besle.
    pub fn observe(&mut self, m: Motion, s: &mut Scorer, dt: f32) {
        let grounded = m.wheels_down >= 4;
        // POWER SLIDE.
        if grounded && m.slip.abs() > SLIDE_SLIP && m.speed > SLIDE_SPEED {
            s.feed(event::POWER_SLIDE, (m.speed * m.slip.abs() / SLIDE_SCALE).min(1.0), dt);
        }
        // HANG TIME: oyun gövdenin yerden 0,75 m yükselmesine bakıyor (`0x56F020`); burada dört
        // teker birden havadaysa.
        if m.wheels_down == 0 && m.speed > 1.0 {
            s.feed(event::HANG_TIME, 1.0, dt);
        }
        // DRAFTING.
        if m.speed > DRAFT_SPEED && m.draft > DRAFT_MIN {
            s.feed(event::DRAFTING, m.draft, dt);
        }
        // REVERSE.
        if m.forward < -REVERSE_SPEED {
            s.feed(event::REVERSE, 1.0, dt);
        }
        // J-TURN.
        if !grounded {
            self.jturn_armed = false;
        } else if !self.jturn_armed {
            self.jturn_armed = m.forward <= -JTURN_BACK;
        } else if m.speed < JTURN_FORWARD {
            // Dönerken 20 mph'nin altına düşen J-turn sayılmıyor: araba hız kesmeden dönmeli.
            self.jturn_armed = false;
        } else if m.forward > JTURN_FORWARD {
            s.fire(event::J_TURN, 1.0);
            self.jturn_armed = false;
        }
        // 360.
        let w = m.yaw_rate.abs();
        if !grounded || (self.spin_steps == 0 && w <= SPIN_START) || (self.spin_steps > 0 && w <= SPIN_KEEP) {
            self.spin_steps = 0;
        } else if self.spin_steps == 0 {
            self.spin_steps = 1;
            self.spin_mark = m.heading;
        } else if angle_between(m.heading, self.spin_mark) >= SPIN_STEP {
            self.spin_steps += 1;
            self.spin_mark = m.heading;
            if self.spin_steps == 3 {
                s.fire(event::SPIN_360, 1.0);
                self.spin_steps = 0;
            }
        }
    }
}

/// İki yön arasındaki açı, `0..π`.
fn angle_between(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(index: u8, continuous: bool, rate: f32, threshold: f32, settle: f32, wait: f32) -> StyleEvent {
        StyleEvent {
            label: 0,
            index,
            parent: index,
            continuous,
            modes: 0x49,
            settle_s: settle,
            wait_s: wait,
            threshold,
            cap: 0.0,
            max_multiplier: 1.0,
            multiplier_step: 1.0,
            unknown: [1.0, 1.0],
            rate,
            limit: -1,
        }
    }

    fn defs() -> Vec<StyleEvent> {
        (0..39u8)
            .map(|i| match i as usize {
                event::POWER_SLIDE => def(i, true, 120.0, 50.0, 0.1, 0.3),
                event::HANG_TIME => def(i, true, 120.0, 50.0, 0.2, 0.0),
                event::SPIN_360 => def(i, false, 400.0, 0.0, 0.0, 0.0),
                event::J_TURN => def(i, false, 500.0, 0.0, 0.0, 0.0),
                _ => StyleEvent { modes: 4, ..def(i, false, 0.0, 0.0, 0.0, 0.0) },
            })
            .collect()
    }

    #[test]
    fn a_slide_long_enough_banks_after_the_wait_and_a_short_one_does_not() {
        let dt = 1.0 / 60.0;
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        // One second at full input: 120 points, banked 0.3 s after it stops.
        for _ in 0..60 {
            s.feed(event::POWER_SLIDE, 1.0, dt);
            s.tick(dt);
        }
        assert_eq!(s.banked, 0.0, "nothing banks while it is still going");
        for _ in 0..30 {
            s.tick(dt);
        }
        assert!((s.banked - 119.0).abs() <= 1.0, "banked {}", s.banked);
        // A quarter second: 30 points, under the 50 threshold — nothing.
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        for _ in 0..15 {
            s.feed(event::POWER_SLIDE, 1.0, dt);
            s.tick(dt);
        }
        for _ in 0..60 {
            s.tick(dt);
        }
        assert_eq!(s.banked, 0.0);
    }

    #[test]
    fn events_outside_the_mode_never_pay_and_1500_points_is_a_tank() {
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        s.fire(event::DRIFT, 1.0);
        s.fire(event::SPIN_360, 1.0);
        assert_eq!(s.banked, 400.0);
        assert_eq!(s.tick(0.01), 400.0, "what fired before the tick is paid by it");
        assert_eq!(s.tick(0.01), 0.0, "and only once");
        assert!((refill(1500.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn three_half_turns_of_spinning_make_a_360() {
        let dt = 1.0 / 60.0;
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        let mut m = Moves::default();
        let mut heading = 0.0f32;
        for _ in 0..120 {
            heading += 3.5 * dt;
            m.observe(
                Motion { speed: 10.0, forward: 5.0, wheels_down: 4, yaw_rate: 3.5, heading, ..Motion::default() },
                &mut s,
                dt,
            );
        }
        assert_eq!(s.banked, 400.0, "two seconds at 3.5 rad/s is 401°: one 360");
    }

    #[test]
    fn a_j_turn_is_fast_backwards_then_fast_forwards() {
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        let mut m = Moves::default();
        let at = |speed: f32, forward: f32| Motion { speed, forward, wheels_down: 4, ..Motion::default() };
        // Backwards at 15 m/s, swung sideways without losing speed, then nose-first.
        m.observe(at(15.0, -15.0), &mut s, 0.1);
        m.observe(at(12.0, 0.0), &mut s, 0.1);
        m.observe(at(12.0, 10.0), &mut s, 0.1);
        assert_eq!(s.banked, 500.0);
        // The same, but stopping to turn round: a three-point turn, not a J-turn.
        let mut s = Scorer::new(&defs(), 1, |_| String::new());
        m.observe(at(15.0, -15.0), &mut s, 0.1);
        m.observe(at(2.0, -2.0), &mut s, 0.1);
        m.observe(at(12.0, 10.0), &mut s, 0.1);
        assert_eq!(s.banked, 0.0);
    }
}
