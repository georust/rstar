use ::rtree::{InsertionStrategy, RTree};
use ::node::{ParentNodeData, RTreeNode, mbr_for_children};
use point::{Point, PointExt};
use params::RTreeParams;
use object::RTreeObject;
use num_traits::{Zero, Bounded};
use typenum::Unsigned;

pub enum RStarInsertionStrategy { }

enum InsertionResult<T, Params>
    where T: RTreeObject,
          Params: RTreeParams
{
    Split(RTreeNode<T, Params>),
    Reinsert(Vec<RTreeNode<T, Params>>, usize),
    Complete,
}

impl InsertionStrategy for RStarInsertionStrategy {
    fn insert<T, Params>(tree: &mut RTree<T, Params>, t: T) 
        where Params: RTreeParams,
              T: RTreeObject,
    {
        if tree.size() == 0 {
            // The root won't be split - adjust the height manually
            tree.set_height(1);
        }
        let mut tree_height = tree.height();

        let mut insertion_stack = vec![(RTreeNode::Leaf(t), 0, true)];

        let mut reinsertions = Vec::with_capacity(tree_height);
        reinsertions.resize(tree_height, true);

        while let Some((next, node_height, can_reinsert)) = insertion_stack.pop() {
            match recursive_insert(tree.root_mut(),
                                   next,
                                   tree_height - node_height - 1,
                                   can_reinsert) {
                InsertionResult::Split(node) => {
                    // The root node was split, create a new root and increase height
                    tree_height += 1;
                    let old_root = ::std::mem::replace(
                        tree.root_mut(), ParentNodeData::new_root());
                    tree.set_height(tree_height);
                    let new_mbr = old_root.mbr.add_mbr(&node.mbr());
                    tree.root_mut().mbr = new_mbr;
                    tree.root_mut().children.push(RTreeNode::Parent(old_root));
                    tree.root_mut().children.push(node);
                },
                InsertionResult::Reinsert(nodes, height) => {
                    let node_height = tree_height - height - 1;
                    let can_reinsert = reinsertions[node_height];
                    reinsertions[node_height] = false;
                    // Schedule elements for reinsertion
                    insertion_stack.extend(nodes.into_iter().map(|n| (n, node_height, can_reinsert)));
                },
                InsertionResult::Complete => (),
            }
        }
    }
}

fn recursive_insert<T, Params>(node: &mut ParentNodeData<T, Params>, 
                               t: RTreeNode<T, Params>, 
                               target_height: usize,
                               allow_reinsert: bool) -> InsertionResult<T, Params>
    where Params: RTreeParams,
          T: RTreeObject,
{
    node.mbr.extend_with_mbr(&t.mbr());
    if target_height == 0 {
        // Force insertion into this node
        node.children.push(t);
        return resolve_overflow(node, allow_reinsert);
    }
    let expand = { 
        let follow = choose_subtree(node, &t);
        recursive_insert(follow, t, target_height - 1, allow_reinsert)
    };
    match expand {
        InsertionResult::Split(child) => {
            node.mbr.extend_with_mbr(&child.mbr());
            node.children.push(child);
            resolve_overflow(node, allow_reinsert)
        },
        InsertionResult::Reinsert(reinsertion_nodes, height) => {
            node.mbr = mbr_for_children(&node.children);
            InsertionResult::Reinsert(reinsertion_nodes, height + 1)
        },
        InsertionResult::Complete => InsertionResult::Complete,
    }
}

