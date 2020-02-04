use std::cmp;
use std::sync::Arc;

use crate::interpreter::{
    analytics, BooleanParamRefinement, Func, FuncError, FuncFlags, FuncInfo, LogMessage, ParamInfo,
    ParamRefinement, Ty, UintParamRefinement, Value,
};
use crate::mesh::{smoothing, topology, NormalStrategy};

pub struct FuncLaplacianSmoothing;

impl Func for FuncLaplacianSmoothing {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Relax",
            return_value_name: "Relaxed Mesh",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Mesh",
                refinement: ParamRefinement::Mesh,
                optional: false,
            },
            ParamInfo {
                name: "Iterations",
                refinement: ParamRefinement::Uint(UintParamRefinement {
                    default_value: Some(1),
                    min_value: Some(0),
                    max_value: Some(255),
                }),
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
        let mesh = args[0].unwrap_mesh();
        let iterations = args[1].unwrap_uint();
        let analyze = args[2].unwrap_boolean();

        let v2v = topology::compute_vertex_to_vertex_topology(mesh);

        let (value, _, _) = smoothing::laplacian_smoothing(
            mesh,
            &v2v,
            cmp::min(255, iterations),
            &[],
            false,
            NormalStrategy::Smooth,
        );

        if analyze {
            analytics::report_mesh_analysis(&value)
                .iter()
                .for_each(|line| log(line.clone()));
        }

        Ok(Value::Mesh(Arc::new(value)))
    }
}
