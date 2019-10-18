use nalgebra::geometry::Point3;

use crate::geometry::{Geometry, Vertices};

/// # Mesh relaxation / Laplacian smoothing
/// Relaxes angles between mesh edges, resulting in a smoother geometry.
/// The number of vertices, faces and the overall topology remains unchanged.
/// The more iterations, the smoother result. 
/// Too many iterations may cause slow calculation time.
/// 
/// The algorithm is based on replacing each vertex position 
/// with an average position of its immediate neighbors
#[allow(dead_code)]
pub fn laplacian_smoothing(geometry: Geometry, iterations: u8) -> Geometry {
    let vertex_to_vertex_topology = geometry.vertex_to_vertex_topology();
    let geometry_vertices = geometry.vertices();
    let mut vertices: Vertices = vec![Point3::origin(); geometry_vertices.len()];
    for _ in 0..iterations {
        for (current_vertex_index, neighbors_indices) in vertex_to_vertex_topology.iter() {
            let mut average_position: Point3<f32> = Point3::origin();
            for neighbor_index in neighbors_indices {
                average_position += geometry_vertices[*neighbor_index] - Point3::origin();
            }
            average_position /= neighbors_indices.len() as f32;
            vertices[current_vertex_index] = average_position;
        }
    }
    Geometry::from_triangle_faces_with_vertices_and_normals(
        geometry.triangle_faces_iter().collect(),
        vertices,
        geometry.normals().to_vec(),
    )
}
