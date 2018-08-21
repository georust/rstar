use envelope::Envelope;
use iterators::{
    LocateAllAtPoint, LocateAllAtPointMut, LocateInEnvelope, LocateInEnvelopeMut, RTreeIterator,
    RTreeIteratorMut,
};
use metrics::RTreeMetrics;
use node::ParentNodeData;
use object::{PointDistance, RTreeObject};
use params::{DefaultParams, RTreeParams};
use point::EuclideanPoint;
use selection_funcs::SelectionFunc;

pub trait InsertionStrategy {
    fn insert<T, Params>(&mut RTree<T, Params>, t: T, metrics: &mut RTreeMetrics)
    where
        Params: RTreeParams,
        T: RTreeObject;
}

impl<T> Default for RTree<T>
where
    T: RTreeObject,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct RTree<T, Params = DefaultParams>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    root: ParentNodeData<T, Params>,
    size: usize,
    height: usize,
}

impl<T> RTree<T>
where
    T: RTreeObject,
{
    pub fn new() -> Self {
        Self::new_with_params()
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    pub fn new_with_params() -> Self {
        RTree {
            root: ParentNodeData::new_root(),
            size: 0,
            height: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn root(&self) -> &ParentNodeData<T, Params> {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut ParentNodeData<T, Params> {
        &mut self.root
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn set_height(&mut self, new_height: usize) {
        self.height = new_height;
    }

    #[cfg(not(feature = "debug"))]
    pub fn insert(&mut self, t: T) {
        Params::DefaultInsertionStrategy::insert(self, t, &mut RTreeMetrics {});
        self.size += 1;
    }

    #[cfg(feature = "debug")]
    pub fn insert(&mut self, t: T, metrics: &mut RTreeMetrics) {
        Params::DefaultInsertionStrategy::insert(self, t, metrics);
        self.size += 1;
    }

    pub fn iter(&self) -> RTreeIterator<T, Params> {
        RTreeIterator::new(self, ())
    }

    pub fn iter_mut(&mut self) -> RTreeIteratorMut<T, Params> {
        RTreeIteratorMut::new(self, ())
    }

    pub fn locate_at_point(&self, point: &<T::Envelope as Envelope>::Point) -> Option<&T> {
        self.locate_all_at_point(point).next()
    }

    pub fn locate_at_point_mut(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> Option<&mut T> {
        self.locate_all_at_point_mut(point).next()
    }

    pub fn locate_all_at_point(
        &self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> LocateAllAtPoint<T, Params> {
        LocateAllAtPoint::new(self, *point)
    }

    pub fn locate_all_at_point_mut(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> LocateAllAtPointMut<T, Params> {
        LocateAllAtPointMut::new(self, *point)
    }

    pub fn locate_in_envelope(&self, envelope: &T::Envelope) -> LocateInEnvelope<T, Params> {
        LocateInEnvelope::new(self, *envelope)
    }

    pub fn locate_in_envelope_mut(
        &mut self,
        envelope: &T::Envelope,
    ) -> LocateInEnvelopeMut<T, Params> {
        LocateInEnvelopeMut::new(self, *envelope)
    }

    pub fn remove_at_point(&mut self, point: &<T::Envelope as Envelope>::Point) -> Option<T> {
        let removal_function = ::removal::RemoveAtPointFunction::new(*point);
        let result = ::removal::remove(self.root_mut(), &removal_function);
        if result.is_some() {
            self.size -= 1;
        }
        result
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject + PartialEq,
{
    pub fn contains(&self, t: &T) -> bool {
        self.locate_in_envelope(&t.envelope()).any(|e| e == t)
    }

    pub fn contains_mut(&mut self, t: &T) -> Option<&mut T> {
        self.locate_in_envelope_mut(&t.envelope()).find(|e| e == &t)
    }

    pub fn remove(&mut self, t: &T) -> Option<T> {
        let removal_function = ::removal::RemoveEqualsFunction::new(t);
        let result = ::removal::remove(self.root_mut(), &removal_function);
        if result.is_some() {
            self.size -= 1;
        }
        result
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: PointDistance,
{
    pub fn nearest_neighbor<'a, 'b>(
        &'a self,
        query_point: &'b <T::Envelope as Envelope>::Point,
    ) -> Option<&'a T>
    where
        'b: 'a,
    {
        if self.size > 0 {
            ::nearest_neighbor::nearest_neighbor(self.root(), query_point)
                .or_else(|| self.nearest_neighbor_iter(query_point).next())
        } else {
            None
        }
    }

    pub fn nearest_neighbor_iter<'a, 'b>(
        &'a self,
        query_point: &'b <T::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = &'a T>
    where
        'b: 'a,
    {
        ::nearest_neighbor::NearestNeighborIterator::new(self.root(), query_point)
    }
}

impl<T, Params> RTree<T, Params>
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: EuclideanPoint,
    Params: RTreeParams,
{
    pub fn bulk_load_with_params(elements: &mut Vec<T>) -> Self {
        let (root, height) = ::bulk_load::bulk_load_with_params(elements);
        RTree {
            root,
            size: elements.len(),
            height,
        }
    }
}

impl<T> RTree<T>
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: EuclideanPoint,
{
    pub fn bulk_load(elements: &mut [T]) -> Self {
        let (root, height) = ::bulk_load::bulk_load_with_params(elements);
        RTree {
            root,
            size: elements.len(),
            height,
        }
    }
}

#[cfg(test)]
mod test {
    use super::RTree;
    use params::RTreeParams;
    use rstar::RStarInsertionStrategy;
    use testutils::create_random_points;

    struct TestParams;
    impl RTreeParams for TestParams {
        const MIN_SIZE: usize = 10;
        const MAX_SIZE: usize = 20;
        const REINSERTION_COUNT: usize = 0;
        type DefaultInsertionStrategy = RStarInsertionStrategy;
    }

    #[test]
    fn test_create_rtree_with_parameters() {
        let tree: RTree<[f32; 2], TestParams> = RTree::new_with_params();
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_insert_single() {
        let mut tree: RTree<_> = RTree::new();
        tree.insert([0.02f32, 0.4f32]);
        assert_eq!(tree.size(), 1);
        assert!(tree.contains(&[0.02, 0.4]));
        assert_eq!(tree.height(), 1);
        assert!(!tree.contains(&[0.3, 0.2]));
    }

    #[test]
    fn test_insert_many() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS, *b"c0unter1nfl4tion");
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
        }
        assert_eq!(tree.size(), NUM_POINTS);
        for p in &points {
            assert!(tree.contains(p));
        }
    }
}
