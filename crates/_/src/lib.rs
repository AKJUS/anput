pub mod actor;
pub mod archetype;
pub mod bundle;
pub mod commands;
pub mod component;
pub mod database;
pub mod entity;
pub mod multiverse;
pub mod observer;
pub mod prefab;
pub mod processor;
pub mod query;
pub mod resources;
pub mod scheduler;
pub mod systems;
pub mod universe;
pub mod view;
pub mod world;

pub use anput_jobs as jobs;

pub mod prelude {
    pub use crate::{
        commands::*, component::*, database::*, entity::*, query::*, resources::*, scheduler::*,
        systems::*, universe::*, world::*,
    };
}

pub mod third_party {
    pub use intuicio_core;
    pub use intuicio_data;
    pub use intuicio_derive;
    pub use intuicio_framework_serde;
}
