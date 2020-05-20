use std::fs::File;
use std::error::Error;
use std::io::prelude::*;
use std::collections::HashMap;

use log::warn;
use clang::*;

pub use clang::TypeKind;

#[derive(Clone, Debug)]
pub struct FinchType {
  pub display_name: String,
  pub kind: TypeKind,
  pub pointee_type: Option<Box<FinchType>>,
  pub canonical_type: Option<Box<FinchType>>,
  pub sizeof: Option<usize>,
}

impl<'tu> From<Type<'tu>> for FinchType {
  fn from(value: Type) -> Self {
    Self {
      display_name: value.get_display_name(),
      kind: value.get_kind(),
      pointee_type: match value.get_pointee_type() {
        Some(ty) =>  Some(Box::new(FinchType::from(ty))),
        None => None,
      },
      canonical_type: {
        let canonical_type = value.get_canonical_type();
        if canonical_type != value {
          Some(Box::new(FinchType::from(canonical_type)))
        } else {
          None
        }
      },
      sizeof: value.get_sizeof().ok(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchNew {
  pub class_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
  pub arg_names: Vec<String>,
  pub arg_types: Vec<FinchType>,
  pub comments: Option<String>,
}

impl FinchNew {
  fn new(class_name: String, fn_name: String, c_fn_name: String, e: Entity) -> Self {
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();
    for arg in e.get_arguments().unwrap() {
      arg_names.push(arg.get_display_name().unwrap());
      arg_types.push(FinchType::from(arg.get_type().unwrap()));
    }

    Self {
      class_name,
      fn_name,
      c_fn_name,
      arg_names,
      arg_types,
      comments: e.get_comment(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchDrop {
  pub class_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
}

#[derive(Clone, Debug)]
pub struct FinchMethod {
  pub class_name: String,
  pub method_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
  pub ret_type: FinchType,
  pub arg_names: Vec<String>,
  pub arg_types: Vec<FinchType>,
  pub comments: Option<String>,
  pub consume: bool,
}

impl FinchMethod {
  fn new(class_name: String, method_name: String, fn_name: String, c_fn_name: String, consume: bool, e: Entity) -> Self {
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();

    let mut args = e.get_arguments().unwrap();
    args.remove(0);
    for arg in args {
      arg_names.push(arg.get_display_name().unwrap());
      arg_types.push(FinchType::from(arg.get_type().unwrap()));
    }

    Self {
      class_name,
      method_name,
      fn_name,
      c_fn_name,
      ret_type: FinchType::from(e.get_result_type().unwrap()),
      arg_names,
      arg_types,
      comments: e.get_comment(),
      consume,
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchStatic {
  pub class_name: String,
  pub method_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
  pub ret_type: FinchType,
  pub arg_names: Vec<String>,
  pub arg_types: Vec<FinchType>,
  pub comments: Option<String>,
}

impl FinchStatic {
  fn new(class_name: String, method_name: String, fn_name: String, c_fn_name: String, e: Entity) -> Self {
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();
    for arg in e.get_arguments().unwrap() {
      arg_names.push(arg.get_display_name().unwrap());
      arg_types.push(FinchType::from(arg.get_type().unwrap()));
    }

    Self {
      class_name,
      method_name,
      fn_name,
      c_fn_name,
      ret_type: FinchType::from(e.get_result_type().unwrap()),
      arg_names,
      arg_types,
      comments: e.get_comment(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchGetter {
  pub class_name: String,
  pub field_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
  pub type_: FinchType,
  pub comments: Option<String>,
}

impl FinchGetter {
  fn new(class_name: String, field_name: String, fn_name: String, c_fn_name: String, e: Entity) -> Self {
    Self {
      class_name,
      field_name,
      fn_name,
      c_fn_name,
      type_: FinchType::from(e.get_result_type().unwrap()),
      comments: e.get_comment(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchSetter {
  pub class_name: String,
  pub field_name: String,
  pub fn_name: String,
  pub c_fn_name: String,
  pub type_: FinchType,
  pub comments: Option<String>,
}

impl FinchSetter {
  fn new(class_name: String, field_name: String, fn_name: String, c_fn_name: String, e: Entity) -> Self {
    Self {
      class_name,
      field_name,
      fn_name,
      c_fn_name,
      type_: FinchType::from(e.get_arguments().unwrap()[1].get_type().unwrap()),
      comments: e.get_comment(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct FinchClass {
  pub name: String,
  pub c_name: String,
  pub comments: Option<String>,
  pub new: Option<FinchNew>,
  pub drop: Option<FinchDrop>,
  pub statics: Vec<FinchStatic>,
  pub methods: Vec<FinchMethod>,
  pub getters: Vec<FinchGetter>,
  pub setters: Vec<FinchSetter>,
}

impl FinchClass {
  fn new(name: String, c_name: String, e: Entity) -> Self {
    Self {
      name,
      c_name,
      comments: e.get_comment(),
      new: None,
      drop: None,
      statics: Vec::new(),
      methods: Vec::new(),
      getters: Vec::new(),
      setters: Vec::new(),
    }
  }
}

struct ParserState {
  in_finch: bool,
  in_internal: bool,
  namespace: Option<String>,
  classes: HashMap<String, FinchClass>
}

fn process_children(state: &mut ParserState, e: Entity) {
  for child in e.get_children() {
    process_entity(state, child);
  }
}

fn process_entity(state: &mut ParserState, e: Entity) {
  match e.get_kind() {
    EntityKind::TranslationUnit => {
      for child in e.get_children() {
        process_entity(state, child);
      }
    }

    EntityKind::Namespace => {
      if !state.in_finch && e.get_display_name().unwrap() == "finch" {
        state.in_finch = true;
        process_children(state, e);
      } else if !state.in_internal && e.get_display_name().unwrap() == "bindgen" {
        state.in_internal = true;
        process_children(state, e);
      } else if state.in_finch && state.in_internal && state.namespace.is_none() {
        state.namespace = Some(e.get_display_name().unwrap());
        process_children(state, e);
      } else if state.in_finch {
        warn!("unknown namespace found '{}'", e.get_display_name().unwrap());
      }
    }

    EntityKind::UnexposedDecl => {
      if state.in_finch && state.in_internal {
        process_children(state, e);
      }
    }

    EntityKind::TypeAliasDecl => {
      if !state.in_finch || !state.in_internal {
        return;
      }

      let ty_name = e.get_name().unwrap();
      if !ty_name.as_str().starts_with("___finch_bindgen") {
        warn!("unknown identifier found '{}'", ty_name);
        return;
      }

      let parts: Vec<&str> = ty_name.as_str().split("___").collect();
      if parts[2] != state.namespace.as_ref().unwrap() {
        warn!("namespace mismatch, expected '{}', got '{}'", state.namespace.as_ref().unwrap(), parts[3]);
        return;
      }

      if parts[3] != "class" {
        warn!("unknown identifier found '{}'", parts[3]);
        return;
      }

      let class_name = parts[4].to_string();
      let _class = state.classes
        .entry(class_name.clone())
        .or_insert(
          FinchClass::new(
            class_name.clone(), 
            format!("finch::bindgen::{}::{}", state.namespace.as_ref().unwrap(), e.get_display_name().unwrap()), 
            e,
          ),
        );
    }

    EntityKind::FunctionDecl => {
      if !state.in_finch || !state.in_internal {
        return;
      }

      let c_fn_name = e.get_name().unwrap();
      if !c_fn_name.as_str().starts_with("___finch_bindgen") {
        warn!("unknown identifier found '{}'", c_fn_name);
        return;
      }

      let parts: Vec<&str> = c_fn_name.as_str().split("___").collect();
      if parts[2] != state.namespace.as_ref().unwrap() {
        warn!("namespace mismatch, expected '{}', got '{}'", state.namespace.as_ref().unwrap(), parts[3]);
        return;
      }

      match parts[3] {
        "class" => {
          let fn_name = format!("finch::bindgen::{}::{}", state.namespace.as_ref().unwrap(), c_fn_name);

          let class_name = parts[4].to_string();
          let class = state.classes.get_mut(&class_name).expect(format!("failed to find class '{}'", class_name).as_str());

          match parts[5] {
            "drop" => {
              class.drop = Some(FinchDrop {
                class_name,
                fn_name: fn_name.to_string(),
                c_fn_name: c_fn_name.to_string(),
              });
            },

            "method" => {
              class.methods.push(FinchMethod::new(class_name, parts[6].to_string(), fn_name.to_string(), c_fn_name.to_string(), false, e));
            }

            "method_consume" => {
              class.methods.push(FinchMethod::new(class_name, parts[6].to_string(), fn_name.to_string(), c_fn_name.to_string(), true, e));
            }
            
            "static" => {
              if parts[6] == "new" {
                class.new = Some(FinchNew::new(class_name, fn_name.to_string(), c_fn_name.to_string(), e));
              } else {
                class.statics.push(FinchStatic::new(class_name, parts[6].to_string(), fn_name.to_string(), c_fn_name.to_string(), e));
              }
            },

            "getter" => {
              class.getters.push(FinchGetter::new(class_name, parts[6].to_string(), fn_name.to_string(), c_fn_name.to_string(), e));
            }

            "setter" => {
              class.setters.push(FinchSetter::new(class_name, parts[6].to_string(), fn_name.to_string(), c_fn_name.to_string(), e));
            }

            x => {
              warn!("unknown identifier found '{}'", x)
            },
          }
        },

        x => {
          warn!("unknown identifier found '{}'", x)
        },
      }

      println!("{:?}", parts);
    }

    _ => {},
  }
}

#[derive(Clone, Debug)]
pub struct FinchError(&'static str);

impl std::fmt::Display for FinchError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl Error for FinchError {
  fn description(&self) -> &str {
    self.0
  }

  fn cause(&self) -> Option<&(dyn Error)> {
    None
  }
}

#[derive(Clone, Debug)]
pub struct FinchOutput {
  pub classes: HashMap<String, FinchClass>,
}

fn get_package_name_from_cargo_toml() -> Result<String, Box<dyn Error>> {
  let mut cargo_toml_file = File::open("Cargo.toml")?;
  let mut cargo_toml = String::new();
  cargo_toml_file.read_to_string(&mut cargo_toml)?;
  let cargo_toml = cargo_toml.parse::<toml::Value>()?;

  let cargo_table;
  if let toml::Value::Table(table) = cargo_toml {
    cargo_table = table;
  } else {
    return Err(Box::new(FinchError("Cargo.toml does not have root table element")));
  }

  let package_value = cargo_table.get("package").ok_or(Box::new(FinchError("Cargo.toml does not have [package] table")))?;
  let package;
  if let toml::Value::Table(package_table) = package_value {
    package = package_table;
  } else {
    return Err(Box::new(FinchError("Cargo.toml does not have [package] table")));
  }

  let name_value = package.get("name").ok_or(Box::new(FinchError("Cargo.toml does not have package name string")))?;
  if let toml::Value::String(name) = name_value {
    Ok(name.to_string())
  } else {
    Err(Box::new(FinchError("Cargo.toml does not have package name string")))
  }
}

pub fn get_package_name(cli: bool) -> Result<String, Box<dyn Error>> {
  if cli {
    get_package_name_from_cargo_toml()
  } else if let Ok(name) = std::env::var("CARGO_PKG_NAME") {
    Ok(name)
  } else {
    get_package_name_from_cargo_toml()
  }
}

pub fn generate(cli: bool) -> Result<FinchOutput, Box<dyn Error>> {
  let name = get_package_name(cli)?;
  let name_underscore = name.replace("-", "_");

  let header_name = format!("{}-finch_bindgen.h", name_underscore);

  cbindgen::Builder::new()
    .with_namespaces(&vec!["finch", "bindgen", &name_underscore])
    .with_parse_expand(&vec![name])
    .with_parse_deps(true)
    .with_parse_include(&vec!["finch-gen"])
    .with_crate(std::env::current_dir().unwrap())
    .generate()?.write_to_file(&header_name);

  let clang = Clang::new().unwrap();

  let index = Index::new(&clang, false, false);
  
  let args = vec!["-std=c++11"];
  let tu = index.parser(header_name).arguments(&args).parse().unwrap();
  let entity = tu.get_entity();

  let mut state = ParserState {
    in_finch: false,
    in_internal: false,
    namespace: None,
    classes: HashMap::new(),
  };

  process_entity(&mut state, entity);

  Ok(FinchOutput {
    classes: state.classes,
  })
}

fn uppercase_first(s: &str) -> String {
  let mut c = s.chars();
  match c.next() {
    None => String::new(),
    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
  }
}

pub fn to_camel_case(s: &str) -> String {
  let mut i = 0;
  s.split("_").map(|x| {
    i += 1;
    if i != 1 {
      uppercase_first(x)
    } else {
      x.to_string()
    }
  }).collect::<String>()
}

pub fn to_pascal_case(s: &str) -> String {
  s.split("_").map(|x| {
    uppercase_first(x)
  }).collect::<String>()
}
