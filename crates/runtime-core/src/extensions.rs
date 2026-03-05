use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::{EmitOptions, MediaType, ParseParams, TranspileModuleOptions, TranspileOptions};
use deno_core::{Extension, ModuleCodeString, ModuleName, RuntimeOptions, SourceMapData};

use crate::permissions::Permissions;

// Bootstrap extension: imports all extension ESM modules so they get evaluated.
//
// deno_core loads extension ESM as side-modules but only evaluates those
// reachable from an `esm_entry_point`.  None of the upstream deno_* extensions
// set an entry point, so we provide one here that pulls everything in.
deno_core::extension!(
    edge_bootstrap,
    esm_entry_point = "ext:edge_bootstrap/bootstrap.js",
    esm = [dir "src", "bootstrap.js"],
);

/// Build the set of Deno extensions to register on every isolate.
///
/// This provides the JS runtime with console, URL, Web APIs, fetch, and crypto.
/// The `edge_bootstrap` extension is registered last — its entry point imports
/// all other extension ESM modules, causing them to be evaluated.
pub fn get_extensions() -> Vec<Extension> {
    vec![
        deno_webidl::deno_webidl::init_ops_and_esm(),
        deno_console::deno_console::init_ops_and_esm(),
        deno_url::deno_url::init_ops_and_esm(),
        deno_web::deno_web::init_ops_and_esm::<Permissions>(
            Arc::new(deno_web::BlobStore::default()),
            None,
        ),
        deno_crypto::deno_crypto::init_ops_and_esm(None),
        deno_telemetry::deno_telemetry::init_ops_and_esm(),
        deno_fetch::deno_fetch::init_ops_and_esm::<Permissions>(deno_fetch::Options::default()),
        deno_net::deno_net::init_ops_and_esm::<Permissions>(None, None),
        deno_tls::deno_tls::init_ops_and_esm(),
        // Bootstrap must be last — its entry point imports all extension modules.
        edge_bootstrap::init_ops_and_esm(),
    ]
}

/// Set the extension transpiler on `RuntimeOptions`.
///
/// Some deno extensions (e.g. `deno_telemetry`) ship TypeScript source that
/// V8 cannot execute directly. This configures TS → JS transpilation during
/// JsRuntime initialisation.
pub fn set_extension_transpiler(opts: &mut RuntimeOptions) {
    opts.extension_transpiler = Some(Rc::new(
        |name: ModuleName, code: ModuleCodeString| {
            let specifier_str: &str = &name;

            let url = deno_core::url::Url::parse(specifier_str)
                .unwrap_or_else(|_| deno_core::url::Url::parse("file:///unknown.ts").unwrap());

            let media_type = MediaType::from_specifier_and_headers(&url, None);

            if !matches!(
                media_type,
                MediaType::TypeScript | MediaType::Mts | MediaType::Cts | MediaType::Tsx
            ) {
                return Ok((code, None));
            }

            let source_text: &str = &code;
            let parsed = deno_ast::parse_module(ParseParams {
                specifier: url,
                text: source_text.into(),
                media_type,
                capture_tokens: false,
                scope_analysis: false,
                maybe_syntax: None,
            })
            .map_err(|e| anyhow::anyhow!("failed to parse {specifier_str}: {e}"))?;

            let transpiled = parsed
                .transpile(
                    &TranspileOptions::default(),
                    &TranspileModuleOptions::default(),
                    &EmitOptions::default(),
                )
                .map_err(|e| anyhow::anyhow!("failed to transpile {specifier_str}: {e}"))?;

            let emitted = transpiled.into_source();
            let source_map = emitted
                .source_map
                .map(|sm| Cow::Owned(sm.into_bytes()) as SourceMapData);

            Ok((ModuleCodeString::from(emitted.text), source_map))
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn init_v8() {
        INIT.call_once(|| {
            deno_core::JsRuntime::init_platform(None, false);
        });
    }

    #[test]
    fn get_extensions_returns_expected_count() {
        let exts = get_extensions();
        assert_eq!(exts.len(), 10, "expected 10 extensions, got {}", exts.len());
    }

    #[test]
    fn set_extension_transpiler_configures_opts() {
        let mut opts = RuntimeOptions::default();
        assert!(opts.extension_transpiler.is_none());
        set_extension_transpiler(&mut opts);
        assert!(opts.extension_transpiler.is_some());
    }

    #[test]
    fn runtime_boots_with_extensions() {
        init_v8();
        let mut opts = RuntimeOptions {
            extensions: get_extensions(),
            ..Default::default()
        };
        set_extension_transpiler(&mut opts);
        let _rt = deno_core::JsRuntime::new(opts);
    }
}
