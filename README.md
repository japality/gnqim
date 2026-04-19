# GN-QIM

**GN-QIM** (Generalized Nonlinear Quantization Index Modulation) — a sublinear power-law QIM scheme for JPEG-robust digital watermarking.

> Reference implementation for:
> *"Sublinear Power-Law Quantization Index Modulation for JPEG-Robust Digital Watermarking"*

## Overview

GN-QIM applies a power-law nonlinear transform **T(c) = sgn(c)·|c|^p** before Quantization Index Modulation in the DCT domain. The GA-optimized exponent **p\* = 0.884** compresses large coefficients while expanding small ones, yielding superior JPEG robustness compared to standard QIM (p=1.0).

### Key Features

- **JPEG-robust** watermarking with BER < 0.1% at JPEG quality 50–90
- **Power-law nonlinearity** before QIM for improved robustness
- **GA-optimized parameters**: p\*=0.884, q=24.3 (evolutionary search over 10⁵+ configurations)
- Operates on **8×8 DCT blocks**, AC coefficient (1,1) in the Y channel
- CRC-32 integrity verification for embedded payloads
- Embed text messages or arbitrary binary files
- Built-in JPEG robustness benchmark with BER analysis

## Algorithm

```
Embed:
  1. Split grayscale image into 8×8 blocks
  2. Forward DCT-II (orthonormal)
  3. Extract AC coefficient c = DCT[1][1]
  4. Forward power-law: t = sgn(c)·|c|^p
  5. QIM embed bit: t' = Q_b(t, q)
  6. Inverse power-law: c' = sgn(t')·|t'|^(1/p)
  7. Inverse DCT, reconstruct image

Extract:
  1. Split stego image into 8×8 blocks
  2. Forward DCT-II
  3. Extract coefficient, apply power-law
  4. Decode bit: b = argmin_k |t - Q_k(t, q)|
```

## Quick Start

### Build

```bash
cargo build --release
```

### Embed a Message

```bash
./target/release/gnqim embed -i cover.png -o stego.png -m "Hello, GN-QIM!"
```

### Extract a Message

```bash
./target/release/gnqim extract -i stego.png
```

### Embed a File

```bash
./target/release/gnqim embed -i cover.png -o stego.png -f secret.txt
```

### Extract to File

```bash
./target/release/gnqim extract -i stego.png -o extracted.txt
```

### Check Capacity

```bash
./target/release/gnqim info -i cover.png
```

### JPEG Robustness Benchmark

```bash
./target/release/gnqim benchmark -i cover.png
```

## Custom Parameters

The default parameters (**p=0.884**, **q=24.3**) are GA-optimized for JPEG robustness. You can override them:

```bash
# Standard QIM (p=1.0)
./target/release/gnqim embed -i cover.png -o stego.png -m "test" -p 1.0

# Custom quantization step
./target/release/gnqim embed -i cover.png -o stego.png -m "test" -q 30.0
```

## Project Structure

```
gnqim/
├── Cargo.toml          # Package manifest
├── LICENSE             # GPLv3 license
├── README.md           # This file
└── src/
    ├── main.rs         # CLI interface (clap)
    ├── lib.rs          # Library entry point
    ├── gnqim.rs        # Core GN-QIM algorithm
    └── dct.rs          # 8×8 DCT-II / IDCT-II
```

## Citation

If you use GN-QIM in your research, please cite:

```bibtex
@software{gnqim2026,
  title     = {GN-QIM: Sublinear Power-Law QIM for JPEG-Robust Digital Watermarking},
  author    = {Japality Limited},
  year      = {2026},
  url       = {https://github.com/japality/gnqim}
}
```

## License

Copyright © 2026 Japality Limited. All rights reserved.

This project is licensed under the [GNU General Public License v3.0](LICENSE).

For commercial licensing inquiries, please contact via [GitHub](https://github.com/japality).
