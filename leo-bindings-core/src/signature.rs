use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct InitialJson {
    imports: Option<HashMap<String, serde_json::Value>>,
    program_scopes: HashMap<String, ProgramScope>,
}

#[derive(Debug, Deserialize)]
struct ProgramScope {
    structs: Vec<(String, StructDef)>,
    mappings: Vec<(String, MappingDef)>,
    functions: Vec<(String, FunctionDef)>,
}

#[derive(Debug, Deserialize)]
struct Identifier {
    name: String,
}

#[derive(Debug, Deserialize)]
struct StructDef {
    identifier: Identifier,
    members: Vec<StructMember>,
    is_record: bool,
}

#[derive(Debug, Deserialize)]
struct StructMember {
    identifier: Identifier,
    #[serde(rename = "type_")]
    type_info: TypeInfo,
    mode: String, // "None", "Public", "Private", "Constant"
}

#[derive(Debug, Deserialize)]
struct FunctionDef {
    identifier: Identifier,
    variant: String,
    input: Vec<Parameter>,
    output: Vec<OutputParameter>,
}

#[derive(Debug, Deserialize)]
struct Parameter {
    identifier: Identifier,
    #[serde(rename = "type_")]
    type_info: TypeInfo,
    mode: String,
}

#[derive(Debug, Deserialize)]
struct OutputParameter {
    #[serde(rename = "type_")]
    type_info: TypeInfo,
    mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TypeInfo {
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
        #[allow(dead_code)]
        future: FutureType,
    },
    Tuple {
        #[serde(rename = "Tuple")]
        tuple: TupleType,
    },
}

#[derive(Debug, Deserialize)]
struct CompositeType {
    path: PathType,
}

#[derive(Debug, Deserialize)]
struct PathType {
    identifier: Identifier,
}

#[derive(Debug, Deserialize)]
struct ArrayType {
    element_type: Box<TypeInfo>,
    length: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct MappingDef {
    identifier: Identifier,
    key_type: TypeInfo,
    value_type: TypeInfo,
}

#[derive(Debug, Deserialize)]
struct FutureType {
    #[allow(dead_code)]
    inputs: Vec<serde_json::Value>,
    #[allow(dead_code)]
    location: Option<serde_json::Value>,
    #[allow(dead_code)]
    is_explicit: bool,
}

#[derive(Debug, Deserialize)]
struct TupleType {
    elements: Vec<TypeInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimplifiedBindings {
    pub program_name: String,
    pub imports: Vec<String>,
    pub records: Vec<StructBinding>,
    pub structs: Vec<StructBinding>,
    pub mappings: Vec<MappingBinding>,
    pub functions: Vec<FunctionBinding>,
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
    pub is_async: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructBinding {
    pub name: String,
    pub members: Vec<MemberDef>,
    pub is_record: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MappingBinding {
    pub name: String,
    pub key_type: String,
    pub value_type: String,
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

pub fn get_signatures(input: String) -> String {
    let initial_json: InitialJson = serde_json::from_str(&input).unwrap();

    let imports: Vec<String> = initial_json
        .imports
        .as_ref()
        .map(|imports_map| imports_map.keys().cloned().collect())
        .unwrap_or_default();

    let (program_name, program_scope) = initial_json.program_scopes.into_iter().next().unwrap();

    let (records, structs): (Vec<StructBinding>, Vec<StructBinding>) = program_scope
        .structs
        .into_iter()
        .map(|(_, struct_def)| {
            let members = struct_def
                .members
                .into_iter()
                .map(|member| MemberDef {
                    name: member.identifier.name,
                    type_name: normalize_type(&member.type_info),
                    mode: member.mode,
                })
                .collect();

            StructBinding {
                name: struct_def.identifier.name,
                members,
                is_record: struct_def.is_record,
            }
        })
        .partition(|binding| binding.is_record);

    let functions: Vec<FunctionBinding> = program_scope
        .functions
        .into_iter()
        .filter_map(|(_, func_def)| {
            if func_def.variant == "Transition" || func_def.variant == "AsyncTransition" {
                let inputs = func_def
                    .input
                    .into_iter()
                    .map(|param| InputParam {
                        name: param.identifier.name,
                        type_name: normalize_type(&param.type_info),
                        mode: param.mode,
                    })
                    .collect();

                let outputs: Vec<OutputType> = func_def
                    .output
                    .into_iter()
                    .map(|output| OutputType {
                        type_name: normalize_type(&output.type_info),
                        mode: output.mode,
                    })
                    .collect();

                let is_async = func_def.variant == "AsyncTransition";

                if is_async {
                    if outputs.is_empty() {
                        panic!("Async function '{}' must have at least a Future output", func_def.identifier.name);
                    }
                    let last_output = &outputs[outputs.len() - 1];
                    if last_output.type_name != "Future" {
                        panic!("Async function '{}' must have Future as the last output, but found '{}'", func_def.identifier.name, last_output.type_name);
                    }
                }

                Some(FunctionBinding {
                    name: func_def.identifier.name,
                    inputs,
                    outputs,
                    is_async,
                })
            } else {
                None
            }
        })
        .collect();

    let mappings: Vec<MappingBinding> = program_scope
        .mappings
        .into_iter()
        .map(|(_, mapping_def)| MappingBinding {
            name: mapping_def.identifier.name,
            key_type: normalize_type(&mapping_def.key_type),
            value_type: normalize_type(&mapping_def.value_type),
        })
        .collect();

    let simplified = SimplifiedBindings {
        program_name,
        imports,
        records,
        structs,
        mappings,
        functions,
    };

    serde_json::to_string_pretty(&simplified).unwrap()
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
        TypeInfo::Composite { composite: comp } => comp.path.identifier.name.clone(),
        TypeInfo::Array { array } => {
            let element_type = normalize_type(&array.element_type);
            let size = extract_array_size(&array.length);
            format!("[{}; {}]", element_type, size)
        }
        TypeInfo::Future { .. } => "Future".to_string(),
        TypeInfo::Tuple { tuple } => {
            let element_types: Vec<String> = tuple.elements.iter().map(normalize_type).collect();
            format!("({})", element_types.join(", "))
        }
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
                            let size = size_str
                                .trim_end_matches("u8")
                                .trim_end_matches("u16")
                                .trim_end_matches("u32")
                                .trim_end_matches("u64")
                                .trim_end_matches("u128")
                                .trim_end_matches("i8")
                                .trim_end_matches("i16")
                                .trim_end_matches("i32")
                                .trim_end_matches("i64")
                                .trim_end_matches("i128");
                            return size.to_string();
                        }
                    }
                }
            } else if let Some(unsuffixed_str) = variant.get("Unsuffixed") {
                if let Some(size_str) = unsuffixed_str.as_str() {
                    return size_str.to_string();
                }
            }
        }
    }
    "UNKNOWN_SIZE".to_string()
}
