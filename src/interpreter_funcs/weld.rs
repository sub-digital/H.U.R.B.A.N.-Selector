use std::sync::Arc;

use crate::interpreter::{
    FloatParamRefinement, Func, FuncError, FuncFlags, FuncInfo, ParamInfo, ParamRefinement, Ty,
    Value,
};
use crate::mesh_tools;

pub struct FuncWeld;

impl Func for FuncWeld {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Weld",
            return_value_name: "Welded Mesh",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::empty()
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Mesh",
                refinement: ParamRefinement::Geometry,
                optional: false,
            },
            ParamInfo {
                name: "Tolerance",
                refinement: ParamRefinement::Float(FloatParamRefinement {
                    default_value: Some(1.0),
                    min_value: Some(0.0),
                    max_value: None,
                }),
                optional: false,
            },
        ]
    }

    fn return_ty(&self) -> Ty {
        Ty::Geometry
    }

    fn call(&mut self, args: &[Value]) -> Result<Value, FuncError> {
        let geometry = args[0].unwrap_geometry();
        let tolerance = args[1].unwrap_float();

        let value = mesh_tools::weld(geometry, tolerance);
        Ok(Value::Geometry(Arc::new(value)))
    }
}
