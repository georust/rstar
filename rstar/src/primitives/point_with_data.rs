use crate::{Point, PointDistance, RTreeObject, AABB};

/// A point with some associated data that can be inserted into an r-tree.
///
/// Often, adding metadata (like a database index) to a point is required before adding them
/// into an r-tree. This struct removes some of the boilerplate required to do so.
///
/// # Example
/// ```
/// use rstar::{RTree, PointDistance};
/// use rstar::primitives::PointWithData;
///
/// type RestaurantLocation = PointWithData<&'static str, [f64; 2]>;
///
/// let mut restaurants = RTree::new();
/// restaurants.insert(RestaurantLocation::new("Pete's Pizza Place", [0.3, 0.2]));
/// restaurants.insert(RestaurantLocation::new("The Great Steak", [-0.8, 0.0]));
/// restaurants.insert(RestaurantLocation::new("Fishy Fortune", [0.2, -0.2]));
///
/// let my_location = [0.0, 0.0];
///
/// // Now find the closest restaurant!
/// let place = restaurants.nearest_neighbor(&my_location).unwrap();
/// println!("Let's go to {}", place.data);
/// println!("It's really close, only {} miles", place.distance_2(&my_location))
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointWithData<T, P> {
    /// Any data associated with a point.
    pub data: T,
    point: P, // Private to prevent modification.
}

impl<T, P> PointWithData<T, P> {
    /// Creates a new `PointWithData` with the provided data.
    pub fn new(data: T, point: P) -> Self {
        PointWithData { data, point }
    }

    /// Returns this point's position.
    pub fn position(&self) -> &P {
        &self.point
    }
}

impl<T, P> RTreeObject for PointWithData<T, P>
where
    P: Point,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        self.point.envelope()
    }
}

impl<T, P> PointDistance for PointWithData<T, P>
where
    P: Point,
{
    fn distance_2(&self, point: &P) -> <P as Point>::Scalar {
        self.point.distance_2(point)
    }

    fn contains_point(&self, point: &P) -> bool {
        self.point.contains_point(point)
    }
}
