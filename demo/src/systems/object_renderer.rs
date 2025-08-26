use std::error::Error;

use crate::{
    components::Visible,
    resources::{Globals, RenderMode},
};
use anput::{
    query::{Include, Lookup, Query},
    systems::SystemContext,
    third_party::intuicio_data::managed::ManagedLazy,
    universe::{Res, UniverseCondition},
    world::{Relation, World},
};
use anput_physics::{
    components::{BodyParticleRelation, PhysicsParticle, Position},
    density_fields::{
        DensityFieldBox, aabb::AabbDensityField, cube::CubeDensityField, sphere::SphereDensityField,
    },
    third_party::vek::Rgba,
};
use send_wrapper::SendWrapper;
use spitfire_draw::{
    context::DrawContext,
    primitives::PrimitivesEmitter,
    utils::{Drawable, ShaderRef, Vertex},
};
use spitfire_glow::graphics::Graphics;

pub struct ShouldRenderObjects;

impl UniverseCondition for ShouldRenderObjects {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .resources
            .get::<true, Globals>()
            .map(|globals| globals.render_mode == RenderMode::Objects)
            .unwrap_or_default()
    }
}

pub fn render_objects(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut draw, graphics, density_field_query, particle_lookup) = context.fetch::<(
        &World,
        Res<true, &mut SendWrapper<DrawContext>>,
        Res<true, &SendWrapper<ManagedLazy<Graphics<Vertex>>>>,
        Query<
            true,
            (
                &DensityFieldBox,
                Option<&Relation<BodyParticleRelation>>,
                Option<&Rgba<f32>>,
                Include<Visible>,
            ),
        >,
        Lookup<true, (&Position, Include<PhysicsParticle>)>,
    )>()?;

    let draw = &mut *draw;
    let graphics = &mut *graphics.write().unwrap();
    let primitives = PrimitivesEmitter::default().shader(ShaderRef::name("color"));

    for (density_field, particles, color, _) in density_field_query.query(world) {
        let particles = particles
            .into_iter()
            .flat_map(|particles| particles.entities());
        let color = color.copied().unwrap_or(Rgba::white());

        if let Some(object) = density_field.as_any().downcast_ref::<AabbDensityField>() {
            let color = color.into_array();

            primitives
                .emit_triangle_fan([
                    Vertex {
                        position: [object.aabb.min.x, object.aabb.min.y],
                        uv: [0.0, 0.0, 0.0],
                        color,
                    },
                    Vertex {
                        position: [object.aabb.max.x, object.aabb.min.y],
                        uv: [0.0, 0.0, 0.0],
                        color,
                    },
                    Vertex {
                        position: [object.aabb.max.x, object.aabb.max.y],
                        uv: [0.0, 0.0, 0.0],
                        color,
                    },
                    Vertex {
                        position: [object.aabb.min.x, object.aabb.max.y],
                        uv: [0.0, 0.0, 0.0],
                        color,
                    },
                ])
                .draw(draw, graphics);
        } else if let Some(object) = density_field
            .as_any()
            .downcast_ref::<CubeDensityField<true>>()
        {
            let color = color.into_array();

            for (position, _) in particle_lookup.lookup(world, particles) {
                let extents = object.total_extents();
                primitives
                    .emit_triangle_fan([
                        Vertex {
                            position: [
                                position.current.x - extents.x,
                                position.current.y - extents.y,
                            ],
                            uv: [0.0, 0.0, 0.0],
                            color,
                        },
                        Vertex {
                            position: [
                                position.current.x + extents.x,
                                position.current.y - extents.y,
                            ],
                            uv: [0.0, 0.0, 0.0],
                            color,
                        },
                        Vertex {
                            position: [
                                position.current.x + extents.x,
                                position.current.y + extents.y,
                            ],
                            uv: [0.0, 0.0, 0.0],
                            color,
                        },
                        Vertex {
                            position: [
                                position.current.x - extents.x,
                                position.current.y + extents.y,
                            ],
                            uv: [0.0, 0.0, 0.0],
                            color,
                        },
                    ])
                    .draw(draw, graphics);
            }
        } else if let Some(object) = density_field
            .as_any()
            .downcast_ref::<SphereDensityField<true>>()
        {
            for (position, _) in particle_lookup.lookup(world, particles) {
                primitives
                    .emit_circle(position.current.into(), object.radius, 0.1)
                    .tint(color)
                    .draw(draw, graphics);
            }
        }
    }

    Ok(())
}
