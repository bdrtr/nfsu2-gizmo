//! Pure, engine-free classification of NFSU2 car parts.
//!
//! This module owns the *presentation policy* for a car's geometry: which material group a
//! part belongs to ([`group`]), what its name says about the variant it belongs to
//! ([`name`]), which configuration the car is in ([`config`]) and, from all three, which
//! parts make up that car ([`select`]). It touches no engine or GPU types — only part names
//! and triangle counts — so it stays trivially unit-testable and reusable across every
//! binary. Keep it that way.

mod config;
mod group;
mod name;
mod select;

pub use config::CarConfig;
pub use group::{group_of, Grp};
pub use name::component_key;
pub use select::{select_car, select_stock_car};
