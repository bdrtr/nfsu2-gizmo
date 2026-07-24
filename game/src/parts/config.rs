//! The car's configuration: which aftermarket part fills each swappable slot.

/// A car configuration: which aftermarket parts replace the stock ones. `0` in any field means
/// "stock" (kit slot `KIT00`), so [`CarConfig::stock`] (all zero) is the default showroom car.
/// The NFSU2 geometry only holds body kits, hood/light styles and widebody sets per car; the
/// universal aftermarket spoilers and rims live in a separate global bundle, so they are not
/// configured here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarConfig {
    /// Body kit `KIT##` sourcing the front bumper, rear bumper and side skirt (`0` = stock).
    pub body_kit: u8,
    /// Hood design `STYLE##` (`0` = stock `KIT00` hood).
    pub hood_style: u8,
    /// Head- and tail-light design `STYLE##` (`STYLE01..14`; `0` = stock).
    pub light_style: u8,
    /// Widebody kit `KITW##` replacing the body and doors (`0` = stock body).
    pub widebody: u8,
}

impl CarConfig {
    /// The stock showroom car (all slots `KIT00`).
    #[must_use]
    pub const fn stock() -> Self {
        Self { body_kit: 0, hood_style: 0, light_style: 0, widebody: 0 }
    }

    /// Build a config from `NFS_KIT` / `NFS_STYLE_HOOD` / `NFS_STYLE_LIGHT` / `NFS_WIDE`
    /// environment variables (each a decimal part number; absent or unparsable = stock).
    #[must_use]
    pub fn from_env() -> Self {
        let n = |k: &str| std::env::var(k).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        Self {
            body_kit: n("NFS_KIT"),
            hood_style: n("NFS_STYLE_HOOD"),
            light_style: n("NFS_STYLE_LIGHT"),
            widebody: n("NFS_WIDE"),
        }
    }
}

impl Default for CarConfig {
    fn default() -> Self {
        Self::stock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_is_all_zero_and_is_the_default() {
        assert_eq!(CarConfig::default(), CarConfig::stock());
        let CarConfig { body_kit, hood_style, light_style, widebody } = CarConfig::stock();
        assert_eq!((body_kit, hood_style, light_style, widebody), (0, 0, 0, 0));
    }
}
