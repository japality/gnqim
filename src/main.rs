// Copyright © 2026 Japality Limited. All rights reserved.
// Licensed under the GNU General Public License v3.0.
// See LICENSE file in the project root for full license information.

/// GN-QIM CLI — Reference implementation for:
///   "Sublinear Power-Law Quantization Index Modulation
///    for JPEG-Robust Digital Watermarking"
use std::io::Cursor;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, ImageFormat};

use gnqim_lib::gnqim;
use gnqim_lib::gnqim::Params;

#[derive(Parser)]
#[command(
    name = "gnqim",
    version,
    about = "GN-QIM: Sublinear Power-Law QIM for JPEG-Robust Digital Watermarking"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Embed a message or file into a cover image
    Embed {
        /// Cover image path (PNG/BMP/TIFF recommended)
        #[arg(short, long)]
        input: PathBuf,

        /// Output stego image path
        #[arg(short, long)]
        output: PathBuf,

        /// Text message to embed
        #[arg(short, long, conflicts_with = "file")]
        message: Option<String>,

        /// Binary file to embed
        #[arg(short, long, conflicts_with = "message")]
        file: Option<PathBuf>,

        /// Power-law exponent (default: 0.884, optimal for JPEG)
        #[arg(short, long, default_value_t = 0.884)]
        p: f64,

        /// Quantization step (default: 24.3)
        #[arg(short, long, default_value_t = 24.3)]
        q: f64,
    },

    /// Extract a hidden message from a stego image
    Extract {
        /// Stego image path
        #[arg(short, long)]
        input: PathBuf,

        /// Save extracted data to file (otherwise print as text)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Power-law exponent (must match embedding)
        #[arg(short, long, default_value_t = 0.884)]
        p: f64,

        /// Quantization step (must match embedding)
        #[arg(short, long, default_value_t = 24.3)]
        q: f64,
    },

    /// Show embedding capacity for an image
    Info {
        /// Image path
        #[arg(short, long)]
        input: PathBuf,
    },

    /// Run JPEG robustness benchmark
    Benchmark {
        /// Cover image path
        #[arg(short, long)]
        input: PathBuf,

        /// Power-law exponent
        #[arg(short, long, default_value_t = 0.884)]
        p: f64,

        /// Quantization step
        #[arg(short, long, default_value_t = 24.3)]
        q: f64,
    },
}

fn load_grayscale(path: &PathBuf) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    Ok((gray.into_raw(), w, h))
}

fn save_grayscale(path: &PathBuf, data: &[u8], w: u32, h: u32) -> Result<(), String> {
    let img = image::GrayImage::from_raw(w, h, data.to_vec())
        .ok_or("Failed to create image buffer")?;
    DynamicImage::from(img)
        .save(path)
        .map_err(|e| format!("Failed to save {}: {}", path.display(), e))
}

