// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.

use std::sync::Arc;

use rolldown_common::{
  EcmaModuleId, ExportsKind, IndexEcmaModules, ModuleId, ModuleType, ResolvedExport, Specifier,
  SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_rstr::Rstr;
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::{
  types::{
    linking_metadata::LinkingMetadataVec, namespace_alias::NamespaceAlias, symbols::Symbols,
  },
  SharedOptions,
};

use super::LinkStage;

#[derive(Clone)]
struct ImportTracker {
  pub importer: EcmaModuleId,
  pub importee: EcmaModuleId,
  pub imported: Specifier,
  pub imported_as: SymbolRef,
}

pub struct MatchingContext {
  tracker_stack: Vec<ImportTracker>,
}

impl MatchingContext {
  fn current_tracker(&self) -> &ImportTracker {
    self.tracker_stack.last().expect("tracker_stack is not empty")
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  // The import is either external or undefined
  _Ignore,
  // "sourceIndex" and "ref" are in use
  Normal { symbol: SymbolRef },
  // "namespaceRef" and "alias" are in use
  Namespace { namespace_ref: SymbolRef },
  // Both "matchImportNormal" and "matchImportNamespace"
  NormalAndNamespace { namespace_ref: SymbolRef, alias: Rstr },
  // The import could not be evaluated due to a cycle
  Cycle,
  // The import resolved to multiple symbols via "export * from"
  Ambiguous,
  NoMatch,
}

#[derive(Debug)]
pub enum ImportStatus {
  // The imported file has no matching export
  NoMatch {
    // importee_id: NormalModuleId,
  },

  // The imported file has a matching export
  Found {
    // owner: NormalModuleId,
    symbol: SymbolRef,
    potentially_ambiguous_export_star_refs: Vec<SymbolRef>,
  },

  // The imported file is CommonJS and has unknown exports
  CommonJS,

  // The import is missing but there is a dynamic fallback object
  DynamicFallback {
    namespace_ref: SymbolRef,
  },

  // The import was treated as a CommonJS import but the file is known to have no exports
  _CommonJSWithoutExports,

  // The imported file was disabled by mapping it to false in the "browser" field of package.json
  _Disabled,

  // The imported file is external and has unknown exports
  External,
}

impl<'link> LinkStage<'link> {
  /// Notices:
  /// - For external import like
  /// ```js
  /// // main.js
  /// import { a } from 'external';
  ///
  /// // foo.js
  /// import { a } from 'external';
  /// export { a }
  /// ```
  ///
  /// Unlike import from normal modules, the imported variable deosn't have a place that declared the variable. So we consider `import { a } from 'external'` in `foo.js` as the declaration statement of `a`.
  pub fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metas.iter_mut_enumerated().par_bridge().for_each(|(module_id, meta)| {
      let module = &self.module_table.ecma_modules[module_id];
      let mut resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| {
          let resolved_export = ResolvedExport {
            symbol_ref: local.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          (name.clone(), resolved_export)
        })
        .collect::<FxHashMap<_, _>>();

      let mut module_stack = vec![];
      if !module.star_exports.is_empty() {
        Self::add_exports_for_export_star(
          &self.module_table.ecma_modules,
          &mut resolved_exports,
          module_id,
          &mut module_stack,
        );
      }
      meta.resolved_exports = resolved_exports;
    });

    let mut binding_ctx = BindImportsAndExportsContext {
      normal_modules: &self.module_table.ecma_modules,
      metas: &mut self.metas,
      symbols: &mut self.symbols,
      input_options: self.input_options,
      errors: Vec::default(),
    };

    self.module_table.ecma_modules.iter().for_each(|module| {
      binding_ctx.match_imports_with_exports(module.id);
    });

    self.errors.extend(binding_ctx.errors);

    self.metas.iter_mut().par_bridge().for_each(|meta| {
      let mut sorted_and_non_ambiguous_resolved_exports = vec![];
      'next_export: for (exported_name, resolved_export) in &meta.resolved_exports {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          let main_ref = self.symbols.par_canonical_ref_for(resolved_export.symbol_ref);

          for ambiguous_ref in potentially_ambiguous_symbol_refs {
            let ambiguous_ref = self.symbols.par_canonical_ref_for(*ambiguous_ref);
            if main_ref != ambiguous_ref {
              continue 'next_export;
            }
          }
        };
        sorted_and_non_ambiguous_resolved_exports.push(exported_name.clone());
      }
      sorted_and_non_ambiguous_resolved_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports = sorted_and_non_ambiguous_resolved_exports;
    });
  }

  pub fn add_exports_for_export_star(
    normal_modules: &IndexEcmaModules,
    resolve_exports: &mut FxHashMap<Rstr, ResolvedExport>,
    module_id: EcmaModuleId,
    module_stack: &mut Vec<EcmaModuleId>,
  ) {
    if module_stack.contains(&module_id) {
      return;
    }

    module_stack.push(module_id);

    let module = &normal_modules[module_id];

    for dep_id in module.star_export_module_ids().filter_map(ModuleId::as_ecma) {
      let dep_module = &normal_modules[dep_id];
      if matches!(dep_module.exports_kind, ExportsKind::CommonJs) {
        continue;
      }

      for (exported_name, named_export) in &dep_module.named_exports {
        // ES6 export star statements ignore exports named "default"
        if exported_name.as_str() == "default" {
          continue;
        }
        // This export star is shadowed if any file in the stack has a matching real named export
        if module_stack
          .iter()
          .any(|id| normal_modules[*id].named_exports.contains_key(exported_name))
        {
          continue;
        }

        // We have filled `resolve_exports` with `named_exports`. If the export is already exists, it means that the importer
        // has a named export with the same name. So the export from dep module is shadowed.
        if let Some(resolved_export) = resolve_exports.get_mut(exported_name) {
          if named_export.referenced != resolved_export.symbol_ref {
            resolved_export
              .potentially_ambiguous_symbol_refs
              .get_or_insert(Vec::default())
              .push(named_export.referenced);
          }
        } else {
          let resolved_export = ResolvedExport {
            symbol_ref: named_export.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          resolve_exports.insert(exported_name.clone(), resolved_export);
        }
      }

      Self::add_exports_for_export_star(normal_modules, resolve_exports, dep_id, module_stack);
    }

    module_stack.pop();
  }
}

