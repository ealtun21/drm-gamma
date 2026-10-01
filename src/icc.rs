//! ICC profile export: sRGB display profile + `vcgt` calibration curve.
//!
//! For compositors that own the gamma LUT and reset it (KDE, GNOME), the
//! persistent path is an ICC profile the compositor applies itself.

/// lcms2-generated sRGB v4 profile (no vcgt).
const SRGB: &[u8] = include_bytes!("srgb.icc");

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

/// Build an sRGB ICC profile whose `vcgt` tag holds the given per-channel LUTs.
pub fn build_profile(red: &[u16], green: &[u16], blue: &[u16]) -> Vec<u8> {
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

    // New table is one entry longer: shift every existing offset by 12.
    let mut out = SRGB[..128].to_vec();
    out.extend((count as u32 + 1).to_be_bytes());
    for i in 0..count {
        let e = 132 + 12 * i;
        out.extend(&SRGB[e..e + 4]);
        out.extend((be32(SRGB, e + 4) + 12).to_be_bytes());
        out.extend(&SRGB[e + 8..e + 12]);
    }
    let mut data = SRGB[data_start..].to_vec();
    data.resize((data.len() + 3) & !3, 0);
    let vcgt_off = data_start + 12 + data.len();
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
        let p = build_profile(&lut, &lut, &lut);
        assert_eq!(be32(&p, 0) as usize, p.len());
        assert_eq!(&p[36..40], b"acsp");
        let n = be32(&p, 128) as usize;
        assert_eq!(n, be32(SRGB, 128) as usize + 1);
        for i in 0..n {
            let e = 132 + 12 * i;
            let (off, size) = (be32(&p, e + 4) as usize, be32(&p, e + 8) as usize);
            assert!(off % 4 == 0 && off + size <= p.len());
            let expect = if i + 1 < n {
                let o = be32(SRGB, e + 4) as usize; // original entry i
                &SRGB[o..o + 4]
            } else {
                &b"vcgt"[..]
            };
            assert_eq!(&p[off..off + 4], expect);
        }
        let vcgt = be32(&p, 132 + 12 * (n - 1) + 4) as usize;
        assert_eq!(
            &p[vcgt + 18 + 2 * 255..vcgt + 18 + 2 * 256],
            &lut[255].to_be_bytes()
        );
    }
}
