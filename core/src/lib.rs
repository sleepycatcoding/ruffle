// This is a new lint with false positives, see https://github.com/rust-lang/rust-clippy/issues/10318
#![allow(clippy::extra_unused_type_parameters)]

#[macro_use]
mod display_object;
pub use display_object::{StageDisplayState, StageScaleMode};

#[macro_use]
extern crate smallvec;

#[macro_use]
extern crate downcast_rs;

#[macro_use]
extern crate num_derive;

#[macro_use]
mod avm1;
mod avm2;
mod binary_data;
pub mod bitmap;
mod character;
pub mod context;
pub mod context_menu;
mod drawing;
mod ecma_conversions;
pub(crate) mod either;
pub mod events;
pub mod focus_tracker;
mod font;
mod frame_lifecycle;
mod html;
mod library;
pub mod limits;
pub mod loader;
mod locale;
mod pixel_bender;
mod player;
mod prelude;
mod streams;
pub mod socket;
pub mod string;
pub mod tag_utils;
pub mod timer;
mod types;
mod vminterface;
mod xml;

pub mod backend;
pub mod compatibility_rules;
pub mod config;
#[cfg(feature = "egui")]
pub mod debug_ui;
pub mod external;
pub mod i18n;
pub mod stub;

pub use avm1::globals::system::SandboxType;
pub use context_menu::ContextMenuItem;
pub use events::PlayerEvent;
pub use indexmap;
pub use loader::LoadBehavior;
pub use player::{Player, PlayerBuilder, StaticCallstack};
pub use ruffle_render::backend::ViewportDimensions;
pub use swf;
pub use swf::Color;
