use std::sync::Arc;

use nalgebra::{Point3, Rotation3, Vector3};

use crate::analytics;
use crate::interpreter::{
    BooleanParamRefinement, Float3ParamRefinement, Func, FuncError, FuncFlags, FuncInfo,
    LogMessage, ParamInfo, ParamRefinement, Ty, Value,
};
use crate::mesh::primitive;

pub struct FuncCreateBox;

impl Func for FuncCreateBox {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Create Box",
            description: "CREATE MESH BOX\n\
                          \n\
                          Creates a new mesh box made of 12 welded triangles \
                          and 8 vertices. \
                          The default size of the box is 1x1x1 model units.\n\
                          \n\
                          The resulting mesh geometry will be named 'Box'.",
            return_value_name: "Box",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Center",
                description: "Center of the box in absolute model units.",
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
                description: "Rotation of the box in degrees.",
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
                description: "Scale of the box as a relative factor.\n\
                              The original size of the box is 1x1x1 model units.",
                refinement: ParamRefinement::Float3(Float3ParamRefinement {
                    default_value_x: Some(1.0),
                    min_value_x: None,
                    max_value_x: None,
                    default_value_y: Some(1.0),
                    min_value_y: None,
                    max_value_y: None,
                    default_value_z: Some(1.0),
                    min_value_z: None,
                    max_value_z: None,
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
        let center = args[0].unwrap_float3();
        let rotate = args[1].unwrap_float3();
        let scale = args[2].unwrap_float3();
        let analyze = args[3].unwrap_boolean();

        let value = primitive::create_box(
            Point3::from(center),
            Rotation3::from_euler_angles(
                rotate[0].to_radians(),
                rotate[1].to_radians(),
                rotate[2].to_radians(),
            ),
            Vector3::from(scale),
        );

        if analyze {
            analytics::report_mesh_analysis(&value, log);
        }

        Ok(Value::Mesh(Arc::new(value)))
    }
}
