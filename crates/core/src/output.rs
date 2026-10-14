use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

/// `dir/ad.uzanti` (ek verilirse `dir/ad-ek.uzanti`) yolunu üretir; doluysa `ad (1).uzanti`,
/// `(2)` ... dener.
///
/// Seçilen ad, boş bir dosya oluşturularak (`create_new`) atomik biçimde rezerve edilir;
/// böylece paralel işçiler ya da eşzamanlı çalışan iki süreç aynı adı alıp birbirinin
/// üzerine yazamaz. Mevcut dosyaların üzerine hiçbir zaman yazılmaz.
pub fn unique_output_path(input: &Path, suffix: Option<&str>, ext: &str) -> io::Result<PathBuf> {
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
    let candidates = std::iter::once(format!("{base}.{ext}"))
        .chain((1u32..).map(|n| format!("{base} ({n}).{ext}")));
    for name in candidates {
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("sonsuz aralıkta boş ad bulunur")
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

    #[test]
    fn appends_suffix_and_never_overwrites() {
        let d = temp_dir("output");
        let input = d.join("foto.png");
        assert_eq!(
            unique_output_path(&input, None, "jpg").unwrap(),
            d.join("foto.jpg")
        );
        assert_eq!(
            unique_output_path(&input, None, "jpg").unwrap(),
            d.join("foto (1).jpg")
        );
        assert_eq!(
            unique_output_path(&input, None, "jpg").unwrap(),
            d.join("foto (2).jpg")
        );
        // Ek verilirse ada eklenir; boş ek "yok" sayılır.
        assert_eq!(
            unique_output_path(&input, Some("web"), "jpg").unwrap(),
            d.join("foto-web.jpg")
        );
        assert_eq!(
            unique_output_path(&input, Some(""), "png").unwrap(),
            d.join("foto.png")
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_callers_get_distinct_paths() {
        let d = temp_dir("race");
        let input = d.join("foto.png");
        let paths: Vec<PathBuf> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..16)
                .map(|_| s.spawn(|| unique_output_path(&input, None, "jpg").unwrap()))
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
