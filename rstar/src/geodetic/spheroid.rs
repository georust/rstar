//! Ellipsoidal (geodesic) refine for the point tree.
//!
//! The spherical index is used as a conservative filter and the exact geodesic
//! distance on a reference ellipsoid (Karney's algorithm, via `geographiclib-rs`) as
//! the exact metric: a filter/refine scheme. Only compiled with the `geodetic-wgs84`
//! feature, which pulls in `geographiclib-rs` – a `std` crate – so the base `geodetic`
//! feature stays no_std.
//!
//! # What "WGS84" means here
//!
//! The default ellipsoid, [`Ellipsoid::WGS84`], is the WGS84 reference *ellipsoid*: a
//! geometric surface fixed by its equatorial radius `a = 6_378_137.0 m` and flattening
//! `1/f = 298.257_223_563`. That figure is part of the defining constants of the
//! system and is shared, unchanged, by every realisation in the WGS84 datum ensemble
//! (the original Doppler frame, G730, G873, …, G2296). It carries no epoch, because an
//! epoch only distinguishes *realisations* of where a point sits, and this module does
//! no datum or reference-frame transformation: it measures the geodesic between the two
//! coordinates it is given, on the chosen ellipsoid. The ensemble's roughly two-metre
//! positional spread therefore lives in the input coordinates (inherited from however
//! they were surveyed), not in the metric, and selecting an epoch would have nothing to
//! act on. Pass [`Ellipsoid::GRS80`] or a custom [`Ellipsoid::new`] to measure on a
//! different ellipsoid.

use geographiclib_rs::{Geodesic, InverseGeodesic};

use super::distance::EARTH_RADIUS_METRES;
use super::GeodeticCoord;

/// A reference ellipsoid of revolution: the geometric surface on which geodesic
/// distances are measured. Fixed by its equatorial (semi-major) radius `a` and its
/// flattening `f`; it carries no epoch or datum realisation (see the [module
/// docs](crate::geodetic)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipsoid {
    /// Equatorial (semi-major) radius `a`, in metres.
    pub equatorial_radius: f64,
    /// Flattening `f = (a - b) / a` (dimensionless).
    pub flattening: f64,
}

impl Ellipsoid {
    /// The WGS84 reference ellipsoid: `a = 6_378_137.0 m`, `1/f = 298.257_223_563`.
    ///
    /// Shared by every realisation in the WGS84 ensemble. The flattening is written the
    /// way `geographiclib-rs` writes it (`1 / (298_257_223_563 / 1e9)`) so that this
    /// constant and `geographiclib_rs::Geodesic::wgs84()` agree to the bit.
    pub const WGS84: Ellipsoid = Ellipsoid {
        equatorial_radius: 6_378_137.0,
        flattening: 1.0 / (298_257_223_563.0 / 1_000_000_000.0),
    };

    /// The GRS80 reference ellipsoid: `a = 6_378_137.0 m`, `1/f = 298.257_222_101`.
    ///
    /// Differs from [`WGS84`](Self::WGS84) only in the trailing digits of the
    /// flattening, a sub-millimetre difference on the surface.
    pub const GRS80: Ellipsoid = Ellipsoid {
        equatorial_radius: 6_378_137.0,
        flattening: 1.0 / (298_257_222_101.0 / 1_000_000_000.0),
    };

    /// An ellipsoid from its equatorial radius `a` (metres) and flattening `f`.
    pub const fn new(equatorial_radius: f64, flattening: f64) -> Self {
        Self {
            equatorial_radius,
            flattening,
        }
    }

    /// An ellipsoid from its equatorial radius `a` (metres) and inverse flattening
    /// `1/f`, the form ellipsoids are usually published in.
    pub const fn from_inverse_flattening(equatorial_radius: f64, inverse_flattening: f64) -> Self {
        Self::new(equatorial_radius, 1.0 / inverse_flattening)
    }

