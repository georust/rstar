use crate::*;

pub type LineRenderData2D = (Point2<f32>, Point2<f32>, Point3<f32>);

pub fn create_render_data_for_tree_2d(tree: &DemoTree2D) -> RenderData {
    let mut lines = Vec::new();
    let mut to_visit = vec![(tree.root(), 0)];
    while let Some((cur, depth)) = to_visit.pop() {
        push_rectangle(&mut lines, get_color_for_depth(depth), &cur.envelope());
        for child in cur.children() {
            match child {
                RTreeNode::Leaf(point) => {
                    push_2d_point(&mut lines, point.clone());
                }
                RTreeNode::Parent(ref data) => {
                    to_visit.push((data, depth + 1));
                }
            }
        }
    }
    RenderData::TwoD(lines)
}

fn push_2d_point(result: &mut Vec<LineRenderData2D>, point: TreePointType2D) {
    let offset = 1.;
    let vertex_color = [0.0f32, 0.0, 1.0].into();
    let lower = [point[0] - offset, point[1] - offset];
    let upper = [point[0] + offset, point[1] + offset];
    push_rectangle(result, vertex_color, &AABB::from_corners(lower, upper));
}

fn push_rectangle(
    result: &mut Vec<LineRenderData2D>,
    color: Point3<f32>,
    envelope: &AABB<TreePointType2D>,
) {
    let c00: Point2<_> = envelope.lower().into();
    let c11: Point2<_> = envelope.upper().into();
    let c01 = Point2::new(c00[0], c11[1]);
    let c10 = Point2::new(c11[0], c00[1]);
    result.push((c00, c01, color));
    result.push((c01, c11, color));
    result.push((c11, c10, color));
    result.push((c10, c00, color));
}
