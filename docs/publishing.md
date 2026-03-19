# Publishing Guide

This guide covers the packages that matter for the current OSS v1 launch surface.

## Package list

### Python

- `klock`
- `klock-langchain`

### npm

- `@klock-protocol/core`

### Not a registry package right now

- `klock` CLI binary

The CLI is currently distributed through GitHub release artifacts, not through npm or PyPI.

---

## The first rule: do not fragment the package names

If the public package names you want already exist and **you control them**, keep publishing on those exact packages.

That means:

- keep using `klock` on PyPI
- keep using `klock-langchain` on PyPI
- keep using `@klock-protocol/core` on npm

Do **not** create extra packages like:

- `klock-core-py`
- `klock-oss`
- `klock-langchain-oss`
- `klock-js-core`

That only creates confusion.

Make a new package name only if one of these is true:

- you do **not** control the existing package
- the existing package is under the wrong owner and cannot be transferred
- the existing package has the wrong long-term product meaning

If you already published an earlier version of these exact packages and you still control them, the correct move is to publish a new version on the same package.

---

## Versioning rule

Before publishing, check whether the exact version number is already live.

If `0.1.0` is already published on a package you control:

- bump the local version before publishing
- use `0.1.1` for a packaging/docs fix release
- use `0.2.0` if the public API or product surface changed materially

Do not try to overwrite an existing version. npm and PyPI will reject that anyway.

Current local versions in this repo:

- `klock`: `0.1.1`
- `klock-langchain`: `0.1.1`
- `@klock-protocol/core`: `0.1.1`

Files:

- `klock-py/pyproject.toml`
- `integrations/klock-langchain/pyproject.toml`
- `klock-js/package.json`

---

## What I verified before publish

The following checks are the release-relevant ones that passed locally:

```bash
cd Klock-OpenSource
cargo check -p klock-py -p klock-cli -p klock-core

cd integrations/klock-langchain
PYTHONPATH=src python3 -m unittest tests.test_tool

cd ../../klock-js
node __test__/index.test.mjs

cd ../../../Klock-Website
npm run build
```

I also verified:

- `npm pack --dry-run`
- `npm publish --dry-run`

And the existing wheel artifacts already present from the prior build are:

- `/tmp/klock-wheels/klock-0.1.0-cp38-abi3-macosx_11_0_arm64.whl`
- `/tmp/klock-wheels/klock_langchain-0.1.0-py3-none-any.whl`

One note about the scripted full verifier:

- `./scripts/verify_oss_v1.sh` failed in this sandbox only because it creates a fresh virtualenv and then tries to download `maturin` from PyPI, but this environment has restricted network access.
- That is an environment issue, not a repo regression.

---

## Recommended publish order

Use this order:

1. `klock` on PyPI
2. `klock-langchain` on PyPI
3. `@klock-protocol/core` on npm
4. tag the repo and push `vX.Y.Z`
5. let GitHub Actions build CLI binaries and attach them to the release

Why this order:

- `klock-langchain` depends on `klock`
- npm is independent of the Python publish
- the release tag should match the version users can actually install

---

## Python publish: `klock`

Package source:

- `Klock-OpenSource/klock-py`

### Build

Preferred:

```bash
cd Klock-OpenSource
python3 -m pip install maturin
maturin build --release --manifest-path klock-py/Cargo.toml
```

### Publish directly with maturin

```bash
cd Klock-OpenSource
maturin publish --release --manifest-path klock-py/Cargo.toml
```

### Or publish from built wheel

```bash
cd Klock-OpenSource
python3 -m pip install twine
python3 -m twine upload target/wheels/*
```

---

## Python publish: `klock-langchain`

Package source:

- `Klock-OpenSource/integrations/klock-langchain`

### Build

```bash
cd Klock-OpenSource/integrations/klock-langchain
python3 -m pip install build twine
python3 -m build
```

### Publish

```bash
python3 -m twine upload dist/*
```

Because `klock-langchain` depends on `klock`, make sure the target version of `klock` is already available on PyPI first.

---

## npm publish: `@klock-protocol/core`

Package source:

- `Klock-OpenSource/klock-js`

### Final checks

```bash
cd Klock-OpenSource/klock-js
node __test__/index.test.mjs
npm pack --dry-run
npm publish --dry-run
```

### Publish

```bash
npm publish --access public
```

This package is already scoped:

- scope: `@klock-protocol`
- package: `core`

That is the right shape. Do not create a second unscoped JS package unless you are forced to.

---

## GitHub release for CLI binaries

The CLI binary is not being published as a registry package in this v1 flow.

Instead:

1. commit the final state
2. create a tag
3. push the tag

```bash
git tag v0.1.1
git push origin v0.1.1
```

That triggers:

- `.github/workflows/release-artifacts.yml`

Which builds:

- macOS binary
- Linux binary
- Windows binary
- Python wheel artifacts for the release

If you bump versions first, use the same version in the tag:

- package version `0.1.1` -> tag `v0.1.1`

---

## What to do if the packages already exist

### Case 1: You own the package and it is the right package

Use it. Publish a new version there.

This is the correct path for launch.

### Case 2: You own the package but it contains older experimental work

Still usually keep it if:

- the name is right
- the audience overlap is the same
- you can publish a clearly improved version

In that case:

- bump the version
- update README and metadata
- publish the new release

### Case 3: You do not own the package

Do not build your launch on a package name you cannot control.

Rename now, not later.

### Case 4: The package exists under you, but the current version number is already used

Just bump the version locally and publish the next version.

---

## My recommendation for this repo

Assuming you control the existing packages:

- publish `klock` again on PyPI
- publish `klock-langchain` again on PyPI
- publish `@klock-protocol/core` again on npm

Do **not** create new package names for launch.

The real thing users should recognize is the package identity, not a trail of renamed packages.

---

## Suggested final release sequence

```bash
# 1. verify repo state
cd Klock-OpenSource
cargo check -p klock-py -p klock-cli -p klock-core

cd integrations/klock-langchain
PYTHONPATH=src python3 -m unittest tests.test_tool

cd ../../klock-js
node __test__/index.test.mjs
npm pack --dry-run
npm publish --dry-run

cd ../../../Klock-Website
npm run build
```

Then:

```bash
# 2. publish Python core
cd Klock-OpenSource
maturin publish --release --manifest-path klock-py/Cargo.toml

# 3. publish Python LangChain adapter
cd integrations/klock-langchain
python3 -m build
python3 -m twine upload dist/*

# 4. publish npm package
cd ../../klock-js
npm publish --access public

# 5. tag release for CLI binaries
cd ../..
git tag v0.1.1
git push origin v0.1.1
```

The repo is already bumped to `0.1.1`, so use `v0.1.1` for the release tag.
