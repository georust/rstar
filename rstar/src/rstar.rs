use rtree::{InsertionStrategy, RTree};
use node::{envelope_for_children, ParentNodeData, RTreeNode};
use point::{Point, PointExt};
use params::RTreeParams;
use object::RTreeObject;
use num_traits::{Bounded, Zero};
use metrics::RTreeMetrics;
use envelope::Envelope;

pub enum RStarInsertionStrategy {
}

enum InsertionResult<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    Split(RTreeNode<T, Params>),
    Reinsert(Vec<RTreeNode<T, Params>>, usize),
    Complete,
}

impl InsertionStrategy for RStarInsertionStrategy {
    fn insert<T, Params>(tree: &mut RTree<T, Params>, t: T, metrics: &mut RTreeMetrics)
    where
        Params: RTreeParams,
        T: RTreeObject,
    {
        metrics.increment_insertions();
        if tree.size() == 0 {
            // The root won't be split - adjust the height manually
            tree.set_height(1);
        }
        let mut tree_height = tree.height();

        let mut insertion_stack = vec![(RTreeNode::Leaf(t), 0, true)];

        let mut reinsertions = Vec::with_capacity(tree_height);
        reinsertions.resize(tree_height, true);

        while let Some((next, node_height, can_reinsert)) = insertion_stack.pop() {
            match recursive_insert(
                tree.root_mut(),
                next,
                tree_height - node_height - 1,
                can_reinsert,
                metrics,
            ) {
                InsertionResult::Split(node) => {
                    // The root node was split, create a new root and increase height
                    tree_height += 1;
                    let old_root = ::std::mem::replace(tree.root_mut(), ParentNodeData::new_root());
                    tree.set_height(tree_height);
                    let new_envelope = old_root.envelope.merged(&node.envelope());
                    tree.root_mut().envelope = new_envelope;
                    tree.root_mut().children.push(RTreeNode::Parent(old_root));
                    tree.root_mut().children.push(node);
                }
                InsertionResult::Reinsert(nodes, height) => {
                    let node_height = tree_height - height - 1;
                    let can_reinsert = reinsertions[node_height];
                    reinsertions[node_height] = false;
                    // Schedule elements for reinsertion
                    insertion_stack
                        .extend(nodes.into_iter().map(|n| (n, node_height, can_reinsert)));
                }
                InsertionResult::Complete => (),
            }
        }
    }
}

fn recursive_insert<T, Params>(
    node: &mut ParentNodeData<T, Params>,
    t: RTreeNode<T, Params>,
    target_height: usize,
    allow_reinsert: bool,
    metrics: &mut RTreeMetrics,
) -> InsertionResult<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    metrics.increment_recursive_insertions();
    node.envelope.merge(&t.envelope());
    if target_height == 0 {
        // Force insertion into this node
        node.children.push(t);
        return resolve_overflow(node, allow_reinsert, metrics);
    }
    let expand = {
        let all_leaves = target_height == 1;
        let follow = choose_subtree(node, &t, all_leaves, metrics);
        recursive_insert(follow, t, target_height - 1, allow_reinsert, metrics)
    };
    match expand {
        InsertionResult::Split(child) => {
            node.envelope.merge(&child.envelope());
            node.children.push(child);
            resolve_overflow(node, allow_reinsert, metrics)
        }
        InsertionResult::Reinsert(reinsertion_nodes, height) => {
            node.envelope = envelope_for_children(&node.children);
            InsertionResult::Reinsert(reinsertion_nodes, height + 1)
        }
        InsertionResult::Complete => InsertionResult::Complete,
    }
}

fn choose_subtree<'a, 'b, T, Params>(
    node: &'a mut ParentNodeData<T, Params>,
    to_insert: &'b RTreeNode<T, Params>,
    all_leaves: bool,
    metrics: &mut RTreeMetrics,
) -> &'a mut ParentNodeData<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    metrics.increment_choose_subtree();
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
        metrics.increment_choose_subtree_outsiders();
        // No inclusion found, subtree depends on overlap and area increase
        if all_leaves {
            metrics.increment_choose_subtree_leaves();
        }
        let mut min = (zero, zero, zero);

        for (index, child1) in node.children.iter().enumerate() {
            let envelope = child1.envelope();
            let mut new_envelope = envelope;
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
    if let RTreeNode::Parent(ref mut data) = node.children[min_index] {
        data
    } else {
        panic!("There must not be leaves on this depth")
    }
}

