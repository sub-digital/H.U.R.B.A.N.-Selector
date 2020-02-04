use std::error;
use std::fmt;

use crate::interpreter::{
    analytics, BooleanParamRefinement, Func, FuncError, FuncFlags, FuncInfo, LogMessage, ParamInfo,
    ParamRefinement, Ty, Value,
};

#[derive(Debug, PartialEq)]
pub enum FuncExtractLargestError {
    Empty,
}

impl fmt::Display for FuncExtractLargestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "No mesh geometry contained in group"),
        }
    }
}

impl error::Error for FuncExtractLargestError {}

pub struct FuncExtractLargest;

impl Func for FuncExtractLargest {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Extract Largest",
            return_value_name: "Extracted Mesh",
        }
    }

    fn flags(&self) -> FuncFlags {
        FuncFlags::PURE
    }

    fn param_info(&self) -> &[ParamInfo] {
        &[
            ParamInfo {
                name: "Group",
                refinement: ParamRefinement::MeshArray,
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
        let mesh_array = args[0].unwrap_mesh_array();
        let analyze = args[1].unwrap_boolean();

        if mesh_array.is_empty() {
            return Err(FuncError::new(FuncExtractLargestError::Empty));
        }

        let mut mesh_iter = mesh_array.iter_refcounted();
        let mut mesh = mesh_iter.next().expect("Array must not be empty");
        let mut largest_face_count = mesh.faces().len();

        for current_mesh in mesh_iter {
            let current_face_count = current_mesh.faces().len();
            if current_face_count > largest_face_count {
                largest_face_count = current_face_count;
                mesh = current_mesh;
            }
        }

        if analyze {
            analytics::report_mesh_analysis(&mesh)
                .iter()
                .for_each(|line| log(line.clone()));
        }

        Ok(Value::Mesh(mesh))
    }
}
