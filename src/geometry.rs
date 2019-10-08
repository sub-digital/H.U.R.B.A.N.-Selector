use std::collections::HashSet;

use arrayvec::ArrayVec;
use std::cmp;

use nalgebra as na;
use nalgebra::base::Vector3;
use nalgebra::geometry::Point3;

use crate::convert::{cast_i32, cast_u32, cast_usize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy)]
pub enum NormalStrategy {
    Sharp,
    // FIXME: add `Smooth`
}

/// Geometric data containing multiple possibly _variable-length_
/// lists of geometric data, such as vertices and normals, and faces -
/// a single list containing the index topology that describes the
/// structure of data in those lists.
///
/// Currently only `Face::Triangle` is supported. It binds vertices
/// and normals in triangular faces. `Face::Triangle` is always
/// ensured to have counter-clockwise winding. Quad or polygonal faces
/// are not supported currently, but might be in the future.
///
/// The geometry data lives in right-handed coordinate space with the
/// XY plane being the ground and Z axis growing upwards.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    faces: Vec<Face>,
    vertices: Vec<Point3<f32>>,
    normals: Vec<Vector3<f32>>,
}

impl Geometry {
    /// Create new triangle face geometry from provided faces and
    /// vertices, and compute normals based on `normal_strategy`.
    ///
    /// # Panics
    /// Panics if faces refer to out-of-bounds vertices.
    pub fn from_triangle_faces_with_vertices_and_computed_normals(
        faces: Vec<(u32, u32, u32)>,
        vertices: Vec<Point3<f32>>,
        normal_strategy: NormalStrategy,
    ) -> Self {
        // FIXME: orphan removal

        let mut normals = Vec::with_capacity(faces.len());
        let vertices_range = 0..cast_u32(vertices.len());
        for &(v1, v2, v3) in &faces {
            assert!(
                vertices_range.contains(&v1),
                "Faces reference out of bounds position data"
            );
            assert!(
                vertices_range.contains(&v2),
                "Faces reference out of bounds position data"
            );
            assert!(
                vertices_range.contains(&v3),
                "Faces reference out of bounds position data"
            );

            // FIXME: computing smooth normals in the future won't be
            // so simple as just computing a normal per face, we will
            // need to analyze larger parts of the geometry
            let face_normal = match normal_strategy {
                NormalStrategy::Sharp => compute_triangle_normal(
                    &vertices[cast_usize(v1)],
                    &vertices[cast_usize(v2)],
                    &vertices[cast_usize(v3)],
                ),
            };

            normals.push(face_normal);
        }

        assert_eq!(normals.len(), faces.len());
        assert_eq!(normals.capacity(), faces.len());

        Self {
            faces: faces
                .into_iter()
                .enumerate()
                .map(|(i, (i1, i2, i3))| {
                    let normal_index = cast_u32(i);
                    TriangleFace::new_separate(i1, i2, i3, normal_index, normal_index, normal_index)
                })
                .map(Face::from)
                .collect(),
            vertices,
            normals,
        }
    }

    /// Create new triangle face geometry from provided faces,
    /// vertices, and normals.
    ///
    /// # Panics
    /// Panics if faces refer to out-of-bounds vertices or normals.
    pub fn from_triangle_faces_with_vertices_and_normals(
        faces: Vec<TriangleFace>,
        vertices: Vec<Point3<f32>>,
        normals: Vec<Vector3<f32>>,
    ) -> Self {
        // FIXME: orphan removal

        let vertices_range = 0..cast_u32(vertices.len());
        let normals_range = 0..cast_u32(normals.len());
        for face in &faces {
            let v = face.vertices;
            let n = face.normals;
            assert!(
                vertices_range.contains(&v.0),
                "Faces reference out of bounds position data"
            );
            assert!(
                vertices_range.contains(&v.1),
                "Faces reference out of bounds position data"
            );
            assert!(
                vertices_range.contains(&v.2),
                "Faces reference out of bounds position data"
            );
            assert!(
                normals_range.contains(&n.0),
                "Faces reference out of bounds normal data"
            );
            assert!(
                normals_range.contains(&n.1),
                "Faces reference out of bounds normal data"
            );
            assert!(
                normals_range.contains(&n.2),
                "Faces reference out of bounds normal data"
            );
        }

        Self {
            faces: faces.into_iter().map(Face::Triangle).collect(),
            vertices,
            normals,
        }
    }

    /// Return a view of all triangle faces in this geometry. Skip all
    /// other types of faces.
    pub fn triangle_faces_iter<'a>(&'a self) -> impl Iterator<Item = TriangleFace> + 'a {
        self.faces.iter().copied().map(|index| match index {
            Face::Triangle(f) => f,
        })
    }

    /// Return count of all triangle faces in this geometry. Skip all
    /// other types of faces.
    pub fn triangle_faces_len(&self) -> usize {
        self.faces
            .iter()
            .filter(|index| match index {
                Face::Triangle(_) => true,
            })
            .count()
    }

    pub fn vertices(&self) -> &[Point3<f32>] {
        &self.vertices
    }

    pub fn vertices_mut(&mut self) -> &mut [Point3<f32>] {
        &mut self.vertices
    }

    pub fn normals(&self) -> &[Vector3<f32>] {
        &self.normals
    }

    pub fn oriented_edges_iter<'a>(&'a self) -> impl Iterator<Item = OrientedEdge> + 'a {
        self.triangle_faces_iter()
            .flat_map(|face| ArrayVec::from(face.to_oriented_edges()).into_iter())
    }

    pub fn unoriented_edges_iter<'a>(&'a self) -> impl Iterator<Item = UnorientedEdge> + 'a {
        self.triangle_faces_iter()
            .flat_map(|face| ArrayVec::from(face.to_unoriented_edges()).into_iter())
    }

    /// Genus of a mesh is the number of holes in topology / conectivity
    /// The mesh must be triangular and watertight
    /// V - E + F = 2 (1 - G)
    pub fn mesh_genus(&self, edges: &HashSet<UnorientedEdge>) -> i32 {
        let vertex_count = cast_i32(self.vertices.len());
        let edge_count = cast_i32(edges.len());
        let face_count = cast_i32(self.faces.len());

        1 - (vertex_count - edge_count + face_count) / 2
    }

    pub fn has_no_orphan_vertices(&self) -> bool {
        let mut used_vertices = HashSet::new();
        for face in self.triangle_faces_iter() {
            used_vertices.insert(face.vertices.0);
            used_vertices.insert(face.vertices.1);
            used_vertices.insert(face.vertices.2);
        }
        used_vertices.len() == self.vertices().len()
    }

    pub fn has_no_orphan_normals(&self) -> bool {
        let mut used_normals = HashSet::new();
        for face in self.triangle_faces_iter() {
            used_normals.insert(face.normals.0);
            used_normals.insert(face.normals.1);
            used_normals.insert(face.normals.2);
        }
        used_normals.len() == self.normals().len()
    }
}

