use std::error;
use std::f32;
use std::fmt;
use std::ops::Bound;
use std::sync::Arc;

use nalgebra::Vector3;

use crate::analytics;
use crate::bounding_box::BoundingBox;
use crate::convert::clamp_cast_u32_to_i16;
use crate::interpreter::{
    BooleanParamRefinement, Float3ParamRefinement, Func, FuncError, FuncFlags, FuncInfo,
    LogMessage, ParamInfo, ParamRefinement, Ty, UintParamRefinement, Value,
};
use crate::mesh::scalar_field::ScalarField;

const VOXEL_COUNT_THRESHOLD: u32 = 50000;

#[derive(Debug, PartialEq)]
pub enum FuncBooleanUnionError {
    WeldFailed,
    EmptyScalarField,
    VoxelDimensionsZero,
    TooManyVoxels(u32, f32, f32, f32),
}

impl fmt::Display for FuncBooleanUnionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FuncBooleanUnionError::WeldFailed => write!(
                f,
                "Welding of separate voxels failed due to high welding proximity tolerance"
            ),
            FuncBooleanUnionError::EmptyScalarField => write!(
                f,
                "Scalar field from input meshes or the resulting mesh is empty"
            ),
            FuncBooleanUnionError::VoxelDimensionsZero => {
                write!(f, "One or more voxel dimensions are zero")
            },
            FuncBooleanUnionError::TooManyVoxels(max_count, x, y, z) => write!(
                f,
                "Too many voxels. Limit set to {}. Try setting voxel size to [{:.3}, {:.3}, {:.3}] or more.",
                max_count, x, y, z
            ),
        }
    }
}

impl error::Error for FuncBooleanUnionError {}

pub struct FuncBooleanUnion;

