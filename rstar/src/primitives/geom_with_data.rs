use crate::envelope::Envelope;
use crate::object::PointDistance;
use crate::{object::RTreeObject, point::Point};

/// An [RTreeObject] with a geometry and some associated data that can be inserted into an r-tree.
///
/// Often, adding metadata (like a database ID) to a geometry is required before adding it
/// into an r-tree. This struct removes some of the boilerplate required to do so.
///
/// **Note:** while the container itself implements [RTreeObject], you will have to go through its
/// [`geom`][Self::geom] method in order to access geometry-specific methods.
///
/// # Example
/// ```
/// use rstar::{RTree, PointDistance};
/// use rstar::primitives::GeomWithData;
///
/// type RestaurantLocation = GeomWithData<[f64; 2], &'static str>;
///
/// let mut restaurants = RTree::new();
/// restaurants.insert(RestaurantLocation::new([0.3, 0.2], "Pete's Pizza Place"));
/// restaurants.insert(RestaurantLocation::new([-0.8, 0.0], "The Great Steak"));
/// restaurants.insert(RestaurantLocation::new([0.2, -0.2], "Fishy Fortune"));
///
/// let my_location = [0.0, 0.0];
///
/// // Now find the closest restaurant!
/// let place = restaurants.nearest_neighbor(&my_location).unwrap();
/// println!("Let's go to {}", place.data);
/// println!("It's really close, only {} miles", place.distance_2(&my_location));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeomWithData<R: RTreeObject, T> {
    geom: R,
    /// Data to be associated with the geometry being stored in the [`RTree`](crate::RTree).
    pub data: T,
}

impl<R: RTreeObject, T> RTreeObject for GeomWithData<R, T> {
    type Envelope = R::Envelope;

    fn envelope(&self) -> Self::Envelope {
        self.geom.envelope()
    }
}

impl<R: PointDistance, T> PointDistance for GeomWithData<R, T> {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        self.geom.distance_2(point)
    }

    fn contains_point(&self, p: &<Self::Envelope as Envelope>::Point) -> bool {
        self.geom.contains_point(p)
    }

    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: <<Self::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Option<<<Self::Envelope as Envelope>::Point as Point>::Scalar> {
        self.geom.distance_2_if_less_or_equal(point, max_distance_2)
    }
}

impl<R: RTreeObject, T> GeomWithData<R, T> {
    /// Create a new [GeomWithData] struct using the provided geometry and data.
    pub fn new(geom: R, data: T) -> Self {
        Self { geom, data }
    }

    /// Get a reference to the container's geometry.
    pub fn geom(&self) -> &R {
        &self.geom
    }
}

#[cfg(test)]
mod test {
    use super::GeomWithData;
    use crate::object::PointDistance;

    use approx::*;

    use crate::{primitives::Line, RTree};

    #[test]
    fn container_in_rtree() {
        let line_1 = GeomWithData::new(Line::new([0.0, 0.0], [1.0, 1.0]), ());
        let line_2 = GeomWithData::new(Line::new([0.0, 0.0], [-1.0, 1.0]), ());
        let tree = RTree::bulk_load(vec![line_1, line_2]);

        assert!(tree.contains(&line_1));
    }

    #[test]
    fn container_edge_distance() {
        let edge = GeomWithData::new(Line::new([0.5, 0.5], [0.5, 2.0]), 1usize);

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
        let line = GeomWithData::new(Line::new([1, -1], [5, 5]), 1usize);

        assert_eq!(line.geom().length_2(), 16 + 36);
    }

    #[test]
    fn container_nearest_neighbour() {
        let mut lines = RTree::new();
        lines.insert(GeomWithData::new(
            Line::new([0.0, 0.0], [1.0, 1.0]),
            "Line A",
        ));
        lines.insert(GeomWithData::new(
            Line::new([0.0, 0.0], [-1.0, 1.0]),
            "Line B",
        ));
        let my_location = [0.0, 0.0];
        // Now find the closest line
        let place = lines.nearest_neighbor(&my_location).unwrap();

        assert_eq!(place.data, "Line A");
    }
}
