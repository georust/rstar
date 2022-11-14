use crate::aabb::AABB;
use crate::envelope::Envelope;
use crate::object::PointDistance;
use crate::object::RTreeObject;
use crate::point::{Point, PointExt};
use num_traits::{One, Zero};

/// A line defined by a start and and end point.
///
/// This struct can be inserted directly into an r-tree.
/// # Type parameters
/// `P`: The line's [Point] type.
///
/// # Example
/// ```
/// use rstar::primitives::Line;
/// use rstar::{RTree, RTreeObject};
///
/// let line_1 = Line::new([0.0, 0.0], [1.0, 1.0]);
/// let line_2 = Line::new([0.0, 0.0], [-1.0, 1.0]);
/// let tree = RTree::bulk_load(vec![line_1, line_2]);
///
/// assert!(tree.contains(&line_1));
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Line<P>
where
    P: Point,
{
    /// The line's start point
    pub from: P,
    /// The line's end point.
    pub to: P,
}

impl<P> Line<P>
where
    P: Point,
{
    /// Creates a new line between two points.
    pub fn new(from: P, to: P) -> Self {
        Line { from, to }
    }
}

impl<P> RTreeObject for Line<P>
where
    P: Point,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.from.clone(), self.to.clone())
    }
}

impl<P> Line<P>
where
    P: Point,
{
    /// Returns the squared length of this line.
    ///
    /// # Example
    /// ```
    /// use rstar::primitives::Line;
    ///
    /// let line = Line::new([3, 3], [7, 6]);
    /// assert_eq!(line.length_2(), 25);
    /// ```
    pub fn length_2(&self) -> P::Scalar {
        self.from.sub(&self.to).length_2()
    }

    fn project_point(&self, query_point: &P) -> P::Scalar {
        let (ref p1, ref p2) = (self.from.clone(), self.to.clone());
        let dir = p2.sub(p1);
        query_point.sub(p1).dot(&dir) / dir.length_2()
    }

    /// Returns the nearest point on this line relative to a given point.
    ///
    /// # Example
    /// ```
    /// use rstar::primitives::Line;
    ///
    /// let line = Line::new([0.0, 0.0], [1., 1.]);
    /// assert_eq!(line.nearest_point(&[0.0, 0.0]), [0.0, 0.0]);
    /// assert_eq!(line.nearest_point(&[1.0, 0.0]), [0.5, 0.5]);
    /// assert_eq!(line.nearest_point(&[10., 12.]), [1.0, 1.0]);
    /// ```
    pub fn nearest_point(&self, query_point: &P) -> P {
        let (p1, p2) = (self.from.clone(), self.to.clone());
        let dir = p2.sub(&p1);
        let s = self.project_point(query_point);
        if P::Scalar::zero() < s && s < One::one() {
            p1.add(&dir.mul(s))
        } else if s <= P::Scalar::zero() {
            p1
        } else {
            p2
        }
    }
}

impl<P> PointDistance for Line<P>
where
    P: Point,
{
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        self.nearest_point(point).sub(point).length_2()
    }
}

#[cfg(test)]
mod test {
    use super::Line;
    use crate::object::PointDistance;
    use approx::*;

    #[test]
    fn edge_distance() {
        let edge = Line::new([0.5, 0.5], [0.5, 2.0]);

        assert_abs_diff_eq!(edge.distance_2(&[0.5, 0.5]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.5]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.5, 1.0]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.0]), 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 3.0]), 0.5 * 0.5 + 1.0);
    }

    #[test]
    fn length_2() {
        let line = Line::new([1, -1], [5, 5]);
        assert_eq!(line.length_2(), 16 + 36);
    }
}
