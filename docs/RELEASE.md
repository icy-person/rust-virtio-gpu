# Release procedure

Create and push a version tag, for example:

```bash
git tag -a v0.1.0 -m 'rust-virtio-gpu 0.1.0'
git push origin v0.1.0
```

The Release workflow then installs the Ubuntu 24.04 Vulkan/Venus dependencies, validates formatting, checks and builds the launcher, runs tests and Clippy, creates a reproducible release directory, writes a build-info file and SHA256 checksum, and publishes the archive to GitHub Releases.

The release is blocked on any build, test or Clippy failure.
