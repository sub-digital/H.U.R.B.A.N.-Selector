use std::error;
use std::f32;
use std::fmt;
use std::sync::Arc;

use nalgebra::Vector3;

use crate::analytics;
use crate::interpreter::{
    BooleanParamRefinement, Float3ParamRefinement, Func, FuncError, FuncFlags, FuncInfo,
    LogMessage, ParamInfo, ParamRefinement, Ty, UintParamRefinement, Value,
};
use crate::mesh::voxel_cloud::VoxelCloud;

#[derive(Debug, PartialEq)]
pub enum FuncBooleanDifferenceError {
    WeldFailed,
    EmptyVoxelCloud,
}

impl fmt::Display for FuncBooleanDifferenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FuncBooleanDifferenceError::WeldFailed => write!(
                f,
                "Welding of separate voxels failed due to high welding proximity tolerance"
            ),
            FuncBooleanDifferenceError::EmptyVoxelCloud => {
                write!(f, "The resulting voxel cloud is empty")
            }
        }
    }
}

impl error::Error for FuncBooleanDifferenceError {}

pub struct FuncBooleanDifference;

impl Func for FuncBooleanDifference {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Difference",
            description: "BOOLEAN DIFFERENCE OF VOXEL CLOUDS FROM TWO MESH GEOMETRIES\n\
            \n\
            Converts the input mesh geometries into voxel clouds, then performs \
            boolean difference of the first to second voxel clouds and eventually \
            materializes the resulting voxel cloud into a welded mesh.\n\
            Boolean difference removes volume of the first geometry where there is \
            a volume in the second geometry (removes the second volume from the first one). \
            It is equivalent to arithmetic subtraction.\n\
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
            The resulting mesh geometry will be named 'Difference Mesh'.",
            return_value_name: "Difference Mesh",
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
                    default_value_x: Some(1.0),
                    min_value_x: Some(f32::MIN_POSITIVE),
                    max_value_x: None,
                    default_value_y: Some(1.0),
                    min_value_y: Some(f32::MIN_POSITIVE),
                    max_value_y: None,
                    default_value_z: Some(1.0),
                    min_value_z: Some(f32::MIN_POSITIVE),
                    max_value_z: None,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Grow",
                description: "The voxelization algorithm puts voxels on the surface of \
                the input mesh geometries.\n\
                The grow option adds several extra layers of voxels on both sides of such \
                voxel volumes. This option generates thicker voxelized meshes.\n\
                In some cases not growing the volume at all may result in \
                a non manifold voxelized mesh.",
                refinement: ParamRefinement::Uint(UintParamRefinement {
                    default_value: Some(1),
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
                name: "Analyze resulting mesh",
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
        let growth_iterations = args[3].unwrap_uint();
        let fill = args[4].unwrap_boolean();
        let analyze = args[5].unwrap_boolean();

        let mut voxel_cloud1 = VoxelCloud::from_mesh(mesh1, &Vector3::from(voxel_dimensions));
        let mut voxel_cloud2 = VoxelCloud::from_mesh(mesh2, &Vector3::from(voxel_dimensions));

        for _ in 0..growth_iterations {
            voxel_cloud1.grow_volume();
            voxel_cloud2.grow_volume();
        }

        if fill {
            voxel_cloud1.fill_volumes();
            voxel_cloud2.fill_volumes();
        }

        voxel_cloud1.boolean_difference(&voxel_cloud2);
        if !voxel_cloud1.contains_voxels() {
            return Err(FuncError::new(FuncBooleanDifferenceError::EmptyVoxelCloud));
        }

        match voxel_cloud1.to_mesh() {
            Some(value) => {
                if analyze {
                    analytics::report_mesh_analysis(&value, log);
                }
                Ok(Value::Mesh(Arc::new(value)))
            }
            None => Err(FuncError::new(FuncBooleanDifferenceError::WeldFailed)),
        }
    }
}
