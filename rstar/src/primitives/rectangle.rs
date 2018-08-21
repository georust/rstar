use aabb::AABB;
use envelope::Envelope;
use object::{RTreeObject, PointDistance};
use point::{EuclideanPoint, Point, PointExt};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimpleRectangle<P>
where
    P: EuclideanPoint,
{
    aabb: AABB<P>
}

impl <P> SimpleRectangle<P> where P: EuclideanPoint {
    pub fn new(from: P, to: P) -> Self {
        SimpleRectangle {
            aabb: AABB::from_corners(&from, &to)
        }
    }
}

impl<P> RTreeObject for SimpleRectangle<P>
where
    P: EuclideanPoint,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

impl<P> SimpleRectangle<P>
where
    P: EuclideanPoint,
{
    pub fn nearest_point(&self, query_point: &P) -> P {
        self.aabb.min_point(query_point)
    }
}

impl<P> PointDistance for SimpleRectangle<P>
where
    P: EuclideanPoint,
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
    use super::SimpleRectangle;
    use object::PointDistance;

    #[test]
    fn rectangle_distance() {
        let rectangle = SimpleRectangle::new([0.5, 0.5], [1.0, 2.0]);

        assert_abs_diff_eq!(rectangle.distance_2(&[0.5, 0.5]), 0.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 0.5]), 0.5 * 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.5, 1.0]), 0.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 0.0]), 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[0.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(rectangle.distance_2(&[1.0, 3.0]), 1.0);
        assert_abs_diff_eq!(rectangle.distance_2(&[1.0, 1.0]), 0.0);
    }
}