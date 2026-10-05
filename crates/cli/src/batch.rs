//! Explorer, çoklu seçimde komutu her dosya için ayrı süreçte çalıştırır. Bu modül
//! o süreçleri tek bir toplu işe birleştirir: her süreç yolunu bir kuyruğa yazar,
//! kilidi alan süreç ("lider") kısa bir sessizlik süresi bekleyip tüm yolları tek
//! seferde işler; diğerleri hemen çıkar. Böylece tek bildirim ve paralel işleme olur.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const STALE_LOCK: Duration = Duration::from_secs(120);

fn queue_dir(id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join("shock-convert-queue").join(safe)
}

/// `file`'ı kuyruğa ekler. Lider olursak sırada bekleyen tüm dosyalarla `process`'i çağırır
/// (kuyruk boşalana kadar, gerekirse birden çok kez). Lider değilsek hemen döner.
pub fn submit(
    id: &str,
    file: &Path,
    quiet: Duration,
    mut process: impl FnMut(Vec<PathBuf>),
) -> io::Result<()> {
    let dir = queue_dir(id);
    fs::create_dir_all(&dir)?;
    enqueue(&dir, file)?;

    while try_lock(&dir) {
        let mut seen = 0;
        loop {
            std::thread::sleep(quiet);
            let n = pending(&dir).len();
            if n == seen {
                break; // sessizlik: yeni dosya gelmiyor
            }
            seen = n;
        }
        let files = drain(&dir);
        if !files.is_empty() {
            process(files);
        }
        let _ = fs::remove_file(dir.join("leader.lock"));
        // Kilidi bıraktığımız sırada gelen dosya kalmış olabilir; varsa döngü yeniden lider olur.
        if pending(&dir).is_empty() {
            break;
        }
    }
    Ok(())
}

fn enqueue(dir: &Path, file: &Path) -> io::Result<()> {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!("{}-{nanos}", std::process::id());
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, file.to_string_lossy().as_bytes())?;
    fs::rename(&tmp, dir.join(format!("{name}.q"))) // okuyucu yarım yazılmış dosya görmesin
}

fn pending(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "q"))
                .collect()
        })
        .unwrap_or_default()
}

fn drain(dir: &Path) -> Vec<PathBuf> {
    let mut out = BTreeSet::new();
    for q in pending(dir) {
        if let Ok(text) = fs::read_to_string(&q) {
            out.insert(PathBuf::from(text));
        }
        let _ = fs::remove_file(q);
    }
    out.into_iter().collect()
}

fn try_lock(dir: &Path) -> bool {
    let lock = dir.join("leader.lock");
    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => return true,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&lock)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > STALE_LOCK);
                if !stale {
                    return false;
                }
                let _ = fs::remove_file(&lock); // lider çökmüş
            }
            Err(_) => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn concurrent_processes_are_merged_and_each_file_handled_once() {
        let id = format!("test-{}", std::process::id());
        let batches: Mutex<Vec<Vec<PathBuf>>> = Mutex::new(Vec::new());

        std::thread::scope(|s| {
            for i in 0..12 {
                let (id, batches) = (&id, &batches);
                s.spawn(move || {
                    std::thread::sleep(Duration::from_millis(i * 5));
                    let f = PathBuf::from(format!("C:\\foto {i}.webp"));
                    submit(id, &f, Duration::from_millis(250), |files| {
                        batches.lock().unwrap().push(files)
                    })
                    .unwrap();
                });
            }
        });

        let batches = batches.into_inner().unwrap();
        let all: Vec<&PathBuf> = batches.iter().flatten().collect();
        let unique: BTreeSet<_> = all.iter().collect();
        assert_eq!(
            all.len(),
            12,
            "her dosya tam bir kez işlenmeli: {batches:?}"
        );
        assert_eq!(unique.len(), 12);
        assert!(
            batches.len() <= 2,
            "süreçler birleşmeli: {} toplu iş",
            batches.len()
        );
        let _ = fs::remove_dir_all(queue_dir(&id));
    }
}
