//! Süren dönüşümün iptal isteği. Bir süreçte aynı anda tek bir iş yürür (sağ tık ya da özel
//! dönüştürme penceresi), bu yüzden basit bir genel değişken yeterlidir.

use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Süren işin sıradaki dosyaya geçmemesini ister.
pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

pub fn cancelled() -> bool {
    CANCEL.load(Ordering::Relaxed)
}

/// Yeni iş başlarken iptal bayrağını indirir.
pub fn start() {
    CANCEL.store(false, Ordering::Relaxed);
}
