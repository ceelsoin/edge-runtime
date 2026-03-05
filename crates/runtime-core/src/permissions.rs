use deno_permissions::PermissionCheckError;

/// Simple permissions container for the edge runtime.
///
/// For now, this grants all permissions. In production, you'd want to restrict
/// network access, file system access, etc. per function.
#[derive(Debug, Clone)]
pub struct Permissions;

impl deno_web::TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
        false
    }
}

impl deno_fetch::FetchPermissions for Permissions {
    fn check_net_url(
        &mut self,
        _url: &deno_core::url::Url,
        _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
        Ok(())
    }

    fn check_read<'a>(
        &mut self,
        _p: &'a std::path::Path,
        _api_name: &str,
    ) -> Result<std::borrow::Cow<'a, std::path::Path>, PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError {
                access: "read access".to_string(),
                name: "read",
            },
        ))
    }
}

impl deno_net::NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
        &mut self,
        _host: &(T, Option<u16>),
        _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
        Ok(())
    }

    fn check_read(
        &mut self,
        _p: &str,
        _api_name: &str,
    ) -> Result<std::path::PathBuf, PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError {
                access: "read access".to_string(),
                name: "read",
            },
        ))
    }

    fn check_write(
        &mut self,
        _p: &str,
        _api_name: &str,
    ) -> Result<std::path::PathBuf, PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError {
                access: "write access".to_string(),
                name: "write",
            },
        ))
    }

    fn check_write_path<'a>(
        &mut self,
        _p: &'a std::path::Path,
        _api_name: &str,
    ) -> Result<std::borrow::Cow<'a, std::path::Path>, PermissionCheckError> {
        Err(PermissionCheckError::PermissionDenied(
            deno_permissions::PermissionDeniedError {
                access: "write access".to_string(),
                name: "write",
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_web::TimersPermission;
    use deno_fetch::FetchPermissions;
    use deno_net::NetPermissions;

    #[test]
    fn timers_disallow_hrtime() {
        let mut perm = Permissions;
        assert!(!perm.allow_hrtime());
    }

    #[test]
    fn fetch_check_net_url_allowed() {
        let mut perm = Permissions;
        let url = deno_core::url::Url::parse("https://example.com").unwrap();
        assert!(perm.check_net_url(&url, "fetch").is_ok());
    }

    #[test]
    fn fetch_check_read_denied() {
        let mut perm = Permissions;
        let result = FetchPermissions::check_read(&mut perm, std::path::Path::new("/etc/passwd"), "Deno.readFile");
        assert!(result.is_err());
    }

    #[test]
    fn net_check_net_allowed() {
        let mut perm = Permissions;
        let host = ("example.com".to_string(), Some(443u16));
        assert!(perm.check_net(&host, "Deno.connect").is_ok());
    }

    #[test]
    fn net_check_read_denied() {
        let mut perm = Permissions;
        assert!(NetPermissions::check_read(&mut perm, "/some/path", "Deno.read").is_err());
    }

    #[test]
    fn net_check_write_denied() {
        let mut perm = Permissions;
        assert!(perm.check_write("/some/path", "Deno.write").is_err());
    }

    #[test]
    fn net_check_write_path_denied() {
        let mut perm = Permissions;
        assert!(perm.check_write_path(std::path::Path::new("/some/path"), "Deno.write").is_err());
    }
}
