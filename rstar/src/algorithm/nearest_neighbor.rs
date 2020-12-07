use crate::node::{ParentNode, RTreeNode};
use crate::point::{min_inline, Point};
use crate::{Envelope, PointDistance, RTreeObject};
use heapless::binary_heap as static_heap;
use num_traits::Bounded;
use std::collections::binary_heap::BinaryHeap;

struct RTreeNodeDistanceWrapper<'a, T>
where
    T: PointDistance + 'a,
{
    node: &'a RTreeNode<T>,
    distance: <<T::Envelope as Envelope>::Point as Point>::Scalar,
}

impl<'a, T> PartialEq for RTreeNodeDistanceWrapper<'a, T>
where
    T: PointDistance,
{
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<'a, T> PartialOrd for RTreeNodeDistanceWrapper<'a, T>
where
    T: PointDistance,
{
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        // Inverse comparison creates a min heap
        other.distance.partial_cmp(&self.distance)
    }
}

impl<'a, T> Eq for RTreeNodeDistanceWrapper<'a, T> where T: PointDistance {}

impl<'a, T> Ord for RTreeNodeDistanceWrapper<'a, T>
where
    T: PointDistance,
{
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl<'a, T> NearestNeighborDistance2Iterator<'a, T>
where
    T: PointDistance,
{
    pub fn new(root: &'a ParentNode<T>, query_point: <T::Envelope as Envelope>::Point) -> Self {
        let mut result = NearestNeighborDistance2Iterator {
            nodes: BinaryHeap::with_capacity(20),
            query_point,
        };
        result.extend_heap(&root.children);
        result
    }

    fn extend_heap(&mut self, children: &'a [RTreeNode<T>]) {
        let &mut NearestNeighborDistance2Iterator {
            ref mut nodes,
            ref query_point,
        } = self;
        nodes.extend(children.iter().map(|child| {
            let distance = match child {
                RTreeNode::Parent(ref data) => data.envelope.distance_2(query_point),
                RTreeNode::Leaf(ref t) => t.distance_2(query_point),
            };

            RTreeNodeDistanceWrapper {
                node: child,
                distance,
            }
        }));
    }
}

impl<'a, T> Iterator for NearestNeighborDistance2Iterator<'a, T>
where
    T: PointDistance,
{
    type Item = (&'a T, <<T::Envelope as Envelope>::Point as Point>::Scalar);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.nodes.pop() {
            match current {
                RTreeNodeDistanceWrapper {
                    node: RTreeNode::Parent(ref data),
                    ..
                } => {
                    self.extend_heap(&data.children);
                }
                RTreeNodeDistanceWrapper {
                    node: RTreeNode::Leaf(ref t),
                    distance,
                } => {
                    return Some((t, distance));
                }
            }
        }
        None
    }
}

pub struct NearestNeighborDistance2Iterator<'a, T>
where
    T: PointDistance + 'a,
{
    nodes: BinaryHeap<RTreeNodeDistanceWrapper<'a, T>>,
    query_point: <T::Envelope as Envelope>::Point,
}

impl<'a, T> NearestNeighborIterator<'a, T>
where
    T: PointDistance,
{
    pub fn new(root: &'a ParentNode<T>, query_point: <T::Envelope as Envelope>::Point) -> Self {
        NearestNeighborIterator {
            iter: NearestNeighborDistance2Iterator::new(root, query_point),
        }
    }
}

impl<'a, T> Iterator for NearestNeighborIterator<'a, T>
where
    T: PointDistance,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(t, _distance)| t)
    }
}

pub struct NearestNeighborIterator<'a, T>
where
    T: PointDistance + 'a,
{
    iter: NearestNeighborDistance2Iterator<'a, T>,
}

enum SmallHeap<T: Ord> {
    Stack(static_heap::BinaryHeap<T, heapless::consts::U32, static_heap::Max>),
    Heap(BinaryHeap<T>),
}

impl<T: Ord> SmallHeap<T> {
    pub fn new() -> Self {
        Self::Stack(static_heap::BinaryHeap::new())
    }

    pub fn pop(&mut self) -> Option<T> {
        match self {
            SmallHeap::Stack(heap) => heap.pop(),
            SmallHeap::Heap(heap) => heap.pop(),
        }
    }

