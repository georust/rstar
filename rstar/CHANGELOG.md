# 0.8.1
## Changed:
 - Fine tuned nearest neighbor iterator inline capacity (see  #39). This should boost performance in some cases.
# 0.8.0
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