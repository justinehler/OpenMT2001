//! Process-retained interface text resources (native 4c1380) and interface
//! images, loaded once and shared by the menus, panels and the city UI graph.
#[path = "ui_text/case_zero_text.rs"]
pub mod case_zero_text;
#[path = "ui_text/images.rs"]
pub mod images;
#[path = "ui_text/resources.rs"]
pub mod resources;

/// Opaque move-stable resource allocation identity, never a native pointer/key.
#[derive(Clone, Debug)]
pub struct ResourceIdentity(std::sync::Arc<()>);
impl ResourceIdentity {
    pub fn new() -> Self {
        Self(std::sync::Arc::new(()))
    }
    pub fn same(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