    pub fn push(&mut self, item: T) {
        match self {
            SmallHeap::Stack(heap) => {
                if let Err(item) = heap.push(item) {
                    // FIXME: This could be done more efficiently if heapless'
                    // BinaryHeap had draining, owning into_iter, or would
                    // expose its data slice.
                    let mut new_heap = BinaryHeap::with_capacity(heap.len() + 1);
                    while let Some(old_item) = heap.pop() {
                        new_heap.push(old_item);
                    }
                    new_heap.push(item);
                    *self = SmallHeap::Heap(new_heap);
                }
            }
            SmallHeap::Heap(heap) => heap.push(item),
        }
    }
}

pub fn nearest_neighbor<'a, T>(
    node: &'a ParentNode<T>,
    query_point: <T::Envelope as Envelope>::Point,
) -> Option<&'a T>
where
    T: PointDistance,
{
    nearest_neighbor_inner(&node.children, query_point).map(|(node, _)| node)
}

fn nearest_neighbor_inner<'a, T>(
    seed_nodes: impl IntoIterator<Item = impl std::borrow::Borrow<&'a RTreeNode<T>>>,
    query_point: <T::Envelope as Envelope>::Point,
) -> Option<(&'a T, <<T::Envelope as Envelope>::Point as Point>::Scalar)>
where
    T: PointDistance,
{
    fn extend_heap<'a, T>(
        nodes: &mut SmallHeap<RTreeNodeDistanceWrapper<'a, T>>,
        source: impl IntoIterator<Item = impl std::borrow::Borrow<&'a RTreeNode<T>>>,
        query_point: <T::Envelope as Envelope>::Point,
        min_max_distance: &mut <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) where
        T: PointDistance + 'a,
    {
        for child in source {
            let distance_if_less_or_equal = match child.borrow() {
                RTreeNode::Parent(ref data) => {
                    let distance = data.envelope.distance_2(&query_point);
                    if distance <= *min_max_distance {
                        Some(distance)
                    } else {
                        None
                    }
                }
                RTreeNode::Leaf(ref t) => {
                    t.distance_2_if_less_or_equal(&query_point, *min_max_distance)
                }
            };
            if let Some(distance) = distance_if_less_or_equal {
                *min_max_distance = min_inline(
                    *min_max_distance,
                    child.borrow().envelope().min_max_dist_2(&query_point),
                );
                nodes.push(RTreeNodeDistanceWrapper {
                    node: child.borrow(),
                    distance,
                });
            }
        }
    }

    // Calculate smallest minmax-distance
    let mut smallest_min_max: <<T::Envelope as Envelope>::Point as Point>::Scalar =
        Bounded::max_value();
    let mut nodes = SmallHeap::new();
    extend_heap(&mut nodes, seed_nodes, query_point, &mut smallest_min_max);
    while let Some(current) = nodes.pop() {
        match current {
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Parent(ref data),
                ..
            } => {
                extend_heap(&mut nodes, &data.children, query_point, &mut smallest_min_max);
            }
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Leaf(ref t),
                distance
            } => {
                return Some((t, distance));
            }
        }
    }
    None
}

/// The maximum number of subtrees to track when doing tree-to-tree
/// all-nearest-neighbors.
const MAX_AKNN_SUBTREES: usize = 16;

/// A nearest neighbor pair with the squared euclidean distance between them.
pub struct NearestNeighbors<'a, 'b, T: PointDistance> {
    /// The nearest neighbor found to `query`'s location.
    pub target: &'a T,
    /// The node whose location was used for the query.
    pub query: &'b T,
    /// Squared euclidean distance between the nodes.
    pub distance_2: <<T::Envelope as Envelope>::Point as Point>::Scalar,
}

/// Yield an iterator over nearest neighbors between a pair of trees.
///
/// Note this is note symmetric. Neighbors are found in `target_node`'s tree for
/// each node in `query_node`'s tree.
pub fn all_nearest_neighbors<'a, T>(
    target_node: &'a ParentNode<T>,
    query_node: &'a ParentNode<T>,
) -> impl Iterator<Item=NearestNeighbors<'a, 'a, T>> + 'a
where
    T: PointDistance + 'a,
{
    AllNearestNeighborsIterator::new(target_node, query_node)
}


