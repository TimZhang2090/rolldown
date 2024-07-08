use oxc::{semantic::SymbolId, span::CompactStr as CompactString};
use rolldown_common::{AstScopes, EcmaModuleId, SymbolRef};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct RuntimeModuleBrief {
  id: EcmaModuleId,
  name_to_symbol: FxHashMap<CompactString, SymbolId>,
}

impl RuntimeModuleBrief {
  pub fn new(id: EcmaModuleId, scope: &AstScopes) -> Self {
    Self {
      id,
      name_to_symbol: scope.get_bindings(scope.root_scope_id()).clone().into_iter().collect(),
    }
  }

  pub fn id(&self) -> EcmaModuleId {
    self.id
  }

  pub fn resolve_symbol(&self, name: &str) -> SymbolRef {
    let symbol_id =
      self.name_to_symbol.get(name).unwrap_or_else(|| panic!("Failed to resolve symbol: {name}"));
    (self.id, *symbol_id).into()
  }
}

pub static ROLLDOWN_RUNTIME_RESOURCE_ID: &str = "rolldown:runtime";