fn resolve_overflow<T, Params>(
    node: &mut ParentNodeData<T, Params>,
    allow_reinsert: bool,
    metrics: &mut RTreeMetrics,
) -> InsertionResult<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    metrics.increment_resolve_overflow();
    if node.children.len() > Params::MAX_SIZE {
        metrics.increment_resolve_overflow_overflows();
        let reinsertion_count = Params::REINSERTION_COUNT;
        if reinsertion_count == 0 || !allow_reinsert {
            // We did already reinsert on that level - split this node
            let offsplit = split(node, metrics);
            InsertionResult::Split(offsplit)
        } else {
            // We didn't attempt to reinsert yet - give it a try
            let reinsertion_nodes = reinsert(node, metrics);
            InsertionResult::Reinsert(reinsertion_nodes, 0)
        }
    } else {
        InsertionResult::Complete
    }
}

fn split<T, Params>(
    node: &mut ParentNodeData<T, Params>,
    metrics: &mut RTreeMetrics,
) -> RTreeNode<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    metrics.increment_splits();
    let axis = get_split_axis(node);
    let zero = <<T::Envelope as Envelope>::Point as Point>::Scalar::zero();
    debug_assert!(node.children.len() >= 2);
    // Sort along axis
    T::Envelope::align_envelopes(axis, &mut node.children, |c| c.envelope());
    let mut best = (zero, zero);
    let min_size = Params::MIN_SIZE;
    let mut best_index = min_size;

    for k in min_size..node.children.len() - min_size + 1 {
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
    let offsplit = node.children.split_off(best_index);
    node.envelope = envelope_for_children(&node.children);
    RTreeNode::Parent(ParentNodeData::new_parent(offsplit))
}

fn get_split_axis<T, Params>(node: &mut ParentNodeData<T, Params>) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let mut best_goodness = <<T::Envelope as Envelope>::Point as Point>::Scalar::max_value();
    let mut best_axis = 0;
    let min_size = Params::MIN_SIZE;
    let until = node.children.len() - min_size + 1;
    for axis in 0.. <T::Envelope as Envelope>::Point::DIMENSIONS {
        // Sort children along the current axis
        T::Envelope::align_envelopes(axis, &mut node.children, |c| c.envelope());
        let mut first_envelope = T::Envelope::new_empty();
        let mut second_envelope = T::Envelope::new_empty();
        for child in &node.children[..min_size] {
            first_envelope.merge(&child.envelope());
        }
        for child in &node.children[until..] {
            second_envelope.merge(&child.envelope());
        }
        for k in min_size..until {
            let mut first_modified = first_envelope;
            let mut second_modified = second_envelope;
            let (l, r) = node.children.split_at(k);
            for child in l {
                first_modified.merge(&child.envelope());
            }
            for child in r {
                second_modified.merge(&child.envelope());
            }

            let margin_value = first_modified.margin_value() + second_modified.margin_value();
            if best_goodness > margin_value {
                best_axis = axis;
                best_goodness = margin_value;
            }
        }
    }
    best_axis
}

#[inline(never)]
fn reinsert<T, Params>(
    node: &mut ParentNodeData<T, Params>,
    metrics: &mut RTreeMetrics,
) -> Vec<RTreeNode<T, Params>>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    metrics.increment_reinsertions();

    let center = node.envelope.center();
    // Sort with increasing order so we can use Vec::split_off
    node.children.sort_by(|l, r| {
        let l_center = l.envelope().center();
        let r_center = r.envelope().center();
        l_center
            .sub(&center)
            .length_2()
            .partial_cmp(&(r_center.sub(&center)).length_2())
            .unwrap()
    });
    let num_children = node.children.len();
    let result = node.children
        .split_off(num_children - Params::REINSERTION_COUNT);
    node.envelope = envelope_for_children(&node.children);
    result
}
