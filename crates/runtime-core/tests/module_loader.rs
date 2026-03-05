//! Integration tests for the EszipModuleLoader.

use std::sync::Arc;

use runtime_core::module_loader::EszipModuleLoader;

/// Test that the module loader resolves relative specifiers correctly.
#[test]
fn module_loader_resolves_relative() {
    use deno_core::{ModuleLoader, ResolutionKind};

    let eszip = eszip::EszipV2::default();
    let loader = EszipModuleLoader::new(Arc::new(eszip));

    let base = "file:///src/main.js";
    let result = loader.resolve("./utils.js", base, ResolutionKind::Import);
    assert!(result.is_ok(), "resolve('./utils.js') should succeed");
    assert_eq!(
        result.unwrap().as_str(),
        "file:///src/utils.js",
        "should resolve to sibling file"
    );
}

/// Test that determine_root_specifier returns the first specifier from the eszip.
#[test]
fn determine_root_specifier_empty_eszip_returns_default() {
    let eszip = eszip::EszipV2::default();
    let result = runtime_core::isolate::determine_root_specifier(&eszip);
    assert!(result.is_ok());
    // Empty eszip has no specifiers, so it should return the default entrypoint.
    let spec = result.unwrap();
    assert_eq!(spec.as_str(), "file:///src/index.ts");
}
