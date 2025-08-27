use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct InitialJson {
    pub program_scopes: HashMap<String, ProgramScope>,
}

#[derive(Debug, Deserialize)]
pub struct ProgramScope {
    pub program_id: ProgramId,
    pub structs: Vec<(String, StructDef)>,
    pub functions: Vec<(String, FunctionDef)>,
}

#[derive(Debug, Deserialize)]
pub struct ProgramId {
    pub name: Identifier,
}

#[derive(Debug, Deserialize)]
pub struct Identifier {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct StructDef {
    pub identifier: Identifier,
    pub members: Vec<StructMember>,
    pub is_record: bool,
}

#[derive(Debug, Deserialize)]
pub struct StructMember {
    pub identifier: Identifier,
    #[serde(rename = "type_")]
    pub type_info: TypeInfo,
    pub mode: String, // "None", "Public", "Private", "Constant"
}

#[derive(Debug, Deserialize)]
pub struct FunctionDef {
    pub identifier: Identifier,
    pub variant: String,
    pub input: Vec<Parameter>,
    pub output: Vec<OutputParameter>,
}

#[derive(Debug, Deserialize)]
pub struct Parameter {
    pub identifier: Identifier,
    #[serde(rename = "type_")]
    pub type_info: TypeInfo,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct OutputParameter {
    #[serde(rename = "type_")]
    pub type_info: TypeInfo,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TypeInfo {
    Simple(String),
    Integer {
        #[serde(rename = "Integer")]
        integer: String,
    },
    Composite {
        #[serde(rename = "Composite")]
        composite: CompositeType,
    },
    Array {
        #[serde(rename = "Array")]
        array: ArrayType,
    },
    Future {
        #[serde(rename = "Future")]
        future: FutureType,
    },
}

#[derive(Debug, Deserialize)]
pub struct CompositeType {
    pub id: Identifier,
}

#[derive(Debug, Deserialize)]
pub struct ArrayType {
    pub element_type: Box<TypeInfo>,
    pub length: serde_json::Value, // Store raw JSON for now, parse size in normalize_type
}

#[derive(Debug, Deserialize)]
pub struct FutureType {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimplifiedBindings {
    pub program_name: String,
    pub records: Vec<RecordDef>,
    pub structs: Vec<RecordDef>,
    pub functions: Vec<FunctionBinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordDef {
    pub name: String,
    pub members: Vec<MemberDef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionBinding {
    pub name: String,
    pub inputs: Vec<InputParam>,
    pub outputs: Vec<OutputType>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputParam {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputType {
    #[serde(rename = "type")]
    pub type_name: String,
    pub mode: String,
}

pub fn get_signatures(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    let initial_json: InitialJson = serde_json::from_str(input)?;

    let (program_name, program_scope) = initial_json
        .program_scopes
        .into_iter()
        .next()
        .ok_or("No program scope found")?;

    let structs_data = program_scope.structs;

    let records: Vec<RecordDef> = structs_data
        .iter()
        .filter_map(|(_, struct_def)| {
            if struct_def.is_record {
                let members = struct_def
                    .members
                    .iter()
                    .map(|member| MemberDef {
                        name: member.identifier.name.clone(),
                        type_name: normalize_type(&member.type_info),
                        mode: if member.mode == "Public" {
                            "Public".to_string()
                        } else {
                            "Private".to_string()
                        },
                    })
                    .collect();

                Some(RecordDef {
                    name: struct_def.identifier.name.clone(),
                    members,
                })
            } else {
                None
            }
        })
        .collect();

    let structs: Vec<RecordDef> = structs_data
        .into_iter()
        .filter_map(|(_, struct_def)| {
            if !struct_def.is_record {
                let members = struct_def
                    .members
                    .into_iter()
                    .map(|member| MemberDef {
                        name: member.identifier.name,
                        type_name: normalize_type(&member.type_info),
                        mode: if member.mode == "Public" {
                            "Public".to_string()
                        } else {
                            "Private".to_string()
                        },
                    })
                    .collect();

                Some(RecordDef {
                    name: struct_def.identifier.name,
                    members,
                })
            } else {
                None
            }
        })
        .collect();

    let functions: Vec<FunctionBinding> = program_scope
        .functions
        .into_iter()
        .filter_map(|(_, func_def)| {
            if func_def.variant == "Transition" {
                let inputs = func_def
                    .input
                    .into_iter()
                    .map(|param| InputParam {
                        name: param.identifier.name,
                        type_name: normalize_type(&param.type_info),
                        mode: if param.mode == "Public" {
                            "Public".to_string()
                        } else {
                            "Private".to_string()
                        },
                    })
                    .collect();

                let outputs = func_def
                    .output
                    .into_iter()
                    .map(|output| OutputType {
                        type_name: normalize_type(&output.type_info),
                        mode: if output.mode == "Public" {
                            "Public".to_string()
                        } else {
                            "Private".to_string()
                        },
                    })
                    .collect();

                Some(FunctionBinding {
                    name: func_def.identifier.name,
                    inputs,
                    outputs,
                })
            } else {
                None
            }
        })
        .collect();

    let simplified = SimplifiedBindings {
        program_name,
        records,
        structs,
        functions,
    };

    serde_json::to_string_pretty(&simplified).map_err(|e| e.into())
}

fn normalize_type(type_info: &TypeInfo) -> String {
    match type_info {
        TypeInfo::Simple(s) => s.clone(),
        TypeInfo::Integer { integer: int_type } => match int_type.as_str() {
            "U8" => "u8".to_string(),
            "U16" => "u16".to_string(),
            "U32" => "u32".to_string(),
            "U64" => "u64".to_string(),
            "U128" => "u128".to_string(),
            "I8" => "i8".to_string(),
            "I16" => "i16".to_string(),
            "I32" => "i32".to_string(),
            "I64" => "i64".to_string(),
            "I128" => "i128".to_string(),
            _ => format!("Unknown_Integer_{}", int_type),
        },
        TypeInfo::Composite { composite: comp } => comp.id.name.clone(),
        TypeInfo::Array { array } => {
            let element_type = normalize_type(&array.element_type);
            let size = extract_array_size(&array.length);
            format!("[{}; {}]", element_type, size)
        }
        TypeInfo::Future { .. } => "Future".to_string(),
    }
}

fn extract_array_size(length_json: &serde_json::Value) -> String {
    // Extract size from JSON structure like:
    // "length": {
    //   "Literal": {
    //     "id": 63,
    //     "variant": {
    //       "Integer": ["U8", "5"]
    //     }
    //   }
    // }

    if let Some(literal) = length_json.get("Literal") {
        if let Some(variant) = literal.get("variant") {
            if let Some(integer_array) = variant.get("Integer") {
                if let Some(array) = integer_array.as_array() {
                    if array.len() == 2 {
                        if let Some(size_str) = array[1].as_str() {
                            return size_str.to_string();
                        }
                    }
                }
            }
        }
    }

    // Fallback - if we can't parse, return a placeholder
    "UNKNOWN_SIZE".to_string()
}
