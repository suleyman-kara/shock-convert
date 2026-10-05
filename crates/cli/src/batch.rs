//! Explorer, çoklu seçimde komutu her dosya için ayrı süreçte çalıştırır. Bu modül
//! o süreçleri tek bir toplu işe birleştirir: her süreç yolunu bir kuyruğa yazar,
//! kilidi alan süreç ("lider") kısa bir sessizlik süresi bekleyip tüm yolları tek
//! seferde işler. Diğer süreçler ("takipçi") dosyaları işlenene kadar bekler.
//!
//! Dayanıklılık: lider kilidi periyodik olarak tazeler (kalp atışı). Lider çökerse kilit
//! `STALE_AFTER` sonra bayatlar ve bekleyen bir takipçi devralır. Kuyruk girdileri ancak
//! iş bittikten sonra silinir; böylece yarım kalan iş kaybolmaz.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

const STALE_AFTER: Duration = Duration::from_secs(5);
const HEARTBEAT: Duration = Duration::from_millis(500);
const FOLLOWER_POLL: Duration = Duration::from_millis(150);

fn queue_dir(id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join("shock-convert-queue").join(safe)
}

/// `file`'ı kuyruğa ekler ve işlenene kadar bekler. Lider olursak sırada bekleyen tüm
/// dosyalarla `process`'i çağırır (kuyruk boşalana kadar, gerekirse birden çok kez).
pub fn submit(
    id: &str,
    file: &Path,
    quiet: Duration,
    mut process: impl FnMut(Vec<PathBuf>),
) -> io::Result<()> {
    let dir = queue_dir(id);
    fs::create_dir_all(&dir)?;
    let mine = enqueue(&dir, file)?;

    while mine.exists() {
        if try_lock(&dir) {
            lead(&dir, quiet, &mut process);
        } else {
            std::thread::sleep(FOLLOWER_POLL);
        }
    }
    Ok(())
}

fn lead(dir: &Path, quiet: Duration, process: &mut impl FnMut(Vec<PathBuf>)) {
    let lock = dir.join("leader.lock");
    let stop = Arc::new(AtomicBool::new(false));
    let heartbeat = {
        let (stop, lock) = (Arc::clone(&stop), lock.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let _ = OpenOptions::new()
                    .write(true)
                    .open(&lock)
                    .and_then(|f| f.set_modified(SystemTime::now()));
                std::thread::sleep(HEARTBEAT);
            }
        })
    };

    loop {
        // Sessizlik: yeni dosya gelmeyene kadar bekle.
        let mut seen = 0;
        loop {
            std::thread::sleep(quiet);
            let n = pending(dir).len();
            if n == seen {
                break;
            }
            seen = n;
        }
        let batch = read_all(dir);
        if batch.is_empty() {
            break;
        }
        let files: Vec<PathBuf> = batch
            .iter()
            .map(|(_, f)| f.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        process(files);
        for (q, _) in &batch {
            let _ = fs::remove_file(q);
        }
        if pending(dir).is_empty() {
            break;
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = heartbeat.join();
    let _ = fs::remove_file(&lock);
}

/// Girdiyi önce `.tmp` olarak yazıp yeniden adlandırır; okuyucu yarım dosya görmesin.
fn enqueue(dir: &Path, file: &Path) -> io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!("{}-{nanos}", std::process::id());
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, file.to_string_lossy().as_bytes())?;
    let q = dir.join(format!("{name}.q"));
    fs::rename(&tmp, &q)?;
    Ok(q)
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

/// Kuyruktaki (girdi dosyası, dosya yolu) çiftleri; okunamayan girdiler atılır.
fn read_all(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    for q in pending(dir) {
        match fs::read_to_string(&q) {
            Ok(text) => out.push((q, PathBuf::from(text))),
            Err(_) => {
                let _ = fs::remove_file(&q);
            }
        }
    }
    out
}

fn lock_is_stale(lock: &Path) -> bool {
    fs::metadata(lock)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > STALE_AFTER)
}

/// Kilidi almayı dener. Bayat (kalp atışı kesilmiş) kilit çökmüş lider demektir; silinip devralınır.
/// Not: iki takipçi aynı anda bayat kilidi görürse nadiren ikisi de lider olabilir; bu yalnızca
/// çökme sonrası olur ve en kötü durumda aynı dosya iki kez dönüştürülür (`(1)` ekiyle).
fn try_lock(dir: &Path) -> bool {
    let lock = dir.join("leader.lock");
    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => return true,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if !lock_is_stale(&lock) {
                    return false;
                }
                let _ = fs::remove_file(&lock);
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

    fn unique_id(tag: &str) -> String {
        format!("test-{tag}-{}", std::process::id())
    }

    #[test]
    fn concurrent_processes_are_merged_and_done_before_return() {
        let id = unique_id("merge");
        let processed: Mutex<Vec<Vec<PathBuf>>> = Mutex::new(Vec::new());

        std::thread::scope(|s| {
            for i in 0..12u64 {
                let (id, processed) = (&id, &processed);
                s.spawn(move || {
                    std::thread::sleep(Duration::from_millis(i * 5));
                    let f = PathBuf::from(format!("C:\\foto {i}.webp"));
                    submit(id, &f, Duration::from_millis(250), |files| {
                        processed.lock().unwrap().push(files);
                    })
                    .unwrap();
                    // Takipçi de dönmeden önce dosyasının işlenmiş olduğunu görmeli.
                    let done = processed.lock().unwrap();
                    assert!(
                        done.iter().flatten().any(|p| *p == f),
                        "{f:?} işlenmeden döndü"
                    );
                });
            }
        });

        let batches = processed.into_inner().unwrap();
        let all: Vec<&PathBuf> = batches.iter().flatten().collect();
        let unique: BTreeSet<_> = all.iter().collect();
        assert_eq!(
            all.len(),
            12,
            "her dosya tam bir kez işlenmeli: {batches:?}"
        );
        assert_eq!(unique.len(), 12);
        assert!(batches.len() <= 2, "{} toplu iş", batches.len());
        let _ = fs::remove_dir_all(queue_dir(&id));
    }

    #[test]
    fn follower_takes_over_when_leader_died() {
        let id = unique_id("dead");
        let dir = queue_dir(&id);
        fs::create_dir_all(&dir).unwrap();
        // Çökmüş lider: eski kilit + işlenmemiş girdi.
        let lock = dir.join("leader.lock");
        let f = fs::File::create(&lock).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
        fs::write(dir.join("1-1.q"), "C:\\yetim.png").unwrap();

        let mut got: Vec<PathBuf> = Vec::new();
        submit(
            &id,
            Path::new("C:\\yeni.png"),
            Duration::from_millis(100),
            |files| {
                got.extend(files);
            },
        )
        .unwrap();

        got.sort();
        assert_eq!(
            got,
            [
                PathBuf::from("C:\\yeni.png"),
                PathBuf::from("C:\\yetim.png")
            ]
        );
        assert!(!lock.exists() && pending(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
