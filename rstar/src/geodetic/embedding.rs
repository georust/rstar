//! The unit-sphere embedding: [`UnitVec`], a Cartesian point on the unit sphere,
//! and the squared-chord helper shared by the leaf metric and the tests.

// `Float` provides trig (`sin`, `cos`, `to_radians`, `abs`) on `f64` in `no_std`
// builds; under `cfg(test)` the inherent methods win, leaving this unused (the same
// pattern as `distance.rs`).
#[allow(unused_imports)]
use num_traits::Float;

use crate::AABB;

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

/// Returns `true` if the eastward longitude arc from `lon_lo` to `lon_hi` (degrees,
/// each in `[-180, 180]`) contains `theta`. When `lon_lo <= lon_hi` the arc is the
/// ordinary closed interval; when `lon_lo > lon_hi` it wraps across the ±180°
/// meridian (so it spans `[lon_lo, 180]` together with `[-180, lon_hi]`).
fn arc_contains(lon_lo: f64, lon_hi: f64, theta: f64) -> bool {
    if lon_lo <= lon_hi {
        lon_lo <= theta && theta <= lon_hi
    } else {
        theta >= lon_lo || theta <= lon_hi
    }
}

/// Returns `(min, max)` of the product `a * b` where `a ∈ [a_lo, a_hi]` and
/// `b ∈ [b_lo, b_hi]` vary independently. The extremes of a product of two
/// independent intervals are among the four corner products.
fn interval_product(a_lo: f64, a_hi: f64, b_lo: f64, b_hi: f64) -> (f64, f64) {
    let products = [a_lo * b_lo, a_lo * b_hi, a_hi * b_lo, a_hi * b_hi];
    let lo = products.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = products.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo, hi)
}

/// A conservative axis-aligned [`AABB<UnitVec>`] containing the embedded spherical
/// region of the longitude/latitude rectangle bounded by the eastward longitude
/// arc `lower.lon -> upper.lon` and the latitude band `[lower.lat, upper.lat]`.
///
/// This is the *filter* box of the filter/refine window query: it contains the
/// embedding of every point in the rectangle (so an index scan over it drops
/// nothing), but, being axis-aligned, it also contains points outside the
/// rectangle, which [`rectangle_contains`] removes in the refine step. Longitude
/// wraps across the antimeridian when `lower.lon > upper.lon`; `lower.lat <=
/// upper.lat` is required.
///
/// The per-axis bound is exact in real arithmetic (then nudged outward by a small
/// margin, see the body): `z = sin(lat)` over the band, and `x = cos(lat) cos(lon)`,
/// `y = cos(lat) sin(lon)` are products of the independent `cos(lat)`,
/// `cos(lon)`/`sin(lon)` ranges (a cardinal longitude the arc passes through pins
/// the relevant cosine/sine to ±1; the equator pins `cos(lat)` to its maximum 1).
pub(crate) fn rectangle_bounding_box(lower: GeodeticCoord, upper: GeodeticCoord) -> AABB<UnitVec> {
    let lat_lo = lower.lat.to_radians();
    let lat_hi = upper.lat.to_radians();

    // z = sin(lat): sin is increasing on [-90, 90] and lower.lat <= upper.lat.
    let z_min = lat_lo.sin();
    let z_max = lat_hi.sin();

    // cos(lat) >= 0; it peaks at the equator, so the band maximum is 1 when the band
    // straddles lat 0, otherwise the cosine of the parallel nearer the equator.
    let cos_lat_lo = lat_lo.cos();
    let cos_lat_hi = lat_hi.cos();
    let cosphi_min = cos_lat_lo.min(cos_lat_hi);
    let cosphi_max = if lower.lat <= 0.0 && 0.0 <= upper.lat {
        1.0
    } else {
        cos_lat_lo.max(cos_lat_hi)
    };

    // cos(lon)/sin(lon) extremes over the arc: a cardinal angle the arc passes
    // through pins the extreme to ±1, otherwise the two endpoints bound it. The
    // ±180° meridian (cos = -1) is reached at either endpoint representation.
    let (lo, hi) = (lower.lon, upper.lon);
    let lon_lo = lo.to_radians();
    let lon_hi = hi.to_radians();
    let coslon_max = if arc_contains(lo, hi, 0.0) {
        1.0
    } else {
        lon_lo.cos().max(lon_hi.cos())
    };
    let coslon_min = if arc_contains(lo, hi, 180.0) || arc_contains(lo, hi, -180.0) {
        -1.0
    } else {
        lon_lo.cos().min(lon_hi.cos())
    };
    let sinlon_max = if arc_contains(lo, hi, 90.0) {
        1.0
    } else {
        lon_lo.sin().max(lon_hi.sin())
    };
    let sinlon_min = if arc_contains(lo, hi, -90.0) {
        -1.0
    } else {
        lon_lo.sin().min(lon_hi.sin())
    };

    let (x_min, x_max) = interval_product(cosphi_min, cosphi_max, coslon_min, coslon_max);
    let (y_min, y_max) = interval_product(cosphi_min, cosphi_max, sinlon_min, sinlon_max);

    // Expand outward by a small margin so the filter never drops a point the refine
    // would keep. This guards two things: ordinary floating-point drift at the box
    // faces, and the pole degeneracy — a point stored at a pole has an arbitrary
    // longitude, so its embedding (x, y ≈ ±cos(90°) ≈ ±6e-17, z = ±1) can sit just
    // outside the longitude-derived x/y bounds. The margin (≈ 6 µm on the sphere) is
    // far below any meaningful separation, and the refine step removes the few extra
    // candidates it admits.
    const MARGIN: f64 = 1e-12;

    AABB::from_corners(
        UnitVec([x_min - MARGIN, y_min - MARGIN, z_min - MARGIN]),
        UnitVec([x_max + MARGIN, y_max + MARGIN, z_max + MARGIN]),
    )
}

