use crate::node::{ParentNode, RTreeNode};
use crate::point::{min_inline, Point};
use crate::{Envelope, PointDistance, RTreeObject};

use alloc::{collections::BinaryHeap, vec, vec::Vec};
use heapless::binary_heap as static_heap;
use num_traits::Bounded;

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
    fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
        // Inverse comparison creates a min heap
        other.distance.partial_cmp(&self.distance)
    }
}

impl<'a, T> Eq for RTreeNodeDistanceWrapper<'a, T> where T: PointDistance {}

impl<'a, T> Ord for RTreeNodeDistanceWrapper<'a, T>
where
    T: PointDistance,
{
    fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
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

pub fn nearest_neighbor<T>(
    node: &ParentNode<T>,
    query_point: <T::Envelope as Envelope>::Point,
) -> Option<&T>
where
    T: PointDistance,
{
    fn extend_heap<'a, T>(
        nodes: &mut SmallHeap<RTreeNodeDistanceWrapper<'a, T>>,
        node: &'a ParentNode<T>,
        query_point: <T::Envelope as Envelope>::Point,
        min_max_distance: &mut <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) where
        T: PointDistance + 'a,
    {
        for child in &node.children {
            let distance_if_less_or_equal = match child {
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
                    child.envelope().min_max_dist_2(&query_point),
                );
                nodes.push(RTreeNodeDistanceWrapper {
                    node: child,
                    distance,
                });
            }
        }
    }

    // Calculate smallest minmax-distance
    let mut smallest_min_max: <<T::Envelope as Envelope>::Point as Point>::Scalar =
        Bounded::max_value();
    let mut nodes = SmallHeap::new();
    extend_heap(&mut nodes, node, query_point, &mut smallest_min_max);
    while let Some(current) = nodes.pop() {
        match current {
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Parent(ref data),
                ..
            } => {
                extend_heap(&mut nodes, data, query_point, &mut smallest_min_max);
            }
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Leaf(ref t),
                ..
            } => {
                return Some(t);
            }
        }
    }
    None
}

pub fn nearest_neighbors<'a, T>(
    node: &'a ParentNode<T>,
    query_point: <T::Envelope as Envelope>::Point,
) -> Vec<&'a T>
where
    T: PointDistance,
{
    let mut nearest_neighbors = NearestNeighborIterator::new(node, query_point);

    let first_nearest_neighbor = match nearest_neighbors.next() {
        None => return vec![], // If we have an empty tree, just return an empty vector.
        Some(nn) => nn,
    };

    // The result will at least contain the first nearest neighbor.
    let mut result = vec![first_nearest_neighbor];

    // We compute the distance to the first nearest neighbor, and use
    // that distance to filter out the rest of the nearest neighbors that are farther
    // than this first neighbor.
    let distance = first_nearest_neighbor.envelope().distance_2(&query_point);
    nearest_neighbors
        .take_while(|nearest_neighbor| {
            let next_distance = nearest_neighbor.envelope().distance_2(&query_point);
            next_distance == distance
        })
        .for_each(|nearest_neighbor| result.push(nearest_neighbor));

    result
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
            let mut closest_dist = ::core::f64::INFINITY;
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
    fn test_nearest_neighbors_empty() {
        let tree: RTree<[f32; 2]> = RTree::new();
        assert!(tree.nearest_neighbors(&[0.0, 213.0]).is_empty());
    }

    #[test]
    fn test_nearest_neighbors() {
        let points = create_random_points(1000, SEED_1);
        let tree = RTree::bulk_load(points.clone());

        let sample_points = create_random_points(50, SEED_2);
        for sample_point in &sample_points {
            let nearest_neighbors = tree.nearest_neighbors(sample_point);
            let mut distance = -1.0;
            for nn in &nearest_neighbors {
                if distance < 0.0 {
                    distance = sample_point.distance_2(nn);
                } else {
                    let new_distance = sample_point.distance_2(nn);
                    assert_eq!(new_distance, distance);
                }
            }
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