    /// Polar (semi-minor) radius `b = a(1 - f)`, in metres.
    pub const fn polar_radius(self) -> f64 {
        self.equatorial_radius * (1.0 - self.flattening)
    }
}

/// A safety factor applied to the curvature-span deviation in
/// [`geodesic_spherical_margin`]. The deviation is a length-scale argument; the factor
/// is headroom against the difference between the geodetic-latitude surface normals the
/// spherical embedding uses and the geocentric directions, and is validated for the
/// supported ellipsoids by the property tests.
const MARGIN_HEADROOM: f64 = 2.0;

/// Relative bound on `|geodesic / spherical - 1|` for any pair of points, given the
/// reference `ellipsoid` and the sphere the index embeds onto (radius
/// [`EARTH_RADIUS_METRES`]).
///
/// The ellipsoid's principal radii of curvature span `[b^2/a, a^2/b]` (equatorial
/// meridional to polar). A spherical great-circle length is a central angle times the
/// fixed sphere radius; the geodesic length is the same kind of angle times a local
/// length scale lying in that curvature span, so the ratio of the two stays within
/// `[b^2/a, a^2/b] / R`. We take the larger one-sided deviation from 1 and pad it by
/// [`MARGIN_HEADROOM`].
///
/// The refine uses the margin to turn a spherical distance into a sound bound on the
/// geodesic distance: deflated it is a lower bound (so nearest-neighbour search never
/// stops early), inflated it widens the radius ball (so a radius query never drops an
/// in-range point). Over-estimating the margin only ever widens the candidate set and
/// stays sound; under-estimating it would not.
pub(crate) const fn geodesic_spherical_margin(ellipsoid: Ellipsoid) -> f64 {
    let a = ellipsoid.equatorial_radius;
    let b = ellipsoid.polar_radius();
    let min_curvature = b * b / a;
    let max_curvature = a * a / b;
    let up = max_curvature / EARTH_RADIUS_METRES - 1.0;
    let down = 1.0 - min_curvature / EARTH_RADIUS_METRES;
    let deviation = if up > down { up } else { down };
    deviation * MARGIN_HEADROOM
}

/// The WGS84 value of [`geodesic_spherical_margin`] (about 1.1%), the named figure for
/// the WGS84 ellipsoid that the soundness property test checks against.
///
/// For WGS84 the curvature span runs from the equatorial meridional
/// `a(1 - e^2) ~= 6_335_439 m` to the polar prime-vertical `a^2/b ~= 6_399_594 m`,
/// against the sphere's `R = 6_371_008.8 m`, a one-sided deviation of about `0.56%`
/// that [`MARGIN_HEADROOM`] doubles. The query paths call
/// [`geodesic_spherical_margin`] directly, so this named form is only used by the tests.
#[cfg(test)]
pub(crate) const GEODESIC_SPHERICAL_MAX_REL_ERROR: f64 =
    geodesic_spherical_margin(Ellipsoid::WGS84);

/// The `geographiclib-rs` geodesic model for `ellipsoid`. Building it computes the
/// geodesic series coefficients, so a query builds it once and reuses it across
/// candidate refinements rather than rebuilding per pair.
pub(crate) fn geoid_for(ellipsoid: Ellipsoid) -> Geodesic {
    // For the WGS84 ellipsoid reuse geographiclib's cached singleton (and its
    // precomputed series) rather than rebuilding it. Bit-pattern equality is exact and
    // deliberate: only the WGS84 constant, or a copy of it, takes this path.
    if ellipsoid.equatorial_radius.to_bits() == Ellipsoid::WGS84.equatorial_radius.to_bits()
        && ellipsoid.flattening.to_bits() == Ellipsoid::WGS84.flattening.to_bits()
    {
        Geodesic::wgs84()
    } else {
        Geodesic::new(ellipsoid.equatorial_radius, ellipsoid.flattening)
    }
}

