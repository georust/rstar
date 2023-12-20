# Unreleased

## Added
- Add optional support for the [mint](https://docs.rs/mint/0.5.9/mint/index.html) crate
- Added cached envelope bulk load benchmark

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