/// A geometry index. Describes topology of geometry data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Triangle(TriangleFace),
}

impl From<TriangleFace> for Face {
    fn from(triangle_face: TriangleFace) -> Face {
        Face::Triangle(triangle_face)
    }
}

/// A triangular face. Contains indices to other geometry data, such
/// as vertices and normals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriangleFace {
    pub vertices: (u32, u32, u32),
    pub normals: (u32, u32, u32),
}

impl TriangleFace {
    pub fn new(i1: u32, i2: u32, i3: u32) -> TriangleFace {
        TriangleFace {
            vertices: (i1, i2, i3),
            normals: (i1, i2, i3),
        }
    }

    pub fn new_separate(
        vi1: u32,
        vi2: u32,
        vi3: u32,
        ni1: u32,
        ni2: u32,
        ni3: u32,
    ) -> TriangleFace {
        TriangleFace {
            vertices: (vi1, vi2, vi3),
            normals: (ni1, ni2, ni3),
        }
    }

    /// Generates 3 oriented edges from the respective triangular face
    pub fn to_oriented_edges(&self) -> [OrientedEdge; 3] {
        [
            OrientedEdge::new(self.vertices.0, self.vertices.1),
            OrientedEdge::new(self.vertices.1, self.vertices.2),
            OrientedEdge::new(self.vertices.2, self.vertices.0),
        ]
    }

    /// Generates 3 unoriented edges from the respective triangular face
    pub fn to_unoriented_edges(&self) -> [UnorientedEdge; 3] {
        [
            UnorientedEdge(OrientedEdge::new(self.vertices.0, self.vertices.1)),
            UnorientedEdge(OrientedEdge::new(self.vertices.1, self.vertices.2)),
            UnorientedEdge(OrientedEdge::new(self.vertices.2, self.vertices.0)),
        ]
    }
}

impl From<(u32, u32, u32)> for TriangleFace {
    fn from((i1, i2, i3): (u32, u32, u32)) -> TriangleFace {
        TriangleFace::new(i1, i2, i3)
    }
}

/// Oriented face edge. Contains indices to other geometry data - vertices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrientedEdge {
    pub vertices: (u32, u32),
}

impl OrientedEdge {
    pub fn new(i1: u32, i2: u32) -> Self {
        assert!(
            i1 != i2,
            "The oriented edge is constituted of the same vertex"
        );
        OrientedEdge { vertices: (i1, i2) }
    }

    pub fn is_reverted(self, other: OrientedEdge) -> bool {
        self.vertices.0 == other.vertices.1 && self.vertices.1 == other.vertices.0
    }
}

impl From<(u32, u32)> for OrientedEdge {
    fn from((i1, i2): (u32, u32)) -> OrientedEdge {
        OrientedEdge::new(i1, i2)
    }
}

/// Implements orientation indifferent hash and equal methods
#[derive(Debug, Clone, Copy, Eq)]
pub struct UnorientedEdge(pub OrientedEdge);

impl PartialEq for UnorientedEdge {
    fn eq(&self, other: &Self) -> bool {
        (self.0.vertices.0 == other.0.vertices.0 && self.0.vertices.1 == other.0.vertices.1)
            || (self.0.vertices.0 == other.0.vertices.1 && self.0.vertices.1 == other.0.vertices.0)
    }
}

impl Hash for UnorientedEdge {
    fn hash<H: Hasher>(&self, state: &mut H) {
        cmp::min(self.0.vertices.0, self.0.vertices.1).hash(state);
        cmp::max(self.0.vertices.0, self.0.vertices.1).hash(state);
    }
}

pub fn plane_same_len(position: [f32; 3], scale: f32) -> Geometry {
    #[rustfmt::skip]
    let vertex_positions = vec![
        v(-1.0, -1.0,  0.0, position, scale),
        v( 1.0, -1.0,  0.0, position, scale),
        v( 1.0,  1.0,  0.0, position, scale),
        v( 1.0,  1.0,  0.0, position, scale),
        v(-1.0,  1.0,  0.0, position, scale),
        v(-1.0, -1.0,  0.0, position, scale),
    ];

    #[rustfmt::skip]
    let vertex_normals = vec![
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
    ];

    #[rustfmt::skip]
    let faces = vec![
        TriangleFace::new(0, 1, 2),
        TriangleFace::new(3, 4, 5),
    ];

    Geometry::from_triangle_faces_with_vertices_and_normals(faces, vertex_positions, vertex_normals)
}

