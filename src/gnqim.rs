// Copyright © 2026 Japality Limited. All rights reserved.
// Licensed under the GNU General Public License v3.0.
// See LICENSE file in the project root for full license information.

/// GN-QIM: Generalized Nonlinear Quantization Index Modulation.
///
/// Core algorithm:
///   1. Split image into 8×8 blocks, apply DCT-II
///   2. For each block, extract coefficient at position (1,1)
///   3. Power-law transform: T(c) = sgn(c)·|c|^p
///   4. QIM embed bit in transformed domain
///   5. Inverse power-law: sgn(t)·|t|^(1/p)
///   6. IDCT, reconstruct image
use crate::dct::{DctBasis, N};

const DCT_ROW: usize = 1;
const DCT_COL: usize = 1;
const EPS: f64 = 1e-10;
const MAGIC: [u8; 4] = [b'G', b'N', b'Q', b'M'];
const HEADER_BYTES: usize = 12; // magic(4) + len(4) + crc(4)

/// Algorithm parameters.
#[derive(Debug, Clone)]
pub struct Params {
    /// Power-law exponent. 0.884 optimal for JPEG, 1.0 = standard QIM.
    pub p: f64,
    /// Quantization step in the transformed domain.
    pub q: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self { p: 0.884, q: 24.3 }
    }
}

// ── Power-law transforms ────────────────────────────────────────────

#[inline]
fn power_fwd(c: f64, p: f64) -> f64 {
    c.signum() * (c.abs() + EPS).powf(p)
}

#[inline]
fn power_inv(t: f64, p: f64) -> f64 {
    t.signum() * (t.abs() + EPS).powf(1.0 / p)
}

// ── QIM quantization ────────────────────────────────────────────────

#[inline]
fn qim_level0(ct: f64, q: f64) -> f64 {
    q * (ct / q).round()
}

#[inline]
fn qim_level1(ct: f64, q: f64) -> f64 {
    q * ((ct - q * 0.5) / q).round() + q * 0.5
}

// ── CRC-32 (IEEE 802.3) ────────────────────────────────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = if (crc & 1) != 0 { 0xEDB8_8320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

// ── Bit packing helpers ─────────────────────────────────────────────

fn bytes_to_bits(data: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(data.len() * 8);
    for &byte in data {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1);
        }
    }
    bits
}

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|chunk| {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                byte |= (bit & 1) << (7 - i);
            }
            byte
        })
        .collect()
}

// ── Block operations ────────────────────────────────────────────────

fn image_to_blocks(gray: &[f64], w: usize, h: usize) -> Vec<[[f64; N]; N]> {
    let bw = w / N;
    let bh = h / N;
    let mut blocks = Vec::with_capacity(bw * bh);
    for by in 0..bh {
        for bx in 0..bw {
            let mut block = [[0.0f64; N]; N];
            for r in 0..N {
                for c in 0..N {
                    block[r][c] = gray[(by * N + r) * w + bx * N + c];
                }
            }
            blocks.push(block);
        }
    }
    blocks
}

fn blocks_to_image(blocks: &[[[f64; N]; N]], w: usize, h: usize) -> Vec<u8> {
    let bw = w / N;
    let _bh = h / N;
    let mut out = vec![0u8; w * h];
    for (idx, block) in blocks.iter().enumerate() {
        let bx = idx % bw;
        let by = idx / bw;
        for r in 0..N {
            for c in 0..N {
                let v = block[r][c].round().clamp(0.0, 255.0) as u8;
                out[(by * N + r) * w + bx * N + c] = v;
            }
        }
    }
    out
}

// ── Public API ──────────────────────────────────────────────────────