/// The exact refine predicate: `true` if `p` lies in the longitude/latitude
/// rectangle bounded by the eastward longitude arc `lower.lon -> upper.lon` and the
/// latitude band `[lower.lat, upper.lat]` (all bounds inclusive). A point at a pole
/// (`|lat| == 90`) has undefined longitude and is included whenever the latitude
/// band reaches it, regardless of its stored longitude.
///
/// # Window region semantics (and how to change them)
///
/// The window is a true longitude/latitude box: its north and south edges are
/// parallels (constant latitude) and its east and west edges are meridians. This is
/// a deliberate choice and differs from PostGIS `geography`, which models a
/// rectangle's edges as great-circle arcs. The two agree on the east/west edges
/// (meridians are great circles) but differ along the top and bottom: a
/// great-circle edge between two points at the same latitude bows toward the nearer
/// pole, so the PostGIS quadrilateral sits slightly poleward of this box. The gap
/// grows with the longitude span and with latitude and vanishes at the equator. The
/// lat/lon box is the more intuitive answer for a points-in-window query and, unlike
/// PostGIS geography, carries no "< 180° per edge" restriction.
///
/// To adopt the PostGIS great-circle-quad semantics instead, two pieces change and
/// nothing else (the filter/refine structure and the index are untouched): this
/// predicate becomes a point-in-spherical-quadrilateral test (the point is inside
/// iff it is on the interior side of all four edge great circles, whose normals are
/// `cross(corner_i, corner_{i+1})` over the unit-vector corners), and
/// [`rectangle_bounding_box`] must inflate the filter box to enclose the poleward
/// bulge of the top/bottom arcs (sample each arc, or take its great-circle vertex).
pub(crate) fn rectangle_contains(
    lower: GeodeticCoord,
    upper: GeodeticCoord,
    p: GeodeticCoord,
) -> bool {
    if p.lat < lower.lat || p.lat > upper.lat {
        return false;
    }
    if p.lat.abs() == 90.0 {
        return true;
    }
    arc_contains(lower.lon, upper.lon, p.lon)
}

#[cfg(test)]
mod tests {
    use super::{
        UnitVec, arc_contains, interval_product, rectangle_bounding_box, rectangle_contains,
        squared_chord,
    };
    use crate::geodetic::coord::GeodeticCoord;
    use crate::{Envelope, Point};
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

    #[test]
    fn arc_contains_non_wrapping_and_wrapping() {
        // Ordinary interval, inclusive at both ends.
        assert!(arc_contains(10.0, 20.0, 15.0));
        assert!(arc_contains(10.0, 20.0, 10.0));
        assert!(arc_contains(10.0, 20.0, 20.0));
        assert!(!arc_contains(10.0, 20.0, 25.0));

        // Wrapping arc across the antimeridian (170 -> 180 -> -170).
        assert!(arc_contains(170.0, -170.0, 175.0));
        assert!(arc_contains(170.0, -170.0, 180.0));
        assert!(arc_contains(170.0, -170.0, -175.0));
        assert!(!arc_contains(170.0, -170.0, 0.0));
        assert!(!arc_contains(170.0, -170.0, 160.0));
    }

