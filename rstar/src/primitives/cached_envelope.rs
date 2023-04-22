use crate::envelope::Envelope;
use crate::object::PointDistance;
use crate::{object::RTreeObject, point::Point};
use core::ops::Deref;

/// An [RTreeObject] with an inner geometry whose envelope is cached to improve efficiency.
///
/// For complex geometry like polygons, computing the envelope can become a bottleneck during
/// tree construction and querying. Hence this combinator computes it once during creation,
/// stores it and then returns a copy.
///
/// **Note:** the container itself implements [RTreeObject] and inner geometry `T` can be
/// accessed via an implementation of `Deref<Target=T>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CachedEnvelope<T: RTreeObject> {
    inner: T,
    cached_env: T::Envelope,
}

impl<T: RTreeObject> RTreeObject for CachedEnvelope<T>
where
    T::Envelope: Clone,
{
    type Envelope = T::Envelope;

    fn envelope(&self) -> Self::Envelope {
        self.cached_env.clone()
    }
}

impl<T: PointDistance> PointDistance for CachedEnvelope<T> {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        self.inner.distance_2(point)
    }

    fn contains_point(&self, p: &<Self::Envelope as Envelope>::Point) -> bool {
        self.inner.contains_point(p)
    }

    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: <<Self::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<<<Self::Envelope as Envelope>::Point as Point>::Scalar> {
        self.inner
            .distance_2_if_less_or_equal(point, max_distance_2)
    }
}

impl<T: RTreeObject> CachedEnvelope<T> {
    /// Create a new [CachedEnvelope] struct using the provided geometry.
    pub fn new(inner: T) -> Self {
        let cached_env = inner.envelope();

        Self { inner, cached_env }
    }
}

impl<T: RTreeObject> Deref for CachedEnvelope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod test {
    use super::CachedEnvelope;
    use crate::object::PointDistance;
    use crate::primitives::GeomWithData;

    use approx::*;

    use crate::{primitives::Line, RTree};

    #[test]
    fn container_in_rtree() {
        let line_1 = CachedEnvelope::new(Line::new([0.0, 0.0], [1.0, 1.0]));
        let line_2 = CachedEnvelope::new(Line::new([0.0, 0.0], [-1.0, 1.0]));
        let tree = RTree::bulk_load(vec![line_1, line_2]);

        assert!(tree.contains(&line_1));
    }

    #[test]
    fn container_edge_distance() {
        let edge = CachedEnvelope::new(Line::new([0.5, 0.5], [0.5, 2.0]));

        assert_abs_diff_eq!(edge.distance_2(&[0.5, 0.5]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.5]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.5, 1.0]), 0.0);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 0.0]), 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[0.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 1.0]), 0.5 * 0.5);
        assert_abs_diff_eq!(edge.distance_2(&[1.0, 3.0]), 0.5 * 0.5 + 1.0);
    }

    #[test]
    fn container_length_2() {
        let line = CachedEnvelope::new(Line::new([1, -1], [5, 5]));

        assert_eq!(line.length_2(), 16 + 36);
    }

    #[test]
    fn container_nearest_neighbour() {
        let mut lines = RTree::new();
        lines.insert(GeomWithData::new(
            CachedEnvelope::new(Line::new([0.0, 0.0], [1.0, 1.0])),
            "Line A",
        ));
        lines.insert(GeomWithData::new(
            CachedEnvelope::new(Line::new([0.0, 0.0], [-1.0, 1.0])),
            "Line B",
        ));
        let my_location = [0.0, 0.0];
        // Now find the closest line
        let place = lines.nearest_neighbor(&my_location).unwrap();

        assert_eq!(place.data, "Line A");
    }
}
