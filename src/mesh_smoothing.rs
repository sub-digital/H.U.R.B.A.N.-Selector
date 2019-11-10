use std::cmp;
use std::collections::hash_map::{Entry, HashMap};
use std::hash::{Hash, Hasher};

use nalgebra as na;
use nalgebra::geometry::Point3;
use smallvec::SmallVec;

use crate::convert::{cast_u32, cast_usize};
use crate::geometry::{Face, Geometry, NormalStrategy};

/// Relaxes angles between mesh edges, resulting in a smoother geometry
///
/// or
///
/// Relaxes angles between mesh edges, while optionally keeping some vertices
/// anchored, resulting in an evenly distributed geometry optionally stretched
/// between the anchor points
///
/// The number of vertices, faces and the overall topology remains unchanged.
/// The more iterations, the smoother result. Too many iterations may cause slow
/// calculation time. In case the stop_when_stable flag is set on, the smoothing
/// stops when the geometry stops transforming between iterations or when it
/// reaches the maximum number of iterations.
///
/// The algorithm is based on replacing each vertex position with an average
/// position of its immediate neighbors.
///
/// - `geometry` - mesh geometry to relax
/// - `iterations` - (maximum) number of times the smoothing algorithm should
///   relax the geometry
/// - `fixed_vertex_indices` - indices of vertices to keep fixed during the
///   relaxation
/// - `stop_when_stable` - the smoothing stops when there is no change between
///   iterations
///
/// returns (smooth_geometry: Geometry, executed_iterations: u32, stable: bool)
pub fn laplacian_smoothing(
    geometry: &Geometry,
    vertex_to_vertex_topology: &HashMap<u32, SmallVec<[u32; 8]>>,
    iterations: u32,
    fixed_vertex_indices: &[u32],
    stop_when_stable: bool,
) -> (Geometry, u32, bool) {
    if iterations == 0 {
        return (geometry.clone(), 0, false);
    }

    let mut vertices: Vec<Point3<f32>> = Vec::from(geometry.vertices());
    let mut geometry_vertices: Vec<Point3<f32>>;

    let mut iteration: u32 = 0;

    // Only relevant when fixed vertices are specified
    let mut stable = !fixed_vertex_indices.is_empty();
    while iteration < iterations {
        stable = !fixed_vertex_indices.is_empty();
        geometry_vertices = vertices.clone();

        for (current_vertex_index, neighbors_indices) in vertex_to_vertex_topology.iter() {
            if fixed_vertex_indices
                .iter()
                .all(|i| i != current_vertex_index)
                && !neighbors_indices.is_empty()
            {
                let mut average_position: Point3<f32> = Point3::origin();
                for neighbor_index in neighbors_indices {
                    average_position += geometry_vertices[cast_usize(*neighbor_index)].coords;
                }
                average_position /= neighbors_indices.len() as f32;
                stable &= approx::relative_eq!(
                    &average_position.coords,
                    &vertices[cast_usize(*current_vertex_index)].coords,
                );
                vertices[cast_usize(*current_vertex_index)] = average_position;
            }
        }
        iteration += 1;

        if stop_when_stable && stable {
            break;
        }
    }

    // FIXME: Calculate smooth normals for the result once we support them
    (
        Geometry::from_faces_with_vertices_and_normals(
            geometry.faces().to_vec(),
            vertices,
            geometry.normals().to_vec(),
        ),
        iteration,
        stable,
    )
}