    #[test]
    fn interval_product_covers_sign_combinations() {
        assert_eq!(interval_product(1.0, 2.0, 3.0, 4.0), (3.0, 8.0));
        assert_eq!(interval_product(-1.0, 2.0, 3.0, 4.0), (-4.0, 8.0));
        assert_eq!(interval_product(-1.0, 2.0, -3.0, 4.0), (-6.0, 8.0));
    }

    #[test]
    fn rectangle_contains_semantics() {
        let lo = GeodeticCoord {
            lon: 10.0,
            lat: 40.0,
        };
        let hi = GeodeticCoord {
            lon: 20.0,
            lat: 50.0,
        };
        assert!(rectangle_contains(
            lo,
            hi,
            GeodeticCoord {
                lon: 15.0,
                lat: 45.0
            }
        ));
        assert!(!rectangle_contains(
            lo,
            hi,
            GeodeticCoord {
                lon: 15.0,
                lat: 55.0
            }
        ));
        assert!(!rectangle_contains(
            lo,
            hi,
            GeodeticCoord {
                lon: 25.0,
                lat: 45.0
            }
        ));

        // Wrapping longitude arc selects either side of the seam, not the far side.
        let wlo = GeodeticCoord {
            lon: 170.0,
            lat: -10.0,
        };
        let whi = GeodeticCoord {
            lon: -170.0,
            lat: 10.0,
        };
        assert!(rectangle_contains(
            wlo,
            whi,
            GeodeticCoord {
                lon: 179.0,
                lat: 0.0
            }
        ));
        assert!(rectangle_contains(
            wlo,
            whi,
            GeodeticCoord {
                lon: -179.0,
                lat: 0.0
            }
        ));
        assert!(!rectangle_contains(
            wlo,
            whi,
            GeodeticCoord { lon: 0.0, lat: 0.0 }
        ));

        // A pole is included whenever the latitude band reaches it, regardless of
        // longitude; a non-pole point outside the arc is not.
        let plo = GeodeticCoord {
            lon: 100.0,
            lat: 80.0,
        };
        let phi = GeodeticCoord {
            lon: 120.0,
            lat: 90.0,
        };
        assert!(rectangle_contains(
            plo,
            phi,
            GeodeticCoord {
                lon: 0.0,
                lat: 90.0
            }
        ));
        assert!(!rectangle_contains(
            plo,
            phi,
            GeodeticCoord {
                lon: 0.0,
                lat: 85.0
            }
        ));
    }

    #[test]
    fn rectangle_bounding_box_contains_region_samples() {
        let lo = GeodeticCoord {
            lon: 10.0,
            lat: 40.0,
        };
        let hi = GeodeticCoord {
            lon: 20.0,
            lat: 50.0,
        };
        let bbox = rectangle_bounding_box(lo, hi);
        // Corners, edge midpoints and centre all embed inside the box.
        for (lon, lat) in [
            (10.0, 40.0),
            (20.0, 40.0),
            (10.0, 50.0),
            (20.0, 50.0),
            (15.0, 40.0),
            (15.0, 50.0),
            (10.0, 45.0),
            (20.0, 45.0),
            (15.0, 45.0),
        ] {
            let v = GeodeticCoord { lon, lat }.to_unit_vector();
            assert!(bbox.contains_point(&v), "box should contain ({lon}, {lat})");
        }
    }

    #[test]
    fn rectangle_bounding_box_pins_cardinal_directions() {
        // A band straddling lon 0 and the equator reaches x = +1 at (0, 0).
        let bbox = rectangle_bounding_box(
            GeodeticCoord {
                lon: -10.0,
                lat: -10.0,
            },
            GeodeticCoord {
                lon: 10.0,
                lat: 10.0,
            },
        );
        assert!(bbox.contains_point(&GeodeticCoord { lon: 0.0, lat: 0.0 }.to_unit_vector()));
        assert_relative_eq!(bbox.upper().0[0], 1.0, epsilon = 1e-9);
    }
}
