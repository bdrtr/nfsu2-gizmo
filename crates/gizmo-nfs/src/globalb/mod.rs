//! Parsing NFSU2's global car database (`GLOBAL/GLOBALB.BUN`).
//!
//! A per-car `GEOMETRY.BIN` carries only *one* wheel mesh; the game reads where to place it at
//! the four corners — plus the wheel radius, mass and much else — from a `CarTypeInfo` record in
//! the global bundle. Each record is `0x890` bytes and begins with the car's collection name, so
//! records are located by their `CARS\<name>\GEOMETRY.BIN` path signature and read by fixed
//! field offsets. Layout is from NFSTools/GlobalLib (`Support.Underground2` CarTypeInfo
//! disassembler); validated against real files (e.g. the 240SX's track = 1.64 m matches its
//! known body width).
//!
//! Axes (car space): **XValue = longitudinal** (fore/aft, + front − rear), **YValue = lateral**
//! (track, + left − right), **RideHeight = vertical**, **Diameter = wheel radius (metres)**.

const REC_SIZE: usize = 0x890;
const OFF_NAME2: usize = 0x20;
const OFF_PATH: usize = 0x40;
const OFF_MANUFACTURER: usize = 0xC0;
const OFF_WHEELS: usize = 0x120;
const WHEEL_STRIDE: usize = 0x30;
const OFF_MASS: usize = 0x220;

// Field offsets within one wheel entry.
const W_FORE_AFT: usize = 0x00; // XValue
const W_RIDE_HEIGHT: usize = 0x08;
const W_RADIUS: usize = 0x10; // Diameter (actually the radius, in metres)
const W_LATERAL: usize = 0x1C; // YValue

/// One wheel's mount, in NFSU2 car space (metres).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct WheelSpec {
    /// Longitudinal (fore/aft) offset from the car origin: `+` toward the front, `−` the rear.
    pub fore_aft: f32,
    /// Lateral (track) offset: `+` is the left side, `−` the right.
    pub lateral: f32,
    /// Vertical height of the wheel centre.
    pub ride_height: f32,
    /// Wheel radius in metres.
    pub radius: f32,
}

/// The subset of a `CarTypeInfo` record needed to place and size a car: its name, the four
/// wheel mounts (front-left, front-right, rear-right, rear-left — file order), and mass.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CarTypeInfo {
    /// Collection name, e.g. `"MUSTANGGT"` (matches the `CARS/<name>/` folder).
    pub name: String,
    /// Manufacturer/display name.
    pub manufacturer: String,
    /// Front-left, front-right, rear-right, rear-left.
    pub wheels: [WheelSpec; 4],
    /// Mass in kilograms.
    pub mass_kg: f32,
}

#[inline]
fn le_f32(b: &[u8], off: usize) -> Option<f32> {
    b.get(off..off + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Read a fixed-width, NUL-terminated ASCII field.
fn cstr(b: &[u8], off: usize, max: usize) -> String {
    let s = b.get(off..(off + max).min(b.len())).unwrap_or(&[]);
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..end]).into_owned()
}

/// A plausible car collection name: non-empty, short, and only `A–Z 0–9 _`.
fn is_car_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 13
        && s.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

fn read_wheel(b: &[u8], base: usize) -> Option<WheelSpec> {
    Some(WheelSpec {
        fore_aft: le_f32(b, base + W_FORE_AFT)?,
        lateral: le_f32(b, base + W_LATERAL)?,
        ride_height: le_f32(b, base + W_RIDE_HEIGHT)?,
        radius: le_f32(b, base + W_RADIUS)?,
    })
}

/// Read the `CarTypeInfo` record beginning at `rec`, or `None` if it doesn't validate.
fn read_record(b: &[u8], rec: usize) -> Option<CarTypeInfo> {
    if rec + REC_SIZE > b.len() {
        return None;
    }
    let name = cstr(b, rec, 0x10);
    // The name is stored twice (0x00 and 0x20); require both to agree — a strong record check.
    if !is_car_name(&name) || cstr(b, rec + OFF_NAME2, 0x10) != name {
        return None;
    }
    let mut wheels = [WheelSpec { fore_aft: 0.0, lateral: 0.0, ride_height: 0.0, radius: 0.0 }; 4];
    for (i, w) in wheels.iter_mut().enumerate() {
        *w = read_wheel(b, rec + OFF_WHEELS + i * WHEEL_STRIDE)?;
    }
    Some(CarTypeInfo {
        name,
        manufacturer: cstr(b, rec + OFF_MANUFACTURER, 0x10),
        wheels,
        // Stored as mass × 1/1000 (a Mustang reads 1.560 → 1560 kg).
        mass_kg: le_f32(b, rec + OFF_MASS)? * 1000.0,
    })
}

