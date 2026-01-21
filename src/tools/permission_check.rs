use crate::security::permission::{
    PermissionHandler, PermissionRequest, PermissionResult, ResourceType,
};
use crate::security::{SecurityValidator, ValidationResult};
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub struct FilePermissionChecker {
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl FilePermissionChecker {
    pub fn new(
        validator: Arc<SecurityValidator>,
        permission_handler: Arc<dyn PermissionHandler>,
    ) -> Self {
        Self {
            validator,
            permission_handler,
            runtime_allowed: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn check_permission(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();

        // Check runtime allowed
        {
            let allowed = self
                .runtime_allowed
                .read()
                .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
            if allowed.contains(&path_str) {
                return Ok(());
            }
        }

        // Validate path
        match self.validator.validate_file_path(path) {
            ValidationResult::Allowed => Ok(()),
            ValidationResult::Denied(reason) => {
                anyhow::bail!("Path denied: {}", reason)
            }
            ValidationResult::RequiresPermission(reason) => {
                let request = PermissionRequest {
                    resource_type: ResourceType::FilePath,
                    action: path_str.clone(),
                    reason,
                };

                match self.permission_handler.request_permission(&request) {
                    PermissionResult::Allow => Ok(()),
                    PermissionResult::Deny => {
                        anyhow::bail!("Permission denied by user")
                    }
                    PermissionResult::AllowAlways(p) => {
                        let mut allowed = self
                            .runtime_allowed
                            .write()
                            .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                        allowed.insert(p);
                        Ok(())
                    }
                    PermissionResult::Quit => {
                        anyhow::bail!("User requested quit")
                    }
                }
            }
        }
    }
}
