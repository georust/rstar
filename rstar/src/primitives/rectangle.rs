use crate::aabb::AABB;
use crate::envelope::Envelope;
use crate::object::{PointDistance, RTreeObject};
use crate::point::{Point, PointExt};

/// An n-dimensional rectangle defined by its two corners.
///
/// This rectangle can be directly inserted into an r-tree.
///
/// *Note*: Despite being called rectangle, this struct can be used
/// with more than two dimensions by using an appropriate point type.
///
/// # Type parameters
/// `P`: The rectangle's [Point] type.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rectangle<P>
where
    P: Point,
{
    aabb: AABB<P>,
}

impl<P> Rectangle<P>
where
    P: Point,
{
    /// Creates a new rectangle defined by two corners.
    pub fn from_corners(corner_1: P, corner_2: P) -> Self {
        AABB::from_corners(corner_1, corner_2).into()
    }

    /// Creates a new rectangle defined by it's [axis aligned bounding box(AABB).
    pub fn from_aabb(aabb: AABB<P>) -> Self {
        Rectangle { aabb }
    }

    /// Returns the rectangle's lower corner.
    ///
    /// This is the point contained within the rectangle with the smallest coordinate value in each
    /// dimension.
    pub fn lower(&self) -> P {
        self.aabb.lower()
    }

    /// Returns the rectangle's upper corner.
    ///
    /// This is the point contained within the AABB with the largest coordinate value in each
    /// dimension.
    pub fn upper(&self) -> P {
        self.aabb.upper()
    }
}

impl<P> From<AABB<P>> for Rectangle<P>
where
    P: Point,
{
    fn from(aabb: AABB<P>) -> Self {
        Self::from_aabb(aabb)
    }
}

impl<P> RTreeObject for Rectangle<P>
where
    P: Point,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb.clone()
    }
}

impl<P> Rectangle<P>
where
    P: Point,
{
    /// Returns the nearest point within this rectangle to a given point.
    ///
    /// If `query_point` is contained within this rectangle, `query_point` is returned.
    pub fn nearest_point(&self, query_point: &P) -> P {
        self.aabb.min_point(query_point)
    }
}

impl<P> PointDistance for Rectangle<P>
where
    P: Point,
{
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        self.nearest_point(point).sub(point).length_2()
    }

    fn contains_point(&self, point: &<Self::Envelope as Envelope>::Point) -> bool {
        self.aabb.contains_point(point)
    }

    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: <<Self::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<<<Self::Envelope as Envelope>::Point as Point>::Scalar> {
        let distance_2 = self.distance_2(point);
        if distance_2 <= max_distance_2 {
            Some(distance_2)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::Rectangle;
    use crate::object::PointDistance;
    use approx::*;

    #[test]
    fn rectangle_distance() {
        let rectangle = Rectangle::from_corners([0.5, 0.5], [1.0, 2.0]);

        assert_abs_diff_eq!(rectangle.distance_2(&[0.5, 0.5]), 0.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 0.5]), 0.5 * 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.5, 1.0]), 0.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 0.0]), 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[1.0, 3.0]), 1.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[1.0, 1.0]), 0.0);
    }
}
