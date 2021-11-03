# Unreleased

## Added
- Add `RTree::drain_*` methods to remove and drain selected items. ([PR](https://github.com/georust/rstar/pull/77))
- Add trait `Point` for tuples containing elements of the same type, up to nine dimensions.

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
