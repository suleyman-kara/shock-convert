use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

/// `dir/ad-ek.uzanti` yolunu üretir; doluysa `ad-ek (1).uzanti`, `(2)` ... dener.
///
/// Seçilen ad, boş bir dosya oluşturularak (`create_new`) atomik biçimde rezerve edilir;
/// böylece paralel işçiler ya da eşzamanlı çalışan iki süreç aynı adı alıp birbirinin
/// üzerine yazamaz. Mevcut dosyaların üzerine hiçbir zaman yazılmaz.
pub fn unique_output_path(input: &Path, suffix: &str, ext: &str) -> io::Result<PathBuf> {
    let dir = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());

    let candidates = std::iter::once(format!("{stem}-{suffix}.{ext}"))
        .chain((1u32..).map(|n| format!("{stem}-{suffix} ({n}).{ext}")));
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
            unique_output_path(&input, "jpg", "jpg").unwrap(),
            d.join("foto-jpg.jpg")
        );
        assert_eq!(
            unique_output_path(&input, "jpg", "jpg").unwrap(),
            d.join("foto-jpg (1).jpg")
        );
        assert_eq!(
            unique_output_path(&input, "jpg", "jpg").unwrap(),
            d.join("foto-jpg (2).jpg")
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_callers_get_distinct_paths() {
        let d = temp_dir("race");
        let input = d.join("foto.png");
        let paths: Vec<PathBuf> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..16)
                .map(|_| s.spawn(|| unique_output_path(&input, "jpg", "jpg").unwrap()))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let unique: std::collections::HashSet<_> = paths.iter().collect();
        assert_eq!(unique.len(), 16);
        let _ = fs::remove_dir_all(&d);
    }
}
