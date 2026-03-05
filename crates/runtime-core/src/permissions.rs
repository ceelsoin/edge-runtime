use std::sync::Arc;

use deno_permissions::{
    Permissions, PermissionsContainer, PermissionsOptions,
    RuntimePermissionDescriptorParser,
};

/// Creates a PermissionsContainer for the edge runtime.
///
/// By default, this grants network access (for fetch, WebSocket, etc.)
/// but denies file system access, environment variables, and subprocess execution.
pub fn create_permissions_container() -> PermissionsContainer {
    let parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));

    // Configure permissions: allow network, deny everything else
    let options = PermissionsOptions {
        allow_env: None,
        deny_env: None,
        ignore_env: None,
        allow_net: Some(vec![]), // Empty vec = allow all network
        deny_net: None,
        allow_ffi: None,
        deny_ffi: None,
        allow_read: None,
        deny_read: None,
        ignore_read: None,
        allow_run: None,
        deny_run: None,
        allow_sys: None,
        deny_sys: None,
        allow_write: None,
        deny_write: None,
        allow_import: Some(vec![]), // Allow imports
        deny_import: None,
        prompt: false, // No interactive prompts
    };

    let permissions = Permissions::from_options(parser.as_ref(), &options)
        .expect("failed to create permissions");

    PermissionsContainer::new(parser, permissions)
}

/// Creates a PermissionsContainer that allows all operations.
/// Use with caution - only for trusted code or development.
pub fn create_allow_all_permissions() -> PermissionsContainer {
    let parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));

    let permissions = Permissions::allow_all();
    PermissionsContainer::new(parser, permissions)
}

/// Creates a PermissionsContainer with custom network allowlist.
pub fn create_permissions_with_network_allowlist(
    allowed_hosts: Vec<String>,
) -> PermissionsContainer {
    let parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));

    let options = PermissionsOptions {
        allow_env: None,
        deny_env: None,
        ignore_env: None,
        allow_net: Some(allowed_hosts),
        deny_net: None,
        allow_ffi: None,
        deny_ffi: None,
        allow_read: None,
        deny_read: None,
        ignore_read: None,
        allow_run: None,
        deny_run: None,
        allow_sys: None,
        deny_sys: None,
        allow_write: None,
        deny_write: None,
        allow_import: Some(vec![]),
        deny_import: None,
        prompt: false,
    };

    let permissions = Permissions::from_options(parser.as_ref(), &options)
        .expect("failed to create permissions");

    PermissionsContainer::new(parser, permissions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_permissions_created_successfully() {
        // Just verify the container can be created without panic
        let _container = create_permissions_container();
    }

    #[test]
    fn allow_all_permissions_created_successfully() {
        // Just verify the container can be created without panic
        let _container = create_allow_all_permissions();
    }

    #[test]
    fn custom_network_allowlist_created_successfully() {
        let hosts = vec!["example.com".to_string(), "api.example.com:443".to_string()];
        // Just verify the container can be created without panic
        let _container = create_permissions_with_network_allowlist(hosts);
    }
}
