use point::{Point, PointExt, EuclideanPoint};
use envelope::Envelope;
use aabb::AABB;

pub trait RTreeObject {
    type Envelope: Envelope;

    fn envelope(&self) -> Self::Envelope;
}

pub trait PointDistance {
    type Point: Point;

    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;
}

impl<P> RTreeObject for P
where
    P: EuclideanPoint,
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
    type Point = P;

    fn distance_2(&self, point: &P) -> P::Scalar {
        <Self as PointExt>::distance_2(self, point)
    }
}
