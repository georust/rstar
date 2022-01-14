use crate::{Point, RTreeObject};

/// An envelope type that encompasses some child nodes.
///
/// An envelope defines how different bounding boxes of inserted children in an r-tree can interact,
/// e.g. how they can be merged or intersected.
/// This trait is not meant to be implemented by the user. Currently, only one implementation
/// exists ([crate::AABB]) and should be used.
pub trait Envelope: Clone + Copy + PartialEq + ::core::fmt::Debug {
    /// The envelope's point type.
    type Point: Point;

    /// Creates a new, empty envelope that does not encompass any child.
    fn new_empty() -> Self;

    /// Returns true if a point is contained within this envelope.
    fn contains_point(&self, point: &Self::Point) -> bool;

    /// Returns true if another envelope is _fully contained_ within `self`.
    fn contains_envelope(&self, aabb: &Self) -> bool;

    /// Extends `self` to contain another envelope.
    fn merge(&mut self, other: &Self);
    /// Returns the minimal envelope containing `self` and another envelope.
    fn merged(&self, other: &Self) -> Self;

    /// Sets `self` to the intersection of `self` and another envelope.
    fn intersects(&self, other: &Self) -> bool;
    /// Returns the area of the intersection of `self` and another envelope.
    fn intersection_area(&self, other: &Self) -> <Self::Point as Point>::Scalar;

    /// Returns this envelope's area. Must be at least 0.
    fn area(&self) -> <Self::Point as Point>::Scalar;

    /// Returns the euclidean distance to the envelope's border.
    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;

    /// Returns the squared min-max distance, a concept that helps to find nearest neighbors efficiently.
    ///
    /// Visually, if an AABB and a point are given, the min-max distance returns the distance at which we
    /// can be assured that an element must be present. This serves as an upper bound during nearest neighbor search.
    ///
    /// # References
    /// [Roussopoulos, Nick, Stephen Kelley, and Frédéric Vincent. "Nearest neighbor queries." ACM sigmod record. Vol. 24. No. 2. ACM, 1995.](https://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.133.2288)
    fn min_max_dist_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;

    /// Returns the envelope's center point.
    fn center(&self) -> Self::Point;

    /// Returns a value proportional to the envelope's perimeter.
    fn perimeter_value(&self) -> <Self::Point as Point>::Scalar;

    /// Sorts a given set of objects with envelopes along one of their axes.
    fn sort_envelopes<T: RTreeObject<Envelope = Self>>(axis: usize, envelopes: &mut [T]);

    /// Partitions objects with an envelope along a certain axis.
    ///
    /// After calling this, envelopes[0..selection_size] are all smaller
    /// than envelopes[selection_size + 1..].
    fn partition_envelopes<T: RTreeObject<Envelope = Self>>(
        axis: usize,
        envelopes: &mut [T],
        selection_size: usize,
    );
}
