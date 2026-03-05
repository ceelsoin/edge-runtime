use std::sync::Arc;

use anyhow::anyhow;
use deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};
use eszip::EszipV2;

/// Module loader that resolves modules from an eszip bundle.
pub struct EszipModuleLoader {
    eszip: Arc<EszipV2>,
}

impl EszipModuleLoader {
    pub fn new(eszip: Arc<EszipV2>) -> Self {
        Self { eszip }
    }
}

impl ModuleLoader for EszipModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, anyhow::Error> {
        deno_core::resolve_import(specifier, referrer)
            .map_err(|e| anyhow!("module resolution failed for '{}' from '{}': {}", specifier, referrer, e))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let specifier = module_specifier.clone();
        let eszip = self.eszip.clone();

        ModuleLoadResponse::Async(Box::pin(async move {
            let module = eszip
                .get_module(specifier.as_str())
                .ok_or_else(|| anyhow!("module not found in eszip: {}", specifier))?;

            let source = module
                .take_source()
                .await
                .ok_or_else(|| anyhow!("module source unavailable: {}", specifier))?;

            let module_type = match module.kind {
                eszip::ModuleKind::JavaScript => ModuleType::JavaScript,
                eszip::ModuleKind::Json => ModuleType::Json,
                _ => ModuleType::JavaScript,
            };

            Ok(ModuleSource::new(
                module_type,
                ModuleSourceCode::Bytes(source.into()),
                &specifier,
                None,
            ))
        }))
    }
}