/// Geodesic distance in metres between two coordinates on a prebuilt `geoid` (Karney's
/// inverse solution). The `geographiclib-rs` argument order is latitude first.
pub(crate) fn geodesic_metres(geoid: &Geodesic, a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    geoid.inverse(a.lat, a.lon, b.lat, b.lon)
}

/// A sound lower bound on the geodesic distance for a pair, given their spherical
/// great-circle distance and the ellipsoid's [`geodesic_spherical_margin`]: deflated by
/// that margin.
pub(crate) fn spherical_lower_bound_metres(spherical_metres: f64, margin: f64) -> f64 {
    spherical_metres * (1.0 - margin)
}

/// A small additive floor (metres) added to the radius-query fetch radius.
///
/// The multiplicative margin collapses to zero as the radius does, but the spherical
/// and geodesic metrics still round differently at the sub-ulp level for (near)
/// coincident points: geographiclib can return an exact `0` where the embedding chord
/// is a denormal positive. The floor keeps the fetch a superset there. Over-fetching is
/// harmless because the exact geodesic filter is the final arbiter, so this only widens
/// the candidate set, never the result.
const RADIUS_FETCH_FLOOR_METRES: f64 = 1e-3;

/// The spherical radius whose ball is guaranteed to contain every point within
/// `radius_metres` geodesic of a query: inflated by the ellipsoid's `margin`
/// ([`geodesic_spherical_margin`]) and the [`RADIUS_FETCH_FLOOR_METRES`] floor.
pub(crate) fn radius_fetch_metres(radius_metres: f64, margin: f64) -> f64 {
    radius_metres / (1.0 - margin) + RADIUS_FETCH_FLOOR_METRES
}

/// Geodesic distance in metres between two coordinates on the given reference
/// `ellipsoid` (Karney's inverse solution via `geographiclib-rs`).
///
/// A user-facing reference, mirroring [`super::distance::haversine_distance`]; it builds
/// the ellipsoid model on each call, so prefer the tree's `*_on_ellipsoid` query methods
/// for bulk work. For the WGS84 ellipsoid see [`geodesic_distance_wgs84`].
///
/// # Units
///
/// - `a`, `b`: [`GeodeticCoord`] with `lon`/`lat` in degrees
/// - returns: distance in metres on `ellipsoid`
pub fn geodesic_distance(a: GeodeticCoord, b: GeodeticCoord, ellipsoid: Ellipsoid) -> f64 {
    geodesic_metres(&geoid_for(ellipsoid), a, b)
}

