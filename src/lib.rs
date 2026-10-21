pub mod profiler;

use profiler::scope;

const SCALE: i128 = 256;
const HALF: i128 = 128;
pub const COLOR: [u8; 3] = [36, 196, 180];

/// Coordenadas de pantalla en punto fijo: 256 unidades por píxel.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
}

impl Vertex {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x: x.checked_mul(256).expect("x fuera de rango"),
            y: y.checked_mul(256).expect("y fuera de rango"),
        }
    }
    pub const fn subpixels(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

pub type Triangle = [Vertex; 3];

#[derive(Clone, Copy, Debug)]
pub enum TriangleMode {
    Wireframe,
    FilledBarycentric,
    FilledScanline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub x: u32,
    pub y: u32,
    pub color: [u8; 3],
}

fn edge(a: Vertex, b: Vertex, x: i128, y: i128) -> i128 {
    (b.x as i128 - a.x as i128) * (y - a.y as i128)
        - (b.y as i128 - a.y as i128) * (x - a.x as i128)
}

fn area(t: Triangle) -> i128 {
    edge(t[0], t[1], t[2].x as i128, t[2].y as i128)
}
fn ceil_div(a: i128, b: i128) -> i128 {
    -(-a).div_euclid(b)
}

fn bounds(t: Triangle, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    if width == 0 || height == 0 || area(t) == 0 {
        return None;
    }
    let xmin = ceil_div(t.iter().map(|v| v.x as i128).min()? - HALF, SCALE).max(0);
    let xmax = (t.iter().map(|v| v.x as i128).max()? - HALF)
        .div_euclid(SCALE)
        .min(width as i128 - 1);
    let ymin = ceil_div(t.iter().map(|v| v.y as i128).min()? - HALF, SCALE).max(0);
    let ymax = (t.iter().map(|v| v.y as i128).max()? - HALF)
        .div_euclid(SCALE)
        .min(height as i128 - 1);
    (xmin <= xmax && ymin <= ymax).then_some((xmin as u32, xmax as u32, ymin as u32, ymax as u32))
}

/// Relleno inclusivo: cualquier centro sobre el borde pertenece al triángulo.
#[inline(never)]
pub fn triangle_barycentric(t: Triangle, width: u32, height: u32) -> Vec<Fragment> {
    let _function = scope("triangle_barycentric");
    let Some((xmin, xmax, ymin, ymax)) = bounds(t, width, height) else {
        return vec![];
    };
    let signed_area = area(t) as f64;
    let mut fragments = Vec::new();
    // Reutilizado por fila; permite perfilar geometría y almacenamiento por separado.
    let mut covered = Vec::new();
    for y in ymin..=ymax {
        {
            let _geometry = scope("barycentric_geometry");
            covered.clear();
            for x in xmin..=xmax {
                let px = x as i128 * SCALE + HALF;
                let py = y as i128 * SCALE + HALF;
                let weights = [
                    edge(t[1], t[2], px, py),
                    edge(t[2], t[0], px, py),
                    edge(t[0], t[1], px, py),
                ]
                .map(|numerator| numerator as f64 / signed_area);
                if weights.iter().all(|&w| w >= 0.0) {
                    covered.push(x);
                }
            }
        }
        {
            let _emit = scope("barycentric_emit");
            for &x in &covered {
                fragments.push(Fragment { x, y, color: COLOR });
            }
        }
    }
    fragments
}

/// Intersección racional exacta con una fila; incluye extremos y horizontales.
fn row_span(t: Triangle, py: i128, width: u32) -> Option<(u32, u32)> {
    let mut left: Option<(i128, i128)> = None;
    let mut right: Option<(i128, i128)> = None;
    let mut include = |n: i128, d: i128| {
        if left.is_none_or(|(ln, ld)| n * ld < ln * d) {
            left = Some((n, d));
        }
        if right.is_none_or(|(rn, rd)| n * rd > rn * d) {
            right = Some((n, d));
        }
    };
    for i in 0..3 {
        let (mut a, mut b) = (t[i], t[(i + 1) % 3]);
        if a.y > b.y {
            std::mem::swap(&mut a, &mut b);
        }
        if py < a.y as i128 || py > b.y as i128 {
            continue;
        }
        let dy = b.y as i128 - a.y as i128;
        if dy == 0 {
            include(a.x as i128, 1);
            include(b.x as i128, 1);
        } else {
            let n = a.x as i128 * dy + (py - a.y as i128) * (b.x as i128 - a.x as i128);
            include(n, dy);
        }
    }
    let (ln, ld) = left?;
    let (rn, rd) = right?;
    let first = ceil_div(ln - HALF * ld, SCALE * ld).max(0);
    let last = (rn - HALF * rd)
        .div_euclid(SCALE * rd)
        .min(width as i128 - 1);
    (first <= last).then_some((first as u32, last as u32))
}

#[inline(never)]
pub fn triangle_scanline(t: Triangle, width: u32, height: u32) -> Vec<Fragment> {
    let _function = scope("triangle_scanline");
    let Some((_, _, ymin, ymax)) = bounds(t, width, height) else {
        return vec![];
    };
    let mut fragments = Vec::new();
    for y in ymin..=ymax {
        let span = {
            let _geometry = scope("scanline_geometry");
            row_span(t, y as i128 * SCALE + HALF, width)
        };
        if let Some((first, last)) = span {
            let _emit = scope("scanline_emit");
            for x in first..=last {
                fragments.push(Fragment { x, y, color: COLOR });
            }
        }
    }
    fragments
}

/// Bresenham; el recorte se aplica antes de emitir cada fragmento.
pub fn line(a: Vertex, b: Vertex, width: u32, height: u32) -> Vec<Fragment> {
    let (mut x, mut y) = ((a.x as i64).div_euclid(256), (a.y as i64).div_euclid(256));
    let (x1, y1) = ((b.x as i64).div_euclid(256), (b.y as i64).div_euclid(256));
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let mut fragments = Vec::new();
    loop {
        if x >= 0 && y >= 0 && x < width as i64 && y < height as i64 {
            fragments.push(Fragment {
                x: x as u32,
                y: y as u32,
                color: COLOR,
            });
        }
        if x == x1 && y == y1 {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
    fragments
}

pub fn triangle(t: Triangle, mode: TriangleMode, width: u32, height: u32) -> Vec<Fragment> {
    match mode {
        TriangleMode::Wireframe => {
            let mut f = line(t[0], t[1], width, height);
            f.extend(line(t[1], t[2], width, height));
            f.extend(line(t[2], t[0], width, height));
            f
        }
        TriangleMode::FilledBarycentric => triangle_barycentric(t, width, height),
        TriangleMode::FilledScanline => triangle_scanline(t, width, height),
    }
}

pub fn rasterize(
    triangles: &[Triangle],
    mode: TriangleMode,
    width: u32,
    height: u32,
) -> Vec<Fragment> {
    let _function = scope("rasterize");
    let mut fragments = Vec::new();
    for &t in triangles {
        fragments.extend(triangle(t, mode, width, height));
    }
    fragments
}

pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 480;

pub fn cases() -> Vec<(&'static str, Triangle)> {
    let v = Vertex::new;
    let large = [v(220, 35), v(610, 130), v(350, 435)];
    vec![
        ("pequeno", [v(35, 30), v(90, 42), v(53, 85)]),
        ("grande", large),
        ("horizontal", [v(30, 120), v(190, 120), v(100, 230)]),
        ("vertical", [v(40, 270), v(40, 420), v(200, 350)]),
        ("orden_inverso", [large[2], large[1], large[0]]),
        (
            "fuera_del_framebuffer",
            [v(-65, 440), v(115, 405), v(80, 530)],
        ),
        ("degenerado", [v(250, 460), v(390, 460), v(530, 460)]),
    ]
}
