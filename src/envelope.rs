use point::Point;

pub trait Envelope {
    type PointType: Point;

    fn contains_point(&self, point: &Self::PointType) -> bool;
    fn contains_envelope(&self, mbr: &Self) -> bool;

    fn merge(&mut self, other: &Self);
    fn intersects(&self, other: &Self) -> bool;

    fn area(&self) -> <Self::PointType as Point>::Scalar;




}