pub struct AllNearestNeighborsIterator<'a, T>
where
    T: PointDistance + 'a,
{
    /// Stack of subtrees of the target tree that are candidate nearest nodes
    /// for each depth of the current location in the query tree.
    stack: Vec<NeighborSubtrees<'a, T>>,
    /// LIFO queue of query nodes whose nearest neighbors or neighest neighbor
    /// covering subtrees are to be found in depth-first order.
    queue: Vec<(&'a RTreeNode<T>, usize)>,
}

impl<'a, T> AllNearestNeighborsIterator<'a, T>
where
    T: PointDistance + 'a,
{
    fn new(
        target_node: &'a ParentNode<T>,
        query_node: &'a ParentNode<T>,
    ) -> Self {
        Self {
            stack: vec![NeighborSubtrees::root(target_node, query_node)],
            queue: query_node.children.iter().map(|child| (child, 0)).collect(),
        }
    }
}

impl<'a, T> Iterator for AllNearestNeighborsIterator<'a, T>
where
    T: PointDistance,
{
    type Item = NearestNeighbors<'a, 'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        // Fetch the next query node from the queue.
        while let Some((node, depth)) = self.queue.pop() {
            match node {
                RTreeNode::Parent(ref node) => {
                    // If a cached subtrees struct to hold the child subtrees
                    // doesn't already exist, create it.
                    if self.stack.len() < depth + 2 {
                        self.stack.push(NeighborSubtrees::empty());
                    }
                    let pair = &mut self.stack[depth..depth+2];
                    let (parent, child) = pair.split_at_mut(1);
                    child[0].child_subtrees(&parent[0], node);

                    // Add children of the query node to the end of the LIFO queue
                    // for depth-first traversal.
                    self.queue.extend(node.children().iter().map(|child| (child, depth + 1)));
                },
                RTreeNode::Leaf(ref leaf) => {
                    // Find the nearest neighbor for `leaf`. The stack at the
                    // node's depth contains subtrees of the target tree that
                    // cover any potential nearest neighbor matches.
                    let subtrees = &self.stack[depth];

                    return nearest_neighbor_inner(
                        &subtrees.target_nodes,
                        // FIXME: inelegant solution to recover leaf's point.
                        leaf.envelope().center()
                    ).map(|(target, distance_2)| NearestNeighbors {
                        query: leaf,
                        target,
                        distance_2
                    })
                }
            }
        }

        None
    }
}


struct NeighborSubtrees<'a, T>
where
    T: PointDistance + 'a
{
    /// Nodes comprising a subtree of the target tree that cover all potential
    /// neighest neighbor matches.
    target_nodes: Vec<&'a RTreeNode<T>>,
}

impl<'a, T> NeighborSubtrees<'a, T>
where
    T: PointDistance + 'a,
{
    fn empty() -> Self {
        Self {
            target_nodes: vec![]
        }
    }

    fn root(
        target_node: &'a ParentNode<T>,
        query_node: &'a ParentNode<T>,
    ) -> Self {
        let target_nodes = target_node.children().iter().collect();
        let preroot = Self {
            target_nodes,
        };
        let mut root = Self::empty();
        root.child_subtrees(&preroot, query_node);
        root
    }

    /// Replace the contents of this subtree with subtrees of `parent`'s target
    /// subtrees that are guaranteed to cover any nearest neighbor queries from
    /// `query_node`.
    fn child_subtrees(
        &mut self,
        parent: &Self,
        query_node: &'a ParentNode<T>,
    ) {
        self.target_nodes.clear();
        if parent.target_nodes.len() < MAX_AKNN_SUBTREES {
            // If the set of target subtrees is not too large, subdivide each
            // subtree into its children so they can be individually pruned
            // by distance to the query.
            parent.target_nodes.iter().for_each(|node| {
                match *node {
                    RTreeNode::Parent(ref parent) => self.target_nodes.extend(&parent.children),
                    leaf @ RTreeNode::Leaf(..) => self.target_nodes.push(leaf),
                }
            });
        } else {
            // If the set of target subtrees is already large, retain it rather
            // than further subdividing.
            self.target_nodes.extend(&parent.target_nodes);
        };

        // For each target subtree, find the distance which guarantees any
        // potential elements in the query node's envelope have a match with
        // the target subtree's envelope. Find the minimal such distance.
        let min_max_dist = self.target_nodes.iter().fold(Bounded::max_value(), |min_max_dist, node| {
            let dist = query_node.envelope.max_min_max_dist_2(&node.envelope());
            min_inline(min_max_dist, dist)
        });

        // Only retain subtrees that potentially have a match with the query
        // node nearer than the min max distance computed above.
        self.target_nodes.retain(|node| {
            let distance = node.envelope().min_dist_2(&query_node.envelope);
            distance <= min_max_dist
        });
    }
}

