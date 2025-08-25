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
}

#[derive(Debug, Deserialize)]
pub struct OutputParameter {
    #[serde(rename = "type_")]
    pub type_info: TypeInfo,
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
pub struct FutureType {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimplifiedBindings {
    pub program_name: String,
    pub records: Vec<RecordDef>,
    pub functions: Vec<FunctionBinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputType {
    #[serde(rename = "type")]
    pub type_name: String,
}

pub fn get_signatures(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    let initial_json: InitialJson = serde_json::from_str(input)?;

    let (program_name, program_scope) = initial_json
        .program_scopes
        .into_iter()
        .next()
        .ok_or("No program scope found")?;

    let records: Vec<RecordDef> = program_scope
        .structs
        .into_iter()
        .filter_map(|(_, struct_def)| {
            if struct_def.is_record {
                let fields = struct_def
                    .members
                    .into_iter()
                    .map(|member| FieldDef {
                        name: member.identifier.name,
                        type_name: normalize_type(&member.type_info),
                    })
                    .collect();

                Some(RecordDef {
                    name: struct_def.identifier.name,
                    fields,
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
                    })
                    .collect();

                let outputs = func_def
                    .output
                    .into_iter()
                    .map(|output| OutputType {
                        type_name: normalize_type(&output.type_info),
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
        TypeInfo::Future { .. } => "Future".to_string(),
    }
}