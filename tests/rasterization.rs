use laboratorio_triangulos::*;
use std::collections::BTreeSet;

fn check(t: Triangle, w: u32, h: u32) {
    let a = triangle_barycentric(t, w, h);
    let b = triangle_scanline(t, w, h);
    assert_eq!(a, b, "Distintas posiciones o colores: {t:?}");
    let unique: BTreeSet<_> = a.iter().map(|f| (f.x, f.y)).collect();
    assert_eq!(a.len(), unique.len(), "Fragmentos duplicados");
    assert!(a.iter().all(|f| f.x < w && f.y < h));
    for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let permuted = order.map(|i| t[i]);
        assert_eq!(a, triangle_barycentric(permuted, w, h));
        assert_eq!(b, triangle_scanline(permuted, w, h));
    }
}

#[test]
fn required_cases_and_permutations() {
    for (_, t) in cases() {
        check(t, WIDTH, HEIGHT);
    }
}

#[test]
fn independent_expected_pixels() {
    let t = [Vertex::new(0, 0), Vertex::new(2, 0), Vertex::new(0, 2)];
    let expected = vec![(0, 0), (1, 0), (0, 1)];
    for mode in [
        TriangleMode::FilledBarycentric,
        TriangleMode::FilledScanline,
    ] {
        assert_eq!(
            triangle(t, mode, 4, 4)
                .iter()
                .map(|f| (f.x, f.y))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn horizontal_and_vertices_on_pixel_centers() {
    let v = Vertex::subpixels;
    check([v(128, 128), v(896, 128), v(384, 896)], 8, 8);
    check([v(128, 896), v(896, 896), v(384, 128)], 8, 8);
    check([v(128, 128), v(128, 896), v(896, 384)], 8, 8);
}

#[test]
fn degenerate_empty_and_outside() {
    let v = Vertex::new;
    for t in [
        [v(0, 0), v(2, 2), v(4, 4)],
        [v(1, 1); 3],
        [v(-9, -9), v(-5, -9), v(-5, -5)],
    ] {
        assert!(triangle_barycentric(t, 8, 8).is_empty());
        assert!(triangle_scanline(t, 8, 8).is_empty());
    }
    check([v(0, 0), v(2, 0), v(0, 2)], 0, 0);
}

#[test]
fn reproducible_subpixel_fuzz() {
    let mut seed = 42_u64;
    for _ in 0..2000 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) % 16384) as i32 - 4096
        };
        let t = std::array::from_fn(|_| Vertex::subpixels(next(), next()));
        check(t, 32, 32);
    }
}

#[test]
fn wireframe_keeps_three_lines() {
    let t = [Vertex::new(1, 1), Vertex::new(7, 1), Vertex::new(1, 7)];
    let a = triangle(t, TriangleMode::Wireframe, 10, 10);
    let mut expected = line(t[0], t[1], 10, 10);
    expected.extend(line(t[1], t[2], 10, 10));
    expected.extend(line(t[2], t[0], 10, 10));
    assert_eq!(a, expected);
    assert!(!a.iter().any(|f| (f.x, f.y) == (2, 2)));
}
