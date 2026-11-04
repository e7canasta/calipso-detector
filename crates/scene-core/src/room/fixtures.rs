//! The dig1 camera (`workshop/43-ocupacion-por-partes/perfiles/dig1.toml`,
//! the same bed as `dig1-partes.toml`), once, for the room's tests: its bed,
//! its footprint on the floor, its headboard as four points and its size, and
//! the frame of three planes they build. The prototype's tolerances live here
//! too: it rounds to four decimals and runs its homographies in float32.

use super::three_planes::ThreePlanes;
use crate::geom::{Point, Quad};
use crate::room::bed_sizes;
use crate::units::PositiveMetres;

pub(crate) fn quad(c: [[f32; 2]; 4]) -> Quad {
    Quad(c.map(|[x, y]| Point { x, y }))
}

pub(crate) fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

pub(crate) fn bed_quad() -> Quad {
    quad([
        [940.0, 465.0],
        [1270.0, 425.0],
        [1635.0, 745.0],
        [1380.0, 1010.0],
    ])
}

pub(crate) fn floor_quad() -> Quad {
    quad([
        [950.0, 610.0],
        [1268.0, 485.0],
        [1627.0, 805.0],
        [1335.0, 1030.0],
    ])
}

pub(crate) fn headboard_quad() -> Quad {
    quad([
        [951.0, 365.0],
        [1281.0, 325.0],
        [1270.0, 425.0],
        [940.0, 465.0],
    ])
}

pub(crate) fn size() -> Option<[PositiveMetres; 2]> {
    bed_sizes([0.9, 2.0])
}

/// The dig1 camera: the frame of its three planes.
pub(crate) fn dig1() -> ThreePlanes {
    ThreePlanes::new(
        &bed_quad(),
        Some(&floor_quad()),
        Some(&headboard_quad()),
        size(),
    )
    .expect("dig1's planes")
}

/// Are the two numbers within `tol`, as the prototype rounds them?
pub(crate) fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}
