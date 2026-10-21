use laboratorio_triangulos::*;
use std::{
    collections::BTreeSet,
    hint::black_box,
    io::{self, Write},
    path::Path,
    time::Instant,
};

const MODES: [TriangleMode; 3] = [
    TriangleMode::Wireframe,
    TriangleMode::FilledBarycentric,
    TriangleMode::FilledScanline,
];
const NAMES: [&str; 3] = ["wireframe", "baricentrico", "scanline"];

fn compare(out: &Path) -> io::Result<()> {
    let mut csv = String::from("caso,baricentrico,scanline,diferencias,duplicados\n");
    for (name, t) in cases() {
        let a = triangle_barycentric(t, WIDTH, HEIGHT);
        let b = triangle_scanline(t, WIDTH, HEIGHT);
        let sa: BTreeSet<_> = a.iter().map(|f| (f.x, f.y)).collect();
        let sb: BTreeSet<_> = b.iter().map(|f| (f.x, f.y)).collect();
        let differences = sa.symmetric_difference(&sb).count();
        let duplicates = a.len() - sa.len() + b.len() - sb.len();
        assert_eq!((differences, duplicates), (0, 0), "Caso {name}");
        csv.push_str(&format!(
            "{name},{},{},{differences},{duplicates}\n",
            a.len(),
            b.len()
        ));
    }
    print!("{csv}");
    std::fs::write(out.join("comparacion.csv"), csv)
}

/// BMP de 24 bits, sin dependencias ni ventana. Serialización fuera del benchmark.
fn write_bmp(path: &Path, fragments: &[Fragment]) -> io::Result<()> {
    let stride = (WIDTH * 3).div_ceil(4) * 4;
    let size = stride * HEIGHT;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"BM")?;
    file.write_all(&(54 + size).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54_u32.to_le_bytes())?;
    file.write_all(&40_u32.to_le_bytes())?;
    file.write_all(&(WIDTH as i32).to_le_bytes())?;
    file.write_all(&(HEIGHT as i32).to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&24_u16.to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&size.to_le_bytes())?;
    file.write_all(&[0; 16])?;
    let mut pixels = vec![0_u8; size as usize];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let i = (y * stride + x * 3) as usize;
            pixels[i..i + 3].copy_from_slice(&[39, 24, 16]);
        }
    }
    for f in fragments {
        let i = ((HEIGHT - 1 - f.y) * stride + f.x * 3) as usize;
        pixels[i..i + 3].copy_from_slice(&[f.color[2], f.color[1], f.color[0]]);
    }
    file.write_all(&pixels)
}

fn benchmark(out: &Path) -> io::Result<()> {
    let triangles: Vec<_> = cases().into_iter().map(|(_, t)| t).collect();
    let iterations = 100;
    let rounds = 9;
    let mut samples: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    let mut csv =
        String::from("modo,ronda,iteraciones,fragmentos_por_ejecucion,total_ms,ms_por_ejecucion\n");
    for mode in MODES {
        for _ in 0..10 {
            black_box(rasterize(black_box(&triangles), mode, WIDTH, HEIGHT));
        }
    }
    // Alternar el orden reduce el sesgo de calentamiento entre modos.
    for round in 0..rounds {
        for offset in 0..3 {
            let index = (round + offset) % 3;
            let mode = MODES[index];
            let count = rasterize(&triangles, mode, WIDTH, HEIGHT).len();
            let start = Instant::now();
            for _ in 0..iterations {
                let fragments = rasterize(black_box(&triangles), mode, WIDTH, HEIGHT);
                black_box(fragments);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            samples[index].push(ms / iterations as f64);
            csv.push_str(&format!(
                "{},{round},{iterations},{count},{ms:.6},{:.6}\n",
                NAMES[index],
                ms / iterations as f64
            ));
        }
    }
    let mut summary = String::from("modo,fragmentos_por_ejecucion,mediana_ms,min_ms,max_ms\n");
    for i in 0..3 {
        samples[i].sort_by(f64::total_cmp);
        summary.push_str(&format!(
            "{},{},{:.6},{:.6},{:.6}\n",
            NAMES[i],
            rasterize(&triangles, MODES[i], WIDTH, HEIGHT).len(),
            samples[i][rounds / 2],
            samples[i][0],
            samples[i][rounds - 1]
        ));
    }
    print!("{summary}");
    std::fs::write(out.join("mediciones.csv"), csv)?;
    std::fs::write(out.join("resumen.csv"), summary)
}

fn main() -> io::Result<()> {
    let out = Path::new("resultados");
    std::fs::create_dir_all(out)?;
    match std::env::args().nth(1).as_deref() {
        Some("--benchmark-triangles") => {
            if cfg!(feature = "profile") {
                return Err(io::Error::other(
                    "Mida sin --features profile para evitar sobrecoste",
                ));
            }
            benchmark(out)?;
        }
        Some("--profile-triangles") => {
            if !cfg!(feature = "profile") {
                return Err(io::Error::other(
                    "Use cargo run --release --features profile -- --profile-triangles",
                ));
            }
            let triangles: Vec<_> = cases().into_iter().map(|(_, t)| t).collect();
            for _ in 0..100 {
                for mode in &MODES[1..] {
                    black_box(rasterize(black_box(&triangles), *mode, WIDTH, HEIGHT));
                }
            }
            profiler::save(out)?;
            println!("Perfil guardado en resultados/perfil.csv y perfil.svg");
        }
        None => {
            compare(out)?;
            let triangles: Vec<_> = cases().into_iter().map(|(_, t)| t).collect();
            for i in 0..3 {
                write_bmp(
                    &out.join(format!("{}.bmp", NAMES[i])),
                    &rasterize(&triangles, MODES[i], WIDTH, HEIGHT),
                )?;
            }
            println!("Tres imágenes BMP guardadas en resultados/");
        }
        _ => {
            return Err(io::Error::other(
                "Argumento desconocido: use --benchmark-triangles o --profile-triangles",
            ))
        }
    }
    Ok(())
}
