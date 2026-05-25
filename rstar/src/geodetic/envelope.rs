use crate::point::{Point, PointExt, max_inline};
use crate::{Envelope, RTreeObject};

use super::coord::GeodeticCoord;

/// A geodetic minimum bounding rectangle: a lat/lon rectangle stored as
/// axis-aligned lower/upper corner points (same layout as [`crate::AABB`]).
/// Coordinates are in degrees.
///
/// Does **not** wrap across the antimeridian (`lower.lon <= upper.lon` is
/// assumed). Callers must split or duplicate items that span ±180 ° at
/// insertion time.
///
/// Note: `area`, `perimeter_value`, and `intersection_area` are computed in degree
/// space and serve only as R*-tree heuristics; they are not geodetic areas or lengths.
/// The degree-space heuristic over-weights polar boxes (a 1° × 1° box near a pole
/// covers far less surface than one at the equator yet scores the same), so split
/// quality may degrade on datasets concentrated near the poles. Query correctness is
/// unaffected: pruning uses [`Envelope::distance_2`], which is exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeodeticEnvelope {
    // Lower-left corner (min lon, min lat).
    pub(crate) lower: GeodeticCoord,
    // Upper-right corner (max lon, max lat).
    pub(crate) upper: GeodeticCoord,
}

impl GeodeticEnvelope {
    /// Returns the lower-left corner (minimum longitude and latitude, in degrees).
    pub fn lower(&self) -> GeodeticCoord {
        self.lower
    }

    /// Returns the upper-right corner (maximum longitude and latitude, in degrees).
    pub fn upper(&self) -> GeodeticCoord {
        self.upper
    }
}

impl Envelope for GeodeticEnvelope {
    type Point = GeodeticCoord;

    fn new_empty() -> Self {
        Self {
            lower: GeodeticCoord::from_value(f64::MAX), // most-positive finite f64: sentinel min
            upper: GeodeticCoord::from_value(f64::MIN), // most-negative finite f64 (not MIN_POSITIVE): sentinel max
        }
    }

    fn is_empty(&self) -> bool {
        self.lower.lon > self.upper.lon
    }

    fn contains_point(&self, point: &GeodeticCoord) -> bool {
        self.lower.all_component_wise(point, |x, y| x <= y)
            && self.upper.all_component_wise(point, |x, y| x >= y)
    }

    fn contains_envelope(&self, other: &Self) -> bool {
        self.lower.all_component_wise(&other.lower, |l, r| l <= r)
            && self.upper.all_component_wise(&other.upper, |l, r| l >= r)
    }

    fn merge(&mut self, other: &Self) {
        self.lower = self.lower.min_point(&other.lower);
        self.upper = self.upper.max_point(&other.upper);
    }

    fn merged(&self, other: &Self) -> Self {
        Self {
            lower: self.lower.min_point(&other.lower),
            upper: self.upper.max_point(&other.upper),
        }
    }

    fn intersects(&self, other: &Self) -> bool {
        self.lower.all_component_wise(&other.upper, |l, r| l <= r)
            && self.upper.all_component_wise(&other.lower, |l, r| l >= r)
    }

    fn intersection_area(&self, other: &Self) -> f64 {
        Self {
            lower: self.lower.max_point(&other.lower),
            upper: self.upper.min_point(&other.upper),
        }
        .area()
    }

    fn area(&self) -> f64 {
        let diag = self.upper.sub(&self.lower);
        diag.fold(1.0_f64, |acc, cur| max_inline(cur, 0.0_f64) * acc)
    }

    /// Great-circle distance in **metres** from the nearest point on the envelope's
    /// boundary to `point` (0 if the point is inside). Note: despite the trait method
    /// name, this is the raw haversine distance, not a squared value. It must use the
    /// same metric as [`crate::PointDistance::distance_2`] on the leaf type.
    fn distance_2(&self, point: &GeodeticCoord) -> f64 {
        crate::geodetic::distance::point_to_mbr_distance(
            *point,
            self.lower.lon,
            self.lower.lat,
            self.upper.lon,
            self.upper.lat,
        )
    }

