//! EXIF (TIFF yapısı) üzerinde küçük, yerinde düzenlemeler. Bloğun boyu ve hiçbir ofset
//! değişmez; böylece üretici notları (MakerNote) gibi ofsetle bağlı veriler bozulmaz.

/// IFD0'daki GPS IFD işaretçisi.
const GPS_IFD_TAG: u16 = 0x8825;

#[derive(Clone, Copy)]
struct Tiff<'a> {
    data: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.data.get(at..at + 2)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.data.get(at..at + 4)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
}

fn type_size(t: u16) -> Option<usize> {
    Some(match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

/// Konum bilgisini (GPS IFD) siler: GPS değerlerinin baytları sıfırlanır ve GPS IFD'si boş
/// (0 girdili) bırakılır. Bozuk ya da tanınmayan blokta hiçbir şey değiştirilmez ve `false` döner.
pub fn strip_gps(exif: &mut [u8]) -> bool {
    let Some(le) = (match exif.get(..4) {
        Some(b"II*\0") => Some(true),
        Some(b"MM\0*") => Some(false),
        _ => None,
    }) else {
        return false;
    };
    let t = Tiff { data: exif, le };
    let Some(gps) = find_gps_ifd(t) else {
        return false;
    };
    // Önce silinecek aralıkları topla (okuma ve yazma ayrı olsun).
    let Some(count) = t.u16(gps) else {
        return false;
    };
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for i in 0..count as usize {
        let entry = gps + 2 + i * 12;
        let (Some(typ), Some(n)) = (t.u16(entry + 2), t.u32(entry + 4)) else {
            return false;
        };
        let Some(len) = type_size(typ).and_then(|s| s.checked_mul(n as usize)) else {
            continue;
        };
        if len > 4 {
            let Some(off) = t.u32(entry + 8) else {
                return false;
            };
            let off = off as usize;
            if off.checked_add(len).is_some_and(|end| end <= exif.len()) {
                ranges.push((off, len));
            }
        }
        ranges.push((entry, 12));
    }
    for (start, len) in ranges {
        exif[start..start + len].fill(0);
    }
    // Girdi sayısı 0: okuyucular boş bir GPS bölümü görür.
    exif[gps..gps + 2].fill(0);
    true
}

fn find_gps_ifd(t: Tiff) -> Option<usize> {
    let ifd0 = t.u32(4)? as usize;
    let count = t.u16(ifd0)? as usize;
    (0..count).find_map(|i| {
        let entry = ifd0 + 2 + i * 12;
        (t.u16(entry)? == GPS_IFD_TAG)
            .then(|| t.u32(entry + 8))
            .flatten()
            .map(|o| o as usize)
            .filter(|&o| o + 2 <= t.data.len())
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// IFD0: Make = "Abc" ve GPS işaretçisi; GPS IFD: enlem referansı "N" (satır içi) ve enlem
    /// (3 RATIONAL = 24 bayt, ofsetle).
    pub(crate) fn exif_with_gps(le: bool) -> Vec<u8> {
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut e: Vec<u8> = if le {
            b"II*\0".to_vec()
        } else {
            b"MM\0*".to_vec()
        };
        e.extend(w32(8));
        // IFD0 @8: 2 girdi
        e.extend(w16(2));
        e.extend(w16(0x010f));
        e.extend(w16(2));
        e.extend(w32(4));
        e.extend(*b"Abc\0");
        e.extend(w16(GPS_IFD_TAG));
        e.extend(w16(4));
        e.extend(w32(1));
        e.extend(w32(38)); // GPS IFD ofseti
        e.extend(w32(0)); // sonraki IFD yok
        assert_eq!(e.len(), 38);
        // GPS IFD @38: 2 girdi
        e.extend(w16(2));
        e.extend(w16(0x0001)); // GPSLatitudeRef
        e.extend(w16(2));
        e.extend(w32(2));
        e.extend(*b"N\0\0\0");
        e.extend(w16(0x0002)); // GPSLatitude
        e.extend(w16(5));
        e.extend(w32(3));
        e.extend(w32(68)); // değerlerin ofseti
        e.extend(w32(0));
        assert_eq!(e.len(), 68);
        for (num, den) in [(41u32, 1u32), (2, 1), (1234, 100)] {
            e.extend(w32(num));
            e.extend(w32(den));
        }
        e
    }

    #[test]
    fn gps_values_are_zeroed_and_rest_is_untouched() {
        for le in [true, false] {
            let mut e = exif_with_gps(le);
            let before = e.clone();
            assert!(strip_gps(&mut e));
            assert_eq!(e.len(), before.len(), "boy değişmemeli");
            assert_eq!(&e[..38], &before[..38], "IFD0 (Make dahil) değişmemeli");
            assert!(e[38..].iter().all(|&b| b == 0), "GPS bölümü tamamen sıfır");
        }
    }

    #[test]
    fn exif_without_gps_or_garbage_is_left_alone() {
        let mut garbage = b"bu exif degil".to_vec();
        assert!(!strip_gps(&mut garbage));
        let mut e = exif_with_gps(true);
        e[22..24].copy_from_slice(&0x1234u16.to_le_bytes()); // GPS işaretçisini başka etiket yap
        let before = e.clone();
        assert!(!strip_gps(&mut e));
        assert_eq!(e, before);
    }
}
