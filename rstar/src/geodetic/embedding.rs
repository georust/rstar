//! The unit-sphere embedding: [`UnitVec`], a Cartesian point on the unit sphere,
//! and the squared-chord helper shared by the leaf metric and the tests.

use super::coord::GeodeticCoord;

/// A point on the unit sphere, stored as Cartesian `(x, y, z)`.
///
/// Frame: `+Z` = North pole, `+X` = (lon 0, lat 0), `+Y` = (lon 90E, lat 0).
/// Right-handed. The components satisfy `x² + y² + z² = 1` in exact arithmetic;
/// embedded vectors are unit length to within a few ulp and must not be
/// renormalised.
///
/// This is a newtype over `[f64; 3]` rather than a bare array so that a Cartesian
/// query cannot be mixed with a planar `[f64; 3]` tree by accident. It implements
/// [`crate::Point`] (`DIMENSIONS = 3`), which makes [`crate::AABB<UnitVec>`] a
/// ready-made 3D envelope.
///
/// `UnitVec` is the *envelope* and *query* point type, never the *leaf* type. The
/// leaf is [`super::GeodeticPoint`], which is deliberately not a `Point`, so the
/// blanket `impl<P: Point> PointDistance for P` does not interfere with the custom
/// leaf metric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitVec(
    /// The Cartesian `(x, y, z)` components on the unit sphere.
    pub [f64; 3],
);

impl crate::Point for UnitVec {
    type Scalar = f64;

    const DIMENSIONS: usize = 3;

    fn generate(mut generator: impl FnMut(usize) -> f64) -> Self {
        UnitVec([generator(0), generator(1), generator(2)])
    }

    fn nth(&self, index: usize) -> f64 {
        self.0[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut f64 {
        &mut self.0[index]
    }
}

impl From<GeodeticCoord> for UnitVec {
    fn from(c: GeodeticCoord) -> Self {
        c.to_unit_vector()
    }
}

impl UnitVec {
    /// Inverse of the embedding: maps the vector back to longitude/latitude in
    /// degrees. Longitude at a pole is reported as `0` (`atan2(0, 0) = 0`).
    pub fn to_coord(self) -> GeodeticCoord {
        GeodeticCoord::from_unit_vector(self)
    }
}

/// Squared chord between two unit vectors: `‖a − b‖²`, identical to squared
/// Euclidean. This is the internal metric of the geodetic index. The leaf
/// [`crate::PointDistance::distance_2`] and the tests share this one definition so
/// the exact-distance formula has a single source of truth.
pub(crate) fn squared_chord(a: UnitVec, b: UnitVec) -> f64 {
    let dx = a.0[0] - b.0[0];
    let dy = a.0[1] - b.0[1];
    let dz = a.0[2] - b.0[2];
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::{UnitVec, squared_chord};
    use crate::Point;
    use crate::geodetic::coord::GeodeticCoord;
    use approx::assert_relative_eq;

    #[test]
    fn point_impl_generate_and_nth() {
        let v = UnitVec::generate(|i| (i as f64) + 1.0);
        assert_eq!(v.0, [1.0, 2.0, 3.0]);
        assert_eq!(v.nth(0), 1.0);
        assert_eq!(v.nth(1), 2.0);
        assert_eq!(v.nth(2), 3.0);
    }

    #[test]
    fn point_impl_nth_mut_updates_component() {
        let mut v = UnitVec([1.0, 2.0, 3.0]);
        *v.nth_mut(1) = 9.0;
        assert_eq!(v.0, [1.0, 9.0, 3.0]);
    }

    #[test]
    fn from_coord_and_to_coord_round_trip() {
        let c = GeodeticCoord {
            lon: 13.4050,
            lat: 52.5200,
        };
        let v = UnitVec::from(c);
        let back = v.to_coord();
        assert_relative_eq!(back.lon, c.lon, epsilon = 1e-9);
        assert_relative_eq!(back.lat, c.lat, epsilon = 1e-9);
    }

    #[test]
    fn squared_chord_matches_manual_dot() {
        let a = UnitVec([1.0, 0.0, 0.0]);
        let b = UnitVec([0.0, 1.0, 0.0]);
        // ‖a − b‖² = 1 + 1 = 2.
        assert_relative_eq!(squared_chord(a, b), 2.0, epsilon = 1e-15);

        // Coincident vectors give zero.
        assert_eq!(squared_chord(a, a), 0.0);

        // Antipodal vectors give 4.
        let c = UnitVec([-1.0, 0.0, 0.0]);
        assert_relative_eq!(squared_chord(a, c), 4.0, epsilon = 1e-15);
    }
}