/// Embed payload into a grayscale image. Returns (stego_pixels, watermark_bits).
pub fn embed(
    gray: &[u8],
    width: usize,
    height: usize,
    payload: &[u8],
    params: &Params,
) -> Result<Vec<u8>, String> {
    let basis = DctBasis::new();
    let w = width - (width % N);
    let h = height - (height % N);

    // Frame: [GNQM][len_u32][crc32][payload]
    let mut framed = Vec::with_capacity(HEADER_BYTES + payload.len());
    framed.extend_from_slice(&MAGIC);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(&crc32(payload).to_be_bytes());
    framed.extend_from_slice(payload);

    let bits = bytes_to_bits(&framed);

    let num_blocks = (w / N) * (h / N);
    if bits.len() > num_blocks {
        let max_payload = if num_blocks > HEADER_BYTES * 8 {
            (num_blocks - HEADER_BYTES * 8) / 8
        } else {
            0
        };
        return Err(format!(
            "Payload too large: need {} blocks, have {} ({} bytes max)",
            bits.len(),
            num_blocks,
            max_payload
        ));
    }

    // Convert to f64
    let gray_f: Vec<f64> = gray.iter().map(|&v| v as f64).collect();
    let mut blocks = image_to_blocks(&gray_f, w, h);

    // Embed bits
    for (i, block) in blocks.iter_mut().enumerate() {
        if i >= bits.len() {
            break;
        }
        let bit = bits[i];

        let mut dct = basis.forward(block);
        let c = dct[DCT_ROW][DCT_COL];

        // Forward power-law
        let ct = power_fwd(c, params.p);

        // QIM embed
        let ct_emb = if bit == 0 {
            qim_level0(ct, params.q)
        } else {
            qim_level1(ct, params.q)
        };

        // Inverse power-law
        dct[DCT_ROW][DCT_COL] = power_inv(ct_emb, params.p);

        *block = basis.inverse(&dct);
    }

    Ok(blocks_to_image(&blocks, w, h))
}

/// Extract payload from a stego grayscale image.
pub fn extract(
    gray: &[u8],
    width: usize,
    height: usize,
    params: &Params,
) -> Result<Vec<u8>, String> {
    let basis = DctBasis::new();
    let w = width - (width % N);
    let h = height - (height % N);

    let gray_f: Vec<f64> = gray.iter().map(|&v| v as f64).collect();
    let blocks = image_to_blocks(&gray_f, w, h);

    // Extract all available bits
    let mut raw_bits: Vec<u8> = Vec::with_capacity(blocks.len());
    for block in &blocks {
        let dct = basis.forward(block);
        let c = dct[DCT_ROW][DCT_COL];
        let ct = power_fwd(c, params.p);

        let d0 = (ct - qim_level0(ct, params.q)).abs();
        let d1 = (ct - qim_level1(ct, params.q)).abs();

        raw_bits.push(if d1 < d0 { 1 } else { 0 });
    }

    // Parse header
    if raw_bits.len() < HEADER_BYTES * 8 {
        return Err("Image too small for header".to_string());
    }

    let header_bytes = bits_to_bytes(&raw_bits[..HEADER_BYTES * 8]);

    if header_bytes[0..4] != MAGIC {
        return Err(format!(
            "Invalid magic: expected GNQM, got {:?}",
            &header_bytes[0..4]
        ));
    }

    let payload_len =
        u32::from_be_bytes([header_bytes[4], header_bytes[5], header_bytes[6], header_bytes[7]])
            as usize;
    let expected_crc =
        u32::from_be_bytes([header_bytes[8], header_bytes[9], header_bytes[10], header_bytes[11]]);

    let total_bits_needed = HEADER_BYTES * 8 + payload_len * 8;
    if total_bits_needed > raw_bits.len() {
        return Err(format!(
            "Truncated: need {} bits, have {}",
            total_bits_needed,
            raw_bits.len()
        ));
    }

    let payload_bits = &raw_bits[HEADER_BYTES * 8..total_bits_needed];
    let payload = bits_to_bytes(payload_bits);

    let actual_crc = crc32(&payload);
    if actual_crc != expected_crc {
        return Err(format!(
            "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
            expected_crc, actual_crc
        ));
    }

    Ok(payload)
}

