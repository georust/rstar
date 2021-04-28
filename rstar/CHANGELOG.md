# 0.8.4 (unreleased)
- Update CI images to Stable Rust 1.50 and 1.51
- Run clippy, rustfmt, update manifest to reflect ownership changes
- Update Criterion and rewrite deprecated benchmark functions
- Add new `RTree::nearest_neighbors` method based on
  [the original implementation](https://github.com/Stoeoef/spade)

# 0.8.3
- Move crate ownership to the georust organization
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
