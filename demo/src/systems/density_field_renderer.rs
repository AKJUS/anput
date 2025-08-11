use crate::{components::Visible, game::PIXEL_SIZE, utils::screen_aabb};
use anput::{
    query::{Include, Query},
    systems::SystemContext,
    third_party::intuicio_data::managed::ManagedLazy,
    universe::Res,
    world::{Relation, World},
};
use anput_physics::{
    PhysicsAccessView, Scalar,
    components::{BodyAccessInfo, BodyParentRelation},
    density_fields::DensityFieldBox,
    third_party::vek::{Rgba, Vec3},
};
use send_wrapper::SendWrapper;
use spitfire_draw::{
    pixels::{Pixels, blend_alpha},
    utils::Vertex,
};
use spitfire_glow::graphics::Graphics;
use std::error::Error;

pub fn render_density_fields(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut pixels, graphics, query) = context.fetch::<(
        &World,
        Res<true, &mut SendWrapper<Pixels>>,
        Res<true, &SendWrapper<ManagedLazy<Graphics<Vertex>>>>,
        Query<
            true,
            (
                &DensityFieldBox,
                &Relation<BodyParentRelation>,
                Option<&Rgba<f32>>,
                Include<Visible>,
            ),
        >,
    )>()?;

    let mut pixels = pixels.access_rgba().blend(blend_alpha);
    let graphics = graphics.write().unwrap();
    let view = PhysicsAccessView::new(world);
    let screen_aabb = screen_aabb(&graphics);

    for (density_field, body_relation, color, _) in query.query(world) {
        let Some(body) = body_relation.entities().next() else {
            continue;
        };
        let info = BodyAccessInfo::new(body, view.clone());
        let aabb = density_field.aabb(&info).intersection(screen_aabb);
        if aabb.size().is_approx_zero() {
            continue;
        }
        let color = color.copied().unwrap_or(Rgba::white());
        let from_x = ((aabb.min.x - screen_aabb.min.x) / PIXEL_SIZE as Scalar) as usize;
        let from_y = ((aabb.min.y - screen_aabb.min.y) / PIXEL_SIZE as Scalar) as usize;
        let to_x = (((aabb.max.x - screen_aabb.min.x) / PIXEL_SIZE as Scalar) as usize + 1)
            .min(pixels.width());
        let to_y = (((aabb.max.y - screen_aabb.min.y) / PIXEL_SIZE as Scalar) as usize + 1)
            .min(pixels.height());
        for y in from_y..to_y {
            for x in from_x..to_x {
                let point = Vec3 {
                    x: (x as Scalar + 0.5) * PIXEL_SIZE as Scalar + screen_aabb.min.x,
                    y: (y as Scalar + 0.5) * PIXEL_SIZE as Scalar + screen_aabb.min.y,
                    z: 0.0,
                };
                let alpha = density_field.density_at_point(point, &info);
                if alpha > Scalar::EPSILON {
                    pixels.blend(
                        [x, y],
                        Rgba {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a * alpha,
                        },
                    );
                }
            }
        }
    }

    Ok(())
}
