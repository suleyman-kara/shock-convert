use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

/// Rezerve edilmiş bir çıktı adı. Dönüştürücü [`path`](Self::path) üzerindeki **geçici** dosyaya
/// (`foto.part.jpg`) yazar; iş bitince [`commit`](Self::commit) onu asıl ada (`foto.jpg`) taşır.
/// Commit edilmeden düşürülürse (hata) geçici dosya silinir. Süreç zorla kapatılsa bile asıl
/// adda yarım ya da boş bir dosya kalmaz; geride kalan, adından belli bir `.part` dosyasıdır.
#[derive(Debug)]
pub struct Reserved {
    temp: PathBuf,
    target: PathBuf,
    committed: bool,
}

impl Reserved {
    /// Yazılacak geçici dosya (uzantısı asıl uzantıyla aynı; ffmpeg gibi araçlar biçimi buradan anlar).
    pub fn path(&self) -> &Path {
        &self.temp
    }

    /// İş bitince dosyanın alacağı ad.
    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Geçici dosyayı asıl ada taşır; o ada bu arada bir dosya gelmişse **üzerine yazmaz**.
    pub fn commit(mut self) -> io::Result<PathBuf> {
        move_no_overwrite(&self.temp, &self.target)?;
        self.committed = true;
        Ok(self.target.clone())
    }
}

impl Drop for Reserved {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

/// Üzerine yazmadan taşır: önce sabit bağlantı (hedef varsa atomik olarak başarısız olur),
/// desteklenmeyen dosya sistemlerinde (FAT/exFAT, bazı ağ paylaşımları) denetleyip yeniden adlandırır.
/// Windows'ta virüs tarayıcısı ya da önizleme dosyayı kısa süre tutabildiği için birkaç kez denenir.
fn move_no_overwrite(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = None;
    for attempt in 0..10 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        match std::fs::hard_link(from, to) {
            Ok(()) => {
                let _ = std::fs::remove_file(from);
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} bu sırada başka bir dosyaca alındı", to.display()),
                ));
            }
            Err(_) if to.exists() => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} bu sırada başka bir dosyaca alındı", to.display()),
                ));
            }
            Err(_) => match std::fs::rename(from, to) {
                Ok(()) => return Ok(()),
                Err(e) => last = Some(e),
            },
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("çıktı taşınamadı")))
}

/// `dir/ad.uzanti` (ek verilirse `dir/ad-ek.uzanti`) adını rezerve eder; doluysa `ad (1).uzanti`,
/// `(2)` ... dener. Mevcut dosyaların üzerine hiçbir zaman yazılmaz.
///
/// Rezervasyon, geçici dosya (`ad.part.uzanti`) `create_new` ile oluşturularak atomik yapılır;
/// böylece paralel işçiler ya da eşzamanlı iki süreç aynı adı alamaz. Asıl adda ya da geçici
/// adda dosya varsa (ör. başka bir dönüşüm sürüyor) sıradaki ad denenir.
pub fn reserve_output(input: &Path, suffix: Option<&str>, ext: &str) -> io::Result<Reserved> {
    let dir = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());

    // Boş ek "ek yok" demektir.
    let base = match suffix.filter(|s| !s.is_empty()) {
        Some(sfx) => format!("{stem}-{sfx}"),
        None => stem,
    };
    let names = std::iter::once(base.clone()).chain((1u32..).map(|n| format!("{base} ({n})")));
    for name in names {
        let target = dir.join(format!("{name}.{ext}"));
        if target.exists() {
            continue;
        }
        let temp = dir.join(format!("{name}.part.{ext}"));
        remove_if_stale(&temp);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
        // Denetim ile rezervasyon arasında asıl ad alınmış olabilir.
        if target.exists() {
            let _ = std::fs::remove_file(&temp);
            continue;
        }
        return Ok(Reserved {
            temp,
            target,
            committed: false,
        });
    }
    unreachable!("sonsuz aralıkta boş ad bulunur")
}

/// Çöken bir dönüşümden kalan geçici dosya adı sonsuza dek tutmasın. Tüm araçlarımız ya sürekli
/// yazar (ffmpeg) ya da en geç birkaç dakikada zaman aşımına uğrar; bir saattir dokunulmamış
/// `.part` dosyası artık kimseye ait değildir.
fn remove_if_stale(temp: &Path) {
    const STALE: std::time::Duration = std::time::Duration::from_secs(60 * 60);
    let old = std::fs::metadata(temp)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > STALE);
    if old {
        let _ = std::fs::remove_file(temp);
    }
}

