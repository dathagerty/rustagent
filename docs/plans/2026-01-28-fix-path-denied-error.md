# Fix "Path denied: Cannot resolve path" Error

**Date:** 2026-01-28
**Status:** Draft (Oracle-reviewed)
**Scope:** Security module path validation

## Problem

When writing to nested non-existent directories (e.g., `allowed_root/new/subdir/file.txt`), the `validate_file_path` function fails with "Cannot resolve path" because:

1. `path.canonicalize()` fails (path doesn't exist)
2. `parent.canonicalize()` also fails (parent doesn't exist either)
3. Returns `Denied("Cannot resolve path")`

Additionally, `~` is not expanded in input paths, but the allowlist does expand it, causing mismatches.

## Solution

### Approach

1. **Expand `~`** in input paths before validation (matching allowlist behavior)
2. **Walk up ancestors** to find the nearest existing directory
3. **Canonicalize the existing ancestor**, then append the non-existent suffix
4. **Reject `..` in the suffix** to prevent directory traversal in the non-canonical portion

This preserves the `starts_with(allowed)` security model while allowing writes to new nested directories.

### Implementation

**Changes to `security/mod.rs`:**

```rust
pub fn validate_file_path(&self, path: &Path) -> ValidationResult {
    // 1. Expand ~ to match allowlist behavior
    let path_str = path.to_string_lossy();
    let expanded = shellexpand::tilde(&path_str);
    let path = PathBuf::from(expanded.as_ref());

    // 2. Try direct canonicalization first
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // 3. Walk up ancestors to find nearest existing dir
            match resolve_nonexistent_path(&path) {
                Ok(p) => p,
                Err(reason) => return ValidationResult::Denied(reason),
            }
        }
    };

    // 4. Check against allowed paths (unchanged)
    for allowed in &self.allowed_paths_canonical {
        if canonical.starts_with(allowed) {
            return ValidationResult::Allowed;
        }
    }

    ValidationResult::RequiresPermission(...)
}
```

**New helper function:**

```rust
use std::path::Component;

fn resolve_nonexistent_path(path: &Path) -> Result<PathBuf, String> {
    // Collect all components for analysis
    let components: Vec<Component> = path.components().collect();

    // Find the split point: last ancestor that can be canonicalized
    for i in (0..=components.len()).rev() {
        let ancestor: PathBuf = components[..i].iter().collect();

        // Handle empty path (relative paths with no existing ancestor)
        if ancestor.as_os_str().is_empty() {
            // Try current directory as implicit ancestor
            if let Ok(canonical) = std::env::current_dir() {
                let suffix = &components[i..];
                return validate_and_build_path(canonical, suffix);
            }
            continue;
        }

        if let Ok(canonical) = ancestor.canonicalize() {
            let suffix = &components[i..];
            return validate_and_build_path(canonical, suffix);
        }
    }

    Err("Cannot resolve path".to_string())
}

fn validate_and_build_path(base: PathBuf, suffix: &[Component]) -> Result<PathBuf, String> {
    let mut resolved = base;

    for component in suffix {
        match component {
            Component::ParentDir => {
                return Err("Path contains invalid traversal (..)".to_string());
            }
            Component::CurDir => {
                // Skip . components (normalize them away)
                continue;
            }
            Component::Normal(name) => {
                resolved = resolved.join(name);
            }
            _ => {
                // Prefix/RootDir shouldn't appear in suffix
                return Err("Invalid path component in suffix".to_string());
            }
        }
    }

    Ok(resolved)
}
```

## Test Cases

| Test | Input | Expected |
|------|-------|----------|
| Nested non-existent dirs | `allowed_root/new/sub/file.txt` | `Allowed` |
| Tilde expansion | `~/project/file.txt` (with `~` in allowlist) | `Allowed` |
| Traversal in suffix | `allowed_root/new/../../../etc/passwd` | `Denied` |
| Traversal via existing path | `allowed_root/../outside/file.txt` | `RequiresPermission` |
| Symlink in allowed root | symlink pointing inside allowed | `Allowed` |
| Completely invalid path | `/nonexistent_root/file.txt` | `RequiresPermission` |
| **Relative path with non-existent parents** | `./new/sub/file.txt` (cwd in allowlist) | `Allowed` |
| **Dot component in suffix** | `allowed_root/new/./sub/file.txt` | `Allowed` (normalized) |
| **Path that already exists** | `allowed_root/existing.txt` | `Allowed` (no regression) |

Tests will use `tempfile` crate to create isolated directories.

## Implementation Phases

### Phase 1: Core Fix (~1 hour)
1. Add `resolve_nonexistent_path` and `validate_and_build_path` helper functions to `security/mod.rs`
2. Update `validate_file_path` to expand `~` and use the new helpers
3. Run existing tests to ensure no regressions

### Phase 2: TOCTOU Mitigation (~30 min)
1. Update `WriteFileTool::execute` in `tools/file.rs`
2. Add post-create canonicalization check after `create_dir_all`
3. Store `Arc<SecurityValidator>` in `WriteFileTool` (if not already available)

### Phase 3: Tests (~45 min)
1. Add `tempfile` dev-dependency if not present
2. Add all test cases to `tests/security_test.rs`
3. Verify all pass

### Phase 4: Verification (~15 min)
1. `cargo clippy` and `cargo fmt`
2. Manual test with the actual tool to confirm the original error is fixed

## Security Considerations

- **Traversal protection:** `Component::ParentDir` (`.."`) is rejected in the non-canonical suffix
- **Dot normalization:** `Component::CurDir` (`.`) is skipped/normalized away
- **Symlink handling:** Canonicalization of existing ancestors resolves symlinks
- **Allowlist matching:** Uses same `~` expansion as allowlist setup
- **No new attack surface:** Still uses `starts_with(allowed)` check on resolved paths

### TOCTOU Mitigation for Writes

The oracle identified a symlink race condition: after validation but before write, an attacker could create a symlink in the newly-created path segment pointing outside the allowed directory.

**Mitigation:** Add a post-create canonicalization check in `WriteFileTool`:

```rust
// In WriteFileTool::execute, after create_dir_all:
if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;

    // Post-create validation: ensure parent resolves inside allowed paths
    let canonical_parent = parent.canonicalize()?;
    match self.validator.validate_file_path(&canonical_parent) {
        ValidationResult::Allowed => {}
        _ => anyhow::bail!("Parent directory resolved outside allowed paths"),
    }
}
```

This adds defense-in-depth against symlink TOCTOU attacks without requiring `cap-std` or `openat`.