    fn min_max_dist_2(&self, _point: &GeodeticCoord) -> f64 {
        // TODO(geodetic): derive a tight upper bound. The Roussopoulos/Kelley/Vincent
        // min-max-distance lemma still holds on the sphere, but the analytical derivation
        // is non-trivial because the far point on a parallel is not generally a corner.
        // Returning MAX disables this pruning optimisation in `RTree::nearest_neighbor`;
        // the `_iter` variants and `nearest_neighbors` are unaffected.
        f64::MAX
    }

    fn center(&self) -> GeodeticCoord {
        self.lower.component_wise(&self.upper, |x, y| (x + y) / 2.0)
    }

    fn perimeter_value(&self) -> f64 {
        let diag = self.upper.sub(&self.lower);
        max_inline(diag.fold(0.0_f64, |acc, value| acc + value), 0.0_f64)
    }

    fn sort_envelopes<T: RTreeObject<Envelope = Self>>(axis: usize, envelopes: &mut [T]) {
        envelopes.sort_unstable_by(|l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .partial_cmp(&r.envelope().lower.nth(axis))
                .unwrap()
        });
    }

    fn partition_envelopes<T: RTreeObject<Envelope = Self>>(
        axis: usize,
        envelopes: &mut [T],
        selection_size: usize,
    ) {
        envelopes.select_nth_unstable_by(selection_size, |l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .partial_cmp(&r.envelope().lower.nth(axis))
                .unwrap()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::GeodeticEnvelope;
    use crate::Envelope;
    use crate::geodetic::coord::GeodeticCoord;
    use crate::geodetic::distance::haversine_distance;
    use approx::assert_relative_eq;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    fn envelope(lon_l: f64, lat_l: f64, lon_h: f64, lat_h: f64) -> GeodeticEnvelope {
        GeodeticEnvelope {
            lower: coord(lon_l, lat_l),
            upper: coord(lon_h, lat_h),
        }
    }

    // new_empty merged with a single point yields a point envelope.
    #[test]
    fn new_empty_merged_with_point_yields_point_envelope() {
        let mut env = GeodeticEnvelope::new_empty();
        let pt = coord(10.0, 40.0);
        let pt_env = envelope(10.0, 40.0, 10.0, 40.0);
        env.merge(&pt_env);
        assert_eq!(env, pt_env);
        assert!(env.contains_point(&pt));
    }

    // `new_empty` reports as empty before any merge.
    #[test]
    fn new_empty_is_empty() {
        assert!(GeodeticEnvelope::new_empty().is_empty());
    }

    // `merged` (the non-mutating sibling of `merge`) absorbs an empty envelope on the
    // left into a finite envelope on the right. Pins the f64::MAX/f64::MIN sentinel
    // contract for `min_point`/`max_point`.
    #[test]
    fn merged_empty_with_finite_returns_finite() {
        let pt_env = envelope(10.0, 40.0, 20.0, 50.0);
        let result = GeodeticEnvelope::new_empty().merged(&pt_env);
        assert_eq!(result, pt_env);
        assert!(!result.is_empty());
    }

    // Symmetric case: finite-on-the-left, empty-on-the-right.
    #[test]
    fn merged_finite_with_empty_returns_finite() {
        let pt_env = envelope(10.0, 40.0, 20.0, 50.0);
        let result = pt_env.merged(&GeodeticEnvelope::new_empty());
        assert_eq!(result, pt_env);
    }

    // contains_point: inside, on boundary, outside.
    #[test]
    fn contains_point_inside() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert!(env.contains_point(&coord(15.0, 45.0)));
    }

    #[test]
    fn contains_point_on_boundary() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert!(env.contains_point(&coord(10.0, 40.0)));
        assert!(env.contains_point(&coord(20.0, 50.0)));
        assert!(env.contains_point(&coord(10.0, 50.0)));
        assert!(env.contains_point(&coord(20.0, 40.0)));
    }

    #[test]
    fn contains_point_outside() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert!(!env.contains_point(&coord(25.0, 45.0)));
        assert!(!env.contains_point(&coord(5.0, 45.0)));
        assert!(!env.contains_point(&coord(15.0, 55.0)));
        assert!(!env.contains_point(&coord(15.0, 35.0)));
    }

    // contains_envelope: a sub-rectangle is contained; an overlapping-but-not-contained one is not.
    #[test]
    fn contains_envelope_sub_rectangle() {
        let outer = envelope(10.0, 40.0, 20.0, 50.0);
        let inner = envelope(12.0, 42.0, 18.0, 48.0);
        assert!(outer.contains_envelope(&inner));
    }

    #[test]
    fn contains_envelope_overlapping_not_contained() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        let overlapping = envelope(15.0, 45.0, 25.0, 55.0);
        assert!(!env.contains_envelope(&overlapping));
    }

    // merge / merged: (10,40)-(20,50) merged with point (25,55) gives lower=(10,40), upper=(25,55).
    #[test]
    fn merge_extends_envelope() {
        let mut env = envelope(10.0, 40.0, 20.0, 50.0);
        let pt_env = envelope(25.0, 55.0, 25.0, 55.0);
        env.merge(&pt_env);
        assert_eq!(env.lower, coord(10.0, 40.0));
        assert_eq!(env.upper, coord(25.0, 55.0));
    }

    #[test]
    fn merged_does_not_mutate_self() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        let pt_env = envelope(25.0, 55.0, 25.0, 55.0);
        let result = env.merged(&pt_env);
        assert_eq!(result.lower, coord(10.0, 40.0));
        assert_eq!(result.upper, coord(25.0, 55.0));
        // Original is unchanged.
        assert_eq!(env.upper, coord(20.0, 50.0));
    }

    // intersects: overlapping vs disjoint.
    #[test]
    fn intersects_overlapping() {
        let a = envelope(10.0, 40.0, 20.0, 50.0);
        let b = envelope(15.0, 45.0, 25.0, 55.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn intersects_disjoint() {
        let a = envelope(10.0, 40.0, 20.0, 50.0);
        let b = envelope(30.0, 40.0, 40.0, 50.0);
        assert!(!a.intersects(&b));
        assert!(!b.intersects(&a));
    }

    // area: rectangle (10,40)-(20,50) has area 10*10 = 100 degree-squared units.
    #[test]
    fn area_known_rectangle() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert_relative_eq!(env.area(), 100.0, epsilon = 1e-10);
    }

    // center: rectangle (10,40)-(20,50) has center (15,45).
    #[test]
    fn center_known_rectangle() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert_eq!(env.center(), coord(15.0, 45.0));
    }

    #[test]
    fn lower_and_upper_accessors_return_corners() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert_eq!(env.lower(), coord(10.0, 40.0));
        assert_eq!(env.upper(), coord(20.0, 50.0));
    }

    // distance_2: degenerate envelope (lower == upper == P) to external Q equals haversine_distance(P, Q).
    #[test]
    fn distance_2_degenerate_envelope_matches_haversine() {
        let p = coord(-0.1278, 51.5074); // London
        let q = coord(-74.006, 40.7128); // NYC
        let degenerate = envelope(p.lon, p.lat, p.lon, p.lat);
        let d = degenerate.distance_2(&q);
        let expected = haversine_distance(p, q);
        assert_relative_eq!(d, expected, epsilon = 1e-6);
    }

    // distance_2: query inside the rectangle gives 0.
    #[test]
    fn distance_2_inside_is_zero() {
        let env = envelope(10.0, 40.0, 20.0, 50.0);
        assert_relative_eq!(env.distance_2(&coord(15.0, 45.0)), 0.0, epsilon = 1e-10);
    }
}
