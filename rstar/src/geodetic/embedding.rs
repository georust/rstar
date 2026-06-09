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
/// [`squared_chord_to_metres`](crate::geodetic::squared_chord_to_metres). See
/// [`GeodeticObject`](crate::geodetic::GeodeticObject) for a worked example.
pub fn squared_chord(a: UnitVec, b: UnitVec) -> f64 {
    // Routed through `PointExt` so the leaf metric and the envelope metric
    // (`AABB::distance_2`, which uses the same `sub`/`length_2` arithmetic) stay
    // a single implementation: the pruning lower bound requires them to agree.
    crate::point::PointExt::distance_2(&a, &b)
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
    let (sin_lat_lo, cos_lat_lo) = lat_lo.sin_cos();
    let (sin_lat_hi, cos_lat_hi) = lat_hi.sin_cos();

    // z = sin(lat): sin is increasing on [-90, 90] and lower.lat <= upper.lat.
    let z_min = sin_lat_lo;
    let z_max = sin_lat_hi;

    // cos(lat) >= 0; it peaks at the equator, so the band maximum is 1 when the band
    // straddles lat 0, otherwise the cosine of the parallel nearer the equator.
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
    let (sin_lon_lo, cos_lon_lo) = lo.to_radians().sin_cos();
    let (sin_lon_hi, cos_lon_hi) = hi.to_radians().sin_cos();
    let coslon_max = if arc_contains(lo, hi, 0.0) {
        1.0
    } else {
        cos_lon_lo.max(cos_lon_hi)
    };
    let coslon_min = if arc_contains(lo, hi, 180.0) || arc_contains(lo, hi, -180.0) {
        -1.0
    } else {
        cos_lon_lo.min(cos_lon_hi)
    };
    let sinlon_max = if arc_contains(lo, hi, 90.0) {
        1.0
    } else {
        sin_lon_lo.max(sin_lon_hi)
    };
    let sinlon_min = if arc_contains(lo, hi, -90.0) {
        -1.0
    } else {
        sin_lon_lo.min(sin_lon_hi)
    };

    let (x_min, x_max) = interval_product(cosphi_min, cosphi_max, coslon_min, coslon_max);
    let (y_min, y_max) = interval_product(cosphi_min, cosphi_max, sinlon_min, sinlon_max);

    // Expand outward by a small margin so the filter never drops a point the refine
    // would keep. This guards two things: ordinary floating-point drift at the box
    // faces, and the pole degeneracy – a pole point embeds to exactly (0, 0, ±1),
    // while a band reaching the pole derives its x/y bounds from cos(90°) ≈ 6e-17,
    // so the exact zero can sit just outside them. The margin (≈ 6 µm on the
    // sphere) is far below any meaningful separation, and the refine step removes
    // the few extra candidates it admits.
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
/// A point on the ±180° seam is matched under either sign: `+180` and `-180` are the
/// same meridian, so the point is inside whenever *either* spelling of its longitude
/// lies in the arc. This makes a window whose edge sits on the seam include a seam
/// point stored with the opposite sign (without it, the inclusive bound would hold
/// for one spelling and silently drop the other).
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
    // ±180° denote the same meridian; accept a seam point under either spelling, so
    // a window edge on the seam includes a point stored with the opposite sign. This
    // mirrors the cardinal pinning in `rectangle_bounding_box`, which likewise tests
    // both ±180° spellings.
    arc_contains(lower.lon, upper.lon, p.lon)
        || (p.lon.abs() == 180.0 && arc_contains(lower.lon, upper.lon, -p.lon))
}

#[cfg(test)]
mod tests {
    use super::{
        arc_contains, interval_product, rectangle_bounding_box, rectangle_contains, squared_chord,
        UnitVec,
    };
    use crate::geodetic::coord::GeodeticCoord;
    use crate::{Envelope, Point};
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
    fn rectangle_contains_seam_point_under_either_spelling() {
        // A non-wrapping window whose east edge sits on the seam (170 -> 180). A seam
        // point is the same meridian whether stored as +180 or -180, so both spellings
        // must be inside; a point well inside is in, one outside the arc is out.
        let east_edge_lo = GeodeticCoord {
            lon: 170.0,
            lat: -10.0,
        };
        let east_edge_hi = GeodeticCoord {
            lon: 180.0,
            lat: 10.0,
        };
        for lon in [180.0, -180.0] {
            assert!(
                rectangle_contains(east_edge_lo, east_edge_hi, GeodeticCoord { lon, lat: 0.0 }),
                "seam point lon={lon} should be inside a window whose east edge is the seam"
            );
        }
        assert!(rectangle_contains(
            east_edge_lo,
            east_edge_hi,
            GeodeticCoord {
                lon: 175.0,
                lat: 0.0
            }
        ));
        assert!(!rectangle_contains(
            east_edge_lo,
            east_edge_hi,
            GeodeticCoord {
                lon: 160.0,
                lat: 0.0
            }
        ));

        // Symmetric case: a non-wrapping window whose west edge is the seam
        // (-180 -> -170). Again both seam spellings are inside.
        let west_edge_lo = GeodeticCoord {
            lon: -180.0,
            lat: -10.0,
        };
        let west_edge_hi = GeodeticCoord {
            lon: -170.0,
            lat: 10.0,
        };
        for lon in [180.0, -180.0] {
            assert!(
                rectangle_contains(west_edge_lo, west_edge_hi, GeodeticCoord { lon, lat: 0.0 }),
                "seam point lon={lon} should be inside a window whose west edge is the seam"
            );
        }

        // A window nowhere near the seam never admits a seam point under either sign.
        let inland_lo = GeodeticCoord {
            lon: 10.0,
            lat: -10.0,
        };
        let inland_hi = GeodeticCoord {
            lon: 20.0,
            lat: 10.0,
        };
        for lon in [180.0, -180.0] {
            assert!(
                !rectangle_contains(inland_lo, inland_hi, GeodeticCoord { lon, lat: 0.0 }),
                "seam point lon={lon} must stay outside an inland window"
            );
        }
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
