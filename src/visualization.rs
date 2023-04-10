use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use room_visual_ext::RoomVisualExt;
use screeps::{CircleStyle, LineStyle, RectStyle, RoomName, RoomXY, TextStyle};
use std::f32::consts::PI;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::StableGraph;
use petgraph::Undirected;
use crate::room_state::StructuresMap;

pub enum Visualization {
    Matrix(Box<RoomMatrix<u8>>),
    Graph(StableGraph<RoomXY, u8, Undirected, u16>),
    Structures(StructuresMap),
}
use crate::visualization::Visualization::*;

pub trait RoomVisualExtExt {
    fn arrow(&self, from: RoomXY, to: RoomXY, style: Option<LineStyle>);
}

impl RoomVisualExtExt for RoomVisualExt {
    fn arrow(&self, start: RoomXY, end: RoomXY, style: Option<LineStyle>) {
        if start != end {
            let start_f32 = (start.x.u8() as f32, start.y.u8() as f32);
            let end_f32 = (end.x.u8() as f32, end.y.u8() as f32);
            self.line(start_f32, end_f32, style.clone());
            let diff = (end_f32.0 - start_f32.0, end_f32.1 - start_f32.1);
            let len = (diff.0 * diff.0 + diff.1 * diff.1).sqrt();
            let tip_base = (diff.0 * 0.35 / len, diff.1 * 0.35 / len);
            let counterclockwise_tip_start = (
                end_f32.0 + tip_base.0 * (-PI * 5.0 / 6.0).cos()
                    - tip_base.1 * (-PI * 5.0 / 6.0).sin(),
                end_f32.1
                    + tip_base.0 * (-PI * 5.0 / 6.0).sin()
                    + tip_base.1 * (-PI * 5.0 / 6.0).cos(),
            );
            self.line(counterclockwise_tip_start, end_f32, style.clone());
            let clockwise_tip_start = (
                end_f32.0 + tip_base.0 * (PI * 5.0 / 6.0).cos()
                    - tip_base.1 * (PI * 5.0 / 6.0).sin(),
                end_f32.1
                    + tip_base.0 * (PI * 5.0 / 6.0).sin()
                    + tip_base.1 * (PI * 5.0 / 6.0).cos(),
            );
            self.line(clockwise_tip_start, end_f32, style);
        }
    }
}

pub fn visualize(room_name: RoomName, visualization: Visualization) {
    let mut vis = RoomVisualExt::new(room_name);
    match visualization {
        Matrix(matrix) => {
            let mut min_value = 255;
            let mut max_non_255_value = 0;
            for (_, value) in matrix.iter() {
                if value < min_value {
                    min_value = value;
                }
                if value != 255 && value > max_non_255_value {
                    max_non_255_value = value;
                }
            }
            let range = (max_non_255_value - min_value) as f32;
            for (xy, value) in matrix.iter() {
                if value != 255 {
                    let opacity = if range > 0.0 {
                        0.2 + 0.6 * (value - min_value) as f32 / range
                    } else {
                        0.4
                    };
                    vis.rect(
                        xy.x.u8() as f32 - 0.5,
                        xy.y.u8() as f32 - 0.5,
                        1.0,
                        1.0,
                        Some(RectStyle::default().fill("#00f").opacity(opacity)),
                    );
                    vis.text(
                        xy.x.u8() as f32,
                        xy.y.u8() as f32 + 0.15,
                        value.to_string(),
                        Some(
                            TextStyle::default()
                                .font(0.5)
                                .color("#fff")
                                .opacity(1.0),
                        ),
                    );
                }
            }
        },
        Graph(graph) => {
            for node in graph.node_indices() {
                let xy = graph[node];
                vis.circle(xy.x.u8() as f32, xy.y.u8() as f32, Some(
                    CircleStyle::default().fill("#fff").radius(0.25).opacity(0.5)
                ));
                for edge in graph.edges(node) {
                    vis.arrow(xy, graph[edge.target()], Some(
                        LineStyle::default().color("#fff").width(0.05).opacity(0.8)
                    ));
                }
            }
        },
        Structures(structures_map) => {
            for (&structure_type, xys) in structures_map.iter() {
                for xy in xys.iter().copied() {
                    vis.structure_roomxy(xy, structure_type, 0.7);
                }
            }
        }
    }
}