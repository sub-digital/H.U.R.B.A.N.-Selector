use std::sync::Arc;

use crate::interpreter::{
    Func, FuncError, FuncFlags, FuncInfo, ParamInfo, ParamRefinement, Ty, Value,
};
use crate::mesh_tools;

pub struct FuncJoinMeshes;

impl Func for FuncJoinMeshes {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Join Meshes",
            return_value_name: "Joined Mesh",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Mesh 1",
                refinement: ParamRefinement::Geometry,
                optional: false,
            },
            ParamInfo {
                name: "Mesh 2",
                refinement: ParamRefinement::Geometry,
                optional: false,
            },
        ]
    }

    fn return_ty(&self) -> Ty {
        Ty::Geometry
    }

    fn call(&mut self, args: &[Value]) -> Result<Value, FuncError> {
        let first_geometry = args[0].unwrap_geometry();
        let second_geometry = args[1].unwrap_geometry();

        let value = mesh_tools::join_meshes(first_geometry, second_geometry);
        Ok(Value::Geometry(Arc::new(value)))
    }
}
