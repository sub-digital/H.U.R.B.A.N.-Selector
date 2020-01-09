use std::sync::Arc;

use nalgebra::{Point3, Rotation3, Vector2, Vector3};

use crate::interpreter::{
    Float2ParamRefinement, Float3ParamRefinement, Func, FuncError, FuncFlags, FuncInfo, LogMessage,
    ParamInfo, ParamRefinement, Ty, Value,
};
use crate::mesh::primitive;
use crate::plane::Plane;

pub struct FuncCreatePlane;

impl Func for FuncCreatePlane {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Create Plane",
            return_value_name: "Plane",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Center",
                refinement: ParamRefinement::Float3(Float3ParamRefinement {
                    default_value_x: Some(0.0),
                    min_value_x: None,
                    max_value_x: None,
                    default_value_y: Some(0.0),
                    min_value_y: None,
                    max_value_y: None,
                    default_value_z: Some(0.0),
                    min_value_z: None,
                    max_value_z: None,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Rotate (deg)",
                refinement: ParamRefinement::Float3(Float3ParamRefinement {
                    default_value_x: Some(0.0),
                    min_value_x: None,
                    max_value_x: None,
                    default_value_y: Some(0.0),
                    min_value_y: None,
                    max_value_y: None,
                    default_value_z: Some(0.0),
                    min_value_z: None,
                    max_value_z: None,
                }),
                optional: false,
            },
            ParamInfo {
                name: "Scale",
                refinement: ParamRefinement::Float2(Float2ParamRefinement {
                    default_value_x: Some(1.0),
                    min_value_x: Some(0.0),
                    max_value_x: None,
                    default_value_y: Some(1.0),
                    min_value_y: Some(0.0),
                    max_value_y: None,
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
        values: &[Value],
        _log: &mut dyn FnMut(LogMessage),
    ) -> Result<Value, FuncError> {
        let center = values[0].unwrap_float3();
        let rotate = values[1].unwrap_float3();
        let scale = values[2].unwrap_float2();

        let rotation = Rotation3::from_euler_angles(
            rotate[0].to_radians(),
            rotate[1].to_radians(),
            rotate[2].to_radians(),
        );

        let plane = Plane::new(
            &Point3::from_slice(&center),
            &rotation.transform_vector(&Vector3::new(1.0, 0.0, 0.0)),
            &rotation.transform_vector(&Vector3::new(0.0, 1.0, 0.0)),
        );

        let value = primitive::create_mesh_plane(plane, Vector2::from(scale));
        Ok(Value::Mesh(Arc::new(value)))
    }
}
