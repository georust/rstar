use core::mem::replace;

use crate::algorithm::selection_functions::SelectionFunction;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::{Envelope, RTree};

use alloc::{vec, vec::Vec};

#[allow(unused_imports)] // Import is required when building without std
use num_traits::Float;

/// Iterator returned by `RTree::drain_*` methods.
///
/// Draining iterator that removes elements of the tree selected by a
/// [`SelectionFunction`]. Returned by
/// [`RTree::drain_with_selection_function`] and related methods.
///
/// # Remarks
///
/// This iterator is similar to the one returned by `Vec::drain` or
/// `Vec::drain_filter`. Dropping the iterator at any point removes only
/// the yielded values (this behaviour is unlike `Vec::drain_*`). Leaking
/// this iterator leads to a leak amplification where all elements of the
/// tree are leaked.
pub struct DrainIterator<'a, T, R, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: SelectionFunction<T>,
{
    node_stack: Vec<(ParentNode<T>, usize, usize)>,
    removal_function: R,
    rtree: &'a mut RTree<T, Params>,
    original_size: usize,
}

impl<'a, T, R, Params> DrainIterator<'a, T, R, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: SelectionFunction<T>,
{
    pub(crate) fn new(rtree: &'a mut RTree<T, Params>, removal_function: R) -> Self {
        // We replace with a root as a brand new RTree in case the iterator is
        // `mem::forgot`ten.

        // Instead of using `new_with_params`, we avoid an allocation for
        // the normal usage and replace root with an empty `Vec`.
        let root = replace(
            rtree.root_mut(),
            ParentNode {
                children: vec![],
                envelope: Envelope::new_empty(),
            },
        );
        let original_size = replace(rtree.size_mut(), 0);

        let m = Params::MIN_SIZE;
        let max_depth = (original_size as f32).log(m as f32).ceil() as usize;
        let mut node_stack = Vec::with_capacity(max_depth);
        node_stack.push((root, 0, 0));

        DrainIterator {
            node_stack,
            original_size,
            removal_function,
            rtree,
        }
    }

    fn pop_node(&mut self, increment_idx: bool) -> Option<(ParentNode<T>, usize)> {
        debug_assert!(!self.node_stack.is_empty());

        let (mut node, _, num_removed) = self.node_stack.pop().unwrap();

        // We only compute envelope for the current node as the parent
        // is taken care of when it is popped.

        // TODO: May be make this a method on `ParentNode`
        if num_removed > 0 {
            node.envelope = crate::node::envelope_for_children(&node.children);
        }

        // If there is no parent, this is the new root node to set back in the rtree
        // O/w, get the new top in stack
        let (parent_node, parent_idx, parent_removed) = match self.node_stack.last_mut() {
            Some(pn) => (&mut pn.0, &mut pn.1, &mut pn.2),
            None => return Some((node, num_removed)),
        };

        // Update the remove count on parent
        *parent_removed += num_removed;

        // If the node has no children, we don't need to add it back to the parent
        if node.children.is_empty() {
            return None;
        }

        // Put the child back (but re-arranged)
        parent_node.children.push(RTreeNode::Parent(node));

        // Swap it with the current item and increment idx.

        // A minor optimization is to avoid the swap in the destructor,
        // where we aren't going to be iterating any more.
        if !increment_idx {
            return None;
        }

        // Note that during iteration, parent_idx may be equal to
        // (previous) children.len(), but this is okay as the swap will be
        // a no-op.
        let parent_len = parent_node.children.len();
        parent_node.children.swap(*parent_idx, parent_len - 1);
        *parent_idx += 1;

        None
    }
}

impl<'a, T, R, Params> Iterator for DrainIterator<'a, T, R, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: SelectionFunction<T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Get reference to top node or return None.
            let (node, idx, remove_count) = match self.node_stack.last_mut() {
                Some(node) => (&mut node.0, &mut node.1, &mut node.2),
                None => return None,
            };

            // Try to find a selected item to return.
            if *idx > 0 || self.removal_function.should_unpack_parent(&node.envelope) {
                while *idx < node.children.len() {
                    match &mut node.children[*idx] {
                        RTreeNode::Parent(_) => {
                            // Swap node with last, remove and return the value.
                            // No need to increment idx as something else has replaced it;
                            // or idx == new len, and we'll handle it in the next iteration.
                            let child = match node.children.swap_remove(*idx) {
                                RTreeNode::Leaf(_) => unreachable!("DrainIterator bug!"),
                                RTreeNode::Parent(node) => node,
                            };
                            self.node_stack.push((child, 0, 0));
                            return self.next();
                        }
                        RTreeNode::Leaf(ref leaf) => {
                            if self.removal_function.should_unpack_leaf(leaf) {
                                // Swap node with last, remove and return the value.
                                // No need to increment idx as something else has replaced it;
                                // or idx == new len, and we'll handle it in the next iteration.
                                *remove_count += 1;
                                return match node.children.swap_remove(*idx) {
                                    RTreeNode::Leaf(data) => Some(data),
                                    _ => unreachable!("RemovalIterator bug!"),
                                };
                            }
                            *idx += 1;
                        }
                    }
                }
            }

            // Pop top node and clean-up if done
            if let Some((new_root, total_removed)) = self.pop_node(true) {
                // This happens if we are done with the iteration.
                // Set the root back in rtree and return None
                *self.rtree.root_mut() = new_root;
                *self.rtree.size_mut() = self.original_size - total_removed;
                return None;
            }
        }
    }
}