/// Performs one iteration of Loop Subdivision on geometry.
///
/// The subdivision works in two steps:
///
/// 1) Split each triangle into 4 smaller triangles,
/// 2) Update the position of each vertex of the mesh based on
///    weighted averages of its neighboring vertex positions,
///    depending on where the vertex is in the topology and whether
///    the vertex is newly created, or did already exist.
///
/// The geometry **must** be triangulated.
///
/// Implementation based on [mdfisher]
/// (https://graphics.stanford.edu/~mdfisher/subdivision.html).
pub fn loop_subdivision(
    geometry: &Geometry,
    vertex_to_vertex_topology: &HashMap<u32, SmallVec<[u32; 8]>>,
    face_to_face_topology: &HashMap<u32, SmallVec<[u32; 8]>>,
) -> Geometry {
    #[derive(Debug, Eq)]
    struct UnorderedPair(u32, u32);

    impl PartialEq for UnorderedPair {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0 && self.1 == other.1 || self.0 == other.1 && self.1 == other.0
        }
    }

    impl Hash for UnorderedPair {
        fn hash<H: Hasher>(&self, state: &mut H) {
            cmp::min(self.0, self.1).hash(state);
            cmp::max(self.0, self.1).hash(state);
        }
    }

    assert!(
        geometry.is_triangulated(),
        "Loop Subdivision is only defined for triangulated meshes",
    );

    let mut vertices: Vec<Point3<f32>> = geometry.vertices().iter().copied().collect();

    // Relocate existing vertices first
    for (i, vertex) in vertices.iter_mut().enumerate() {
        let neighbors = &vertex_to_vertex_topology[&cast_u32(i)];

        match neighbors.len() {
            // N == 0 means this is an orphan vertex. N == 1 can't
            // happen in our mesh representation.
            0 | 1 => (),
            2 => {
                // For edge valency N == 2 (a naked edge vertex), use
                // (3/4, 1/8, 1/8) relocation scheme.

                let vi1 = cast_usize(neighbors[0]);
                let vi2 = cast_usize(neighbors[1]);

                let v1 = geometry.vertices()[vi1];
                let v2 = geometry.vertices()[vi2];

                *vertex = Point3::origin()
                    + vertex.coords * 3.0 / 4.0
                    + v1.coords * 1.0 / 8.0
                    + v2.coords * 1.0 / 8.0;
            }
            3 => {
                // For edge valency N == 3, use (1 - N*BETA, BETA,
                // BETA, BETA) relocation scheme, where BETA is 3/16.

                const N: f32 = 3.0;
                const BETA: f32 = 3.0 / 16.0;

                let vi1 = cast_usize(neighbors[0]);
                let vi2 = cast_usize(neighbors[1]);
                let vi3 = cast_usize(neighbors[2]);

                let v1 = geometry.vertices()[vi1];
                let v2 = geometry.vertices()[vi2];
                let v3 = geometry.vertices()[vi3];

                *vertex = Point3::origin()
                    + vertex.coords * (1.0 - N * BETA)
                    + v1.coords * BETA
                    + v2.coords * BETA
                    + v3.coords * BETA;
            }
            n => {
                // For edge valency N >= 3, use (1 - N*BETA, BETA,
                // ...) relocation scheme, where BETA is 3 / (8*N).

                let n_f32 = n as f32;
                let beta = 3.0 / (8.0 * n_f32);

                *vertex = Point3::origin() + vertex.coords * (1.0 - n_f32 * beta);
                for vi in neighbors {
                    let v = geometry.vertices()[cast_usize(*vi)];
                    *vertex += v.coords * beta;
                }
            }
        }
    }

    // Subdivide existing triangle faces and create new vertices

    let faces_len_estimate = geometry.faces().len() * 4;
    let mut faces: Vec<(u32, u32, u32)> = Vec::with_capacity(faces_len_estimate);

    // We will be creating new mid-edge vertices per face soon. Faces
    // will share and re-use these newly created vertices.

    // The key is an unordered pair of faces that share the mid-edge
    // vertex. The value is the index of the vertex they share.
    let mut created_mid_vertex_indices: HashMap<UnorderedPair, u32> = HashMap::new();

    for (face_index, face) in geometry.faces().iter().enumerate() {
        let face_index_u32 = cast_u32(face_index);
        match face {
            Face::Triangle(triangle_face) => {
                let (vi1, vi2, vi3) = triangle_face.vertices;
                let face_neighbors = &face_to_face_topology[&face_index_u32];

                // Our current face should have up to 3 neighboring
                // faces. The mid vertices we are going to create need
                // to be shared with those faces if they exist, so
                // that they are only created once. The array below
                // will be filled with either vertices created here,
                // or obtained from `created_mid_vertex_indices`
                // cache.
                let mut mid_vertex_indices: [Option<u32>; 3] = [None, None, None];

                for (edge_index, (vi_from, vi_to)) in
                    [(vi1, vi2), (vi2, vi3), (vi3, vi1)].iter().enumerate()
                {
                    let neighbor_face_index = face_neighbors
                        .iter()
                        .copied()
                        .map(|i| (i, geometry.faces()[cast_usize(i)]))
                        .find_map(|(i, face)| {
                            if face.contains_vertex(*vi_from) && face.contains_vertex(*vi_to) {
                                Some(i)
                            } else {
                                None
                            }
                        });

                    let mid_vertex_index = if let Some(neighbor_face_index) = neighbor_face_index {
                        let pair = UnorderedPair(face_index_u32, neighbor_face_index);

                        match created_mid_vertex_indices.entry(pair) {
                            // The vertex exists and was therefore
                            // already relocated by visiting a
                            // neighboring face in a previous
                            // iteration
                            Entry::Occupied(occupied) => *occupied.get(),
                            Entry::Vacant(vacant) => {
                                // Create and relocate the vertex
                                // using the (1/8, 3/8, 3/8, 1/8)
                                // scheme. Since there is a neighbor
                                // face, we also write the created
                                // vertex to the cache to be picked up
                                // by subsequent iterations.

                                let edge_vertex_from = geometry.vertices()[cast_usize(*vi_from)];
                                let edge_vertex_to = geometry.vertices()[cast_usize(*vi_to)];

                                let face1 = geometry.faces()[face_index];
                                let face2 = geometry.faces()[cast_usize(neighbor_face_index)];

                                // Find the two vertices that are
                                // opposite to the shared edge of the
                                // face pair.
                                let (opposite_vertex_index1, opposite_vertex_index2) =
                                    match (face1, face2) {
                                        (
                                            Face::Triangle(triangle_face1),
                                            Face::Triangle(triangle_face2),
                                        ) => {
                                            let f1vi1 = triangle_face1.vertices.0;
                                            let f1vi2 = triangle_face1.vertices.1;
                                            let f1vi3 = triangle_face1.vertices.2;

                                            let f2vi1 = triangle_face2.vertices.0;
                                            let f2vi2 = triangle_face2.vertices.1;
                                            let f2vi3 = triangle_face2.vertices.2;

                                            let f1v = [f1vi1, f1vi2, f1vi3];
                                            let f2v = [f2vi1, f2vi2, f2vi3];

                                            let f1_opposite_vertex = f1v
                                                .iter()
                                                .copied()
                                                .find(|vi| !f2v.contains(&vi))
                                                .expect("Failed to find opposite vertex");
                                            let f2_opposite_vertex = f2v
                                                .iter()
                                                .copied()
                                                .find(|vi| !f1v.contains(&vi))
                                                .expect("Failed to find opposite vertex");

                                            (f1_opposite_vertex, f2_opposite_vertex)
                                        }
                                    };

                                let opposite_vertex1 =
                                    geometry.vertices()[cast_usize(opposite_vertex_index1)];
                                let opposite_vertex2 =
                                    geometry.vertices()[cast_usize(opposite_vertex_index2)];

                                let new_vertex = Point3::origin()
                                    + opposite_vertex1.coords * 1.0 / 8.0
                                    + opposite_vertex2.coords * 1.0 / 8.0
                                    + edge_vertex_from.coords * 3.0 / 8.0
                                    + edge_vertex_to.coords * 3.0 / 8.0;

                                let index = cast_u32(vertices.len());
                                vacant.insert(index);
                                vertices.push(new_vertex);

                                index
                            }
                        }
                    } else {
                        // Create and relocate the vertex using the (1/2, 1/2) scheme
                        let vertex_from = geometry.vertices()[cast_usize(*vi_from)];
                        let vertex_to = geometry.vertices()[cast_usize(*vi_to)];

                        let new_vertex = na::center(&vertex_from, &vertex_to);

                        let index = cast_u32(vertices.len());
                        vertices.push(new_vertex);

                        index
                    };

                    mid_vertex_indices[edge_index] = Some(mid_vertex_index);
                }

                let mid_v1v2_index =
                    mid_vertex_indices[0].expect("Must have been produced by earlier loop");
                let mid_v2v3_index =
                    mid_vertex_indices[1].expect("Must have been produced by earlier loop");
                let mid_v3v1_index =
                    mid_vertex_indices[2].expect("Must have been produced by earlier loop");

                faces.push((vi1, mid_v1v2_index, mid_v3v1_index));
                faces.push((vi2, mid_v2v3_index, mid_v1v2_index));
                faces.push((vi3, mid_v3v1_index, mid_v2v3_index));
                faces.push((mid_v1v2_index, mid_v2v3_index, mid_v3v1_index));
            }
        }
    }

    assert_eq!(faces.len(), faces_len_estimate);
    assert_eq!(faces.capacity(), faces_len_estimate);

    // FIXME: Calculate better normals here? Maybe use `Smooth` strategy once we have it?
    Geometry::from_triangle_faces_with_vertices_and_computed_normals(
        faces,
        vertices,
        NormalStrategy::Sharp,
    )
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use nalgebra;

    use crate::edge_analysis;
    use crate::geometry::{self, Geometry, NormalStrategy, OrientedEdge, Vertices};
    use crate::mesh_analysis;
    use crate::mesh_topology_analysis;

    use super::*;

    // FIXME: Snapshot testing
    fn torus() -> (Vec<(u32, u32, u32)>, Vertices) {
        let vertices = vec![
            Point3::new(0.566987, -1.129e-11, 0.25),
            Point3::new(-0.716506, 1.241025, 0.25),
            Point3::new(-0.283494, 0.491025, 0.25),
            Point3::new(-0.716506, -1.241025, 0.25),
            Point3::new(-0.283494, -0.491025, 0.25),
            Point3::new(1.0, -1.129e-11, -0.5),
            Point3::new(1.433013, -1.129e-11, 0.25),
            Point3::new(-0.5, 0.866025, -0.5),
            Point3::new(-0.5, -0.866025, -0.5),
        ];

        let faces = vec![
            (4, 3, 6),
            (0, 6, 2),
            (2, 1, 3),
            (8, 4, 0),
            (3, 8, 6),
            (5, 0, 7),
            (6, 5, 7),
            (7, 2, 4),
            (1, 7, 8),
            (4, 6, 0),
            (6, 1, 2),
            (2, 3, 4),
            (8, 0, 5),
            (8, 5, 6),
            (0, 2, 7),
            (6, 7, 1),
            (7, 4, 8),
            (1, 8, 3),
        ];

        (faces, vertices)
    }

    fn triple_torus() -> (Vec<(u32, u32, u32)>, Vertices) {
        let vertices = vec![
            Point3::new(15.566987, -1.129e-11, 0.25),
            Point3::new(14.283494, 1.241025, 0.25),
            Point3::new(14.716506, 0.491025, 0.25),
            Point3::new(14.283494, -1.241025, 0.25),
            Point3::new(14.716506, -0.491025, 0.25),
            Point3::new(16.0, 0.75, 0.25),
            Point3::new(15.149519, 1.241025, 0.25),
            Point3::new(16.0, 1.732051, 0.25),
            Point3::new(16.108253, 0.1875, -0.5),
            Point3::new(16.433012, -1.129e-11, 0.25),
            Point3::new(14.716506, 1.991025, 0.25),
            Point3::new(15.566987, 2.482051, 0.25),
            Point3::new(14.283494, 3.723076, 0.25),
            Point3::new(14.716506, 2.973076, 0.25),
            Point3::new(14.554127, 1.334775, -0.5),
            Point3::new(14.5, -0.866025, -0.5),
            Point3::new(14.5, 3.348076, -0.5),
            Point3::new(16.108253, 2.294551, -0.5),
            Point3::new(16.433012, 2.482051, 0.25),
        ];

        let faces = vec![
            (4, 3, 0),
            (0, 9, 1),
            (2, 1, 3),
            (7, 5, 9),
            (5, 6, 9),
            (6, 7, 18),
            (15, 4, 0),
            (3, 15, 9),
            (10, 1, 11),
            (11, 18, 12),
            (13, 12, 1),
            (14, 2, 15),
            (1, 14, 15),
            (8, 0, 2),
            (8, 14, 6),
            (16, 13, 10),
            (12, 16, 1),
            (17, 8, 7),
            (18, 9, 8),
            (14, 17, 6),
            (17, 11, 16),
            (18, 17, 16),
            (14, 10, 17),
            (3, 9, 0),
            (0, 1, 2),
            (2, 3, 4),
            (7, 9, 18),
            (6, 1, 9),
            (6, 18, 1),
            (15, 0, 8),
            (15, 8, 9),
            (1, 18, 11),
            (11, 12, 13),
            (13, 1, 10),
            (2, 4, 15),
            (1, 15, 3),
            (8, 2, 14),
            (8, 6, 5),
            (16, 10, 14),
            (16, 14, 1),
            (8, 5, 7),
            (18, 8, 17),
            (17, 7, 6),
            (11, 13, 16),
            (18, 16, 12),
            (10, 11, 17),
        ];

        (faces, vertices)
    }

    fn shape_for_smoothing_with_anchors() -> (Vec<(u32, u32, u32)>, Vertices) {
        let vertices = vec![
            Point3::new(30.21796, -6.119943, 0.0),
            Point3::new(32.031532, 1.328689, 0.0),
            Point3::new(33.875141, -3.522298, 3.718605),
            Point3::new(34.571838, -2.071111, 2.77835),
            Point3::new(34.778172, -5.285372, 3.718605),
            Point3::new(36.243252, -3.80194, 3.718605),
            Point3::new(36.741604, -10.146505, 0.0),
            Point3::new(39.676025, 1.905633, 0.0),
            Point3::new(42.587009, -5.186427, 0.0),
        ];

        let faces = vec![
            (4, 8, 5),
            (4, 6, 8),
            (5, 8, 7),
            (3, 5, 7),
            (0, 2, 1),
            (1, 2, 3),
            (0, 4, 2),
            (1, 3, 7),
            (0, 6, 4),
            (2, 4, 5),
            (2, 5, 3),
        ];

        (faces, vertices)
    }

    fn shape_for_smoothing_with_anchors_50_iterations() -> (Vec<(u32, u32, u32)>, Vertices) {
        let vertices = vec![
            Point3::new(30.21796, -6.119943, 0.0),
            Point3::new(32.031532, 1.328689, 0.0),
            Point3::new(34.491065, -2.551039, 0.0),
            Point3::new(36.00632, -0.404003, 0.0),
            Point3::new(36.372859, -5.260642, 0.0),
            Point3::new(37.826656, -2.299296, 0.0),
            Point3::new(36.741604, -10.146505, 0.0),
            Point3::new(39.676025, 1.905633, 0.0),
            Point3::new(42.587009, -5.186427, 0.0),
        ];

        let faces = vec![
            (4, 8, 5),
            (4, 6, 8),
            (5, 8, 7),
            (3, 5, 7),
            (0, 2, 1),
            (1, 2, 3),
            (0, 4, 2),
            (1, 3, 7),
            (0, 6, 4),
            (2, 4, 5),
            (2, 5, 3),
        ];

        (faces, vertices)
    }

    #[test]
    fn test_laplacian_smoothing_preserves_face_vertex_normal_count() {
        let (faces, vertices) = torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );

        let vertex_to_vertex_topology =
            mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let (relaxed_geometry_0, _, _) =
            laplacian_smoothing(&geometry, &vertex_to_vertex_topology, 0, &[], false);
        let (relaxed_geometry_1, _, _) =
            laplacian_smoothing(&geometry, &vertex_to_vertex_topology, 1, &[], false);
        let (relaxed_geometry_10, _, _) =
            laplacian_smoothing(&geometry, &vertex_to_vertex_topology, 10, &[], false);

        assert_eq!(relaxed_geometry_0.faces().len(), geometry.faces().len(),);
        assert_eq!(relaxed_geometry_1.faces().len(), geometry.faces().len(),);
        assert_eq!(relaxed_geometry_10.faces().len(), geometry.faces().len(),);
        assert_eq!(
            relaxed_geometry_0.vertices().len(),
            geometry.vertices().len(),
        );
        assert_eq!(
            relaxed_geometry_1.vertices().len(),
            geometry.vertices().len(),
        );
        assert_eq!(
            relaxed_geometry_10.vertices().len(),
            geometry.vertices().len(),
        );
        assert_eq!(relaxed_geometry_0.normals().len(), geometry.normals().len());
        assert_eq!(relaxed_geometry_1.normals().len(), geometry.normals().len());
        assert_eq!(
            relaxed_geometry_10.normals().len(),
            geometry.normals().len(),
        );
    }

    #[test]
    fn test_laplacian_smoothing_preserves_original_geometry_with_0_iterations() {
        let (faces, vertices) = triple_torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces,
            vertices,
            NormalStrategy::Sharp,
        );
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);

        let (relaxed_geometry, _, _) = laplacian_smoothing(&geometry, &v2v, 0, &[], false);
        assert_eq!(geometry, relaxed_geometry);
    }

    #[test]
    fn test_laplacian_smoothing_snapshot_triple_torus_1_iteration() {
        let (faces, vertices) = triple_torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces,
            vertices,
            NormalStrategy::Sharp,
        );
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);

        let (relaxed_geometry, _, _) = laplacian_smoothing(&geometry, &v2v, 1, &[], false);
        insta::assert_json_snapshot!(
            "triple_torus_after_1_iteration_of_laplacian_smoothing",
            &relaxed_geometry
        );
    }

    #[test]
    fn test_laplacian_smoothing_snapshot_triple_torus_2_iterations() {
        let (faces, vertices) = triple_torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces,
            vertices,
            NormalStrategy::Sharp,
        );
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);

        let (relaxed_geometry, _, _) = laplacian_smoothing(&geometry, &v2v, 2, &[], false);
        insta::assert_json_snapshot!(
            "triple_torus_after_2_iteration2_of_laplacian_smoothing",
            &relaxed_geometry
        );
    }

    #[test]
    fn test_laplacian_smoothing_snapshot_triple_torus_3_iterations() {
        let (faces, vertices) = triple_torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces,
            vertices,
            NormalStrategy::Sharp,
        );
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);

        let (relaxed_geometry, _, _) = laplacian_smoothing(&geometry, &v2v, 3, &[], false);
        insta::assert_json_snapshot!(
            "triple_torus_after_3_iterations_of_laplacian_smoothing",
            &relaxed_geometry
        );
    }

    #[test]
    fn test_laplacian_smoothing_with_anchors() {
        let (faces, vertices) = shape_for_smoothing_with_anchors();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );

        let fixed_vertex_indices: Vec<u32> = vec![0, 1, 7, 8, 6];

        let (faces_correct, vertices_correct) = shape_for_smoothing_with_anchors_50_iterations();
        let test_geometry_correct =
            Geometry::from_triangle_faces_with_vertices_and_computed_normals(
                faces_correct.clone(),
                vertices_correct.clone(),
                NormalStrategy::Sharp,
            );

        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let (relaxed_geometry, _, _) =
            laplacian_smoothing(&geometry, &v2v, 50, &fixed_vertex_indices, false);

        let relaxed_geometry_faces = relaxed_geometry.faces();
        let test_geometry_faces = test_geometry_correct.faces();

        assert_eq!(relaxed_geometry_faces, test_geometry_faces);

        const TOLERANCE_SQUARED: f32 = 0.01 * 0.01;

        let relaxed_geometry_vertices = relaxed_geometry.vertices();
        let test_geometry_vertices = test_geometry_correct.vertices();

        for i in 0..test_geometry_vertices.len() {
            assert!(
                nalgebra::distance_squared(
                    &test_geometry_vertices[i],
                    &relaxed_geometry_vertices[i]
                ) < TOLERANCE_SQUARED
            );
        }
    }

    #[test]
    fn test_laplacian_smoothing_with_anchors_find_border_vertices() {
        let (faces, vertices) = shape_for_smoothing_with_anchors();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );

        let oriented_edges: Vec<OrientedEdge> = geometry.oriented_edges_iter().collect();
        let edge_sharing_map = edge_analysis::edge_sharing(&oriented_edges);
        let fixed_vertex_indices =
            Vec::from_iter(mesh_analysis::border_vertex_indices(&edge_sharing_map).into_iter());

        let (faces_correct, vertices_correct) = shape_for_smoothing_with_anchors_50_iterations();
        let test_geometry_correct =
            Geometry::from_triangle_faces_with_vertices_and_computed_normals(
                faces_correct.clone(),
                vertices_correct.clone(),
                NormalStrategy::Sharp,
            );
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let (relaxed_geometry, _, _) =
            laplacian_smoothing(&geometry, &v2v, 50, &fixed_vertex_indices, false);

        let relaxed_geometry_faces = relaxed_geometry.faces();
        let test_geometry_faces = test_geometry_correct.faces();

        assert_eq!(relaxed_geometry_faces, test_geometry_faces);

        let relaxed_geometry_vertices = relaxed_geometry.vertices();
        let test_geometry_vertices = test_geometry_correct.vertices();

        for i in 0..test_geometry_vertices.len() {
            assert!(test_geometry_vertices[i].coords.relative_eq(
                &relaxed_geometry_vertices[i].coords,
                0.001,
                0.001,
            ));
        }
    }

    #[test]
    fn test_laplacian_smoothing_with_anchors_stop_when_stable_find_border_vertices() {
        let (faces, vertices) = shape_for_smoothing_with_anchors();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );

        let oriented_edges: Vec<OrientedEdge> = geometry.oriented_edges_iter().collect();
        let edge_sharing_map = edge_analysis::edge_sharing(&oriented_edges);
        let fixed_vertex_indices =
            Vec::from_iter(mesh_analysis::border_vertex_indices(&edge_sharing_map).into_iter());

        let (faces_correct, vertices_correct) = shape_for_smoothing_with_anchors_50_iterations();
        let test_geometry_correct =
            Geometry::from_triangle_faces_with_vertices_and_computed_normals(
                faces_correct.clone(),
                vertices_correct.clone(),
                NormalStrategy::Sharp,
            );

        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let (relaxed_geometry, _, _) =
            laplacian_smoothing(&geometry, &v2v, 255, &fixed_vertex_indices, true);

        let relaxed_geometry_faces = relaxed_geometry.faces();
        let test_geometry_faces = test_geometry_correct.faces();

        assert_eq!(relaxed_geometry_faces, test_geometry_faces);

        let relaxed_geometry_vertices = relaxed_geometry.vertices();
        let test_geometry_vertices = test_geometry_correct.vertices();

        for i in 0..test_geometry_vertices.len() {
            assert!(test_geometry_vertices[i].coords.relative_eq(
                &relaxed_geometry_vertices[i].coords,
                0.001,
                0.001,
            ));
        }
    }

    #[test]
    fn test_loop_subdivision_snapshot_uv_sphere() {
        let geometry = geometry::uv_sphere([0.0; 3], 1.0, 2, 3);
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let f2f = mesh_topology_analysis::face_to_face_topology(&geometry);

        let subdivided_geometry = loop_subdivision(&geometry, &v2v, &f2f);

        insta::assert_json_snapshot!(
            "uv_sphere_2_3_after_1_iteration_of_loop_subdivision",
            &subdivided_geometry
        );
    }

    #[test]
    fn test_loop_subdivision_snapshot_cube_sharp() {
        let geometry = geometry::cube_sharp([0.0; 3], 1.0);
        let v2v = mesh_topology_analysis::vertex_to_vertex_topology(&geometry);
        let f2f = mesh_topology_analysis::face_to_face_topology(&geometry);

        let subdivided_geometry = loop_subdivision(&geometry, &v2v, &f2f);

        insta::assert_json_snapshot!(
            "cube_sharp_after_1_iteration_of_loop_subdivision",
            &subdivided_geometry
        );
    }
}