/// Extract raw watermark bits (no header parsing). Used for BER testing.
pub fn extract_raw_bits(
    gray: &[u8],
    width: usize,
    height: usize,
    num_bits: usize,
    params: &Params,
) -> Vec<u8> {
    let basis = DctBasis::new();
    let w = width - (width % N);
    let h = height - (height % N);

    let gray_f: Vec<f64> = gray.iter().map(|&v| v as f64).collect();
    let blocks = image_to_blocks(&gray_f, w, h);

    let mut bits = Vec::with_capacity(num_bits);
    for (i, block) in blocks.iter().enumerate() {
        if i >= num_bits {
            break;
        }
        let dct = basis.forward(block);
        let c = dct[DCT_ROW][DCT_COL];
        let ct = power_fwd(c, params.p);
        let d0 = (ct - qim_level0(ct, params.q)).abs();
        let d1 = (ct - qim_level1(ct, params.q)).abs();
        bits.push(if d1 < d0 { 1 } else { 0 });
    }
    bits
}

/// Embed raw bits without header framing. Used for BER testing.
pub fn embed_raw_bits(
    gray: &[u8],
    width: usize,
    height: usize,
    bits: &[u8],
    params: &Params,
) -> Vec<u8> {
    let basis = DctBasis::new();
    let w = width - (width % N);
    let h = height - (height % N);

    let gray_f: Vec<f64> = gray.iter().map(|&v| v as f64).collect();
    let mut blocks = image_to_blocks(&gray_f, w, h);

    for (i, block) in blocks.iter_mut().enumerate() {
        if i >= bits.len() {
            break;
        }
        let mut dct = basis.forward(block);
        let c = dct[DCT_ROW][DCT_COL];
        let ct = power_fwd(c, params.p);
        let ct_emb = if bits[i] == 0 {
            qim_level0(ct, params.q)
        } else {
            qim_level1(ct, params.q)
        };
        dct[DCT_ROW][DCT_COL] = power_inv(ct_emb, params.p);
        *block = basis.inverse(&dct);
    }

    blocks_to_image(&blocks, w, h)
}

/// Compute PSNR between two grayscale images.
pub fn psnr(original: &[u8], modified: &[u8]) -> f64 {
    assert_eq!(original.len(), modified.len());
    let mse: f64 = original
        .iter()
        .zip(modified.iter())
        .map(|(&a, &b)| {
            let d = a as f64 - b as f64;
            d * d
        })
        .sum::<f64>()
        / original.len() as f64;
    if mse < 1e-10 {
        return 99.99;
    }
    10.0 * (255.0 * 255.0 / mse).log10()
}

/// Compute Bit Error Rate between two bit vectors.
pub fn ber(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let errors = a[..n].iter().zip(b[..n].iter()).filter(|(&x, &y)| x != y).count();
    errors as f64 / n as f64
}

/// Report embedding capacity for an image.
pub fn capacity(width: usize, height: usize) -> (usize, usize) {
    let w = width - (width % N);
    let h = height - (height % N);
    let total_blocks = (w / N) * (h / N);
    let payload_bits = total_blocks.saturating_sub(HEADER_BYTES * 8);
    let payload_bytes = payload_bits / 8;
    (total_blocks, payload_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_extract_roundtrip() {
        let w = 128;
        let h = 128;
        let gray: Vec<u8> = (0..w * h).map(|i| ((i * 7 + 13) % 256) as u8).collect();
        let payload = b"Hello GN-QIM!";
        let params = Params::default();

        let stego = embed(&gray, w, h, payload, &params).unwrap();
        let extracted = extract(&stego, w, h, &params).unwrap();
        assert_eq!(&extracted, payload);
    }

    #[test]
    fn psnr_identical() {
        let img = vec![128u8; 64];
        assert!(psnr(&img, &img) > 90.0);
    }

    #[test]
    fn ber_identical() {
        let a = vec![0, 1, 0, 1, 1];
        assert_eq!(ber(&a, &a), 0.0);
    }
}
