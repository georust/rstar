use crate::aabb::AABB;
use crate::envelope::Envelope;
use num_traits::{One, Zero};
use crate::object::PointDistance;
use crate::object::RTreeObject;
use crate::point::{EuclideanPoint, Point, PointExt};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimpleEdge<P>
where
    P: EuclideanPoint,
{
    from: P,
    to: P,
}

impl <P> SimpleEdge<P> where P: EuclideanPoint {
    pub fn new(from: P, to: P) -> Self {
        SimpleEdge {
            from, to
        }
    }
}

impl<P> RTreeObject for SimpleEdge<P>
where
    P: EuclideanPoint,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(&self.from, &self.to)
    }
}

impl<P> SimpleEdge<P>
where
    P: EuclideanPoint,
{
    fn project_point(&self, query_point: &P) -> P::Scalar {
        let (ref p1, ref p2) = (self.from, self.to);
        let dir = p2.sub(p1);
        query_point.sub(p1).dot(&dir) / dir.length_2()
    }

    pub fn nearest_point(&self, query_point: &P) -> P {
        let (p1, p2) = (self.from, self.to);
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

impl<P> PointDistance for SimpleEdge<P>
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
    use super::SimpleEdge;
    use crate::object::PointDistance;

    #[test]
    fn edge_distance() {
        let edge = SimpleEdge::new([0.5, 0.5], [0.5, 2.0]);

        assert_abs_diff_eq!(edge.distance_2(&[0.5, 0.5]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.5]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.5, 1.0]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.0]), 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 3.0]), 0.5 * 0.5 + 1.0);
    }
}