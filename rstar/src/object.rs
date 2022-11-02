use crate::aabb::AABB;
use crate::envelope::Envelope;
use crate::point::{Point, PointExt};

/// An object that can be inserted into an r-tree.
///
/// This trait must be implemented for any object to be inserted into an r-tree.
/// Some simple objects that already implement this trait can be found in the
/// [crate::primitives] module.
///
/// The only property required of such an object is its [crate::Envelope].
/// Most simply, this method should return the [axis aligned bounding box](AABB)
/// of the object. Other envelope types may be supported in the future.
///
/// *Note*: It is a logic error if an object's envelope changes after insertion into
/// an r-tree.
///
/// # Type parameters
/// `Envelope`: The object's envelope type. At the moment, only [AABB] is
/// available.
///
/// # Example implementation
/// ```
/// use rstar::{RTreeObject, AABB};
///
/// struct Player
/// {
///     name: String,
///     x_coordinate: f64,
///     y_coordinate: f64
/// }
///
/// impl RTreeObject for Player
/// {
///     type Envelope = AABB<[f64; 2]>;
///
///     fn envelope(&self) -> Self::Envelope
///     {
///         AABB::from_point([self.x_coordinate, self.y_coordinate])
///     }
/// }
///
/// use rstar::RTree;
///
/// let mut tree = RTree::new();
///
/// // Insert a few players...
/// tree.insert(Player {
///     name: "Forlorn Freeman".into(),
///     x_coordinate: 1.,
///     y_coordinate: 0.
/// });
/// tree.insert(Player {
///     name: "Sarah Croft".into(),
///     x_coordinate: 0.5,
///     y_coordinate: 0.5,
/// });
/// tree.insert(Player {
///     name: "Geralt of Trivia".into(),
///     x_coordinate: 0.,
///     y_coordinate: 2.,
/// });
///
/// // Now we are ready to ask some questions!
/// let envelope = AABB::from_point([0.5, 0.5]);
/// let likely_sarah_croft = tree.locate_in_envelope(&envelope).next();
/// println!("Found {:?} lurking around at (0.5, 0.5)!", likely_sarah_croft.unwrap().name);
/// # assert!(likely_sarah_croft.is_some());
///
/// let unit_square = AABB::from_corners([-1.0, -1.0], [1., 1.]);
/// for player in tree.locate_in_envelope(&unit_square) {
///    println!("And here is {:?} spelunking in the unit square.", player.name);
/// }
/// # assert_eq!(tree.locate_in_envelope(&unit_square).count(), 2);
/// ```
pub trait RTreeObject {
    /// The object's envelope type. Usually, [AABB] will be the right choice.
    /// This type also defines the object's dimensionality.
    type Envelope: Envelope;

    /// Returns the object's envelope.
    ///
    /// Usually, this will return the object's [axis aligned bounding box](AABB).
    fn envelope(&self) -> Self::Envelope;
}

/// Defines objects which can calculate their minimal distance to a point.
///
/// This trait is most notably necessary for support of [nearest_neighbor](struct.RTree#method.nearest_neighbor)
/// queries.
///
/// # Example
/// ```
/// use rstar::{RTreeObject, PointDistance, AABB};
///
/// struct Circle
/// {
///     origin: [f32; 2],
///     radius: f32,
/// }
///
/// impl RTreeObject for Circle {
///     type Envelope = AABB<[f32; 2]>;
///
///     fn envelope(&self) -> Self::Envelope {
///         let corner_1 = [self.origin[0] - self.radius, self.origin[1] - self.radius];
///         let corner_2 = [self.origin[0] + self.radius, self.origin[1] + self.radius];
///         AABB::from_corners(corner_1, corner_2)
///     }
/// }
///
/// impl PointDistance for Circle
/// {
///     fn distance_2(&self, point: &[f32; 2]) -> f32
///     {
///         let d_x = self.origin[0] - point[0];
///         let d_y = self.origin[1] - point[1];
///         let distance_to_origin = (d_x * d_x + d_y * d_y).sqrt();
///         let distance_to_ring = distance_to_origin - self.radius;
///         let distance_to_circle = f32::max(0.0, distance_to_ring);
///         // We must return the squared distance!
///         distance_to_circle * distance_to_circle
///     }
///
///     // This implementation is not required but more efficient since it
///     // omits the calculation of a square root
///     fn contains_point(&self, point: &[f32; 2]) -> bool
///     {
///         let d_x = self.origin[0] - point[0];
///         let d_y = self.origin[1] - point[1];
///         let distance_to_origin_2 = (d_x * d_x + d_y * d_y);
///         let radius_2 = self.radius * self.radius;
///         distance_to_origin_2 <= radius_2
///     }
/// }
///
///
/// let circle = Circle {
///     origin: [1.0, 0.0],
///     radius: 1.0,
/// };
///
/// assert_eq!(circle.distance_2(&[-1.0, 0.0]), 1.0);
/// assert_eq!(circle.distance_2(&[-2.0, 0.0]), 4.0);
/// assert!(circle.contains_point(&[1.0, 0.0]));
/// ```
pub trait PointDistance: RTreeObject {
    /// Returns the squared euclidean distance between an object to a point.
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar;

    /// Returns `true` if a point is contained within this object.
    ///
    /// By default, any point returning a `distance_2` less than or equal to zero is considered to be
    /// contained within `self`. Changing this default behavior is advised if calculating the squared distance
    /// is more computationally expensive than a point containment check.
    fn contains_point(&self, point: &<Self::Envelope as Envelope>::Point) -> bool {
        self.distance_2(point) <= num_traits::zero()
    }

    /// Returns the squared distance to this object, or `None` if the distance
    /// is larger than a given maximum value.
    ///
    /// Some algorithms only need to know an object's distance
    /// if it is less than or equal to a maximum value. In these cases, it may be
    /// faster to calculate a lower bound of the distance first and returning
    /// early if the object cannot be closer than the given maximum.
    ///
    /// The provided default implementation will use the distance to the object's
    /// envelope as a lower bound.
    ///
    /// If performance is critical and the object's distance calculation is fast,
    /// it may be beneficial to overwrite this implementation.
    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: <<Self::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<<<Self::Envelope as Envelope>::Point as Point>::Scalar> {
        let envelope_distance = self.envelope().distance_2(point);
        if envelope_distance <= max_distance_2 {
            let distance_2 = self.distance_2(point);
            if distance_2 <= max_distance_2 {
                return Some(distance_2);
            }
        }
        None
    }
}

impl<P> RTreeObject for P
where
    P: Point,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> AABB<P> {
        AABB::from_point(self.clone())
    }
}

impl<P> PointDistance for P
where
    P: Point,
{
    fn distance_2(&self, point: &P) -> P::Scalar {
        <Self as PointExt>::distance_2(self, point)
    }

    fn contains_point(&self, point: &<Self::Envelope as Envelope>::Point) -> bool {
        self == point
    }

    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: <<Self::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<P::Scalar> {
        let distance_2 = <Self as PointExt>::distance_2(self, point);
        if distance_2 <= max_distance_2 {
            Some(distance_2)
        } else {
            None
        }
    }
}
