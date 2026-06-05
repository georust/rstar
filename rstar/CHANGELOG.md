# Unreleased

## Added
- `geodetic::GeodeticLineString`: a great-circle polyline leaf for the geodetic R-tree,
  with arc-aware bounding boxes and nearest-point distance. Built via
  `try_from_lonlat` from `(f64, f64)` / `[f64; 2]` / `GeodeticCoord` coordinates.
- `geodetic::GeodeticPolygon` and `geodetic::GeodeticRing`: a filled spherical-polygon
  leaf with great-circle ray-cast membership (zero distance inside, exact via robust
  predicates), accepting any simple polygon including ones larger than a hemisphere.
  Ring orientation follows OGC/GeoJSON (exterior counter-clockwise as seen from outside,
  holes clockwise) and is respected as given.
- `GeodeticRTree` is now generic over its leaf type
  (`GeodeticRTree<G: GeodeticObject = GeodeticPoint>`) via the open `GeodeticObject`
  marker trait, so it indexes extent geometry as well as points. The point API is
  unchanged: the default type parameter keeps the bare `GeodeticRTree` spelling.
- Public great-circle primitives for downstream geodetic leaf types:
  `geodetic::{arc_bounding_box, arc_contains_point, nearest_point_on_arc, arc_distance_2}`
  and `geodetic::squared_chord`.
- `geodetic::haversine_distance`, the spherical great-circle reference distance, and
  `geodetic::EARTH_RADIUS_METRES`, the sphere radius the index embeds onto (the GRS80 mean
  radius), exported at the module root alongside `squared_chord_to_metres` and
  `metres_to_squared_chord`.
- `GeodeticError::TooFewPoints`, `GeodeticError::EdgeSpansHalfCircle`, and
  `GeodeticError::RingNotClosed` for the linestring and polygon structural
  preconditions; infallible `From<(f64, f64)>` and `From<[f64; 2]>` for `GeodeticCoord`.
- `geodetic-wgs84` feature: an ellipsoidal geodesic refine for the point
  `GeodeticRTree`, layered on the spherical index as a filter/refine. The reference
  ellipsoid is a `geodetic::Ellipsoid` value (`Ellipsoid::WGS84`, `Ellipsoid::GRS80`, or
  a custom `Ellipsoid::new` / `from_inverse_flattening`); "WGS84" denotes the WGS84
  reference ellipsoid, which is epoch- and realisation-independent, and no datum
  transformation is performed. `nearest_neighbor_on_ellipsoid`,
  `nearest_neighbor_with_distance_on_ellipsoid`, and `locate_within_distance_on_ellipsoid`
  re-rank or filter the spherical candidates by the exact geodesic distance (Karney, via
  `geographiclib-rs`); `geodetic::geodesic_distance(a, b, ellipsoid)` is the standalone
  reference distance. The `_wgs84`-suffixed methods and `geodetic::geodesic_distance_wgs84`
  are shorthands for the WGS84 ellipsoid. The feature requires `std`; the base `geodetic`
  feature stays no_std.


# 0.13.0