fn choose_subtree<'a, 'b, T, Params>(node: &'a mut ParentNodeData<T, Params>, 
                                     to_insert: &'b RTreeNode<T, Params>) 
                                     -> &'a mut ParentNodeData<T, Params> 
    where T: RTreeObject,
          Params: RTreeParams,
{
    let zero: <T::Point as Point>::Scalar = Zero::zero();
    let insertion_mbr = to_insert.mbr();
    let mut inclusion_count = 0;
    let mut min_area = <T::Point as Point>::Scalar::max_value();
    let mut min_index = 0;
    for (index, child) in node.children.iter().enumerate() {
        let mbr = child.mbr();
        if mbr.contains_mbr(&insertion_mbr) {
            inclusion_count += 1;
            let area = mbr.area();
            if area < min_area {
                min_area = area;
                min_index = index;
            }
        }
    }
    if inclusion_count == 0 {
        // No inclusion found, subtree depends on overlap and area increase
        let all_leaves = node.children.first().unwrap().is_leaf();
        let mut min = (zero, zero, zero);

        for (index, child1) in node.children.iter().enumerate() {
            let mbr = child1.mbr();
            let mut new_mbr = mbr.clone();
            new_mbr.extend_with_mbr(&insertion_mbr);
            let overlap_increase = if all_leaves {
                // Calculate minimal overlap increase
                let mut overlap = zero;
                let mut new_overlap = zero;
                for child2 in node.children.iter() {
                    if child1 as *const _ != child2 as *const _ {
                        let child_mbr = child2.mbr();
                        overlap = overlap.clone() + mbr.intersection(&child_mbr).area();
                        new_overlap = new_overlap.clone() + new_mbr.intersection(&child_mbr).area();
                    }
                }
                let overlap_increase = new_overlap - overlap;
                overlap_increase
            } else {
                // Don't calculate overlap increase if not all children are leaves
                zero
            };
            // Calculate area increase and area
            let area = new_mbr.area();
            let area_increase = area.clone() - mbr.area();
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

fn resolve_overflow<T, Params>(node: &mut ParentNodeData<T, Params>,
                               allow_reinsert: bool) -> InsertionResult<T, Params> 
    where T: RTreeObject,
          Params: RTreeParams
{
    if node.children.len() > Params::MaxSize::to_usize() {
        let reinsertion_count = Params::ReinsertionCount::to_usize();
        if reinsertion_count == 0 || !allow_reinsert {
            // We did already reinsert on that level - split this node
            let offsplit = split(node);
            InsertionResult::Split(offsplit)
        } else {
            // We didn't attempt to reinsert yet - give it a try
            let reinsertion_nodes = reinsert(node);
            InsertionResult::Reinsert(reinsertion_nodes, 0)
        }
    } else {
        InsertionResult::Complete
    }
}

fn split<T, Params>(node: &mut ParentNodeData<T, Params>) -> RTreeNode<T, Params> 
    where T: RTreeObject,
          Params: RTreeParams
{
    let axis = get_split_axis(node);
    let zero = <T::Point as Point>::Scalar::zero();
    debug_assert!(node.children.len() >= 2);
    // Sort along axis
    node.children.sort_by(|l, r| l.mbr().lower().nth(axis).partial_cmp(&r.mbr().lower().nth(axis)).unwrap());
    let mut best = (zero, zero);
    let min_size = Params::MinSize::to_usize();
    let mut best_index = min_size;

    for k in min_size .. node.children.len() - min_size + 1 {
        let mut first_mbr = node.children[k - 1].mbr();
        let mut second_mbr = node.children[k].mbr();
        let (l, r) = node.children.split_at(k);
        for child in l {
            first_mbr.extend_with_mbr(&child.mbr());
        }
        for child in r {
            second_mbr.extend_with_mbr(&child.mbr());
        }

        let overlap_value = first_mbr.intersection(&second_mbr).area();
        let area_value = first_mbr.area() + second_mbr.area();
        let new_best = (overlap_value, area_value);
        if new_best < best || k == min_size {
            best = new_best;
            best_index = k;
        }
    }
    let offsplit = node.children.split_off(best_index);
    node.mbr = mbr_for_children(&node.children);
    let result = RTreeNode::Parent(ParentNodeData::new_parent(offsplit));
    
    result
}

fn get_split_axis<T, Params>(node: &mut ParentNodeData<T, Params>) -> usize 
    where T: RTreeObject,
      Params: RTreeParams
{
    let mut best_goodness = <T::Point as Point>::Scalar::zero();
    let mut best_axis = 0;
    let min_size = Params::MinSize::to_usize();
    for axis in 0 .. T::Point::dimensions() {
        // Sort children along the current axis
        node.children.sort_by(|l, r| l.mbr().lower().nth(axis)
                              .partial_cmp(&r.mbr().lower().nth(axis)).unwrap());
        for k in min_size .. node.children.len() - min_size + 1 {
            let mut first_mbr = node.children[k - 1].mbr();
            let mut second_mbr = node.children[k].mbr();
            let (l, r) = node.children.split_at(k);
            for child in l {
                first_mbr.extend_with_mbr(&child.mbr());
            }
            for child in r {
                second_mbr.extend_with_mbr(&child.mbr());
            }

            let margin_value = first_mbr.diagonal_sum() + second_mbr.diagonal_sum();
            if best_goodness > margin_value || axis == 0 {
                best_axis = axis;
                best_goodness = margin_value;
            }
        }
    }
    best_axis
}


#[inline(never)]
fn reinsert<T, Params>(node: &mut ParentNodeData<T, Params>) -> Vec<RTreeNode<T, Params>> 
    where T: RTreeObject,
      Params: RTreeParams,
{
    let lower = node.mbr.lower();
    // Sort with increasing order so we can use Vec::split_off
    node.children.sort_by(|l, r| {
        let l_lower = l.mbr().lower();
        let r_lower = r.mbr().lower();
        l_lower.sub(&lower).length_2()
            .partial_cmp(&(r_lower.sub(&lower)).length_2()).unwrap()
    });
    let num_children = node.children.len();
    let result = node.children.split_off(num_children - Params::ReinsertionCount::to_usize());
    node.mbr = mbr_for_children(&node.children);
    result
}
