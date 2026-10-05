//! Platforma bağlı kısımlar. Windows dışında yalnızca derleme/test için taslaklar vardır.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::*;
