//! Out-of-process plugin-contract boundary definitions.
//!
//! This crate owns plugin descriptors and future wire-level contracts. It must
//! not expose an in-process Rust ABI or implement plugin process management.

/// A description of an eventual out-of-process plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    name: String,
}

impl PluginDescriptor {
    /// Creates a descriptor with the plugin's declared name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the plugin's declared name.
    pub fn name(&self) -> &str {
        &self.name
    }
}