impl<'a, T, R, Params> Drop for DrainIterator<'a, T, R, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: SelectionFunction<T>,
{
    fn drop(&mut self) {
        // Re-assemble back the original rtree and update envelope as we
        // re-assemble.
        if self.node_stack.is_empty() {
            // The iteration handled everything, nothing to do.
            return;
        }

        loop {
            debug_assert!(!self.node_stack.is_empty());
            if let Some((new_root, total_removed)) = self.pop_node(false) {
                *self.rtree.root_mut() = new_root;
                *self.rtree.size_mut() = self.original_size - total_removed;
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::mem::forget;

    use crate::algorithm::selection_functions::{SelectAllFunc, SelectInEnvelopeFuncIntersecting};
    use crate::point::PointExt;
    use crate::primitives::Line;
    use crate::test_utilities::{create_random_points, create_random_rectangles, SEED_1, SEED_2};
    use crate::{RTree, AABB};

    use super::*;

    #[test]
    fn test_remove_and_insert() {
        const SIZE: usize = 1000;
        let points = create_random_points(SIZE, SEED_1);
        let later_insertions = create_random_points(SIZE, SEED_2);
        let mut tree = RTree::bulk_load(points.clone());
        for (point_to_remove, point_to_add) in points.iter().zip(later_insertions.iter()) {
            assert!(tree.remove_at_point(point_to_remove).is_some());
            tree.insert(*point_to_add);
        }
        assert_eq!(tree.size(), SIZE);
        assert!(points.iter().all(|p| !tree.contains(p)));
        assert!(later_insertions.iter().all(|p| tree.contains(p)));
        for point in &later_insertions {
            assert!(tree.remove_at_point(point).is_some());
        }
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_remove_and_insert_rectangles() {
        const SIZE: usize = 1000;
        let initial_rectangles = create_random_rectangles(SIZE, SEED_1);
        let new_rectangles = create_random_rectangles(SIZE, SEED_2);
        let mut tree = RTree::bulk_load(initial_rectangles.clone());

        for (rectangle_to_remove, rectangle_to_add) in
            initial_rectangles.iter().zip(new_rectangles.iter())
        {
            assert!(tree.remove(rectangle_to_remove).is_some());
            tree.insert(*rectangle_to_add);
        }
        assert_eq!(tree.size(), SIZE);
        assert!(initial_rectangles.iter().all(|p| !tree.contains(p)));
        assert!(new_rectangles.iter().all(|p| tree.contains(p)));
        for rectangle in &new_rectangles {
            assert!(tree.contains(rectangle));
        }
        for rectangle in &initial_rectangles {
            assert!(!tree.contains(rectangle));
        }
        for rectangle in &new_rectangles {
            assert!(tree.remove(rectangle).is_some());
        }
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_remove_at_point() {
        let points = create_random_points(1000, SEED_1);
        let mut tree = RTree::bulk_load(points.clone());
        for point in &points {
            let size_before_removal = tree.size();
            assert!(tree.remove_at_point(point).is_some());
            assert!(tree.remove_at_point(&[1000.0, 1000.0]).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }

    #[test]
    fn test_remove() {
        let points = create_random_points(1000, SEED_1);
        let offsets = create_random_points(1000, SEED_2);
        let scaled = offsets.iter().map(|p| p.mul(0.05));
        let edges: Vec<_> = points
            .iter()
            .zip(scaled)
            .map(|(from, offset)| Line::new(*from, from.add(&offset)))
            .collect();
        let mut tree = RTree::bulk_load(edges.clone());
        for edge in &edges {
            let size_before_removal = tree.size();
            assert!(tree.remove(edge).is_some());
            assert!(tree.remove(edge).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }

    #[test]
    fn test_drain_iterator() {
        const SIZE: usize = 1000;
        let points = create_random_points(SIZE, SEED_1);
        let mut tree = RTree::bulk_load(points.clone());

        let drain_count = DrainIterator::new(&mut tree, SelectAllFunc)
            .take(250)
            .count();
        assert_eq!(drain_count, 250);
        assert_eq!(tree.size(), 750);

        let drain_count = DrainIterator::new(&mut tree, SelectAllFunc)
            .take(250)
            .count();
        assert_eq!(drain_count, 250);
        assert_eq!(tree.size(), 500);

        // Test Drain forget soundness
        forget(DrainIterator::new(&mut tree, SelectAllFunc));
        // Check tree has no nodes
        // Tests below will check the same tree can be used again
        assert_eq!(tree.size(), 0);

        let points = create_random_points(1000, SEED_1);
        points.clone().into_iter().for_each(|pt| tree.insert(pt));

        // The total for this is 406 (for SEED_1)
        let env = AABB::from_corners([-2., -0.6], [0.5, 0.85]);

        let sel = SelectInEnvelopeFuncIntersecting::new(env);
        let drain_count = DrainIterator::new(&mut tree, sel).take(80).count();
        assert_eq!(drain_count, 80);

        let sel = SelectInEnvelopeFuncIntersecting::new(env);
        let drain_count = DrainIterator::new(&mut tree, sel).count();
        assert_eq!(drain_count, 326);

        let sel = SelectInEnvelopeFuncIntersecting::new(env);
        let sel_count = tree.locate_with_selection_function(sel).count();
        assert_eq!(sel_count, 0);
        assert_eq!(tree.size(), 1000 - 80 - 326);
    }
}
