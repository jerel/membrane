use serde_reflection::{ContainerFormat, Format, Named, VariantFormat};
use std::process::exit;
use tracing::error;

impl crate::Membrane {
  pub fn with_child_borrows(&self, from_namespace: &str, r#type: &str) -> Vec<String> {
    let namespace_registry = match self.namespaced_registry.get(from_namespace) {
      Some(Ok(registry)) => registry,
      _ => {
        error!(
          "`{ns}::{type}` was borrowed but the namespace `{ns}` doesn't exist.",
          r#type = r#type,
          ns = from_namespace
        );

        // TODO, make nicer error handling
        exit(1);
      }
    };

    let mut children = match namespace_registry.get(r#type) {
      Some(ContainerFormat::Struct(named)) => named
        .iter()
        .filter_map(filter_named)
        .flatten()
        .flat_map(|x| self.with_child_borrows(from_namespace, &x))
        .collect::<Vec<String>>(),
      Some(ContainerFormat::Enum(btree)) => {
        let mut variants = btree
          .values()
          .filter_map(|item| match &item.value {
            VariantFormat::Struct(named) => {
              Some(
                named
                  .iter()
                  .filter_map(filter_named)
                  .flatten()
                  .collect::<Vec<String>>(),
              )
            }
            VariantFormat::NewType(format) => match extract_name(format) {
              // if this type matches the parent type then we quit recursing
              Some(names) if !names.contains(&r#type.to_string()) => Some(names.iter().flat_map(|r#type| {
                self.with_child_borrows(from_namespace, r#type)
              }).collect()),
              _ => None
            },
            _ => None,
          })
          .flatten()
          .collect::<Vec<String>>();

        // unless C style enums have been disabled we will import a C style enum's extension
        if self.c_style_enums
          && btree
            .values()
            .all(|item| matches!(&item.value, VariantFormat::Unit))
        {
          variants.extend(vec![format!("{}Extension", r#type)]);
        }

        variants
      }
      Some(ContainerFormat::NewTypeStruct(format)) => {
        match extract_name(format) {
          Some(names) => names.iter().flat_map(|r#type| {
            self.with_child_borrows(from_namespace, r#type)
          }).collect(),
          None => vec![]
        }
      }
      Some(container) => unreachable!(
        "This is a Membrane bug. A type was borrowed that was not handled in the import algorithm: {:?}", container
      ),
      None => {
        error!("Attempted to borrow `{ns}::{type}` but type `{type}` wasn't found in the API exported by public functions", ns = from_namespace, r#type = r#type);
        exit(1);
      }
    };

    children.push(r#type.to_string());
    children
  }
}

fn extract_name(format: &Format) -> Option<Vec<String>> {
  match format {
    Format::TypeName(name) => Some(vec![name.clone()]),
    Format::Option(boxed_name) => extract_name(boxed_name),
    Format::Seq(seq) => extract_name(seq),
    Format::Tuple(formats) => Some(formats.iter().filter_map(extract_name).flatten().collect()),
    _ => None,
  }
}

fn filter_named(item: &Named<Format>) -> Option<Vec<String>> {
  extract_name(&item.value)
}
