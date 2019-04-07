use crate::*;

pub type LineRenderData3D = (Point3<f32>, Point3<f32>, Point3<f32>);
pub type PointRenderData3D = (Point3<f32>, Point3<f32>);

pub fn create_render_data_for_tree_3d(tree: &DemoTree3D) -> RenderData {
    let mut vertices = Vec::new();
    let mut lines = Vec::new();
    let vertex_color = [0.0, 0.0, 1.0].into();
    let mut to_visit = vec![(tree.root(), 0)];
    while let Some((cur, depth)) = to_visit.pop() {
        push_cuboid(&mut lines, get_color_for_depth(depth), &cur.envelope());
        for child in cur.children() {
            match child {
                RTreeNode::Leaf(point) => vertices.push((
                    Point3::new(point[0] as f32, point[1] as f32, point[2] as f32),
                    vertex_color,
                )),
                RTreeNode::Parent(ref data) => {
                    to_visit.push((data, depth + 1));
                }
            }
        }
    }
    RenderData::ThreeD(lines, vertices)
}

fn push_cuboid(
    result: &mut Vec<LineRenderData3D>,
    color: Point3<f32>,
    envelope: &AABB<TreePointType3D>,
) {
    let c000: Point3<_> = envelope.lower().into();
    let c111: Point3<_> = envelope.upper().into();
    let c001 = Point3::new(c000[0], c000[1], c111[2]);
    let c010 = Point3::new(c000[0], c111[1], c000[2]);
    let c011 = Point3::new(c000[0], c111[1], c111[2]);
    let c100 = Point3::new(c111[0], c000[1], c000[2]);
    let c101 = Point3::new(c111[0], c000[1], c111[2]);
    let c110 = Point3::new(c111[0], c111[1], c000[2]);
    result.push((c000, c001, color));
    result.push((c000, c010, color));
    result.push((c000, c100, color));
    result.push((c001, c011, color));
    result.push((c001, c101, color));
    result.push((c010, c011, color));
    result.push((c010, c110, color));
    result.push((c011, c111, color));
    result.push((c100, c101, color));
    result.push((c100, c110, color));
    result.push((c101, c111, color));
    result.push((c110, c111, color));
}
