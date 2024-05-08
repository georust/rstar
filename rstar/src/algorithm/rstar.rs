use crate::envelope::Envelope;
use crate::node::{envelope_for_children, ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::{InsertionStrategy, RTreeParams};
use crate::point::{Point, PointExt};
use crate::rtree::RTree;

#[cfg(not(test))]
use alloc::vec::Vec;
use num_traits::{Bounded, Zero};

/// Inserts points according to the r-star heuristic.
///
/// The r*-heuristic focusses on good insertion quality at the costs of
/// insertion performance. This strategy is best for use cases with few
/// insertions and many nearest neighbor queries.
///
/// `RStarInsertionStrategy` is used as the default insertion strategy.
/// See [InsertionStrategy] for more information on insertion strategies.
pub enum RStarInsertionStrategy {}

enum InsertionResult<T>
where
    T: RTreeObject,
{
    Split(RTreeNode<T>),
    Reinsert(Vec<RTreeNode<T>>, usize),
    Complete,
}

impl InsertionStrategy for RStarInsertionStrategy {
    fn insert<T, Params>(tree: &mut RTree<T, Params>, t: T)
    where
        Params: RTreeParams,
        T: RTreeObject,
    {
        use InsertionAction::*;

        enum InsertionAction<T: RTreeObject> {
            PerformSplit(RTreeNode<T>),
            PerformReinsert(RTreeNode<T>),
        }

        let first = recursive_insert::<_, Params>(tree.root_mut(), RTreeNode::Leaf(t), 0);
        let mut target_height = 0;
        let mut insertion_stack = Vec::new();
        match first {
            InsertionResult::Split(node) => insertion_stack.push(PerformSplit(node)),
            InsertionResult::Reinsert(nodes_to_reinsert, real_target_height) => {
                insertion_stack.extend(nodes_to_reinsert.into_iter().map(PerformReinsert));
                target_height = real_target_height;
            }
            InsertionResult::Complete => {}
        };

        while let Some(next) = insertion_stack.pop() {
            match next {
                PerformSplit(node) => {
                    // The root node was split, create a new root and increase height
                    let new_root = ParentNode::new_root::<Params>();
                    let old_root = ::core::mem::replace(tree.root_mut(), new_root);
                    let new_envelope = old_root.envelope.merged(&node.envelope());
                    let root = tree.root_mut();
                    root.envelope = new_envelope;
                    root.children.push(RTreeNode::Parent(old_root));
                    root.children.push(node);
                    target_height += 1;
                }
                PerformReinsert(node_to_reinsert) => {
                    let root = tree.root_mut();
                    match forced_insertion::<T, Params>(root, node_to_reinsert, target_height) {
                        InsertionResult::Split(node) => insertion_stack.push(PerformSplit(node)),
                        InsertionResult::Reinsert(_, _) => {
                            panic!("Unexpected reinsert. This is a bug in rstar.")
                        }
                        InsertionResult::Complete => {}
                    }
                }
            }
        }
    }
}

fn forced_insertion<T, Params>(
    node: &mut ParentNode<T>,
    t: RTreeNode<T>,
    target_height: usize,
) -> InsertionResult<T>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    node.envelope.merge(&t.envelope());
    let expand_index = choose_subtree(node, &t);

    if target_height == 0 || node.children.len() < expand_index {
        // Force insertion into this node
        node.children.push(t);
        return resolve_overflow_without_reinsertion::<_, Params>(node);
    }

    if let RTreeNode::Parent(ref mut follow) = node.children[expand_index] {
        match forced_insertion::<_, Params>(follow, t, target_height - 1) {
            InsertionResult::Split(child) => {
                node.envelope.merge(&child.envelope());
                node.children.push(child);
                resolve_overflow_without_reinsertion::<_, Params>(node)
            }
            other => other,
        }
    } else {
        unreachable!("This is a bug in rstar.")
    }
}

fn recursive_insert<T, Params>(
    node: &mut ParentNode<T>,
    t: RTreeNode<T>,
    current_height: usize,
) -> InsertionResult<T>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    node.envelope.merge(&t.envelope());
    let expand_index = choose_subtree(node, &t);

    if node.children.len() < expand_index {
        // Force insertion into this node
        node.children.push(t);
        return resolve_overflow::<_, Params>(node, current_height);
    }

    let expand = if let RTreeNode::Parent(ref mut follow) = node.children[expand_index] {
        recursive_insert::<_, Params>(follow, t, current_height + 1)
    } else {
        panic!("This is a bug in rstar.")
    };

    match expand {
        InsertionResult::Split(child) => {
            node.envelope.merge(&child.envelope());
            node.children.push(child);
            resolve_overflow::<_, Params>(node, current_height)
        }
        InsertionResult::Reinsert(a, b) => {
            node.envelope = envelope_for_children(&node.children);
            InsertionResult::Reinsert(a, b)
        }
        other => other,
    }
}

