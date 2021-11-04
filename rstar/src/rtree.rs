use crate::algorithm::bulk_load;
use crate::algorithm::intersection_iterator::IntersectionIterator;
use crate::algorithm::iterators::*;
use crate::algorithm::nearest_neighbor;
use crate::algorithm::removal;
use crate::algorithm::removal::DrainIterator;
use crate::algorithm::selection_functions::*;
use crate::envelope::Envelope;
use crate::node::ParentNode;
use crate::object::{PointDistance, RTreeObject};
use crate::params::{verify_parameters, DefaultParams, InsertionStrategy, RTreeParams};
use crate::Point;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

impl<T, Params> Default for RTree<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    fn default() -> Self {
        Self::new_with_params()
    }
}

/// An n-dimensional r-tree data structure.
///
/// # R-Trees
/// R-Trees are data structures containing multi-dimensional objects like points, rectangles
/// or polygons. They are optimized for retrieving the nearest neighbor at any point.
///
/// R-trees can efficiently find answers to queries like "Find the nearest point of a polygon",
/// "Find all police stations within a rectangle" or "Find the 10 nearest restaurants, sorted
/// by their distances". Compared to a naive implementation for these scenarios that runs
/// in `O(n)` for `n` inserted elements, r-trees reduce this time to `O(log(n))`.
///
/// However, creating an r-tree is time consuming
/// and runs in `O(n * log(n))`. Thus, r-trees are suited best if many queries and only few
/// insertions are made. rstar also supports [bulk loading](RTree::bulk_load),
/// which cuts down the constant factors when creating an r-tree significantly compared to
/// sequential insertions.
///
/// R-trees are also _dynamic_: points can be inserted and removed from an existing tree.
///
/// ## Partitioning heuristics
/// The inserted objects are internally partitioned into several boxes which should have small
/// overlap and volume. This is done heuristically. While the originally proposed heuristic focused
/// on fast insertion operations, the resulting r-trees were often suboptimally structured. Another
/// heuristic, called `R*-tree` (r-star-tree), was proposed to improve the tree structure at the cost of
/// longer insertion operations and is currently the crate's only implemented
/// [insertion strategy].
///
/// ## Further reading
/// For more information refer to the [wikipedia article](https://en.wikipedia.org/wiki/R-tree).
///
/// # Usage
/// The items inserted into an r-tree must implement the [RTreeObject]
/// trait. To support nearest neighbor queries, implement the [PointDistance]
/// trait. Some useful geometric primitives that implement the above traits can be found in the
/// [crate::primitives]x module. Several primitives in the [`geo-types`](https://docs.rs/geo-types/) crate also
/// implement these traits.
///
/// ## Example
/// ```
/// use rstar::RTree;
///
/// let mut tree = RTree::new();
/// tree.insert([0.1, 0.0f32]);
/// tree.insert([0.2, 0.1]);
/// tree.insert([0.3, 0.0]);
///
/// assert_eq!(tree.nearest_neighbor(&[0.4, -0.1]), Some(&[0.3, 0.0]));
/// tree.remove(&[0.3, 0.0]);
/// assert_eq!(tree.nearest_neighbor(&[0.4, 0.3]), Some(&[0.2, 0.1]));
///
/// assert_eq!(tree.size(), 2);
/// // &RTree implements IntoIterator!
/// for point in &tree {
///     println!("Tree contains a point {:?}", point);
/// }
/// ```
///
/// ## Supported point types
/// All types implementing the [Point] trait can be used as underlying point type.
/// By default, fixed size arrays can be used as points.
///
/// ## Type Parameters
/// * `T`: The type of objects stored in the r-tree.
/// * `Params`: Compile time parameters that change the r-tree's internal layout. Refer to the
/// [RTreeParams] trait for more information.
///
/// ## Defining methods generic over r-trees
/// If a library defines a method that should be generic over the r-tree type signature, make
/// sure to include both type parameters like this:
/// ```
/// # use rstar::{RTree,RTreeObject, RTreeParams};
/// pub fn generic_rtree_function<T, Params>(tree: &mut RTree<T, Params>)
/// where
///   T: RTreeObject,
///   Params: RTreeParams
/// {
///   // ...
/// }
/// ```
/// Otherwise, any user of `generic_rtree_function` would be forced to use
/// a tree with default parameters.
///
/// # Runtime and Performance
/// The runtime of query operations (e.g. `nearest neighbor` or `contains`) is usually
/// `O(log(n))`, where `n` refers to the number of elements contained in the r-tree.
/// A naive sequential algorithm would take `O(n)` time. However, r-trees incur higher
/// build up times: inserting an element into an r-tree costs `O(log(n))` time.
///
/// ## Bulk loading
/// In many scenarios, insertion is only carried out once for many points. In this case,
/// [RTree::bulk_load] will be considerably faster. Its total run time
/// is still `O(log(n))`.
///
/// ## Element distribution
/// The tree's performance heavily relies on the spatial distribution of its elements.
/// Best performance is achieved if:
///  * No element is inserted more than once
///  * The overlapping area of elements is as small as
///    possible.
///
/// For the edge case that all elements are overlapping (e.g, one and the same element
/// is contained `n` times), the performance of most operations usually degrades to `O(n)`.
///
/// # (De)Serialization
/// Enable the `serde` feature for [Serde](https://crates.io/crates/serde) support.
///
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(
        serialize = "T: Serialize, T::Envelope: Serialize",
        deserialize = "T: Deserialize<'de>, T::Envelope: Deserialize<'de>"
    ))
)]
pub struct RTree<T, Params = DefaultParams>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    root: ParentNode<T>,
    size: usize,
    _params: ::std::marker::PhantomData<Params>,
}

