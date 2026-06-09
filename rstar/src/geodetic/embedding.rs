//! The unit-sphere embedding: [`UnitVec`], a Cartesian point on the unit sphere,
//! and the squared-chord helper shared by the leaf metric and the tests.

// `Float` provides trig (`sin`, `cos`, `to_radians`, `abs`) on `f64` in `no_std`
// builds; under `cfg(test)` the inherent methods win, leaving this unused (the same
// pattern as `distance.rs`).
#[allow(unused_imports)]
use num_traits::Float;

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
/// `UnitVec` is the *envelope* and *query* point type, never the *leaf* type.
/// Geodetic leaf types are deliberately not `Point`s, so the blanket
/// `impl<P: Point> PointDistance for P` does not interfere with the custom
/// leaf metric.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnitVec(
    /// The Cartesian `(x, y, z)` components.
    ///
    /// Unit length (`x² + y² + z² = 1`) when this `UnitVec` is an embedded geodetic
    /// point or query, built via `From<GeodeticCoord>` /
    /// [`GeodeticCoord::to_unit_vector`]. The same type also serves as the envelope
    /// corner type for [`crate::AABB<UnitVec>`], where instances are general 3D
    /// bounds and are not unit length.
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

/// Squared chord between two unit vectors: `‖a − b‖²`, identical to squared Euclidean.
/// This is the internal metric of the geodetic index.
///
/// For unit vectors, `‖a − b‖² = 2 − 2(a · b) = 2 − 2cos(d) = 4 sin²(d/2)`, with `d` the
/// great-circle angle between them. So the squared chord lies in `[0, 4]` and is strictly
/// increasing in `d` over `[0, π]`: ordering by squared chord is ordering by great-circle
/// distance, which is what lets the Euclidean point-to-box distance prune correctly.
///
/// `a` and `b` are points on the unit sphere; embed a
/// [`GeodeticCoord`](crate::geodetic::GeodeticCoord) with `UnitVec::from(coord)` or
/// [`to_unit_vector`](crate::geodetic::GeodeticCoord::to_unit_vector). A custom
/// point-like leaf returns this from its
/// [`distance_2`](crate::PointDistance::distance_2), in the same squared-chord units the
/// [`AABB<UnitVec>`](crate::AABB) envelope uses; convert to metres with
/// [`squared_chord_to_metres`](crate::geodetic::squared_chord_to_metres).
pub fn squared_chord(a: UnitVec, b: UnitVec) -> f64 {
    // Routed through `PointExt` so the leaf metric and the envelope metric
    // (`AABB::distance_2`, which uses the same `sub`/`length_2` arithmetic) stay
    // a single implementation: the pruning lower bound requires them to agree.
    crate::point::PointExt::distance_2(&a, &b)
}

#[cfg(test)]
mod tests {
    use super::{squared_chord, UnitVec};
    use crate::geodetic::coord::GeodeticCoord;
    use crate::Point;
    use approx::assert_relative_eq;

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let v = GeodeticCoord {
            lon: 13.4050,
            lat: 52.5200,
        }
        .to_unit_vector();
        let json = serde_json::to_string(&v).expect("serialise");
        let back: UnitVec = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(v, back);
    }

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