pub fn plane_var_len(position: [f32; 3], scale: f32) -> Geometry {
    #[rustfmt::skip]
    let vertex_positions = vec![
        v(-1.0, -1.0,  0.0, position, scale),
        v( 1.0, -1.0,  0.0, position, scale),
        v( 1.0,  1.0,  0.0, position, scale),
        v( 1.0,  1.0,  0.0, position, scale),
        v(-1.0,  1.0,  0.0, position, scale),
        v(-1.0, -1.0,  0.0, position, scale),
    ];

    #[rustfmt::skip]
    let vertex_normals = vec![
        n( 0.0,  0.0,  1.0),
    ];

    #[rustfmt::skip]
    let faces = vec![
        TriangleFace::new_separate(0, 1, 2, 0, 0, 0),
        TriangleFace::new_separate(3, 4, 5, 0, 0, 0),
    ];

    Geometry::from_triangle_faces_with_vertices_and_normals(faces, vertex_positions, vertex_normals)
}

pub fn cube_smooth_same_len(position: [f32; 3], scale: f32) -> Geometry {
    #[rustfmt::skip]
    let vertex_positions = vec![
        // back
        v(-1.0,  1.0, -1.0, position, scale),
        v(-1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0, -1.0, position, scale),
        // front
        v(-1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0,  1.0, position, scale),
        v(-1.0, -1.0,  1.0, position, scale),
    ];

    // FIXME: make const once float arithmetic is stabilized in const fns
    // let sqrt_3 = 3.0f32.sqrt();
    let frac_1_sqrt_3 = 1.0 / 3.0_f32.sqrt();

    #[rustfmt::skip]
    let vertex_normals = vec![
        // back
        n(-frac_1_sqrt_3,  frac_1_sqrt_3, -frac_1_sqrt_3),
        n(-frac_1_sqrt_3,  frac_1_sqrt_3,  frac_1_sqrt_3),
        n( frac_1_sqrt_3,  frac_1_sqrt_3,  frac_1_sqrt_3),
        n( frac_1_sqrt_3,  frac_1_sqrt_3, -frac_1_sqrt_3),
        // front
        n(-frac_1_sqrt_3, -frac_1_sqrt_3, -frac_1_sqrt_3),
        n( frac_1_sqrt_3, -frac_1_sqrt_3, -frac_1_sqrt_3),
        n( frac_1_sqrt_3, -frac_1_sqrt_3,  frac_1_sqrt_3),
        n(-frac_1_sqrt_3, -frac_1_sqrt_3,  frac_1_sqrt_3),
    ];

    #[rustfmt::skip]
    let faces = vec![
        // back
        TriangleFace::new(0, 1, 2),
        TriangleFace::new(2, 3, 0),
        // front
        TriangleFace::new(4, 5, 6),
        TriangleFace::new(6, 7, 4),
        // top
        TriangleFace::new(7, 6, 2),
        TriangleFace::new(2, 1, 7),
        // bottom
        TriangleFace::new(4, 0, 3),
        TriangleFace::new(3, 5, 4),
        // right
        TriangleFace::new(5, 3, 2),
        TriangleFace::new(2, 6, 5),
        // left
        TriangleFace::new(4, 7, 1),
        TriangleFace::new(1, 0, 4),
    ];

    Geometry::from_triangle_faces_with_vertices_and_normals(faces, vertex_positions, vertex_normals)
}

#[deprecated(note = "Don't use, generates open geometry")]
// FIXME: Remove eventually
pub fn cube_sharp_same_len(position: [f32; 3], scale: f32) -> Geometry {
    #[rustfmt::skip]
    let vertex_positions = vec![
        // back
        v(-1.0,  1.0, -1.0, position, scale),
        v(-1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0, -1.0, position, scale),
        // front
        v(-1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0,  1.0, position, scale),
        v(-1.0, -1.0,  1.0, position, scale),
        // top
        v(-1.0,  1.0,  1.0, position, scale),
        v(-1.0, -1.0,  1.0, position, scale),
        v( 1.0, -1.0,  1.0, position, scale),
        v( 1.0,  1.0,  1.0, position, scale),
        // bottom
        v(-1.0,  1.0, -1.0, position, scale),
        v( 1.0,  1.0, -1.0, position, scale),
        v( 1.0, -1.0, -1.0, position, scale),
        v(-1.0, -1.0, -1.0, position, scale),
        // right
        v( 1.0,  1.0, -1.0, position, scale),
        v( 1.0,  1.0,  1.0, position, scale),
        v( 1.0, -1.0,  1.0, position, scale),
        v( 1.0, -1.0, -1.0, position, scale),
        // left
        v(-1.0,  1.0, -1.0, position, scale),
        v(-1.0, -1.0, -1.0, position, scale),
        v(-1.0, -1.0,  1.0, position, scale),
        v(-1.0,  1.0,  1.0, position, scale),
    ];

    #[rustfmt::skip]
    let vertex_normals = vec![
        // back
        n( 0.0,  1.0,  0.0),
        n( 0.0,  1.0,  0.0),
        n( 0.0,  1.0,  0.0),
        n( 0.0,  1.0,  0.0),
        // front
        n( 0.0, -1.0,  0.0),
        n( 0.0, -1.0,  0.0),
        n( 0.0, -1.0,  0.0),
        n( 0.0, -1.0,  0.0),
        // top
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        n( 0.0,  0.0,  1.0),
        // bottom
        n( 0.0,  0.0, -1.0),
        n( 0.0,  0.0, -1.0),
        n( 0.0,  0.0, -1.0),
        n( 0.0,  0.0, -1.0),
        // right
        n( 1.0,  0.0,  0.0),
        n( 1.0,  0.0,  0.0),
        n( 1.0,  0.0,  0.0),
        n( 1.0,  0.0,  0.0),
        // left
        n(-1.0,  0.0,  0.0),
        n(-1.0,  0.0,  0.0),
        n(-1.0,  0.0,  0.0),
        n(-1.0,  0.0,  0.0),
    ];

    #[rustfmt::skip]
    let faces = vec![
        // back
        TriangleFace::new(0, 1, 2),
        TriangleFace::new(2, 3, 0),
        // front
        TriangleFace::new(4, 5, 6),
        TriangleFace::new(6, 7, 4),
        // top
        TriangleFace::new(8, 9, 10),
        TriangleFace::new(10, 11, 8),
        // bottom
        TriangleFace::new(12, 13, 14),
        TriangleFace::new(14, 15, 12),
        // right
        TriangleFace::new(16, 17, 18),
        TriangleFace::new(18, 19, 16),
        // left
        TriangleFace::new(20, 21, 22),
        TriangleFace::new(22, 23, 20),
    ];

    Geometry::from_triangle_faces_with_vertices_and_normals(faces, vertex_positions, vertex_normals)
}

