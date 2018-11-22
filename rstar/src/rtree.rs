use crate::envelope::Envelope;
use crate::algorithm::iterators::*;
use crate::structures::node::ParentNodeData;
use crate::object::{PointDistance, RTreeObject};
use crate::params::{DefaultParams, RTreeParams};
use crate::algorithm::removal;
use crate::algorithm::removal::*;
use crate::algorithm::selection_functions::*;
use crate::Point;
use crate::algorithm::nearest_neighbor;
use crate::algorithm::bulk_load;

/// Defines how points are inserted into an r-tree.
///
/// Different strategies try to minimize both _insertion time_ (how long does it take to add a new
/// object into the tree?) and _querying time_ (how long does an average nearest neighbor query
/// take?).
/// Currently, only one insertion strategy is implemented: R* (R-star) insertion. R* insertion
/// tries to minimize querying performance while yielding reasonable insertion times, making it a
/// good default strategy. More strategies might be implemented in the future.
///
/// This trait is not meant to be implemented by the user.
pub trait InsertionStrategy {
    fn insert<T, Params>(tree: &mut RTree<T, Params>, t: T)
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

/// An n-dimensional R-tree data structure.
///
/// # R-Trees
/// R-trees are tree data structures for multi dimensional data which support efficient
/// insertion operations and nearest neighbor queries. Also, other types of queries, like
/// retrieving all objects within a rectangle or a circle, can be implemented efficiently.
///
/// # Usage
/// The items inserted into an r-tree must implement the [RTreeObject](trait.RTreeObject.html)
/// trait. To support nearest neighbor queries, implement the [PointDistance](trait.PointDistance.html)
/// trait. Some useful geometric primitives that implement the above traits can be found in the
/// [primitives](mod.primitives.html) module.
/// ## Example
/// // TODO
///
/// ## Supported point types
/// All types implementing the [Point](trait.Point.html) trait can be used as underlying point type.
/// By default, fixed size arrays can be used as points.
///
/// # Type Parameters
/// `T`: The type of objects stored in the r-tree.
/// `Params`: Compile time parameters that change the r-trees internal layout. Please refer to the
/// [RTreeParams](trait.RTreeParams.html) trait for more information.
pub struct RTree<T, Params = DefaultParams>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    root: ParentNodeData<T>,
    size: usize,
    _params: ::std::marker::PhantomData<Params>,
}

impl<T> RTree<T>
where
    T: RTreeObject,
{
    /// Creates a new, empty r-tree.
    ///
    /// The created r-tree is configured with default parameters.
    pub fn new() -> Self {
        Self::new_with_params()
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    /// Creates a new, empty r-tree.
    ///
    /// The tree's compile time parameters must be specified. Please refer to the
    /// [RTreeParams](trait.RTreeParams.html) trait for more information.
    pub fn new_with_params() -> Self {
        RTree {
            root: ParentNodeData::new_root::<Params>(),
            size: 0,
            _params: Default::default(),
        }
    }

    /// Returns the number of objects in an r-tree.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn root(&self) -> &ParentNodeData<T> {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut ParentNodeData<T> {
        &mut self.root
    }

    pub fn iter(&self) -> RTreeIterator<T> {
        RTreeIterator::new(self, SelectAllFunc)
    }

    pub fn iter_mut(&mut self) -> RTreeIteratorMut<T> {
        RTreeIteratorMut::new(self, SelectAllFunc)
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
    ) -> LocateAllAtPoint<T> {
        LocateAllAtPoint::new(self, SelectAtPointFunc::new(*point))
    }

    pub fn locate_all_at_point_mut(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> LocateAllAtPointMut<T> {
        LocateAllAtPointMut::new(self, SelectAtPointFunc::new(*point))
    }

    pub fn locate_in_envelope(&self, envelope: &T::Envelope) -> LocateInEnvelope<T> {
        // println!("Locate_in_envelope {:?}", envelope);
        LocateInEnvelope::new(self, SelectInEnvelopeFunc::new(*envelope))
    }

    pub fn locate_in_envelope_mut(&mut self, envelope: &T::Envelope) -> LocateInEnvelopeMut<T> {
        LocateInEnvelopeMut::new(self, SelectInEnvelopeFunc::new(*envelope))
    }

    pub fn locate_in_envelope_intersecting(
        &self,
        envelope: &T::Envelope,
    ) -> LocateInEnvelopeIntersecting<T> {
        LocateInEnvelopeIntersecting::new(self, SelectInEnvelopeFuncIntersecting::new(*envelope))
    }

    pub fn locate_in_envelope_intersecting_mut(
        &mut self,
        envelope: &T::Envelope,
    ) -> LocateInEnvelopeIntersectingMut<T> {
        LocateInEnvelopeIntersectingMut::new(self, SelectInEnvelopeFuncIntersecting::new(*envelope))
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: PointDistance,
{
    pub fn remove_with_distance_function(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
        distance_2: <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<T> {
        let removal_function = RemoveWithDistanceFunction::new(*point, distance_2);
        let result = remove::<_, Params, _>(self.root_mut(), &removal_function);
        if result.is_some() {
            self.size -= 1;
        }
        result
    }

    pub fn remove_at_point(&mut self, point: &<T::Envelope as Envelope>::Point) -> Option<T> {
        let removal_function = RemoveAtPointFunction::new(*point);
        let result = removal::remove::<_, Params, _>(self.root_mut(), &removal_function);
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
        let removal_function = RemoveEqualsFunction::new(t);
        let result = removal::remove::<_, Params, _>(self.root_mut(), &removal_function);
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
            nearest_neighbor::nearest_neighbor(self.root(), query_point)
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
        nearest_neighbor::NearestNeighborIterator::new(self.root(), query_point)
    }
}

impl<T, Params> RTree<T, Params>
where
    T: RTreeObject + ::std::fmt::Debug,
    Params: RTreeParams,
{
    pub fn insert(&mut self, t: T) {
        // println!("insert {:?}", t);
        Params::DefaultInsertionStrategy::insert(self, t);
        self.size += 1;
    }
}

impl<T, Params> RTree<T, Params>
where
    T: RTreeObject + Clone + ::std::fmt::Debug,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    pub fn bulk_load_with_params(elements: &mut Vec<T>) -> Self {
        let root = 
        bulk_load::bulk_load_with_params::<_, Params>(elements);
        RTree {
            root,
            size: elements.len(),
            _params: Default::default(),
        }
    }
}

impl<T> RTree<T>
where
    T: RTreeObject + Clone + ::std::fmt::Debug,
    <T::Envelope as Envelope>::Point: Point,
{
    pub fn bulk_load(elements: &mut [T]) -> Self {
        // println!("bulk load\n{:?}", elements);
        RTree {
            root: bulk_load::bulk_load_with_params::<_, DefaultParams>(elements),
            size: elements.len(),
            _params: Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::RTree;
    use crate::params::RTreeParams;
    use crate::algorithm::rstar::RStarInsertionStrategy;
    use crate::test_utilities::create_random_points;

    struct TestParams;
    impl RTreeParams for TestParams {
        const MIN_SIZE: usize = 10;
        const MAX_SIZE: usize = 20;
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
        assert!(!tree.contains(&[0.3, 0.2]));
    }

    #[test]
    fn test_insert_many() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS, *b"c0un<er1nfl4ti>n");
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