/// Rezerve edip yazar ve asıl ada taşır: tek seferde bayt yazan çağıranlar için.
pub fn write_output(
    input: &Path,
    suffix: Option<&str>,
    ext: &str,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    let r = reserve_output(input, suffix, ext)?;
    std::fs::write(r.path(), bytes)?;
    r.commit()
}

/// Çıktının tarihlerini kaynağınkiyle aynı yapar: değiştirilme ve erişim; Windows'ta oluşturulma da.
/// Böylece dönüştürülen fotoğraflar arşivde ve "tarihe göre sırala"da yerini korur.
pub fn copy_file_times(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = std::fs::metadata(src)?;
    let mut times = std::fs::FileTimes::new().set_modified(meta.modified()?);
    if let Ok(accessed) = meta.accessed() {
        times = times.set_accessed(accessed);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTimesExt;
        if let Ok(created) = meta.created() {
            times = times.set_created(created);
        }
    }
    OpenOptions::new().write(true).open(dst)?.set_times(times)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shock-convert-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn done(input: &Path, suffix: Option<&str>, ext: &str) -> PathBuf {
        write_output(input, suffix, ext, b"x").unwrap()
    }

    #[test]
    fn appends_suffix_and_never_overwrites() {
        let d = temp_dir("output");
        let input = d.join("foto.png");
        assert_eq!(done(&input, None, "jpg"), d.join("foto.jpg"));
        assert_eq!(done(&input, None, "jpg"), d.join("foto (1).jpg"));
        assert_eq!(done(&input, None, "jpg"), d.join("foto (2).jpg"));
        // Ek verilirse ada eklenir; boş ek "yok" sayılır.
        assert_eq!(done(&input, Some("web"), "jpg"), d.join("foto-web.jpg"));
        assert_eq!(done(&input, Some(""), "png"), d.join("foto.png"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn writes_go_to_part_file_and_failed_jobs_leave_nothing() {
        let d = temp_dir("part");
        let input = d.join("video.mp4");
        let r = reserve_output(&input, None, "mp3").unwrap();
        assert_eq!(r.path(), d.join("video.part.mp3"));
        assert_eq!(r.target(), d.join("video.mp3"));
        assert!(
            !d.join("video.mp3").exists(),
            "asıl adda boş dosya olmamalı"
        );
        // Sürmekte olan dönüşüm varken ikinci iş başka ad alır.
        let r2 = reserve_output(&input, None, "mp3").unwrap();
        assert_eq!(r2.target(), d.join("video (1).mp3"));
        drop(r2); // hata: geçici dosya silinir
        assert!(!d.join("video (1).part.mp3").exists());
        fs::write(r.path(), b"ses").unwrap();
        let out = r.commit().unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"ses");
        assert!(!d.join("video.part.mp3").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn stale_part_file_from_a_crash_is_reused_but_a_fresh_one_is_not() {
        let d = temp_dir("stale");
        let input = d.join("v.mp4");
        fs::write(d.join("v.part.mp3"), b"").unwrap();
        // Taze: başka bir iş sürüyor olabilir, dokunulmaz.
        assert_eq!(done(&input, None, "mp3"), d.join("v (1).mp3"));
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
        fs::write(d.join("w.part.mp3"), b"").unwrap();
        fs::File::options()
            .write(true)
            .open(d.join("w.part.mp3"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(done(&d.join("w.mp4"), None, "mp3"), d.join("w.mp3"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn commit_never_overwrites_a_file_that_appeared_meanwhile() {
        let d = temp_dir("appeared");
        let input = d.join("a.png");
        let r = reserve_output(&input, None, "jpg").unwrap();
        fs::write(d.join("a.jpg"), b"kullanicinin").unwrap();
        fs::write(r.path(), b"bizim").unwrap();
        assert!(r.commit().is_err());
        assert_eq!(fs::read(d.join("a.jpg")).unwrap(), b"kullanicinin");
        assert!(!d.join("a.part.jpg").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_callers_get_distinct_paths() {
        let d = temp_dir("race");
        let input = d.join("foto.png");
        let paths: Vec<PathBuf> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..16)
                .map(|_| s.spawn(|| done(&input, None, "jpg")))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let unique: std::collections::HashSet<_> = paths.iter().collect();
        assert_eq!(unique.len(), 16);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn output_gets_source_modification_time() {
        let d = temp_dir("times");
        let src = d.join("eski.png");
        fs::write(&src, b"x").unwrap();
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
        fs::File::options()
            .write(true)
            .open(&src)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let dst = d.join("eski.jpg");
        fs::write(&dst, b"y").unwrap();
        copy_file_times(&src, &dst).unwrap();
        assert_eq!(fs::metadata(&dst).unwrap().modified().unwrap(), old);
        let _ = fs::remove_dir_all(&d);
    }
}
