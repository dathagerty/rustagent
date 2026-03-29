fn main() {
    #[cfg(feature = "bundle-ui")]
    {
        println!("cargo:rerun-if-changed=web/src");
        println!("cargo:rerun-if-changed=web/package.json");

        let web_dir = "web";

        // Check if web/ directory exists
        if !std::path::Path::new(web_dir).exists() {
            panic!(
                "web/ directory not found. The bundle-ui feature requires the web UI source. \
                 See docs/plans/v2-architecture.md Phase 4 for setup instructions."
            );
        }

        let status = std::process::Command::new("bun")
            .args(["install"])
            .current_dir(web_dir)
            .status()
            .expect("bun must be installed to build with bundle-ui feature");
        assert!(status.success(), "bun install failed");

        let status = std::process::Command::new("bun")
            .args(["run", "build"])
            .current_dir(web_dir)
            .status()
            .expect("bun run build failed");
        assert!(status.success(), "frontend build failed");
    }
}