pub fn cube_sharp_var_len(position: [f32; 3], scale: f32) -> Geometry {
    #[rustfmt::skip]
    let vertex_positions = vec![
        // back
        v(-1.0,  1.0, -1.0, position, scale),
        v(-1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0,  1.0, position, scale),
        v( 1.0,  1.0, -1.0, position, scale),
        // front
        v(-1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0, -1.0, position, scale),
        v( 1.0, -1.0,  1.0, position, scale),
        v(-1.0, -1.0,  1.0, position, scale),
    ];

    #[rustfmt::skip]
    let vertex_normals = vec![
        // back
        n( 0.0,  1.0,  0.0),
        // front
        n( 0.0, -1.0,  0.0),
        // top
        n( 0.0,  0.0,  1.0),
        // bottom
        n( 0.0,  0.0, -1.0),
        // right
        n( 1.0,  0.0,  0.0),
        // left
        n(-1.0,  0.0,  0.0),
    ];

    #[rustfmt::skip]
    let faces = vec![
        // back
        TriangleFace::new_separate(0, 1, 2, 0, 0, 0),
        TriangleFace::new_separate(2, 3, 0, 0, 0, 0),
        // front
        TriangleFace::new_separate(4, 5, 6, 1, 1, 1),
        TriangleFace::new_separate(6, 7, 4, 1, 1, 1),
        // top
        TriangleFace::new_separate(7, 6, 2, 2, 2, 2),
        TriangleFace::new_separate(2, 1, 7, 2, 2, 2),
        // bottom
        TriangleFace::new_separate(4, 0, 3, 3, 3, 3),
        TriangleFace::new_separate(3, 5, 4, 3, 3, 3),
        // right
        TriangleFace::new_separate(5, 3, 2, 4, 4, 4),
        TriangleFace::new_separate(2, 6, 5, 4, 4, 4),
        // left
        TriangleFace::new_separate(4, 7, 1, 5, 5, 5),
        TriangleFace::new_separate(1, 0, 4, 5, 5, 5),
    ];

    Geometry::from_triangle_faces_with_vertices_and_normals(faces, vertex_positions, vertex_normals)
}

/// Create UV Sphere primitive at `position` with `scale`,
/// `n_parallels` and `n_meridians`.
///
/// # Panics
/// Panics if number of parallels is less than 2 or number of
/// meridians is less than 3.
pub fn uv_sphere(position: [f32; 3], scale: f32, n_parallels: u32, n_meridians: u32) -> Geometry {
    assert!(n_parallels >= 2, "Need at least 2 paralells");
    assert!(n_meridians >= 3, "Need at least 3 meridians");

    // Add the poles
    let lat_line_max = n_parallels + 2;
    // Add the last, wrapping meridian
    let lng_line_max = n_meridians + 1;

    use std::f32::consts::PI;
    const TWO_PI: f32 = 2.0 * PI;

    // 1 North pole + 1 South pole + `n_parallels` * `n_meridians`
    let vertex_data_count = cast_usize(2 + n_parallels * n_meridians);
    let mut vertex_positions = Vec::with_capacity(vertex_data_count);

    // Produce vertex data for bands in between parallels

    for lat_line in 0..n_parallels {
        for lng_line in 0..n_meridians {
            let polar_t = (lat_line + 1) as f32 / (lat_line_max - 1) as f32;
            let azimuthal_t = lng_line as f32 / (lng_line_max - 1) as f32;

            let x = (PI * polar_t).sin() * (TWO_PI * azimuthal_t).cos();
            let y = (PI * polar_t).sin() * (TWO_PI * azimuthal_t).sin();
            let z = (PI * polar_t).cos();

            vertex_positions.push(v(x, y, z, position, scale));
        }
    }

    // Triangles from North and South poles to the nearest band + 2 * quads in bands
    let faces_count = cast_usize(2 * n_meridians + 2 * n_meridians * (n_parallels - 1));
    let mut faces = Vec::with_capacity(faces_count);

    // Produce faces for bands in-between parallels

    for i in 1..n_parallels {
        for j in 0..n_meridians {
            // Produces 2 CCW wound triangles: (p1, p2, p3) and (p3, p4, p1)

            let p1 = i * n_meridians + j;
            let p2 = i * n_meridians + ((j + 1) % n_meridians);

            let p4 = (i - 1) * n_meridians + j;
            let p3 = (i - 1) * n_meridians + ((j + 1) % n_meridians);

            faces.push((p1, p2, p3));
            faces.push((p3, p4, p1));
        }
    }

    // Add vertex data and band-connecting faces for North and South poles

    let north_pole = cast_u32(vertex_positions.len());
    vertex_positions.push(v(0.0, 0.0, 1.0, position, scale));

    let south_pole = cast_u32(vertex_positions.len());
    vertex_positions.push(v(0.0, 0.0, -1.0, position, scale));

    for i in 0..n_meridians {
        let north_p1 = i;
        let north_p2 = (i + 1) % n_meridians;

        let south_p1 = (n_parallels - 1) * n_meridians + i;
        let south_p2 = (n_parallels - 1) * n_meridians + ((i + 1) % n_meridians);

        faces.push((north_p1, north_p2, north_pole));
        faces.push((south_p2, south_p1, south_pole));
    }

    assert_eq!(vertex_positions.len(), vertex_data_count);
    assert_eq!(vertex_positions.capacity(), vertex_data_count);
    assert_eq!(faces.len(), faces_count);
    assert_eq!(faces.capacity(), faces_count);

    Geometry::from_triangle_faces_with_vertices_and_computed_normals(
        faces,
        vertex_positions,
        NormalStrategy::Sharp,
    )
}