/// Geodesic distance in metres between two coordinates on the WGS84 ellipsoid;
/// `geodesic_distance(a, b, Ellipsoid::WGS84)`.
///
/// # Units
///
/// - `a`, `b`: [`GeodeticCoord`] with `lon`/`lat` in degrees
/// - returns: distance in metres on the WGS84 ellipsoid
pub fn geodesic_distance_wgs84(a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    geodesic_distance(a, b, Ellipsoid::WGS84)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    // --- the Ellipsoid type ---

    /// `Ellipsoid::WGS84` is bit-identical to the model `geographiclib-rs` builds for
    /// WGS84, so `geoid_for(Ellipsoid::WGS84)` takes the cached-singleton path and every
    /// geodesic result matches geographiclib exactly.
    #[test]
    fn wgs84_constant_matches_geographiclib() {
        let geoid = Geodesic::wgs84();
        assert_eq!(
            Ellipsoid::WGS84.equatorial_radius,
            geoid.equatorial_radius()
        );
        assert_eq!(Ellipsoid::WGS84.flattening, geoid.flattening());
    }

    /// The published inverse flattenings, via `from_inverse_flattening`.
    #[test]
    fn named_ellipsoids_have_their_published_figures() {
        assert_eq!(Ellipsoid::WGS84.equatorial_radius, 6_378_137.0);
        assert_relative_eq!(
            1.0 / Ellipsoid::WGS84.flattening,
            298.257_223_563,
            epsilon = 1e-9
        );
        assert_relative_eq!(
            1.0 / Ellipsoid::GRS80.flattening,
            298.257_222_101,
            epsilon = 1e-9
        );
        assert_eq!(
            Ellipsoid::from_inverse_flattening(6_378_137.0, 298.257_223_563),
            Ellipsoid::WGS84
        );
        // b = a(1 - f); WGS84 polar radius is the textbook 6_356_752.314 m.
        assert_relative_eq!(
            Ellipsoid::WGS84.polar_radius(),
            6_356_752.314_245,
            epsilon = 1e-3
        );
    }

    // --- geodesic distance anchored to textbook ellipsoid values ---

    /// One degree of longitude along the equator is the WGS84 equatorial radius times
    /// one degree in radians: `a * pi / 180 = 6_378_137 * pi / 180 = 111_319.4908 m`.
    /// Independent of `geographiclib-rs` (it is just the equatorial circumference / 360).
    #[test]
    fn geodesic_equatorial_degree_matches_equatorial_radius() {
        let d = geodesic_distance_wgs84(coord(0.0, 0.0), coord(1.0, 0.0));
        assert_relative_eq!(d, 111_319.490_8, epsilon = 1e-2);
    }

    /// The WGS84 meridian quadrant – the geodesic from the equator to the pole along a
    /// meridian – is the published constant `10_001_965.729 m`, fixed by the ellipsoid's
    /// flattening (independently of `geographiclib-rs`: e.g. via the third-flattening
    /// series `a/(1+n) * (1 + n^2/4 + ...) * pi/2`). A flattening error would show here
    /// even though it stays inside the spherical margin checked elsewhere.
    #[test]
    fn geodesic_meridian_quadrant_matches_published_constant() {
        let d = geodesic_distance_wgs84(coord(0.0, 0.0), coord(0.0, 90.0));
        assert_relative_eq!(d, 10_001_965.729, epsilon = 1.0);
    }

    #[test]
    fn geodesic_is_symmetric_and_zero_on_self() {
        let a = coord(-74.006, 40.7128);
        let b = coord(-0.1278, 51.5074);
        assert_relative_eq!(
            geodesic_distance_wgs84(a, b),
            geodesic_distance_wgs84(b, a),
            epsilon = 1e-6
        );
        assert_relative_eq!(geodesic_distance_wgs84(a, a), 0.0, epsilon = 1e-6);
    }

    /// The WGS84 convenience function is exactly the general one on `Ellipsoid::WGS84`,
    /// and a different ellipsoid gives a different (here, slightly larger) distance.
    #[test]
    fn geodesic_distance_dispatches_on_ellipsoid() {
        let a = coord(-74.006, 40.7128);
        let b = coord(-0.1278, 51.5074);
        assert_eq!(
            geodesic_distance_wgs84(a, b),
            geodesic_distance(a, b, Ellipsoid::WGS84)
        );
        // GRS80 differs from WGS84 only in the flattening's trailing digits, so the same
        // pair is within a millimetre over this ~5600 km transatlantic line.
        assert_relative_eq!(
            geodesic_distance(a, b, Ellipsoid::GRS80),
            geodesic_distance_wgs84(a, b),
            epsilon = 1e-3
        );
    }

    // --- the soundness margin, validated against geographiclib-rs ---

    #[test]
    fn lower_bound_and_fetch_bracket_the_spherical_distance() {
        // The deflated bound is below, the inflated fetch radius above, the spherical
        // value, both by exactly the margin.
        let s = 1_000_000.0;
        let margin = GEODESIC_SPHERICAL_MAX_REL_ERROR;
        assert_relative_eq!(
            spherical_lower_bound_metres(s, margin),
            s * (1.0 - margin),
            epsilon = 1e-6
        );
        assert!(radius_fetch_metres(s, margin) > s);
        assert!(spherical_lower_bound_metres(s, margin) < s);
    }
}
