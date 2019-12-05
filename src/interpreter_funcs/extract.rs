use std::error;
use std::fmt;

use crate::interpreter::{
    Func, FuncError, FuncFlags, FuncInfo, ParamInfo, ParamRefinement, Ty, UintParamRefinement,
    Value,
};

#[derive(Debug, PartialEq)]
pub enum FuncExtractError {
    Empty,
}

impl fmt::Display for FuncExtractError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "No mesh geometry contained in group"),
        }
    }
}

impl error::Error for FuncExtractError {}

pub struct FuncExtract;

impl Func for FuncExtract {
    fn info(&self) -> &FuncInfo {
        &FuncInfo {
            name: "Extract",
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
                refinement: ParamRefinement::GeometryArray,
                optional: false,
            },
            ParamInfo {
                name: "Index",
                refinement: ParamRefinement::Uint(UintParamRefinement {
                    default_value: Some(0),
                    min_value: Some(0),
                    max_value: None,
                }),
                optional: false,
            },
        ]
    }

    fn return_ty(&self) -> Ty {
        Ty::Geometry
    }

    fn call(&mut self, values: &[Value]) -> Result<Value, FuncError> {
        let geometry_array = values[0].unwrap_geometry_array();
        let index = values[1].unwrap_uint();

        if geometry_array.is_empty() {
            return Err(FuncError::new(FuncExtractError::Empty));
        }

        // FIXME: @Correctness The wrapping index is just a temporary
        // crutch until we can report errors to the user.
        let wrapping_index = index % geometry_array.len();
        let value = geometry_array
            .get_refcounted(wrapping_index)
            .expect("Array must not be empty");

        Ok(Value::Geometry(value))
    }
}