impl Func for FuncBooleanUnion {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Union",
            description: "BOOLEAN UNION OF VOXEL CLOUDS FROM TWO MESH GEOMETRIES\n\
            \n\
            Converts the input mesh geometries into voxel clouds, then performs \
            boolean union of the first to second voxel clouds and eventually \
            materializes the resulting voxel cloud into a welded mesh. \
            Boolean union joins the volumes of the input mesh geometries \
            (does not remove any volume). It is equivalent to logical OR operation.\n\
            \n\
            Voxels are three-dimensional pixels. They exist in a regular three-dimensional \
            grid of arbitrary dimensions (voxel size). The voxel can be turned on \
            (be a volume) or off (be a void). The voxels can be materialized as \
            rectangular blocks. Voxelized meshes can be effectively smoothened by \
            Laplacian relaxation.
            \n\
            The input meshes will be marked used and thus invisible in the viewport. \
            They can still be used in subsequent operations.\n\
            \n\
            The resulting mesh geometry will be named 'Union Mesh'.",
            return_value_name: "Union Mesh",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Mesh 1",
                description: "First input mesh.",
                refinement: ParamRefinement::Mesh,
                optional: false,
            },
            ParamInfo {
                name: "Mesh 2",
                description: "Second input mesh.",
                refinement: ParamRefinement::Mesh,
                optional: false,
            },
            ParamInfo {
                name: "Voxel Size",
                description: "Size of a single cell in the regular three-dimensional voxel grid.\n\
                High values produce coarser results, low values may increase precision but produce \
                heavier geometry that significantly affect performance. Too high values produce \
                single large voxel, too low values may generate holes in the resulting geometry.",
                refinement: ParamRefinement::Float3(Float3ParamRefinement {
                    min_value: Some(f32::MIN_POSITIVE),
                    max_value: None,
                    default_value_x: Some(1.0),
                    default_value_y: Some(1.0),
                    default_value_z: Some(1.0),
                }),
                optional: false,
            },
            ParamInfo {
                name: "Grow",
                description: "The voxelization algorithm puts voxels on the surface of \
                the input mesh geometries.\n\
                \n\
                The grow option adds several extra layers of voxels on both sides of such \
                voxel volumes. This option generates thicker voxelized meshes. \
                In some cases not growing the volume at all may result in \
                a non manifold voxelized mesh.",
                refinement: ParamRefinement::Uint(UintParamRefinement {
                    default_value: Some(0),
                    min_value: None,
                    max_value: None,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Fill Closed Volumes",
                description: "Treats the insides of watertight mesh geometries as volumes.\n\
                If this option is off, the resulting voxelized mesh geometries will have two \
                separate mesh shells: one for outer surface, the other for inner surface of \
                hollow watertight mesh.",
                refinement: ParamRefinement::Boolean(BooleanParamRefinement {
                    default_value: true,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Prevent Unsafe Settings",
                description: "Stop computation and throw error if the calculation may be too slow.",
                refinement: ParamRefinement::Boolean(BooleanParamRefinement {
                    default_value: true,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Bounding Box Analysis",
                description: "Reports basic and quick analytic information on the created mesh.",
                refinement: ParamRefinement::Boolean(BooleanParamRefinement {
                    default_value: true,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Detailed Mesh Analysis",
                description: "Reports detailed analytic information on the created mesh.\n\
                              The analysis may be slow, therefore it is by default off.",
                refinement: ParamRefinement::Boolean(BooleanParamRefinement {
                    default_value: false,
                }),
                optional: false,
            },
        ]
    }

    fn return_ty(&self) -> Ty {
        Ty::Mesh
    }

    fn call(
        &mut self,
        args: &[Value],
        log: &mut dyn FnMut(LogMessage),
    ) -> Result<Value, FuncError> {
        let mesh1 = args[0].unwrap_mesh();
        let mesh2 = args[1].unwrap_mesh();
        let voxel_dimensions = args[2].unwrap_float3();
        let growth_u32 = args[3].unwrap_uint();
        let growth_i16 = clamp_cast_u32_to_i16(growth_u32);
        let fill = args[4].unwrap_boolean();
        let error_if_large = args[5].unwrap_boolean();
        let analyze_bbox = args[6].unwrap_boolean();
        let analyze_mesh = args[7].unwrap_boolean();

        let bbox1 = mesh1.bounding_box();
        let bbox2 = mesh2.bounding_box();
        let bbox =
            BoundingBox::union([bbox1, bbox2].iter().copied()).expect("Failed to create union box");
        let bbox_diagonal = bbox.diagonal();
        let voxel_count = (bbox_diagonal.x / voxel_dimensions[0]).ceil() as u32
            * (bbox_diagonal.y / voxel_dimensions[1]).ceil() as u32
            * (bbox_diagonal.z / voxel_dimensions[2]).ceil() as u32;

        log(LogMessage::info(format!("Voxel count = {}", voxel_count)));

        if error_if_large && voxel_count > VOXEL_COUNT_THRESHOLD {
            let vy_over_vx = voxel_dimensions[1] / voxel_dimensions[0];
            let vz_over_vx = voxel_dimensions[2] / voxel_dimensions[0];
            let vx = ((bbox_diagonal.x * bbox_diagonal.y * bbox_diagonal.z)
                / (VOXEL_COUNT_THRESHOLD as f32 * vy_over_vx * vz_over_vx))
                .cbrt();
            let vy = vx * vy_over_vx;
            let vz = vx * vz_over_vx;

            // The equation doesn't take rounding into consideration, hence the
            // arbitrary multiplication by 1.1.
            let error = FuncError::new(FuncBooleanUnionError::TooManyVoxels(
                VOXEL_COUNT_THRESHOLD,
                vx * 1.1,
                vy * 1.1,
                vz * 1.1,
            ));
            log(LogMessage::error(format!("Error: {}", error)));
            return Err(error);
        }

        if voxel_dimensions
            .iter()
            .any(|dimension| approx::relative_eq!(*dimension, 0.0))
        {
            let error = FuncError::new(FuncBooleanUnionError::VoxelDimensionsZero);
            log(LogMessage::error(format!("Error: {}", error)));
            return Err(error);
        }

        let mut scalar_field1 =
            ScalarField::from_mesh(mesh1, &Vector3::from(voxel_dimensions), 0_i16, growth_u32);
        let mut scalar_field2 =
            ScalarField::from_mesh(mesh2, &Vector3::from(voxel_dimensions), 0_i16, growth_u32);

        scalar_field1.compute_distance_filed(&(0..=0));
        scalar_field2.compute_distance_filed(&(0..=0));

        let boolean_union_range = &(-growth_i16..=growth_i16);

        scalar_field1.boolean_union(boolean_union_range, &scalar_field2, boolean_union_range);

        let meshing_range = if fill {
            (Bound::Unbounded, Bound::Included(growth_i16))
        } else {
            (Bound::Included(-growth_i16), Bound::Included(growth_i16))
        };

        if !scalar_field1.contains_voxels_within_range(&meshing_range) {
            let error = FuncError::new(FuncBooleanUnionError::EmptyScalarField);
            log(LogMessage::error(format!("Error: {}", error)));
            return Err(error);
        }

        match scalar_field1.to_mesh(&meshing_range) {
            Some(value) => {
                if analyze_bbox {
                    analytics::report_bounding_box_analysis(&value, log);
                }
                if analyze_mesh {
                    analytics::report_mesh_analysis(&value, log);
                }
                Ok(Value::Mesh(Arc::new(value)))
            }
            None => {
                let error = FuncError::new(FuncBooleanUnionError::WeldFailed);
                log(LogMessage::error(format!("Error: {}", error)));
                Err(error)
            }
        }
    }
}
