// Copyright © 2026 Japality Limited. All rights reserved.
// Licensed under the GNU General Public License v3.0.
// See LICENSE file in the project root for full license information.

/// 8×8 DCT-II / IDCT-II with orthonormal basis.
use std::f64::consts::PI;

pub const N: usize = 8;

/// Precomputed orthonormal DCT-II basis matrix.
/// C[k][n] = alpha_k * cos(pi * k * (2n+1) / 16)
pub struct DctBasis {
    pub c: [[f64; N]; N],
    pub ct: [[f64; N]; N],
}

impl DctBasis {
    pub fn new() -> Self {
        let mut c = [[0.0f64; N]; N];
        for k in 0..N {
            let alpha = if k == 0 {
                (1.0 / N as f64).sqrt()
            } else {
                (2.0 / N as f64).sqrt()
            };
            for n in 0..N {
                c[k][n] =
                    alpha * (PI * k as f64 * (2.0 * n as f64 + 1.0) / (2.0 * N as f64)).cos();
            }
        }
        let ct = transpose(&c);
        Self { c, ct }
    }

    /// Forward 2D DCT: D = C · block · Cᵀ
    pub fn forward(&self, block: &[[f64; N]; N]) -> [[f64; N]; N] {
        let temp = matmul(&self.c, block);
        matmul(&temp, &self.ct)
    }

    /// Inverse 2D DCT: block = Cᵀ · D · C
    pub fn inverse(&self, dct: &[[f64; N]; N]) -> [[f64; N]; N] {
        let temp = matmul(&self.ct, dct);
        matmul(&temp, &self.c)
    }
}

fn matmul(a: &[[f64; N]; N], b: &[[f64; N]; N]) -> [[f64; N]; N] {
    let mut r = [[0.0f64; N]; N];
    for i in 0..N {
        for j in 0..N {
            let mut s = 0.0;
            for k in 0..N {
                s += a[i][k] * b[k][j];
            }
            r[i][j] = s;
        }
    }
    r
}

fn transpose(m: &[[f64; N]; N]) -> [[f64; N]; N] {
    let mut t = [[0.0f64; N]; N];
    for i in 0..N {
        for j in 0..N {
            t[j][i] = m[i][j];
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let basis = DctBasis::new();
        let mut block = [[0.0f64; N]; N];
        for i in 0..N {
            for j in 0..N {
                block[i][j] = (i * N + j) as f64 * 3.7 + 10.0;
            }
        }
        let dct = basis.forward(&block);
        let recon = basis.inverse(&dct);
        for i in 0..N {
            for j in 0..N {
                assert!(
                    (block[i][j] - recon[i][j]).abs() < 1e-10,
                    "Mismatch at [{i}][{j}]: {} vs {}",
                    block[i][j],
                    recon[i][j]
                );
            }
        }
    }
}
