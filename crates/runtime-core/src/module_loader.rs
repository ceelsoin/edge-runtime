use std::sync::Arc;

use deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    ResolutionKind, ModuleLoadOptions, ModuleLoadReferrer, error::ModuleLoaderError,
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
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        deno_core::resolve_import(specifier, referrer)
            .map_err(|e| ModuleLoaderError::from(
                deno_error::JsErrorBox::generic(format!("module resolution failed: {}", e))
            ))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let specifier = module_specifier.clone();
        let eszip = self.eszip.clone();

        ModuleLoadResponse::Async(Box::pin(async move {
            let module = eszip
                .get_module(specifier.as_str())
                .ok_or_else(|| ModuleLoaderError::from(
                    deno_error::JsErrorBox::generic(format!("module not found in eszip: {}", specifier))
                ))?;

            let source = module
                .take_source()
                .await
                .ok_or_else(|| ModuleLoaderError::from(
                    deno_error::JsErrorBox::generic(format!("module source unavailable: {}", specifier))
                ))?;

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