/// Parse every `CarTypeInfo` record from a decompressed `GLOBALB.BUN`.
///
/// Records are found by their `CARS\<name>\GEOMETRY.BIN` path signature (at record offset
/// `0x40`) and validated by the doubly-stored name, so the scan tolerates the surrounding
/// database chunks without decoding them.
#[must_use]
pub fn parse_cartypeinfos(globalb: &[u8]) -> Vec<CarTypeInfo> {
    let mut out = Vec::new();
    let mut seen_end = 0usize; // avoid overlapping matches
    let needle = b"CARS";
    let mut i = 0usize;
    while i + OFF_PATH < globalb.len() {
        let Some(rel) = globalb[i..].windows(needle.len()).position(|w| w == needle) else {
            break;
        };
        let path_pos = i + rel;
        i = path_pos + 1;
        if path_pos < OFF_PATH {
            continue;
        }
        let rec = path_pos - OFF_PATH;
        if rec < seen_end {
            continue;
        }
        // Confirm this is the GEOMETRY path field, not an incidental "CARS".
        let path = cstr(globalb, path_pos, 0x30);
        if !path.contains("GEOMETRY") {
            continue;
        }
        if let Some(info) = read_record(globalb, rec) {
            seen_end = rec + REC_SIZE;
            out.push(info);
        }
    }
    out
}

/// Find one car's record by collection name (case-sensitive, e.g. `"240SX"`).
#[must_use]
pub fn find_car(globalb: &[u8], name: &str) -> Option<CarTypeInfo> {
    parse_cartypeinfos(globalb).into_iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic 0x890 record with a name, path, and one known wheel, exercising the reader
    /// without shipping any copyrighted game data.
    #[test]
    fn reads_a_synthetic_record() {
        let mut b = vec![0u8; REC_SIZE + 16];
        b[..5].copy_from_slice(b"TESTX");
        b[OFF_NAME2..OFF_NAME2 + 5].copy_from_slice(b"TESTX");
        let path = b"CARS\\TESTX\\GEOMETRY.BIN";
        b[OFF_PATH..OFF_PATH + path.len()].copy_from_slice(path);
        let put = |b: &mut [u8], o: usize, v: f32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        // Front-left wheel.
        put(&mut b, OFF_WHEELS + W_FORE_AFT, 1.44);
        put(&mut b, OFF_WHEELS + W_LATERAL, 0.86);
        put(&mut b, OFF_WHEELS + W_RIDE_HEIGHT, 0.17);
        put(&mut b, OFF_WHEELS + W_RADIUS, 0.34);
        put(&mut b, OFF_MASS, 1.56);

        let cars = parse_cartypeinfos(&b);
        assert_eq!(cars.len(), 1);
        let c = &cars[0];
        assert_eq!(c.name, "TESTX");
        assert!((c.mass_kg - 1560.0).abs() < 1e-3);
        assert!((c.wheels[0].fore_aft - 1.44).abs() < 1e-4);
        assert!((c.wheels[0].lateral - 0.86).abs() < 1e-4);
        assert!((c.wheels[0].radius - 0.34).abs() < 1e-4);
        assert_eq!(find_car(&b, "TESTX").as_ref(), Some(c));
        assert!(find_car(&b, "NOPE").is_none());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_cartypeinfos(&[]).is_empty());
        assert!(parse_cartypeinfos(&[0u8; 32]).is_empty());
        // "CARS" present but no valid record around it.
        let mut b = vec![0u8; 0x40];
        b.extend_from_slice(b"CARS but not a real path");
        assert!(parse_cartypeinfos(&b).is_empty());
    }
}