fn jpeg_roundtrip(gray: &[u8], w: u32, h: u32, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode(gray, w, h, image::ExtendedColorType::L8)
        .map_err(|e| format!("JPEG encode failed: {e}"))?;

    let decoded = image::load_from_memory_with_format(&buf.into_inner(), ImageFormat::Jpeg)
        .map_err(|e| format!("JPEG decode failed: {e}"))?;
    Ok(decoded.to_luma8().into_raw())
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Embed {
            input,
            output,
            message,
            file,
            p,
            q,
        } => {
            let payload = match (message, file) {
                (Some(msg), _) => msg.into_bytes(),
                (_, Some(path)) => std::fs::read(&path).unwrap_or_else(|e| {
                    eprintln!("Error reading {}: {e}", path.display());
                    std::process::exit(1);
                }),
                _ => {
                    eprintln!("Error: provide --message or --file");
                    std::process::exit(1);
                }
            };

            let (gray, w, h) = load_grayscale(&input).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });

            let params = Params { p, q };
            let (total_blocks, max_bytes) = gnqim::capacity(w as usize, h as usize);

            println!("Cover:    {}×{} ({} blocks, {} bytes capacity)", w, h, total_blocks, max_bytes);
            println!("Payload:  {} bytes", payload.len());
            println!("Params:   p={:.3}, q={:.1}", p, q);

            if payload.len() > max_bytes {
                eprintln!("Error: payload exceeds capacity ({} > {} bytes)", payload.len(), max_bytes);
                std::process::exit(1);
            }

            let stego = gnqim::embed(&gray, w as usize, h as usize, &payload, &params)
                .unwrap_or_else(|e| {
                    eprintln!("Embed error: {e}");
                    std::process::exit(1);
                });

            let psnr_val = gnqim::psnr(&gray, &stego);
            println!("PSNR:     {:.2} dB", psnr_val);

            save_grayscale(&output, &stego, w, h).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });

            println!("Saved:    {}", output.display());
        }

        Commands::Extract {
            input,
            output,
            p,
            q,
        } => {
            let (gray, w, h) = load_grayscale(&input).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });

            let params = Params { p, q };
            let payload = gnqim::extract(&gray, w as usize, h as usize, &params)
                .unwrap_or_else(|e| {
                    eprintln!("Extract error: {e}");
                    std::process::exit(1);
                });

            println!("Extracted {} bytes", payload.len());

            if let Some(out_path) = output {
                std::fs::write(&out_path, &payload).unwrap_or_else(|e| {
                    eprintln!("Failed to write {}: {e}", out_path.display());
                    std::process::exit(1);
                });
                println!("Saved: {}", out_path.display());
            } else {
                match String::from_utf8(payload.clone()) {
                    Ok(text) => println!("Message: {text}"),
                    Err(_) => println!("Binary data ({} bytes, use -o to save)", payload.len()),
                }
            }
        }

        Commands::Info { input } => {
            let img = image::open(&input).unwrap_or_else(|e| {
                eprintln!("Failed to open {}: {e}", input.display());
                std::process::exit(1);
            });
            let (w, h) = img.dimensions();
            let (total_blocks, max_bytes) = gnqim::capacity(w as usize, h as usize);
            println!("Image:    {}×{}", w, h);
            println!("Blocks:   {} ({}×{})", total_blocks, w / 8, h / 8);
            println!("Capacity: {} bits = {} bytes", total_blocks, max_bytes);
        }

        Commands::Benchmark {
            input,
            p,
            q,
        } => {
            let (gray, w, h) = load_grayscale(&input).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });

            let params = Params { p, q };
            let num_blocks = ((w as usize) / 8) * ((h as usize) / 8);

            // Generate random watermark bits
            let wm_bits: Vec<u8> = (0..num_blocks)
                .map(|i| ((i * 7 + 3) % 2) as u8)
                .collect();

            let stego =
                gnqim::embed_raw_bits(&gray, w as usize, h as usize, &wm_bits, &params);
            let psnr_val = gnqim::psnr(&gray, &stego);

            println!("╔══════════════════════════════════════════════╗");
            println!("║     GN-QIM Robustness Benchmark             ║");
            println!("╠══════════════════════════════════════════════╣");
            println!("║ Image:  {}×{} ({} blocks)          ", w, h, num_blocks);
            println!("║ Params: p={:.3}, q={:.1}", p, q);
            println!("║ PSNR:   {:.2} dB", psnr_val);
            println!("╠══════════════════════════════════════════════╣");
            println!("║ Attack               │  BER (%)             ║");
            println!("╟──────────────────────┼──────────────────────╢");

            // No attack
            {
                let ext = gnqim::extract_raw_bits(
                    &stego, w as usize, h as usize, wm_bits.len(), &params,
                );
                let b = gnqim::ber(&wm_bits, &ext) * 100.0;
                println!("║ No attack            │  {:>7.3}%             ║", b);
            }

            // JPEG attacks
            for quality in [30, 50, 70, 80, 90, 95] {
                match jpeg_roundtrip(&stego, w, h, quality) {
                    Ok(attacked) => {
                        let ext = gnqim::extract_raw_bits(
                            &attacked,
                            w as usize,
                            h as usize,
                            wm_bits.len(),
                            &params,
                        );
                        let b = gnqim::ber(&wm_bits, &ext) * 100.0;
                        println!("║ JPEG Q={:<3}            │  {:>7.3}%             ║", quality, b);
                    }
                    Err(e) => {
                        println!("║ JPEG Q={:<3}            │  ERROR: {}       ║", quality, e);
                    }
                }
            }

            println!("╚══════════════════════════════════════════════╝");

            // Also compare with standard QIM (p=1.0)
            let params_qim = Params { p: 1.0, q: params.q };
            let stego_qim =
                gnqim::embed_raw_bits(&gray, w as usize, h as usize, &wm_bits, &params_qim);
            let psnr_qim = gnqim::psnr(&gray, &stego_qim);

            println!();
            println!("Comparison: Standard QIM (p=1.0, q={:.1}) PSNR={:.2} dB", params.q, psnr_qim);
            for quality in [50, 70, 90] {
                if let Ok(attacked) = jpeg_roundtrip(&stego_qim, w, h, quality) {
                    let ext = gnqim::extract_raw_bits(
                        &attacked,
                        w as usize,
                        h as usize,
                        wm_bits.len(),
                        &params_qim,
                    );
                    let b = gnqim::ber(&wm_bits, &ext) * 100.0;
                    println!("  JPEG Q={}: BER = {:.3}%", quality, b);
                }
            }
        }
    }
}