## Added
- Added missing re-exports of nearest neighbor iterators so they can be named in downstream crates. ([PR](https://github.com/georust/rstar/pull/186))
- Added nearest neighbor search with distance squared: `nearest_neighbor_with_distance_2` and `nearest_neighbors_with_distance_2` methods ([PR](https://github.com/georust/rstar/pull/191)).
- Added `root` doc examples and traversal docs
- Implemented `RTreeObject` for `Arc<T>` and `Rc<T>`
- Added `Envelope::is_empty`. ([PR](https://github.com/georust/rstar/pull/190))
- New `AABB::from_center` utility constructor
- New `AABB::from_bounds` utility constructor

## Fixed
- Fix excessive memory retention in `bulk_load` from `Vec::split_off` over-capacity
- Fix transcription error in implementation of OMT bulk loading leading to nodes which are too large.

## Changed
- **BREAKING** Made `RStar` methods take `Point` and `Envelope` as owned values where it makes sense ([PR](https://github.com/georust/rstar/pull/189))
- Fix Clippy warning (surfaced in Rust 1.89) related to lifetime elision
- MSRV is now 1.85 (released on 2025-02-20 and shipped in Debian Trixie)
- Fix incorrect assertion message in `verify_parameters` of `rstar/src/params.rs`


# 0.12.2

## Changed
- Reverted the change to `AABB::new_empty` while still avoiding overflow panics applying selections on empty trees ([PR](https://github.com/georust/rstar/pull/184))

# 0.12.1 **YANKED**

## Added
- Provide selection methods based on internal iteration ([PR](https://github.com/georust/rstar/pull/164))
- Add ObjectRef combinator to build tree referencing objects elsewhere ([PR](https://github.com/georust/rstar/pull/178))

## Changed
- Switched to unstable sort for envelopes and node reinsertion ([PR](https://github.com/georust/rstar/pull/160))
- Use a more tame value for `AABB::new_empty` to avoid overflow panics applying selections on empty trees ([PR](https://github.com/georust/rstar/pull/162))
- Avoid infinite recursion due to numerical instability when computing the number of clusters ([PR](https://github.com/georust/rstar/pull/166))


# 0.12.0

## Added
- Add optional support for the [mint](https://docs.rs/mint/0.5.9/mint/index.html) crate
- Implemented `IntoIter` for `RTree`, i.e. added a owning iterator
- Added cached envelope bulk load benchmark
- Implemented `Hash` for `AABB`, `Line`, and `Rectangle`, provided the `Point` used implements `Hash` itself
- Implemented `Default` for `DefaultParams`

## Changed
- Fixed a stack overflow error in `DrainIterator::next`
- Clarified that the distance measure in `distance_2` is not restricted to euclidean distance
- updated to `heapless=0.8`
- Updated CI config to use merge queue ([PR](https://github.com/georust/rstar/pull/143))

# 0.11.0

## Added
- Add `CachedEnvelope` combinator which simplifies memoizing envelope computations. ([PR](https://github.com/georust/rstar/pull/118))
- `Point` is now implemented as const generic for any length of `RTreeNum` array

## Changed
- Increase our MSRV to Rust 1.63 following that of the `geo` crate.  ([PR](https://github.com/georust/rstar/pull/124))

# 0.10.0

## Added
- Added method `RTree::drain()`.
- Changed license field to [SPDX 2.1 license expression](https://spdx.dev/spdx-specification-21-web-version/#h.jxpfx0ykyb60)

## Changed
- fixed all clippy lint issues
- Fixed error when setting MIN_SIZE = 1 in `RTreeParams` and added assert for positive MIN_SIZE
- BREAKING: Removed the `Copy` bound from `Point` and `Envelope`. ([PR](https://github.com/georust/rstar/pull/103))

# 0.9.3
## Changed
- Removed dependency on `pdqselect` ([PR](https://github.com/georust/rstar/pull/85))
- New **minimal supported rust version (MSRV): 1.51.0**
- Replace all usages of `std` with `core` & `alloc` to make `rstar` fit for
  `no_std`. ([PR](https://github.com/georust/rstar/pull/83))
- Updated `heapless` dependency to 0.7 to make use of const generics. ([PR](https://github.com/georust/rstar/pull/87))


# 0.9.2
- Add `RTree::drain_*` methods to remove and drain selected items. ([PR](https://github.com/georust/rstar/pull/77))
- Add trait `Point` for tuples containing elements of the same type, up to nine dimensions.
- Pinned `pdqselect` to 0.1.0 as 0.1.1 has switched to the 2021 edition

## Changed
- Expose all iterator types in `crate::iterators` module ([PR](https://github.com/georust/rstar/pull/77))

# 0.9.1

## Added
- A generic container for a geometry and associated data: `GeomWithData` ([PR](https://github.com/georust/rstar/pull/74))

# 0.9.0

## Added
- `RTree::nearest_neighbors` method based on
  [spade crate's implementation](https://github.com/Stoeoef/spade)

## Changed
- Fix floating point inconsistency in `min_max_dist_2` ([PR](https://github.com/georust/rstar/pull/40)).
- BREAKING: `Point::generate` function now accepts a `impl FnMut`. Custom implementations of `Point` must change to
  accept `impl FnMut` instead of `impl Fn`. Callers of `Point::generate` should not require changes.
- Update CI images to Stable Rust 1.50 and 1.51
- Run clippy, rustfmt, update manifest to reflect ownership changes
- Update Criterion and rewrite deprecated benchmark functions
- Remove unused imports
- Remove executable bit from files
- Fix typos, modernize links

# 0.8.3
## Changed
- Move crate ownership to the georust organization
## Fixed
- Update dependencies to remove heapless 0.5, which has a known vulnerability

# 0.8.2 - 2020-08-01
## Fixed:
 - Fixed a rare panic when calling `insert` (See #45)

# 0.8.1 - 2020-06-18
## Changed:

 - Fine tuned nearest neighbor iterator inline capacity (see  #39). This should boost performance in some cases.

# 0.8.0 - 2020-05-25
## Fixed:

 - Bugfix: `RTree::locate_with_selection_function_mut` sometimes returned too many elements for small trees.
## Changed:
 - Deprecated `RTree::nearest_neighbor_iter_with_distance`. The name is misleading, use `RTree::nearest_neighbor_iter_with_distance_2` instead.
 - Some performance improvements, see #38 and #35

## Added
 - Added `nearest_neighbor_iter_with_distance_2` #31

# 0.7.1 - 2020-01-16
## Changed:
 - `RTree::intersection_candidates_with_other_tree` can now calculate intersections of trees of different item types (see #23)

# 0.7.0 - 2019-11-25
## Added:
 - `RTree::remove_with_selection_function`
 - `RTree::pop_nearest_neighbor`
 - Added CHANGELOG.md
