use smallvec::SmallVec;
use node::{ParentNodeData, RTreeNode};
use params::RTreeParams;
use object::RTreeObject;
use point::{Point, min_inline};
use num_traits::{Bounded};
use envelope::Envelope;

pub fn nearest_neighbor<'a, T, Params> (
    node: &'a ParentNodeData<T, Params>,
    point: &T::Point,
    nearest_distance: &mut <T::Point as Point>::Scalar)
    -> Option<&'a T>
    where Params: RTreeParams,
          T: RTreeObject
{
    let mut nearest = None;
    // Calculate smallest minmax-distance
    let mut smallest_min_max: <T::Point as Point>::Scalar = Bounded::max_value();
    for child in node.children.iter() {
        let new_min = child.mbr().min_max_dist_2(point);
        smallest_min_max = min_inline(smallest_min_max, new_min);
    }
    let mut sorted: SmallVec<[_; 8]> = SmallVec::new();
    for child in node.children.iter() {
        let min_dist = child.mbr().distance_2(point);
        if min_dist <= smallest_min_max {
            sorted.push((child, min_dist));
        }
    }
    sorted.sort_by(|l, r| l.1.partial_cmp(&r.1).unwrap());

    for &(child, min_dist) in sorted.iter() {
        if min_dist > *nearest_distance {
            // Prune this element
            break;
        }
        match child {
            &RTreeNode::Parent(ref data) => {
                if let Some(t) = nearest_neighbor(data, point, nearest_distance) {
                    nearest = Some(t);
                }
            },
            &RTreeNode::Leaf(ref t) => {
                let distance = t.distance_2(point);
                if distance < *nearest_distance {
                    nearest = Some(t);
                    *nearest_distance = distance;
                }
            }
        }
    }
    nearest
}

#[cfg(test)]
mod test {
    use testutils::create_random_points;
    use rtree::RTree;

    #[test]
    fn test_nearest_neighbor_empty() {
        let tree: RTree<[f32; 2]> = RTree::new();
        assert!(tree.nearest_neighbor(&[0.0, 213.0]).is_none());
    }

    #[test]
    fn test_nearest_neighbor() {
        let points = create_random_points(1000, [10, 233, 588812, 411112]);
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
        }
        let sample_points = create_random_points(100, [66, 123, 12345, 112]);
        for sample_point in &sample_points {
            let mut nearest = None;
            let mut closest_dist = ::std::f32::INFINITY;
            for point in &points {
                let delta = [point[0] - sample_point[0], point[1] - sample_point[1]];
                let new_dist = delta[0] * delta[0] + delta[1] * delta[1];
                if new_dist < closest_dist {
                    closest_dist = new_dist;
                    nearest = Some(point);
                }
            }
            assert!(nearest == tree.nearest_neighbor(sample_point));
        }
    }
}