struct BindImportsAndExportsContext<'a> {
  pub normal_modules: &'a IndexEcmaModules,
  pub metas: &'a mut LinkingMetadataVec,
  pub symbols: &'a mut Symbols,
  pub input_options: &'a SharedOptions,
  pub errors: Vec<BuildError>,
}

impl<'a> BindImportsAndExportsContext<'a> {
  fn match_imports_with_exports(&mut self, module_id: EcmaModuleId) {
    let module = &self.normal_modules[module_id];
    for (imported_as_ref, named_import) in &module.named_imports {
      let match_import_span = tracing::trace_span!(
        "MATCH_IMPORT",
        module_id = module.stable_resource_id,
        imported_specifier = format!("{}", named_import.imported)
      );
      let _enter = match_import_span.enter();

      let rec = &module.import_records[named_import.record_id];
      let ret = match rec.resolved_module {
        ModuleId::External(_) => MatchImportKind::Normal { symbol: *imported_as_ref },
        ModuleId::Normal(importee_id) => self.match_import_with_export(
          self.normal_modules,
          &mut MatchingContext { tracker_stack: Vec::default() },
          ImportTracker {
            importer: module_id,
            importee: importee_id,
            imported: named_import.imported.clone(),
            imported_as: *imported_as_ref,
          },
        ),
      };
      tracing::trace!("Got match result {:?}", ret);
      match ret {
        MatchImportKind::_Ignore | MatchImportKind::Ambiguous | MatchImportKind::Cycle => {}
        MatchImportKind::Normal { symbol } => {
          self.symbols.union(*imported_as_ref, symbol);
        }
        MatchImportKind::Namespace { namespace_ref } => {
          self.symbols.union(*imported_as_ref, namespace_ref);
        }
        MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
          self.symbols.get_mut(*imported_as_ref).namespace_alias =
            Some(NamespaceAlias { property_name: alias, namespace_ref });
        }
        MatchImportKind::NoMatch => {
          let importee = &self.normal_modules[rec.resolved_module.as_ecma().unwrap()];
          self.errors.push(BuildError::missing_export(
            module.stable_resource_id.clone(),
            importee.stable_resource_id.clone(),
            Arc::clone(&module.source),
            named_import.imported.to_string(),
            named_import.span_imported,
          ));
        }
      }
    }
  }

  fn advance_import_tracker(&self, ctx: &mut MatchingContext) -> ImportStatus {
    let tracker = ctx.current_tracker();
    let importer = &self.normal_modules[tracker.importer];
    let named_import = &importer.named_imports[&tracker.imported_as];

    // Is this an external file?
    let importee_id = importer.import_records[named_import.record_id].resolved_module;
    let importee_id = match importee_id {
      ModuleId::Normal(importee_id) => importee_id,
      ModuleId::External(_) => return ImportStatus::External,
    };

    // Is this a named import of a file without any exports?
    let importee = &self.normal_modules[importee_id];
    debug_assert!(matches!(importee.exports_kind, ExportsKind::Esm | ExportsKind::CommonJs));
    // TODO: Deal with https://github.com/evanw/esbuild/blob/109449e5b80886f7bc7fc7e0cee745a0221eef8d/internal/linker/linker.go#L3062-L3072

    if matches!(importee.exports_kind, ExportsKind::CommonJs) {
      return ImportStatus::CommonJS;
    }

    match &named_import.imported {
      Specifier::Star => ImportStatus::Found {
        symbol: importee.namespace_object_ref,
        // owner: importee_id,
        potentially_ambiguous_export_star_refs: vec![],
      },
      Specifier::Literal(literal_imported) => {
        if let Some(export) = self.metas[importee_id].resolved_exports.get(literal_imported) {
          ImportStatus::Found {
            // owner: importee_id,
            symbol: export.symbol_ref,
            potentially_ambiguous_export_star_refs: export
              .potentially_ambiguous_symbol_refs
              .clone()
              .unwrap_or_default(),
          }
        } else if self.metas[importee_id].has_dynamic_exports {
          ImportStatus::DynamicFallback { namespace_ref: importee.namespace_object_ref }
        } else {
          ImportStatus::NoMatch {}
        }
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  fn match_import_with_export(
    &mut self,
    normal_modules: &IndexEcmaModules,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = normal_modules[tracker.importer].stable_resource_id,
      importee = normal_modules[tracker.importee].stable_resource_id,
      imported_specifier = format!("{}", tracker.imported)
    );
    let _enter = tracking_span.enter();

    let mut ambiguous_results = vec![];
    let ret = loop {
      for prev_tracker in ctx.tracker_stack.iter().rev() {
        if prev_tracker.importer == tracker.importer
          && prev_tracker.imported_as == tracker.imported_as
        {
          // Cycle import. No need to continue, just return
          return MatchImportKind::Cycle;
        }
      }
      ctx.tracker_stack.push(tracker.clone());
      let import_status = self.advance_import_tracker(ctx);
      tracing::trace!("Got import_status {:?}", import_status);
      let importer = &self.normal_modules[tracker.importer];
      let named_import = &importer.named_imports[&tracker.imported_as];
      let importer_record = &importer.import_records[named_import.record_id];

      let kind = match import_status {
        ImportStatus::CommonJS => match &tracker.imported {
          Specifier::Star => {
            MatchImportKind::Namespace { namespace_ref: importer_record.namespace_ref }
          }
          Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
            namespace_ref: importer_record.namespace_ref,
            alias: alias.clone(),
          },
        },
        ImportStatus::DynamicFallback { namespace_ref } => match &tracker.imported {
          Specifier::Star => MatchImportKind::Namespace { namespace_ref },
          Specifier::Literal(alias) => {
            MatchImportKind::NormalAndNamespace { namespace_ref, alias: alias.clone() }
          }
        },
        ImportStatus::NoMatch { .. } => {
          break MatchImportKind::NoMatch;
        }
        ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs, .. } => {
          for ambiguous_ref in &potentially_ambiguous_export_star_refs {
            let ambiguous_ref_owner = &normal_modules[ambiguous_ref.owner];
            if let Some(another_named_import) = ambiguous_ref_owner.named_imports.get(ambiguous_ref)
            {
              let rec = &ambiguous_ref_owner.import_records[another_named_import.record_id];
              let ambiguous_result = match rec.resolved_module {
                ModuleId::Normal(importee_id) => self.match_import_with_export(
                  normal_modules,
                  &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                  ImportTracker {
                    importer: ambiguous_ref_owner.id,
                    importee: importee_id,
                    imported: another_named_import.imported.clone(),
                    imported_as: another_named_import.imported_as,
                  },
                ),
                ModuleId::External(_) => {
                  MatchImportKind::Normal { symbol: another_named_import.imported_as }
                }
              };
              ambiguous_results.push(ambiguous_result);
            } else {
              ambiguous_results.push(MatchImportKind::Normal { symbol: *ambiguous_ref });
            }
          }

          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = &normal_modules[symbol.owner];
          if let Some(another_named_import) = owner.named_imports.get(&symbol) {
            let rec = &owner.import_records[another_named_import.record_id];
            match rec.resolved_module {
              ModuleId::External(_) => {
                break MatchImportKind::Normal { symbol: another_named_import.imported_as };
              }
              ModuleId::Normal(importee_id) => {
                tracker.importee = importee_id;
                tracker.importer = owner.id;
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                continue;
              }
            }
          }

          break MatchImportKind::Normal { symbol };
        }
        ImportStatus::_CommonJSWithoutExports => todo!(),
        ImportStatus::_Disabled => todo!(),
        ImportStatus::External => todo!(),
      };
      break kind;
    };

    tracing::trace!("ambiguous_results {:#?}", ambiguous_results);
    tracing::trace!("ret {:#?}", ret);

    for ambiguous_result in ambiguous_results {
      if ambiguous_result != ret {
        return MatchImportKind::Ambiguous;
      }
    }

    let importee = &self.normal_modules[tracker.importee];
    if (self.input_options.shim_missing_exports
      || matches!(importee.module_type, ModuleType::Empty))
      && matches!(ret, MatchImportKind::NoMatch)
    {
      match &tracker.imported {
        Specifier::Star => unreachable!("star should always exist, no need to shim"),
        Specifier::Literal(imported) => {
          let shimmed_symbol_ref = self.metas[tracker.importee]
            .shimmed_missing_exports
            .entry(imported.clone())
            .or_insert_with(|| {
              self.symbols.create_symbol(tracker.importee, imported.clone().to_string().into())
            });
          return MatchImportKind::Normal { symbol: *shimmed_symbol_ref };
        }
      }
    }

    ret
  }
}