fn choose_subtree<T>(node: &ParentNode<T>, to_insert: &RTreeNode<T>) -> usize
where
    T: RTreeObject,
{
    let all_leaves = match node.children.first() {
        Some(RTreeNode::Leaf(_)) => return usize::MAX,
        Some(RTreeNode::Parent(ref data)) => data
            .children
            .first()
            .map(RTreeNode::is_leaf)
            .unwrap_or(true),
        None => return usize::MAX,
    };

    let zero: <<T::Envelope as Envelope>::Point as Point>::Scalar = Zero::zero();
    let insertion_envelope = to_insert.envelope();
    let mut inclusion_count = 0;
    let mut min_area = <<T::Envelope as Envelope>::Point as Point>::Scalar::max_value();
    let mut min_index = 0;
    for (index, child) in node.children.iter().enumerate() {
        let envelope = child.envelope();
        if envelope.contains_envelope(&insertion_envelope) {
            inclusion_count += 1;
            let area = envelope.area();
            if area < min_area {
                min_area = area;
                min_index = index;
            }
        }
    }
    if inclusion_count == 0 {
        // No inclusion found, subtree depends on overlap and area increase
        let mut min = (zero, zero, zero);

        for (index, child1) in node.children.iter().enumerate() {
            let envelope = child1.envelope();
            let mut new_envelope = envelope.clone();
            new_envelope.merge(&insertion_envelope);
            let overlap_increase = if all_leaves {
                // Calculate minimal overlap increase
                let mut overlap = zero;
                let mut new_overlap = zero;
                for child2 in &node.children {
                    if child1 as *const _ != child2 as *const _ {
                        let child_envelope = child2.envelope();
                        let temp1 = envelope.intersection_area(&child_envelope);
                        overlap = overlap + temp1;
                        let temp2 = new_envelope.intersection_area(&child_envelope);
                        new_overlap = new_overlap + temp2;
                    }
                }
                new_overlap - overlap
            } else {
                // Don't calculate overlap increase if not all children are leaves
                zero
            };
            // Calculate area increase and area
            let area = new_envelope.area();
            let area_increase = area - envelope.area();
            let new_min = (overlap_increase, area_increase, area);
            if new_min < min || index == 0 {
                min = new_min;
                min_index = index;
            }
        }
    }
    min_index
}

// Never returns a request for reinsertion
fn resolve_overflow_without_reinsertion<T, Params>(node: &mut ParentNode<T>) -> InsertionResult<T>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    if node.children.len() > Params::MAX_SIZE {
        let off_split = split::<_, Params>(node);
        InsertionResult::Split(off_split)
    } else {
        InsertionResult::Complete
    }
}

fn resolve_overflow<T, Params>(node: &mut ParentNode<T>, current_depth: usize) -> InsertionResult<T>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    if Params::REINSERTION_COUNT == 0 {
        resolve_overflow_without_reinsertion::<_, Params>(node)
    } else if node.children.len() > Params::MAX_SIZE {
        let nodes_for_reinsertion = get_nodes_for_reinsertion::<_, Params>(node);
        InsertionResult::Reinsert(nodes_for_reinsertion, current_depth)
    } else {
        InsertionResult::Complete
    }
}

fn split<T, Params>(node: &mut ParentNode<T>) -> RTreeNode<T>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let axis = get_split_axis::<_, Params>(node);
    let zero = <<T::Envelope as Envelope>::Point as Point>::Scalar::zero();
    debug_assert!(node.children.len() >= 2);
    // Sort along axis
    T::Envelope::sort_envelopes(axis, &mut node.children);
    let mut best = (zero, zero);
    let min_size = Params::MIN_SIZE;
    let mut best_index = min_size;

    for k in min_size..=node.children.len() - min_size {
        let mut first_envelope = node.children[k - 1].envelope();
        let mut second_envelope = node.children[k].envelope();
        let (l, r) = node.children.split_at(k);
        for child in l {
            first_envelope.merge(&child.envelope());
        }
        for child in r {
            second_envelope.merge(&child.envelope());
        }

        let overlap_value = first_envelope.intersection_area(&second_envelope);
        let area_value = first_envelope.area() + second_envelope.area();
        let new_best = (overlap_value, area_value);
        if new_best < best || k == min_size {
            best = new_best;
            best_index = k;
        }
    }
    let off_split = node.children.split_off(best_index);
    node.envelope = envelope_for_children(&node.children);
    RTreeNode::Parent(ParentNode::new_parent(off_split))
}

fn get_split_axis<T, Params>(node: &mut ParentNode<T>) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let mut best_goodness = <<T::Envelope as Envelope>::Point as Point>::Scalar::max_value();
    let mut best_axis = 0;
    let min_size = Params::MIN_SIZE;
    let until = node.children.len() - min_size + 1;
    for axis in 0..<T::Envelope as Envelope>::Point::DIMENSIONS {
        // Sort children along the current axis
        T::Envelope::sort_envelopes(axis, &mut node.children);
        let mut first_envelope = T::Envelope::new_empty();
        let mut second_envelope = T::Envelope::new_empty();
        for child in &node.children[..min_size] {
            first_envelope.merge(&child.envelope());
        }
        for child in &node.children[until..] {
            second_envelope.merge(&child.envelope());
        }
        for k in min_size..until {
            let mut first_modified = first_envelope.clone();
            let mut second_modified = second_envelope.clone();
            let (l, r) = node.children.split_at(k);
            for child in l {
                first_modified.merge(&child.envelope());
            }
            for child in r {
                second_modified.merge(&child.envelope());
            }

            let perimeter_value =
                first_modified.perimeter_value() + second_modified.perimeter_value();
            if best_goodness > perimeter_value {
                best_axis = axis;
                best_goodness = perimeter_value;
            }
        }
    }
    best_axis
}

fn get_nodes_for_reinsertion<T, Params>(node: &mut ParentNode<T>) -> Vec<RTreeNode<T>>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let center = node.envelope.center();
    // Sort with increasing order so we can use Vec::split_off
    node.children.sort_unstable_by(|l, r| {
        let l_center = l.envelope().center();
        let r_center = r.envelope().center();
        l_center
            .sub(&center)
            .length_2()
            .partial_cmp(&(r_center.sub(&center)).length_2())
            .unwrap()
    });
    let num_children = node.children.len();
    let result = node
        .children
        .split_off(num_children - Params::REINSERTION_COUNT);
    node.envelope = envelope_for_children(&node.children);
    result
}
