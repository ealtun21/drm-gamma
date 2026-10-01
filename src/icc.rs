//! ICC profile export: sRGB display profile + `vcgt` calibration curve.
//!
//! For compositors that own the gamma LUT and reset it (KDE, GNOME), the
//! persistent path is an ICC profile the compositor applies itself.

/// lcms2-generated sRGB v4 profile (no vcgt).
const SRGB: &[u8] = include_bytes!("srgb.icc");

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

/// Offset of tag `sig` in `SRGB`.
fn tag(sig: &[u8; 4]) -> usize {
    let count = be32(SRGB, 128) as usize;
    (0..count)
        .map(|i| 132 + 12 * i)
        .find(|&e| &SRGB[e..e + 4] == sig)
        .map(|e| be32(SRGB, e + 4) as usize)
        .expect("tag in srgb.icc")
}

/// rXYZ/gXYZ/bXYZ moved toward (s < 1 → away from) white by 1/s. A colour-managed
/// compositor maps sRGB onto these narrower primaries, i.e. saturates by s.
/// White (their sum) stays put.
fn scale_primaries(profile: &mut [u8], shift: usize, saturation: f64) {
    let sigs = [b"rXYZ", b"gXYZ", b"bXYZ"];
    let read = |p: &[u8], at: usize| be32(p, at) as i32 as f64 / 65536.0;
    let cols = sigs.map(|s| {
        let at = tag(s) + 8;
        [0, 1, 2].map(|k| read(SRGB, at + 4 * k))
    });
    let white: [f64; 3] = std::array::from_fn(|k| cols.iter().map(|c| c[k]).sum());
    // ponytail: ICC can't express true grayscale (primaries → ∞); floor at 0.1.
    let t = 1.0 / saturation.max(0.1);
    for (sig, col) in sigs.iter().zip(cols) {
        let at = tag(sig) + 8 - shift;
        let weight = col[1] / white[1]; // luminance share of this primary
        for k in 0..3 {
            let v = t * col[k] + (1.0 - t) * weight * white[k];
            let fixed = (v * 65536.0).round() as i32;
            profile[at + 4 * k..at + 4 * k + 4].copy_from_slice(&fixed.to_be_bytes());
        }
    }
}

/// Build an sRGB ICC profile whose `vcgt` tag holds the given per-channel LUTs,
/// with primaries adjusted for `saturation` (1.0 = plain sRGB).
pub fn build_profile(red: &[u16], green: &[u16], blue: &[u16], saturation: f64) -> Vec<u8> {
    let count = be32(SRGB, 128) as usize;
    let data_start = 132 + 12 * count;

    let mut vcgt = b"vcgt\0\0\0\0".to_vec();
    vcgt.extend(0u32.to_be_bytes()); // table type
    vcgt.extend(3u16.to_be_bytes()); // channels
    vcgt.extend((red.len() as u16).to_be_bytes());
    vcgt.extend(2u16.to_be_bytes()); // bytes per entry
    for v in red.iter().chain(green).chain(blue) {
        vcgt.extend(v.to_be_bytes());
    }

    // Swap `chrm` for `vcgt`: same table size, so data offsets stay valid.
    // KWin takes primaries from `chrm` over rXYZ/gXYZ/bXYZ, and it's optional.
    let mut out = SRGB[..132].to_vec();
    for i in 0..count {
        let e = 132 + 12 * i;
        if &SRGB[e..e + 4] != b"chrm" {
            out.extend(&SRGB[e..e + 12]);
        }
    }
    assert_eq!(out.len(), data_start - 12, "srgb.icc has a chrm tag");
    let mut data = SRGB[data_start..].to_vec();
    if saturation != 1.0 {
        scale_primaries(&mut data, data_start, saturation);
    }
    data.resize((data.len() + 3) & !3, 0);
    let vcgt_off = data_start + data.len();
    out.extend(b"vcgt");
    out.extend((vcgt_off as u32).to_be_bytes());
    out.extend((vcgt.len() as u32).to_be_bytes());
    out.extend(data);
    out.extend(vcgt);

    let len = out.len() as u32;
    out[0..4].copy_from_slice(&len.to_be_bytes());
    out[84..100].fill(0); // profile ID (MD5) no longer valid; zero = unset
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_layout() {
        let lut: Vec<u16> = (0..256).map(|i| (i * 257) as u16).collect();
        let p = build_profile(&lut, &lut, &lut, 1.0);
        assert_eq!(be32(&p, 0) as usize, p.len());
        assert_eq!(&p[36..40], b"acsp");
        let n = be32(&p, 128) as usize;
        assert_eq!(n, be32(SRGB, 128) as usize);
        let sigs: Vec<_> = (0..n).map(|i| &p[132 + 12 * i..136 + 12 * i]).collect();
        assert!(!sigs.contains(&&b"chrm"[..]));
        assert_eq!(sigs[n - 1], b"vcgt");
        for (i, sig) in sigs.iter().enumerate() {
            let e = 132 + 12 * i;
            let (off, size) = (be32(&p, e + 4) as usize, be32(&p, e + 8) as usize);
            assert!(off % 4 == 0 && off + size <= p.len());
            // Tag data starts with its type; for XYZ/vcgt that's distinct per sig.
            if sig == b"vcgt" {
                assert_eq!(&p[off..off + 4], b"vcgt");
            } else {
                assert_eq!(&p[off..off + 4], &SRGB[off..off + 4]);
            }
        }
        let vcgt = be32(&p, 132 + 12 * (n - 1) + 4) as usize;
        assert_eq!(
            &p[vcgt + 18 + 2 * 255..vcgt + 18 + 2 * 256],
            &lut[255].to_be_bytes()
        );
    }

    #[test]
    fn test_saturation_primaries() {
        let lut = [0u16; 2];
        let xyz = |p: &[u8], sig| {
            let at = be32(
                p,
                132 + 12
                    * (0..20)
                        .find(|i| &p[132 + 12 * i..136 + 12 * i] == sig)
                        .unwrap()
                    + 4,
            ) as usize;
            [0, 1, 2].map(|k| be32(p, at + 8 + 4 * k) as i32)
        };
        let plain = build_profile(&lut, &lut, &lut, 1.0);
        for s in [0.5, 1.5] {
            let p = build_profile(&lut, &lut, &lut, s);
            let sum = |p: &[u8]| -> [i32; 3] {
                let c = [b"rXYZ", b"gXYZ", b"bXYZ"].map(|t| xyz(p, t));
                std::array::from_fn(|k| c.iter().map(|v| v[k]).sum())
            };
            // White point preserved (±rounding), red primary moved.
            let (a, b) = (sum(&plain), sum(&p));
            assert!((0..3).all(|k| (a[k] - b[k]).abs() <= 2));
            assert_ne!(xyz(&plain, b"rXYZ"), xyz(&p, b"rXYZ"));
        }
        // s>1 pulls red toward white: less X-dominant.
        let r = xyz(&build_profile(&lut, &lut, &lut, 1.5), b"rXYZ");
        assert!(r[0] < xyz(&plain, b"rXYZ")[0]);
    }
}
