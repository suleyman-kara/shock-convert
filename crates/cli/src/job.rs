//! Süren dönüşümün süreç geneli durumu: iptal isteği ve o anki dosyanın ilerlemesi.
//! Bir süreçte aynı anda tek bir iş yürür (sağ tık ya da özel dönüştürme penceresi),
//! bu yüzden basit genel değişkenler yeterlidir.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static CANCEL: AtomicBool = AtomicBool::new(false);
static TRACK: AtomicBool = AtomicBool::new(false);
/// O anki dosyanın ilerlemesi, onbinde bir; `u32::MAX` bilinmiyor.
static PROGRESS: AtomicU32 = AtomicU32::new(u32::MAX);

/// Süren işi durdurmasını ister: harici araçlar sonlandırılır, sıradaki dosyaya geçilmez.
pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

pub fn cancelled() -> bool {
    CANCEL.load(Ordering::Relaxed)
}

/// Yeni iş başlarken: iptal bayrağını indirir; `track` ilerleme ölçümünü açar (ek maliyetli
/// olabildiği için yalnızca birisi izliyorsa).
pub fn start(track: bool) {
    CANCEL.store(false, Ordering::Relaxed);
    TRACK.store(track, Ordering::Relaxed);
    set_progress(None);
}

pub fn tracking() -> bool {
    TRACK.load(Ordering::Relaxed)
}

pub fn set_progress(fraction: Option<f64>) {
    let v = fraction.map_or(u32::MAX, |f| (f.clamp(0.0, 1.0) * 10_000.0) as u32);
    PROGRESS.store(v, Ordering::Relaxed);
}

pub fn progress() -> Option<f64> {
    match PROGRESS.load(Ordering::Relaxed) {
        u32::MAX => None,
        v => Some(f64::from(v) / 10_000.0),
    }
}

pub const CANCELLED: &str = "iptal edildi";