struct DebugHelper<'a, T, Params>
where
    T: RTreeObject + ::std::fmt::Debug + 'a,
    Params: RTreeParams + 'a,
{
    rtree: &'a RTree<T, Params>,
}

impl<'a, T, Params> ::std::fmt::Debug for DebugHelper<'a, T, Params>
where
    T: RTreeObject + ::std::fmt::Debug,
    Params: RTreeParams,
{
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.debug_set().entries(self.rtree.iter()).finish()
    }
}

impl<T, Params> ::std::fmt::Debug for RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject + ::std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter
            .debug_struct("RTree")
            .field("size", &self.size)
            .field("items", &DebugHelper { rtree: &self })
            .finish()
    }
}

impl<T> RTree<T>
where
    T: RTreeObject,
{
    /// Creates a new, empty r-tree.
    ///
    /// The created r-tree is configured with [default parameters](DefaultParams).
    pub fn new() -> Self {
        Self::new_with_params()
    }

    /// Creates a new r-tree with some elements already inserted.
    ///
    /// This method should be the preferred way for creating r-trees. It both
    /// runs faster and yields an r-tree with better internal structure that
    /// improves query performance.
    ///
    /// This method implements the overlap minimizing top-down bulk loading algorithm (OMT)
    /// as described in [this paper by Lee and Lee (2003)](http://ceur-ws.org/Vol-74/files/FORUM_18.pdf).
    ///
    /// # Runtime
    /// Bulk loading runs in `O(n * log(n))`, where `n` is the number of loaded
    /// elements.
    pub fn bulk_load(elements: Vec<T>) -> Self {
        Self::bulk_load_with_params(elements)
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject,
{
    /// Creates a new, empty r-tree.
    ///
    /// The tree's compile time parameters must be specified. Refer to the
    /// [RTreeParams] trait for more information and a usage example.
    pub fn new_with_params() -> Self {
        verify_parameters::<T, Params>();
        RTree {
            root: ParentNode::new_root::<Params>(),
            size: 0,
            _params: Default::default(),
        }
    }

    /// Creates a new r-tree with some given elements and configurable parameters.
    ///
    /// For more information refer to [RTree::bulk_load]
    /// and [RTreeParams].
    pub fn bulk_load_with_params(elements: Vec<T>) -> Self {
        Self::new_from_bulk_loading(elements, bulk_load::bulk_load_sequential::<_, Params>)
    }

    /// Returns the number of objects in an r-tree.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    ///
    /// let mut tree = RTree::new();
    /// assert_eq!(tree.size(), 0);
    /// tree.insert([0.0, 1.0, 2.0]);
    /// assert_eq!(tree.size(), 1);
    /// tree.remove(&[0.0, 1.0, 2.0]);
    /// assert_eq!(tree.size(), 0);
    /// ```
    pub fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn size_mut(&mut self) -> &mut usize {
        &mut self.size
    }

    /// Returns an iterator over all elements contained in the tree.
    ///
    /// The order in which the elements are returned is not specified.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// let tree = RTree::bulk_load(vec![(0.0, 0.1), (0.3, 0.2), (0.4, 0.2)]);
    /// for point in tree.iter() {
    ///     println!("This tree contains point {:?}", point);
    /// }
    /// ```
    pub fn iter(&self) -> RTreeIterator<T> {
        RTreeIterator::new(&self.root, SelectAllFunc)
    }

    /// Returns an iterator over all mutable elements contained in the tree.
    ///
    /// The order in which the elements are returned is not specified.
    ///
    /// *Note*: It is a logic error to change an inserted item's position or dimensions. This
    /// method is primarily meant for own implementations of [RTreeObject]
    /// which can contain arbitrary additional data.
    /// If the position or location of an inserted object need to change, you will need to [RTree::remove]
    /// and reinsert it.
    ///
    pub fn iter_mut(&mut self) -> RTreeIteratorMut<T> {
        RTreeIteratorMut::new(&mut self.root, SelectAllFunc)
    }

    /// Returns all elements contained in an [Envelope].
    ///
    /// Usually, an envelope is an [axis aligned bounding box](crate::AABB). This
    /// method can be used to retrieve all elements that are fully contained within an envelope.
    ///
    /// # Example
    /// ```
    /// use rstar::{RTree, AABB};
    /// let mut tree = RTree::bulk_load(vec![
    ///   [0.0, 0.0],
    ///   [0.0, 1.0],
    ///   [1.0, 1.0]
    /// ]);
    /// let half_unit_square = AABB::from_corners([0.0, 0.0], [0.5, 1.0]);
    /// let unit_square = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    /// let elements_in_half_unit_square = tree.locate_in_envelope(&half_unit_square);
    /// let elements_in_unit_square = tree.locate_in_envelope(&unit_square);
    /// assert_eq!(elements_in_half_unit_square.count(), 2);
    /// assert_eq!(elements_in_unit_square.count(), 3);
    /// ```
    pub fn locate_in_envelope(&self, envelope: &T::Envelope) -> LocateInEnvelope<T> {
        LocateInEnvelope::new(&self.root, SelectInEnvelopeFunction::new(*envelope))
    }

    /// Mutable variant of [locate_in_envelope](#method.locate_in_envelope).
    pub fn locate_in_envelope_mut(&mut self, envelope: &T::Envelope) -> LocateInEnvelopeMut<T> {
        LocateInEnvelopeMut::new(&mut self.root, SelectInEnvelopeFunction::new(*envelope))
    }

    /// Draining variant of [locate_in_envelope](#method.locate_in_envelope).
    pub fn drain_in_envelope(&mut self, envelope: T::Envelope) -> DrainIterator<T, SelectInEnvelopeFunction<T>, Params> {
        let sel = SelectInEnvelopeFunction::new(envelope);
        self.drain_with_selection_function(sel)
    }

    /// Returns all elements whose envelope intersects a given envelope.
    ///
    /// Any element fully contained within an envelope is also returned by this method. Two
    /// envelopes that "touch" each other (e.g. by sharing only a common corner) are also
    /// considered to intersect. Usually, an envelope is an [axis aligned bounding box](crate::AABB).
    /// This method will return all elements whose AABB has some common area with
    /// a given AABB.
    ///
    /// # Example
    /// ```
    /// use rstar::{RTree, AABB};
    /// use rstar::primitives::Rectangle;
    ///
    /// let left_piece = AABB::from_corners([0.0, 0.0], [0.4, 1.0]);
    /// let right_piece = AABB::from_corners([0.6, 0.0], [1.0, 1.0]);
    /// let middle_piece = AABB::from_corners([0.25, 0.0], [0.75, 1.0]);
    ///
    /// let mut tree = RTree::<Rectangle<_>>::bulk_load(vec![
    ///   left_piece.into(),
    ///   right_piece.into(),
    ///   middle_piece.into(),
    /// ]);
    ///
    /// let elements_intersecting_left_piece = tree.locate_in_envelope_intersecting(&left_piece);
    /// // The left piece should not intersect the right piece!
    /// assert_eq!(elements_intersecting_left_piece.count(), 2);
    /// let elements_intersecting_middle = tree.locate_in_envelope_intersecting(&middle_piece);
    /// // Only the middle piece intersects all pieces within the tree
    /// assert_eq!(elements_intersecting_middle.count(), 3);
    ///
    /// let large_piece = AABB::from_corners([-100., -100.], [100., 100.]);
    /// let elements_intersecting_large_piece = tree.locate_in_envelope_intersecting(&large_piece);
    /// // Any element that is fully contained should also be returned:
    /// assert_eq!(elements_intersecting_large_piece.count(), 3);
    pub fn locate_in_envelope_intersecting(
        &self,
        envelope: &T::Envelope,
    ) -> LocateInEnvelopeIntersecting<T> {
        LocateInEnvelopeIntersecting::new(
            &self.root,
            SelectInEnvelopeFuncIntersecting::new(*envelope),
        )
    }

    /// Mutable variant of [locate_in_envelope_intersecting](#method.locate_in_envelope_intersecting)
    pub fn locate_in_envelope_intersecting_mut(
        &mut self,
        envelope: &T::Envelope,
    ) -> LocateInEnvelopeIntersectingMut<T> {
        LocateInEnvelopeIntersectingMut::new(
            &mut self.root,
            SelectInEnvelopeFuncIntersecting::new(*envelope),
        )
    }

    /// Locates elements in the r-tree defined by a selection function.
    ///
    /// Refer to the documentation of [`SelectionFunction`] for
    /// more information.
    ///
    /// Usually, other `locate` methods should cover most common use cases. This method is only required
    /// in more specific situations.
    pub fn locate_with_selection_function<S: SelectionFunction<T>>(
        &self,
        selection_function: S,
    ) -> impl Iterator<Item = &T> {
        SelectionIterator::new(&self.root, selection_function)
    }

    /// Mutable variant of [`locate_with_selection_function`](#method.locate_with_selection_function).
    pub fn locate_with_selection_function_mut<S: SelectionFunction<T>>(
        &mut self,
        selection_function: S,
    ) -> impl Iterator<Item = &mut T> {
        SelectionIteratorMut::new(&mut self.root, selection_function)
    }

    /// Returns all possible intersecting objects of this and another tree.
    ///
    /// This will return all objects whose _envelopes_ intersect. No geometric intersection
    /// checking is performed.
    pub fn intersection_candidates_with_other_tree<'a, U>(
        &'a self,
        other: &'a RTree<U>,
    ) -> IntersectionIterator<T, U>
    where
        U: RTreeObject<Envelope = T::Envelope>,
    {
        IntersectionIterator::new(self.root(), other.root())
    }

    /// Returns the tree's root node.
    ///
    /// Usually, you will not need to call this method. However, for debugging purposes or for
    /// advanced algorithms, knowledge about the tree's internal structure may be required.
    /// For these cases, this method serves as an entry point.
    pub fn root(&self) -> &ParentNode<T> {
        &self.root
    }

    pub(crate) fn root_mut(&mut self) -> &mut ParentNode<T> {
        &mut self.root
    }

    fn new_from_bulk_loading(
        elements: Vec<T>,
        root_loader: impl Fn(Vec<T>) -> ParentNode<T>,
    ) -> Self {
        verify_parameters::<T, Params>();
        let size = elements.len();
        let root = if size == 0 {
            ParentNode::new_root::<Params>()
        } else {
            root_loader(elements)
        };
        RTree {
            root,
            size,
            _params: Default::default(),
        }
    }

    /// Removes and returns a single element from the tree. The element to remove is specified
    /// by a [`SelectionFunction`].
    ///
    /// See also: [`RTree::remove`], [`RTree::remove_at_point`]
    ///
    pub fn remove_with_selection_function<F>(&mut self, function: F) -> Option<T>
    where
        F: SelectionFunction<T>,
    {
        removal::DrainIterator::new(self, function).take(1).last()
    }

    /// Drain elements selected by a [`SelectionFunction`]. Returns an
    /// iterator that successively removes selected elements and returns
    /// them. This is the most generic drain API, see also:
    /// [`RTree::drain_in_envelope_intersecting`],
    /// [`RTree::drain_within_distance`].
    ///
    /// # Remarks
    ///
    /// This API is similar to `Vec::drain_filter`, but stopping the
    /// iteration would stop the removal. However, the returned iterator
    /// must be properly dropped. Leaking this iterator leads to a leak
    /// amplification, where all the elements in the tree are leaked.
    pub fn drain_with_selection_function<F>(&mut self, function: F) -> DrainIterator<T, F, Params>
    where
        F: SelectionFunction<T>,
    {
        removal::DrainIterator::new(self, function)
    }

    /// Drains elements intersecting the `envelope`. Similar to
    /// `locate_in_envelope_intersecting`, except the elements are removed
    /// and returned via an iterator.
    pub fn drain_in_envelope_intersecting(&mut self, envelope: T::Envelope) -> DrainIterator<T, SelectInEnvelopeFuncIntersecting<T>, Params>
    {
        let selection_function = SelectInEnvelopeFuncIntersecting::new(envelope);
        self.drain_with_selection_function(selection_function)
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: PointDistance,
{
    /// Returns a single object that covers a given point.
    ///
    /// Method [contains_point](PointDistance::contains_point)
    /// is used to determine if a tree element contains the given point.
    ///
    /// If multiple elements contain the given point, any of them is returned.
    pub fn locate_at_point(&self, point: &<T::Envelope as Envelope>::Point) -> Option<&T> {
        self.locate_all_at_point(point).next()
    }

    /// Mutable variant of [RTree::locate_at_point].
    pub fn locate_at_point_mut(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> Option<&mut T> {
        self.locate_all_at_point_mut(point).next()
    }

    /// Locate all elements containing a given point.
    ///
    /// Method [PointDistance::contains_point] is used
    /// to determine if a tree element contains the given point.
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// use rstar::primitives::Rectangle;
    ///
    /// let tree = RTree::bulk_load(vec![
    ///   Rectangle::from_corners([0.0, 0.0], [2.0, 2.0]),
    ///   Rectangle::from_corners([1.0, 1.0], [3.0, 3.0])
    /// ]);
    ///
    /// assert_eq!(tree.locate_all_at_point(&[1.5, 1.5]).count(), 2);
    /// assert_eq!(tree.locate_all_at_point(&[0.0, 0.0]).count(), 1);
    /// assert_eq!(tree.locate_all_at_point(&[-1., 0.0]).count(), 0);
    /// ```
    pub fn locate_all_at_point(
        &self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> LocateAllAtPoint<T> {
        LocateAllAtPoint::new(&self.root, SelectAtPointFunction::new(*point))
    }

    /// Mutable variant of [locate_all_at_point](#method.locate_all_at_point).
    pub fn locate_all_at_point_mut(
        &mut self,
        point: &<T::Envelope as Envelope>::Point,
    ) -> LocateAllAtPointMut<T> {
        LocateAllAtPointMut::new(&mut self.root, SelectAtPointFunction::new(*point))
    }

    /// Removes an element containing a given point.
    ///
    /// The removed element, if any, is returned. If multiple elements cover the given point,
    /// only one of them is removed and returned.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// use rstar::primitives::Rectangle;
    ///
    /// let mut tree = RTree::bulk_load(vec![
    ///   Rectangle::from_corners([0.0, 0.0], [2.0, 2.0]),
    ///   Rectangle::from_corners([1.0, 1.0], [3.0, 3.0])
    /// ]);
    ///
    /// assert!(tree.remove_at_point(&[1.5, 1.5]).is_some());
    /// assert!(tree.remove_at_point(&[1.5, 1.5]).is_some());
    /// assert!(tree.remove_at_point(&[1.5, 1.5]).is_none());
    ///```
    pub fn remove_at_point(&mut self, point: &<T::Envelope as Envelope>::Point) -> Option<T> {
        let removal_function = SelectAtPointFunction::new(*point);
        self.remove_with_selection_function(removal_function)
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: RTreeObject + PartialEq,
{
    /// Returns `true` if a given element is equal (`==`) to an element in the
    /// r-tree.
    ///
    /// This method will only work correctly if two equal elements also have the
    /// same envelope.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    ///
    /// let mut tree = RTree::new();
    /// assert!(!tree.contains(&[0.0, 2.0]));
    /// tree.insert([0.0, 2.0]);
    /// assert!(tree.contains(&[0.0, 2.0]));
    /// ```
    pub fn contains(&self, t: &T) -> bool {
        self.locate_in_envelope(&t.envelope()).any(|e| e == t)
    }

    /// Removes and returns an element of the r-tree equal (`==`) to a given element.
    ///
    /// If multiple elements equal to the given elements are contained in the tree, only
    /// one of them is removed and returned.
    ///
    /// This method will only work correctly if two equal elements also have the
    /// same envelope.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    ///
    /// let mut tree = RTree::new();
    /// tree.insert([0.0, 2.0]);
    /// // The element can be inserted twice just fine
    /// tree.insert([0.0, 2.0]);
    /// assert!(tree.remove(&[0.0, 2.0]).is_some());
    /// assert!(tree.remove(&[0.0, 2.0]).is_some());
    /// assert!(tree.remove(&[0.0, 2.0]).is_none());
    /// ```
    pub fn remove(&mut self, t: &T) -> Option<T> {
        let removal_function = SelectEqualsFunction::new(t);
        self.remove_with_selection_function(removal_function)
    }
}

impl<T, Params> RTree<T, Params>
where
    Params: RTreeParams,
    T: PointDistance,
{
    /// Returns the nearest neighbor for a given point.
    ///
    /// The distance is calculated by calling
    /// [PointDistance::distance_2]
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// let tree = RTree::bulk_load(vec![
    ///   [0.0, 0.0],
    ///   [0.0, 1.0],
    /// ]);
    /// assert_eq!(tree.nearest_neighbor(&[-1., 0.0]), Some(&[0.0, 0.0]));
    /// assert_eq!(tree.nearest_neighbor(&[0.0, 2.0]), Some(&[0.0, 1.0]));
    /// ```
    pub fn nearest_neighbor(&self, query_point: &<T::Envelope as Envelope>::Point) -> Option<&T> {
        if self.size > 0 {
            // The single-nearest-neighbor retrieval may in rare cases return None due to
            // rounding issues. The iterator will still work, though.
            nearest_neighbor::nearest_neighbor(&self.root, *query_point)
                .or_else(|| self.nearest_neighbor_iter(query_point).next())
        } else {
            None
        }
    }

    /// Returns the nearest neighbors for a given point.
    ///
    /// The distance is calculated by calling
    /// [PointDistance::distance_2]
    ///
    /// All returned values will have the exact same distance from the given query point.
    /// Returns an empty `Vec` if the tree is empty.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// let tree = RTree::bulk_load(vec![
    ///   [0.0, 0.0],
    ///   [0.0, 1.0],
    ///   [1.0, 0.0],
    /// ]);
    /// assert_eq!(tree.nearest_neighbors(&[1.0, 1.0]), &[&[0.0, 1.0], &[1.0, 0.0]]);
    /// assert_eq!(tree.nearest_neighbors(&[0.01, 0.01]), &[&[0.0, 0.0]]);
    /// ```
    pub fn nearest_neighbors(&self, query_point: &<T::Envelope as Envelope>::Point) -> Vec<&T> {
        nearest_neighbor::nearest_neighbors(&self.root, *query_point)
    }

    /// Returns all elements of the tree within a certain distance.
    ///
    /// The elements may be returned in any order. Each returned element
    /// will have a squared distance less or equal to the given squared distance.
    ///
    /// This method makes use of [PointDistance::distance_2_if_less_or_equal].
    /// If performance is critical and the distance calculation to the object is fast,
    /// overwriting this function may be beneficial.
    pub fn locate_within_distance(
        &self,
        query_point: <T::Envelope as Envelope>::Point,
        max_squared_radius: <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> LocateWithinDistanceIterator<T> {
        let selection_function = SelectWithinDistanceFunction::new(query_point, max_squared_radius);
        LocateWithinDistanceIterator::new(self.root(), selection_function)
    }

    /// Drain all elements of the tree within a certain distance.
    ///
    /// Similar to [`RTree::locate_within_distance`], but removes and
    /// returns the elements via an iterator.
    pub fn drain_within_distance(
        &mut self,
        query_point: <T::Envelope as Envelope>::Point,
        max_squared_radius: <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> DrainIterator<T, SelectWithinDistanceFunction<T>, Params> {
        let selection_function = SelectWithinDistanceFunction::new(query_point, max_squared_radius);
        self.drain_with_selection_function(selection_function)
    }

    /// Returns all elements of the tree sorted by their distance to a given point.
    ///
    /// # Runtime
    /// Every `next()` call runs in `O(log(n))`. Creating the iterator runs in
    /// `O(log(n))`.
    /// The [r-tree documentation](RTree) contains more information about
    /// r-tree performance.
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// let tree = RTree::bulk_load(vec![
    ///   [0.0, 0.0],
    ///   [0.0, 1.0],
    /// ]);
    ///
    /// let nearest_neighbors = tree.nearest_neighbor_iter(&[0.5, 0.0]).collect::<Vec<_>>();
    /// assert_eq!(nearest_neighbors, vec![&[0.0, 0.0], &[0.0, 1.0]]);
    /// ```
    pub fn nearest_neighbor_iter(
        &self,
        query_point: &<T::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = &T> {
        nearest_neighbor::NearestNeighborIterator::new(&self.root, *query_point)
    }

    /// Returns `(element, distance^2)` tuples of the tree sorted by their distance to a given point.
    ///
    /// The distance is calculated by calling
    /// [PointDistance::distance_2].
    #[deprecated(note = "Please use nearest_neighbor_iter_with_distance_2 instead")]
    pub fn nearest_neighbor_iter_with_distance(
        &self,
        query_point: &<T::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = (&T, <<T::Envelope as Envelope>::Point as Point>::Scalar)> {
        nearest_neighbor::NearestNeighborDistance2Iterator::new(&self.root, *query_point)
    }

    /// Returns `(element, distance^2)` tuples of the tree sorted by their distance to a given point.
    ///
    /// The distance is calculated by calling
    /// [PointDistance::distance_2].
    pub fn nearest_neighbor_iter_with_distance_2(
        &self,
        query_point: &<T::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = (&T, <<T::Envelope as Envelope>::Point as Point>::Scalar)> {
        nearest_neighbor::NearestNeighborDistance2Iterator::new(&self.root, *query_point)
    }

    /// Removes the nearest neighbor for a given point and returns it.
    ///
    /// The distance is calculated by calling
    /// [PointDistance::distance_2].
    ///
    /// # Example
    /// ```
    /// use rstar::RTree;
    /// let mut tree = RTree::bulk_load(vec![
    ///   [0.0, 0.0],
    ///   [0.0, 1.0],
    /// ]);
    /// assert_eq!(tree.pop_nearest_neighbor(&[0.0, 0.0]), Some([0.0, 0.0]));
    /// assert_eq!(tree.pop_nearest_neighbor(&[0.0, 0.0]), Some([0.0, 1.0]));
    /// assert_eq!(tree.pop_nearest_neighbor(&[0.0, 0.0]), None);
    /// ```
    pub fn pop_nearest_neighbor(
        &mut self,
        query_point: &<T::Envelope as Envelope>::Point,
    ) -> Option<T> {
        if let Some(neighbor) = self.nearest_neighbor(query_point) {
            let removal_function = SelectByAddressFunction::new(neighbor.envelope(), neighbor);
            self.remove_with_selection_function(removal_function)
        } else {
            None
        }
    }
}

impl<T, Params> RTree<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    /// Inserts a new element into the r-tree.
    ///
    /// If the element is already present in the tree, it will now be present twice.
    ///
    /// # Runtime
    /// This method runs in `O(log(n))`.
    /// The [r-tree documentation](RTree) contains more information about
    /// r-tree performance.
    pub fn insert(&mut self, t: T) {
        Params::DefaultInsertionStrategy::insert(self, t);
        self.size += 1;
    }
}

impl<T, Params> RTree<T, Params>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
}

impl<'a, T, Params> IntoIterator for &'a RTree<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type IntoIter = RTreeIterator<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, Params> IntoIterator for &'a mut RTree<T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type IntoIter = RTreeIteratorMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod test {
    use super::RTree;
    use crate::algorithm::rstar::RStarInsertionStrategy;
    use crate::params::RTreeParams;
    use crate::test_utilities::{create_random_points, SEED_1};
    use crate::DefaultParams;

    struct TestParams;
    impl RTreeParams for TestParams {
        const MIN_SIZE: usize = 10;
        const MAX_SIZE: usize = 20;
        const REINSERTION_COUNT: usize = 1;
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
        let points = create_random_points(NUM_POINTS, SEED_1);
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
            tree.root.sanity_check::<DefaultParams>(true);
        }
        assert_eq!(tree.size(), NUM_POINTS);
        for p in &points {
            assert!(tree.contains(p));
        }
    }

    #[test]
    fn test_fmt_debug() {
        let tree = RTree::bulk_load(vec![[0, 1], [0, 1]]);
        let debug: String = format!("{:?}", tree);
        assert_eq!(debug, "RTree { size: 2, items: {[0, 1], [0, 1]} }");
    }

    #[test]
    fn test_default() {
        let tree: RTree<[f32; 2]> = Default::default();
        assert_eq!(tree.size(), 0);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialization() {
        use crate::test_utilities::create_random_integers;

        use serde_json;
        const SIZE: usize = 20;
        let points = create_random_integers::<[i32; 2]>(SIZE, SEED_1);
        let tree = RTree::bulk_load(points.clone());
        let json = serde_json::to_string(&tree).expect("Serializing tree failed");
        let parsed: RTree<[i32; 2]> =
            serde_json::from_str(&json).expect("Deserializing tree failed");
        assert_eq!(parsed.size(), SIZE);
        for point in &points {
            assert!(parsed.contains(point));
        }
    }

    #[test]
    fn test_bulk_load_crash() {
        let bulk_nodes = vec![
            [570.0, 1080.0, 89.0],
            [30.0, 1080.0, 627.0],
            [1916.0, 1080.0, 68.0],
            [274.0, 1080.0, 790.0],
            [476.0, 1080.0, 895.0],
            [1557.0, 1080.0, 250.0],
            [1546.0, 1080.0, 883.0],
            [1512.0, 1080.0, 610.0],
            [1729.0, 1080.0, 358.0],
            [1841.0, 1080.0, 434.0],
            [1752.0, 1080.0, 696.0],
            [1674.0, 1080.0, 705.0],
            [136.0, 1080.0, 22.0],
            [1593.0, 1080.0, 71.0],
            [586.0, 1080.0, 272.0],
            [348.0, 1080.0, 373.0],
            [502.0, 1080.0, 2.0],
            [1488.0, 1080.0, 1072.0],
            [31.0, 1080.0, 526.0],
            [1695.0, 1080.0, 559.0],
            [1663.0, 1080.0, 298.0],
            [316.0, 1080.0, 417.0],
            [1348.0, 1080.0, 731.0],
            [784.0, 1080.0, 126.0],
            [225.0, 1080.0, 847.0],
            [79.0, 1080.0, 819.0],
            [320.0, 1080.0, 504.0],
            [1714.0, 1080.0, 1026.0],
            [264.0, 1080.0, 229.0],
            [108.0, 1080.0, 158.0],
            [1665.0, 1080.0, 604.0],
            [496.0, 1080.0, 231.0],
            [1813.0, 1080.0, 865.0],
            [1200.0, 1080.0, 326.0],
            [1661.0, 1080.0, 818.0],
            [135.0, 1080.0, 229.0],
            [424.0, 1080.0, 1016.0],
            [1708.0, 1080.0, 791.0],
            [1626.0, 1080.0, 682.0],
            [442.0, 1080.0, 895.0],
        ];

        let nodes = vec![
            [1916.0, 1060.0, 68.0],
            [1664.0, 1060.0, 298.0],
            [1594.0, 1060.0, 71.0],
            [225.0, 1060.0, 846.0],
            [1841.0, 1060.0, 434.0],
            [502.0, 1060.0, 2.0],
            [1625.5852, 1060.0122, 682.0],
            [1348.5273, 1060.0029, 731.08124],
            [316.36127, 1060.0298, 418.24515],
            [1729.3253, 1060.0023, 358.50134],
        ];
        let mut tree = RTree::bulk_load(bulk_nodes);
        for node in nodes {
            // Bulk loading will create nodes larger than Params::MAX_SIZE,
            // which is intentional and not harmful.
            tree.insert(node);
            tree.root().sanity_check::<DefaultParams>(false);
        }
    }
}
