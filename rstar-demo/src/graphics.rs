// Copyright 2017 The Spade Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use glium;
use glium::{Display, DrawParameters, Program, Surface, VertexBuffer};
use rstar::node::RTreeNode;
use rstar::{RTree, AABB};
use Point;

const VERTEX_SHADER_SRC: &str = r#"
    #version 140
    in vec2 pos;
    in vec3 color;

    out vec3 fragment_color;
    void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    fragment_color = color;
        }
    "#;

const FRAGMENT_SHADER_SRC: &str = r#"
    #version 140
    out vec4 out_color;
    in vec3 fragment_color;
    void main() {
    out_color = vec4(fragment_color, 1.0);
        }
    "#;

pub struct RenderData {
    program: Program,
    pub edges_buffer: VertexBuffer<Vertex>,
    pub vertices_buffer: VertexBuffer<Vertex>,
    pub selection_buffer: VertexBuffer<Vertex>,
    pub selection_lines_buffer: VertexBuffer<Vertex>,
}

impl RenderData {
    pub fn new(display: &Display) -> RenderData {
        let program =
            Program::from_source(display, VERTEX_SHADER_SRC, FRAGMENT_SHADER_SRC, None).unwrap();
        let edges_buffer = VertexBuffer::new(display, &[]).unwrap();
        let vertices_buffer = VertexBuffer::new(display, &[]).unwrap();
        let selection_buffer = VertexBuffer::new(display, &[]).unwrap();
        let selection_lines_buffer = VertexBuffer::new(display, &[]).unwrap();
        RenderData {
            program,
            edges_buffer,
            vertices_buffer,
            selection_buffer,
            selection_lines_buffer,
        }
    }

    pub fn draw(&self, display: &Display) {
        let mut target = display.draw();
        target.clear_color(1.0, 1.0, 1.0, 1.0);
        let indices = glium::index::NoIndices(glium::index::PrimitiveType::LinesList);
        let parameters = DrawParameters {
            line_width: Some(1.0),
            ..Default::default()
        };

        target
            .draw(
                &self.edges_buffer,
                &indices,
                &self.program,
                &glium::uniforms::EmptyUniforms,
                &parameters,
            )
            .unwrap();

        let parameters = DrawParameters {
            point_size: Some(3.0),
            line_width: Some(2.0),
            ..Default::default()
        };

        target
            .draw(
                &self.selection_buffer,
                &indices,
                &self.program,
                &glium::uniforms::EmptyUniforms,
                &parameters,
            )
            .unwrap();

        target
            .draw(
                &self.selection_lines_buffer,
                &indices,
                &self.program,
                &glium::uniforms::EmptyUniforms,
                &parameters,
            )
            .unwrap();

        let indices = glium::index::NoIndices(glium::index::PrimitiveType::Points);
        target
            .draw(
                &self.vertices_buffer,
                &indices,
                &self.program,
                &glium::uniforms::EmptyUniforms,
                &parameters,
            )
            .unwrap();

        target.finish().unwrap();
    }

    pub fn update_rtree_buffers(&mut self, display: &Display, tree: &RTree<Point>) {
        let mut edges = Vec::new();
        let vertices = get_tree_edges(&tree, &mut edges);
        self.edges_buffer = VertexBuffer::new(display, &edges).unwrap();
        self.vertices_buffer = VertexBuffer::new(display, &vertices).unwrap();
    }

    pub fn update_selection(&mut self, display: &Display, points: &[Point]) {
        let color = [1.0, 0.0, 0.0];
        let mut vertices = Vec::new();
        for point in points {
            push_cross(&mut vertices, point, color);
        }
        self.selection_buffer = VertexBuffer::new(display, &vertices).unwrap();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 3],
}

implement_vertex!(Vertex, pos, color);
impl Vertex {
    pub fn new(pos: [f32; 2], color: [f32; 3]) -> Vertex {
        Vertex { pos, color }
    }
}

pub fn push_rectangle(vec: &mut Vec<Vertex>, rect: &AABB<Point>, color: [f32; 3]) {
    let v0 = [rect.lower()[0] as f32, rect.lower()[1] as f32];
    let v2 = [rect.upper()[0] as f32, rect.upper()[1] as f32];;
    let v1 = [v2[0] as f32, v0[1] as f32];
    let v3 = [v0[0] as f32, v2[1] as f32];
    vec.extend(
        [v0, v1, v1, v2, v2, v3, v3, v0]
            .iter()
            .cloned()
            .map(|v| Vertex::new(v, color)),
    );
}

pub fn push_cross(vec: &mut Vec<Vertex>, pos: &Point, color: [f32; 3]) {
    let delta = 0.015;
    let v0 = [pos[0] as f32 + delta, pos[1] as f32 + delta];
    let v1 = [pos[0] as f32 - delta, pos[1] as f32 - delta];
    let v2 = [pos[0] as f32 + delta, pos[1] as f32 - delta];
    let v3 = [pos[0] as f32 - delta, pos[1] as f32 + delta];
    vec.extend(
        [v0, v1, v2, v3]
            .iter()
            .cloned()
            .map(|v| Vertex::new(v, color)),
    );
}

pub fn get_color_for_depth(depth: usize) -> [f32; 3] {
    match depth {
        0 => [0., 0., 0.],
        1 => [0.85, 0., 0.85],
        2 => [0., 0.85, 0.85],
        3 => [0., 0., 0.55],
        4 => [0.85, 0., 0.],
        _ => [0., 0.85, 0.85],
    }
}

fn get_tree_edges(tree: &RTree<Point>, buffer: &mut Vec<Vertex>) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let vertex_color = [0.0, 0.0, 1.0];
    let mut to_visit = vec![(tree.root(), 0)];
    while let Some((cur, depth)) = to_visit.pop() {
        push_rectangle(buffer, &cur.envelope, get_color_for_depth(depth));
        for child in &cur.children {
            match child {
                RTreeNode::Leaf(point) => vertices.push(Vertex::new(
                    [point[0] as f32, point[1] as f32],
                    vertex_color,
                )),
                RTreeNode::Parent(ref data) => {
                    to_visit.push((data, depth + 1));
                }
            }
        }
    }
    vertices
}
