use crate::structures::aabb::AABB;
use crate::envelope::Envelope;
use crate::point::{Point, PointExt};

pub trait RTreeObject {
    type Envelope: Envelope;

    fn envelope(&self) -> Self::Envelope;
}

pub trait PointDistance: RTreeObject {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar;

    fn contains_point(&self, point: &<Self::Envelope as Envelope>::Point) -> bool {
        self.distance_2(point) <= num_traits::zero()
    }
}

impl<P> RTreeObject for P
where
    P: Point,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> AABB<P> {
        AABB::from_point(*self)
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
}
