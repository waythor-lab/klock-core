# Releasing OSS v1

Klock OSS v1 has three public deliverables:

- Python package: `klock`
- JavaScript package: `@klock-protocol/core`
- CLI binary: `klock`

## GitHub Actions

This repo now has two release lanes:

- `.github/workflows/release.yml` for the npm release flow already driven by Changesets
- `.github/workflows/release-artifacts.yml` for CLI binaries and Python wheel artifacts on `v*` tags

## Required secrets

To publish the full OSS v1 surface, configure:

- `NPM_TOKEN` for `@klock-protocol/core`
- `PYPI_API_TOKEN` if you later extend the Python wheel job to publish directly to PyPI

The current artifact workflow uploads Python wheels and CLI binaries to the GitHub release attached to the tag, and publishes `klock` to PyPI when `PYPI_API_TOKEN` is configured.

## Local pre-release check

Run the full verification path before tagging:

```bash
cd Klock-OpenSource
./scripts/verify_oss_v1.sh
```

For package-by-package publish instructions, see:

- `docs/publishing.md`

## Tagging

Create a release tag:

```bash
git tag v0.1.2
git push origin v0.1.2
```

That triggers:

- CLI binaries for Linux, macOS, and Windows
- Python wheel builds for the same release

## Recommended release checklist

1. Run `./scripts/verify_oss_v1.sh`
2. Confirm the proof scripts still show silent overwrite without Klock and Wait-Die coordination with Klock
3. Confirm website docs match the tagged product surface
4. Tag and push `vX.Y.Z`
