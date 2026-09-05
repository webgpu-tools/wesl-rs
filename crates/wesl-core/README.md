# Stable type definitions for WESL packages

This crate contains stable type definitions for WESL packages, used by the packager for crates
published to crates-io.

In shader crates, we recommend to use `wesl-core` as a runtime dependency instead of `wesl`,
as it is more lightweight and its version is only bumped when the packaging format changes.