pub fn compute_bounding_sphere(geometries: &[Geometry]) -> (Point3<f32>, f32) {
    let centroid = compute_centroid(geometries);
    let mut max_distance = 0.0;

    for geometry in geometries {
        for vertex in &geometry.vertices {
            // Can't use `distance_squared` for values 0..1

            // FIXME: @Optimization Benchmark this against a 0..1 vs
            // 1..inf branching version using distance_squared for 1..inf
            let distance = na::distance(&centroid, vertex);
            if distance > max_distance {
                max_distance = distance;
            }
        }
    }

    (centroid, max_distance)
}

pub fn compute_centroid(geometries: &[Geometry]) -> Point3<f32> {
    let mut vertex_count = 0;
    let mut centroid = Point3::origin();
    for geometry in geometries {
        vertex_count += geometry.vertices.len();
        for vertex in &geometry.vertices {
            let v = vertex - Point3::origin();
            centroid += v;
        }
    }

    centroid / (vertex_count as f32)
}

pub fn find_closest_point(position: &Point3<f32>, geometry: &Geometry) -> Option<Point3<f32>> {
    let vertices = geometry.vertices();
    if vertices.is_empty() {
        return None;
    }

    let mut closest = vertices[0];
    // FIXME: @Optimization benchmark `distance` vs `distance_squared`
    // with branching (0..1, 1..inf)
    let mut closest_distance = na::distance(position, &closest);
    for point in &vertices[1..] {
        let distance = na::distance(position, &point);
        if distance < closest_distance {
            closest = *point;
            closest_distance = distance;
        }
    }

    Some(closest)
}

fn v(x: f32, y: f32, z: f32, translation: [f32; 3], scale: f32) -> Point3<f32> {
    Point3::new(
        scale * x + translation[0],
        scale * y + translation[1],
        scale * z + translation[2],
    )
}

fn n(x: f32, y: f32, z: f32) -> Vector3<f32> {
    Vector3::new(x, y, z)
}

