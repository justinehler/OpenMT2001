//! Retained camera authority, UI adapters and explicit source integration boundaries.
#[path="camera_live/registration.rs"]
pub(crate) mod registration;
#[path="camera_live/native_initializers.rs"]
pub(crate) mod native_initializers;
#[path="camera_live/owned_factories.rs"]
pub(crate) mod owned_factories;
#[path="camera_live/process_install.rs"]
pub(crate) mod process_install;
#[path="camera_live/process_lifetime.rs"]
pub(crate) mod process_lifetime;
#[path="camera_live/strategic_factory.rs"]
pub(crate) mod strategic_factory;

#[path="camera_live/group_dispatch.rs"] pub(crate) mod group_dispatch;
#[path="camera_live/group_bridge.rs"] pub(crate) mod group_bridge;
#[path="camera_live/chat_ring.rs"] pub(crate) mod chat_ring;
#[path="camera_live/resident_descriptors.rs"] pub(crate) mod resident_descriptors;
#[path="camera_live/continuation.rs"] pub(crate) mod continuation;
#[path="camera_live/continuation_bridge.rs"] pub(crate) mod continuation_bridge;

#[path="camera_live/f_generic.rs"] pub(crate) mod f_generic;
#[path="camera_live/upper_f.rs"] pub(crate) mod upper_f;
#[path="camera_live/strategic_panel.rs"] pub(crate) mod strategic_panel;
#[path="camera_live/case_zero.rs"] pub mod case_zero;
#[path="camera_live/ui_text_resources.rs"] pub(crate) mod ui_text_resources;
#[path="camera_live/case_zero_text.rs"] pub mod case_zero_text;
#[path="camera_live/other_child_resets.rs"] pub mod other_child_resets;
#[path="camera_live/case_zero_factories.rs"] pub mod case_zero_factories;

#[path="camera_live/process_image.rs"] pub(crate) mod process_image;
#[path = "camera_live/property_child.rs"]
pub(crate) mod property_child;
#[path = "camera_live/property_inputs.rs"]
pub(crate) mod property_inputs;
#[path = "camera_live/main_model.rs"]
pub(crate) mod main_model;
#[path = "camera_live/bindings.rs"]
pub(crate) mod bindings;
#[path = "camera_live/panel_models.rs"]
pub(crate) mod panel_models;
#[path = "camera_live/live_inputs.rs"]
pub(crate) mod live_inputs;
#[path = "camera_live/main_panel.rs"]
pub(crate) mod main_panel;
#[path = "camera_live/property_descriptors.rs"]
pub(crate) mod property_descriptors;
#[path = "camera_live/ui_selection.rs"]
pub(crate) mod ui_selection;
#[path = "camera_live/ui_graph.rs"]
pub(crate) mod ui_graph;
#[path = "camera_live/focus.rs"]
pub(crate) mod focus;
#[path = "camera_live/navigation.rs"]
pub(crate) mod navigation;
#[path = "camera_live/owner.rs"]
pub(crate) mod owner;
#[path = "camera_live/endpoint.rs"]
pub(crate) mod endpoint;
#[path = "camera_live/context.rs"]
pub(crate) mod context;
#[path = "camera_live/selection.rs"]
pub(crate) mod selection;
#[path = "camera_live/producer.rs"]
mod producer;
#[path = "camera_live/input.rs"]
mod input;
#[path = "camera_live/cursor.rs"]
mod cursor;
#[path = "camera_live/audio.rs"]
mod audio;

pub(crate) use context::{Globals, OriginalGraphicsPolicy, State, UiContext, UiJoins,
    NavigationUi, SoundBoundary, Intent, ui_cursor_pass};
pub(crate) use cursor::Cursor;
pub(crate) use input::PhysicalInput;
pub(crate) use producer::selected_business;
pub(crate) use audio::service_available;

#[cfg(test)]
#[path="camera_live/root_component_tests.rs"]
mod root_component_tests;

#[path="camera_live/primitive_factory.rs"]
pub(crate) mod primitive_factory;

#[path="camera_live/process_images.rs"] pub(crate) mod process_images;
#[path="camera_live/main_construction.rs"] pub(crate) mod main_construction;
#[cfg(test)]
#[path="camera_live/process_owner_tests.rs"] mod process_owner_tests;
