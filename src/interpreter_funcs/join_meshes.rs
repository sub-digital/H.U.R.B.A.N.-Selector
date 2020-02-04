use std::sync::Arc;

use crate::interpreter::{
    analytics, BooleanParamRefinement, Func, FuncError, FuncFlags, FuncInfo, LogMessage, ParamInfo,
    ParamRefinement, Ty, Value,
};
use crate::mesh::tools;

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
                refinement: ParamRefinement::Mesh,
                optional: false,
            },
            ParamInfo {
                name: "Mesh 2",
                refinement: ParamRefinement::Mesh,
                optional: false,
            },
            ParamInfo {
                name: "Analyze resulting mesh",
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
        let meshes = vec![args[0].unwrap_mesh(), args[1].unwrap_mesh()];
        let analyze = args[2].unwrap_boolean();

        let value = tools::join_multiple_meshes(meshes);

        if analyze {
            analytics::report_mesh_analysis(&value)
                .iter()
                .for_each(|line| log(line.clone()));
        }

        Ok(Value::Mesh(Arc::new(value)))
    }
}
