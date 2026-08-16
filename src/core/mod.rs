//! GUI'den bağımsız çekirdek: yollar, yapılandırma, günlükleme ve save işlemleri.

pub mod config;
pub mod error;
pub mod log;
pub mod ops;
pub mod paths;
pub mod platform;

pub use error::OpError;