#[cfg(test)]
mod test {
    use crate::object::PointDistance;
    use crate::rtree::RTree;
    use crate::test_utilities::*;

    #[test]
    fn test_nearest_neighbor_empty() {
        let tree: RTree<[f32; 2]> = RTree::new();
        assert!(tree.nearest_neighbor(&[0.0, 213.0]).is_none());
    }

    #[test]
    fn test_nearest_neighbor() {
        let points = create_random_points(1000, SEED_1);
        let tree = RTree::bulk_load(points.clone());

        let sample_points = create_random_points(100, SEED_2);
        for sample_point in &sample_points {
            let mut nearest = None;
            let mut closest_dist = ::std::f64::INFINITY;
            for point in &points {
                let delta = [point[0] - sample_point[0], point[1] - sample_point[1]];
                let new_dist = delta[0] * delta[0] + delta[1] * delta[1];
                if new_dist < closest_dist {
                    closest_dist = new_dist;
                    nearest = Some(point);
                }
            }
            assert_eq!(nearest, tree.nearest_neighbor(sample_point));
        }
    }

    #[test]
    fn test_all_nearest_neighbors() {
        let points = create_random_points(1_000, SEED_1);
        let tree = RTree::bulk_load(points.clone());

        let mut tree_sequential = RTree::new();
        for point in &points {
            tree_sequential.insert(*point);
        }

        // Test that in identical trees, all-nearest-neighbors match the
        // identical nodes with themselves.
        for neighbors in super::all_nearest_neighbors(tree.root(), tree_sequential.root()) {
            assert_eq!(neighbors.query, neighbors.target);
            assert_eq!(neighbors.distance_2, 0.0);
        }

        assert_eq!(super::all_nearest_neighbors(tree.root(), tree_sequential.root()).count(), points.len());

        // For different trees, test that the all-nearest-neighbor results match
        // individual nearest neighbors.
        // From random testing, the large number of points is necessary to catch
        // errors in the pruning algorithm.
        let sample_points = create_random_points(10_000, SEED_2);
        let sample_tree = RTree::bulk_load(sample_points.clone());
        for neighbors in super::all_nearest_neighbors(tree.root(), sample_tree.root()) {
            let single_neighbor = tree.nearest_neighbor(neighbors.query);
            assert_eq!(Some(neighbors.target), single_neighbor);
        }
    }

    #[test]
    fn test_nearest_neighbor_iterator() {
        let mut points = create_random_points(1000, SEED_1);
        let tree = RTree::bulk_load(points.clone());

        let sample_points = create_random_points(50, SEED_2);
        for sample_point in &sample_points {
            points.sort_by(|r, l| {
                r.distance_2(sample_point)
                    .partial_cmp(&l.distance_2(&sample_point))
                    .unwrap()
            });
            let collected: Vec<_> = tree.nearest_neighbor_iter(sample_point).cloned().collect();
            assert_eq!(points, collected);
        }
    }

    #[test]
    fn test_nearest_neighbor_iterator_with_distance_2() {
        let points = create_random_points(1000, SEED_2);
        let tree = RTree::bulk_load(points.clone());

        let sample_points = create_random_points(50, SEED_1);
        for sample_point in &sample_points {
            let mut last_distance = 0.0;
            for (point, distance) in tree.nearest_neighbor_iter_with_distance_2(&sample_point) {
                assert_eq!(point.distance_2(sample_point), distance);
                assert!(last_distance < distance);
                last_distance = distance;
            }
        }
    }
}