fn compute_triangle_normal(p1: &Point3<f32>, p2: &Point3<f32>, p3: &Point3<f32>) -> Vector3<f32> {
    let u = p2 - p1;
    let v = p3 - p1;

    Vector3::cross(&u, &v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn quad() -> (Vec<(u32, u32, u32)>, Vec<Point3<f32>>) {
        #[rustfmt::skip]
        let vertices = vec![
            v(-1.0, -1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v( 1.0, -1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v( 1.0,  1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v(-1.0,  1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
        ];

        #[rustfmt::skip]
        let faces = vec![
            (0, 1, 2),
            (2, 3, 0),
        ];

        (faces, vertices)
    }

    fn quad_with_normals() -> (Vec<TriangleFace>, Vec<Point3<f32>>, Vec<Vector3<f32>>) {
        #[rustfmt::skip]
        let vertices = vec![
            v(-1.0, -1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v( 1.0, -1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v( 1.0,  1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
            v(-1.0,  1.0,  0.0, [0.0, 0.0, 0.0], 1.0),
        ];

        #[rustfmt::skip]
        let normals = vec![
            n( 0.0,  0.0,  1.0),
            n( 0.0,  0.0,  1.0),
            n( 0.0,  0.0,  1.0),
            n( 0.0,  0.0,  1.0),
        ];

        #[rustfmt::skip]
        let faces = vec![
            TriangleFace::new(0, 1, 2),
            TriangleFace::new(2, 3, 0),
        ];

        (faces, vertices, normals)
    }

    fn torus() -> (Vec<(u32, u32, u32)>, Vec<Point3<f32>>) {
        #[rustfmt::skip]
            let vertices = vec![
            Point3::new(1.0, 0.0, 0.25),
            Point3::new(0.821486, 0.0, 0.175022),
            Point3::new(0.75, 0.0, 0.0),
            Point3::new(0.824978, 0.0, -0.178514),
            Point3::new(1.0, 0.0, -0.25),
            Point3::new(0.891778, 0.76512, 0.178514),
            Point3::new(0.758946, 0.651154, 0.25),
            Point3::new(0.623464, 0.534914, 0.175022),
            Point3::new(0.569209, 0.488365, 0.0),
            Point3::new(0.626113, 0.537187, -0.178514),
            Point3::new(0.758946, 0.651154, -0.25),
            Point3::new(0.894428, 0.767394, -0.175022),
            Point3::new(0.948682, 0.813942, 0.0),
            Point3::new(0.201501, 1.157616, 0.178514),
            Point3::new(0.171487, 0.985186, 0.25),
            Point3::new(0.140874, 0.809317, 0.175022),
            Point3::new(0.128615, 0.73889, 0.0),
            Point3::new(0.141473, 0.812757, -0.178514),
            Point3::new(0.171487, 0.985186, -0.25),
            Point3::new(0.202099, 1.161056, -0.175022),
            Point3::new(0.214358, 1.231483, 0.0),
            Point3::new(-0.600858, 1.009776, 0.178514),
            Point3::new(-0.511359, 0.859367, 0.25),
            Point3::new(-0.420074, 0.705959, 0.175022),
            Point3::new(-0.383519, 0.644526, 0.0),
            Point3::new(-0.421859, 0.708959, -0.178514),
            Point3::new(-0.511359, 0.859367, -0.25),
            Point3::new(-0.602643, 1.012776, -0.175022),
            Point3::new(-0.639198, 1.074209, 0.0),
            Point3::new(-1.105913, 0.397032, 0.178514),
            Point3::new(-0.941185, 0.337893, 0.25),
            Point3::new(-0.77317, 0.277574, 0.175022),
            Point3::new(-0.705888, 0.25342, 0.0),
            Point3::new(-0.776456, 0.278754, -0.178514),
            Point3::new(-0.941185, 0.337893, -0.25),
            Point3::new(-1.109199, 0.398211, -0.175022),
            Point3::new(-1.176481, 0.422366, 0.0),
            Point3::new(-1.099292, -0.415012, 0.178514),
            Point3::new(-0.93555, -0.353195, 0.25),
            Point3::new(-0.768541, -0.290145, 0.175022),
            Point3::new(-0.701662, -0.264896, 0.0),
            Point3::new(-0.771807, -0.291378, -0.178514),
            Point3::new(-0.93555, -0.353195, -0.25),
            Point3::new(-1.102558, -0.416245, -0.175022),
            Point3::new(-1.169437, -0.441494, 0.0),
            Point3::new(-0.5808, -1.021445, 0.178514),
            Point3::new(-0.494288, -0.869298, 0.25),
            Point3::new(-0.406051, -0.714117, 0.175022),
            Point3::new(-0.370716, -0.651974, 0.0),
            Point3::new(-0.407777, -0.717151, -0.178514),
            Point3::new(-0.494288, -0.869298, -0.25),
            Point3::new(-0.582525, -1.02448, -0.175022),
            Point3::new(-0.61786, -1.086623, 0.0),
            Point3::new(0.21476, -1.15523, 0.178514),
            Point3::new(0.182771, -0.983156, 0.25),
            Point3::new(0.150144, -0.807649, 0.175022),
            Point3::new(0.137078, -0.737367, 0.0),
            Point3::new(0.150782, -0.811081, -0.178514),
            Point3::new(0.182771, -0.983156, -0.25),
            Point3::new(0.215398, -1.158662, -0.175022),
            Point3::new(0.228463, -1.228944, 0.0),
            Point3::new(0.964571, -0.795049, 0.0),
            Point3::new(0.906714, -0.74736, 0.178514),
            Point3::new(0.771657, -0.636039, 0.25),
            Point3::new(0.633906, -0.522497, 0.175022),
            Point3::new(0.578743, -0.477029, 0.0),
            Point3::new(0.6366, -0.524718, -0.178514),
            Point3::new(0.771657, -0.636039, -0.25),
            Point3::new(0.909408, -0.749581, -0.175022),
            Point3::new(1.25, 0.0, 0.0),
            Point3::new(1.175022, 0.0, 0.178514),
            Point3::new(1.178514, 0.0, -0.175022),
        ];

        #[rustfmt::skip]
            let faces = vec![
            (5, 70, 69),
            (6, 0, 5),
            (7, 1, 6),
            (8, 2, 1),
            (9, 3, 8),
            (10, 4, 3),
            (11, 71, 10),
            (12, 69, 11),
            (13, 5, 20),
            (14, 6, 5),
            (15, 7, 14),
            (16, 8, 7),
            (17, 9, 16),
            (18, 10, 9),
            (19, 11, 10),
            (20, 12, 11),
            (21, 13, 20),
            (22, 14, 21),
            (23, 15, 22),
            (24, 16, 15),
            (25, 17, 24),
            (26, 18, 17),
            (27, 19, 18),
            (28, 20, 27),
            (29, 21, 28),
            (30, 22, 29),
            (31, 23, 22),
            (32, 24, 23),
            (33, 25, 32),
            (34, 26, 25),
            (35, 27, 26),
            (36, 28, 35),
            (37, 29, 44),
            (38, 30, 37),
            (39, 31, 30),
            (40, 32, 39),
            (41, 33, 32),
            (42, 34, 33),
            (43, 35, 34),
            (44, 36, 35),
            (45, 37, 52),
            (46, 38, 37),
            (47, 39, 38),
            (48, 40, 47),
            (49, 41, 40),
            (50, 42, 49),
            (51, 43, 42),
            (52, 44, 43),
            (53, 45, 60),
            (54, 46, 45),
            (55, 47, 54),
            (56, 48, 55),
            (57, 49, 48),
            (58, 50, 49),
            (59, 51, 58),
            (60, 52, 59),
            (62, 53, 60),
            (63, 54, 62),
            (64, 55, 63),
            (65, 56, 64),
            (66, 57, 65),
            (67, 58, 57),
            (68, 59, 67),
            (61, 60, 68),
            (70, 62, 61),
            (0, 63, 62),
            (1, 64, 0),
            (2, 65, 1),
            (3, 66, 2),
            (4, 67, 66),
            (71, 68, 4),
            (69, 61, 71),
            (5, 69, 12),
            (0, 70, 5),
            (1, 0, 6),
            (8, 1, 7),
            (3, 2, 8),
            (10, 3, 9),
            (71, 4, 10),
            (69, 71, 11),
            (5, 12, 20),
            (14, 5, 13),
            (7, 6, 14),
            (16, 7, 15),
            (9, 8, 16),
            (18, 9, 17),
            (19, 10, 18),
            (20, 11, 19),
            (21, 20, 28),
            (14, 13, 21),
            (15, 14, 22),
            (24, 15, 23),
            (17, 16, 24),
            (26, 17, 25),
            (27, 18, 26),
            (20, 19, 27),
            (29, 28, 36),
            (22, 21, 29),
            (31, 22, 30),
            (32, 23, 31),
            (25, 24, 32),
            (34, 25, 33),
            (35, 26, 34),
            (28, 27, 35),
            (29, 36, 44),
            (30, 29, 37),
            (39, 30, 38),
            (32, 31, 39),
            (41, 32, 40),
            (42, 33, 41),
            (43, 34, 42),
            (44, 35, 43),
            (37, 44, 52),
            (46, 37, 45),
            (47, 38, 46),
            (40, 39, 47),
            (49, 40, 48),
            (42, 41, 49),
            (51, 42, 50),
            (52, 43, 51),
            (45, 52, 60),
            (54, 45, 53),
            (47, 46, 54),
            (48, 47, 55),
            (57, 48, 56),
            (58, 49, 57),
            (51, 50, 58),
            (52, 51, 59),
            (62, 60, 61),
            (54, 53, 62),
            (55, 54, 63),
            (56, 55, 64),
            (57, 56, 65),
            (67, 57, 66),
            (59, 58, 67),
            (60, 59, 68),
            (70, 61, 69),
            (0, 62, 70),
            (64, 63, 0),
            (65, 64, 1),
            (66, 65, 2),
            (4, 66, 3),
            (68, 67, 4),
            (61, 68, 71),
        ];

        (faces, vertices)
    }

    #[test]
    fn test_geometry_from_triangle_faces_with_vertices_and_computed_normals() {
        let (faces, vertices) = quad();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );
        let geometry_faces: Vec<_> = geometry.triangle_faces_iter().collect();

        assert_eq!(vertices.as_slice(), geometry.vertices());
        assert_eq!(
            faces,
            geometry_faces
                .into_iter()
                .map(|triangle_face| triangle_face.vertices)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    #[should_panic(expected = "Faces reference out of bounds position data")]
    fn test_geometry_from_triangle_faces_with_vertices_and_computed_normals_bounds_check() {
        let (_, vertices) = quad();
        #[rustfmt::skip]
        let faces = vec![
            (0, 1, 2),
            (2, 3, 4),
        ];

        Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces,
            vertices,
            NormalStrategy::Sharp,
        );
    }

    #[test]
    fn test_geometry_from_triangle_faces_with_vertices_and_normals() {
        let (faces, vertices, normals) = quad_with_normals();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );
        let geometry_faces: Vec<_> = geometry.triangle_faces_iter().collect();

        assert_eq!(vertices.as_slice(), geometry.vertices());
        assert_eq!(normals.as_slice(), geometry.normals());
        assert_eq!(faces.as_slice(), geometry_faces.as_slice());
    }

    #[test]
    #[should_panic(expected = "Faces reference out of bounds position data")]
    fn test_geometry_from_triangle_faces_with_vertices_and_normals_bounds_check() {
        let (_, vertices, normals) = quad_with_normals();
        #[rustfmt::skip]
        let faces = vec![
            TriangleFace::new(0, 1, 2),
            TriangleFace::new(2, 3, 4),
        ];

        Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );
    }

    #[test]
    fn test_oriented_edge_eq_returns_true() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(0, 1);
        assert_eq!(oriented_edge_one_way, oriented_edge_other_way);
    }

    #[test]
    fn test_oriented_edge_eq_returns_false_because_different() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(2, 1);
        assert_ne!(oriented_edge_one_way, oriented_edge_other_way);
    }

    #[test]
    fn test_oriented_edge_eq_returns_false_because_reverted() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(1, 0);
        assert_ne!(oriented_edge_one_way, oriented_edge_other_way);
    }

    #[test]
    #[should_panic(expected = "The oriented edge is constituted of the same vertex")]
    fn test_oriented_edge_constructor_consists_of_the_same_vertex_should_panic() {
        OrientedEdge::new(0, 0);
    }

    #[test]
    fn test_oriented_edge_constructor_doesnnt_consist_of_the_same_vertex_should_pass() {
        OrientedEdge::new(0, 1);
    }

    #[test]
    fn test_oriented_edge_is_reverted_returns_true_because_same() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(1, 0);
        assert!(oriented_edge_one_way.is_reverted(oriented_edge_other_way));
    }

    #[test]
    fn test_oriented_edge_is_reverted_returns_false_because_is_same() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(0, 1);
        assert!(!oriented_edge_one_way.is_reverted(oriented_edge_other_way));
    }

    #[test]
    fn test_oriented_edge_is_reverted_returns_false_because_is_different() {
        let oriented_edge_one_way = OrientedEdge::new(0, 1);
        let oriented_edge_other_way = OrientedEdge::new(2, 1);
        assert!(!oriented_edge_one_way.is_reverted(oriented_edge_other_way));
    }

    #[test]
    fn test_unoriented_edge_eq_returns_true_because_same() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(0, 1));
        assert_eq!(unoriented_edge_one_way, unoriented_edge_other_way);
    }

    #[test]
    fn test_unoriented_edge_eq_returns_true_because_reverted() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(1, 0));
        assert_eq!(unoriented_edge_one_way, unoriented_edge_other_way);
    }

    #[test]
    fn test_unoriented_edge_eq_returns_false_because_different() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(2, 1));
        assert_ne!(unoriented_edge_one_way, unoriented_edge_other_way);
    }

    #[test]
    fn test_unoriented_edge_hash_returns_true_because_same() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let mut hasher_1 = DefaultHasher::new();
        let mut hasher_2 = DefaultHasher::new();
        unoriented_edge_one_way.hash(&mut hasher_1);
        unoriented_edge_other_way.hash(&mut hasher_2);
        assert_eq!(hasher_1.finish(), hasher_2.finish());
    }

    #[test]
    fn test_unoriented_edge_hash_returns_true_because_reverted() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(1, 0));
        let mut hasher_1 = DefaultHasher::new();
        let mut hasher_2 = DefaultHasher::new();
        unoriented_edge_one_way.hash(&mut hasher_1);
        unoriented_edge_other_way.hash(&mut hasher_2);
        assert_eq!(hasher_1.finish(), hasher_2.finish());
    }

    #[test]
    fn test_unoriented_edge_hash_returns_false_because_different() {
        let unoriented_edge_one_way = UnorientedEdge(OrientedEdge::new(0, 1));
        let unoriented_edge_other_way = UnorientedEdge(OrientedEdge::new(2, 1));
        let mut hasher_1 = DefaultHasher::new();
        let mut hasher_2 = DefaultHasher::new();
        unoriented_edge_one_way.hash(&mut hasher_1);
        unoriented_edge_other_way.hash(&mut hasher_2);
        assert_ne!(hasher_1.finish(), hasher_2.finish());
    }

    #[test]
    fn test_triangle_face_to_oriented_edges() {
        let face = TriangleFace::new(0, 1, 2);

        let oriented_edges_correct: [OrientedEdge; 3] = [
            OrientedEdge::new(0, 1),
            OrientedEdge::new(1, 2),
            OrientedEdge::new(2, 0),
        ];

        let oriented_edges_to_check: [OrientedEdge; 3] = face.to_oriented_edges();

        assert_eq!(oriented_edges_to_check[0], oriented_edges_correct[0]);
        assert_eq!(oriented_edges_to_check[1], oriented_edges_correct[1]);
        assert_eq!(oriented_edges_to_check[2], oriented_edges_correct[2]);
    }

    #[test]
    fn test_triangle_face_to_unoriented_edges() {
        let face = TriangleFace::new(0, 1, 2);

        let unoriented_edges_correct: [UnorientedEdge; 3] = [
            UnorientedEdge(OrientedEdge::new(0, 1)),
            UnorientedEdge(OrientedEdge::new(1, 2)),
            UnorientedEdge(OrientedEdge::new(2, 0)),
        ];

        let unoriented_edges_to_check: [UnorientedEdge; 3] = face.to_unoriented_edges();

        assert_eq!(unoriented_edges_to_check[0], unoriented_edges_correct[0]);
        assert_eq!(unoriented_edges_to_check[1], unoriented_edges_correct[1]);
        assert_eq!(unoriented_edges_to_check[2], unoriented_edges_correct[2]);
    }

    #[test]
    fn test_has_no_orphan_vertices_returns_true_if_there_are_some() {
        let (faces, vertices, normals) = quad_with_normals();

        let geometry_without_orphans = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );

        assert!(geometry_without_orphans.has_no_orphan_vertices());
    }

    #[test]
    fn test_has_no_orphan_vertices_returns_false_if_there_are_none() {
        let (faces, vertices, normals) = quad_with_normals();
        let extra_vertex = vec![v(0.0, 0.0, 0.0, [0.0, 0.0, 0.0], 1.0)];
        let vertices_extended = [&vertices[..], &extra_vertex[..]].concat();

        let geometry_with_orphans = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices_extended.clone(),
            normals.clone(),
        );

        assert!(!geometry_with_orphans.has_no_orphan_vertices());
    }

    #[test]
    fn test_has_no_orphan_normals_returns_true_if_there_are_some() {
        let (faces, vertices, normals) = quad_with_normals();

        let geometry_without_orphans = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );

        assert!(geometry_without_orphans.has_no_orphan_normals());
    }

    #[test]
    fn test_geometry_unoriented_edges_iter() {
        let (faces, vertices, normals) = quad_with_normals();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );
        let unoriented_edges_correct = vec![
            UnorientedEdge(OrientedEdge::new(0, 1)),
            UnorientedEdge(OrientedEdge::new(1, 2)),
            UnorientedEdge(OrientedEdge::new(2, 0)),
            UnorientedEdge(OrientedEdge::new(2, 3)),
            UnorientedEdge(OrientedEdge::new(3, 0)),
            UnorientedEdge(OrientedEdge::new(0, 2)),
        ];
        let unoriented_edges_to_check: Vec<UnorientedEdge> =
            geometry.unoriented_edges_iter().collect();

        assert!(unoriented_edges_to_check
            .iter()
            .all(|u_e| unoriented_edges_correct.iter().any(|e| e == u_e)));

        let len_1 = unoriented_edges_to_check.len();
        let len_2 = unoriented_edges_correct.len();
        assert_eq!(
            len_1, len_2,
            "unoriented_edges_to_check.len() = {}, unoriented_edges_correct.len() = {}",
            len_1, len_2
        );
    }

    #[test]
    fn test_geometry_oriented_edges_iter() {
        let (faces, vertices, normals) = quad_with_normals();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals.clone(),
        );

        let oriented_edges_correct = vec![
            OrientedEdge::new(0, 1),
            OrientedEdge::new(1, 2),
            OrientedEdge::new(2, 0),
            OrientedEdge::new(2, 3),
            OrientedEdge::new(3, 0),
            OrientedEdge::new(0, 2),
        ];
        let oriented_edges_to_check: Vec<OrientedEdge> = geometry.oriented_edges_iter().collect();

        assert!(oriented_edges_to_check
            .iter()
            .all(|o_e| oriented_edges_correct.iter().any(|e| e == o_e)));

        let len_1 = oriented_edges_to_check.len();
        let len_2 = oriented_edges_correct.len();

        assert_eq!(
            len_1, len_2,
            "oriented_edges_to_check.len() = {}, oriented_edges_correct.len() = {}",
            len_1, len_2
        );
    }

    #[test]
    fn test_has_no_orphan_normals_returns_false_if_there_are_none() {
        let (faces, vertices, normals) = quad_with_normals();
        let extra_normal = vec![n(0.0, 0.0, 0.0)];
        let normals_extended = [&normals[..], &extra_normal[..]].concat();

        let geometry_with_orphans = Geometry::from_triangle_faces_with_vertices_and_normals(
            faces.clone(),
            vertices.clone(),
            normals_extended.clone(),
        );

        assert!(!geometry_with_orphans.has_no_orphan_normals());
    }

    #[test]
    fn test_geometry_mesh_genus_box_should_be_0() {
        let geometry = cube_sharp_var_len([0.0, 0.0, 0.0], 1.0);
        let edges: HashSet<UnorientedEdge> = geometry.unoriented_edges_iter().collect();

        let genus = geometry.mesh_genus(&edges);
        assert_eq!(genus, 0);
    }

    #[test]
    fn test_geometry_mesh_genus_torus_should_be_1() {
        let (faces, vertices) = torus();
        let geometry = Geometry::from_triangle_faces_with_vertices_and_computed_normals(
            faces.clone(),
            vertices.clone(),
            NormalStrategy::Sharp,
        );
        let edges: HashSet<UnorientedEdge> = geometry.unoriented_edges_iter().collect();

        let genus = geometry.mesh_genus(&edges);
        assert_eq!(genus, 1);
    }

}
