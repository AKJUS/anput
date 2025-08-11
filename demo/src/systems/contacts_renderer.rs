use crate::utils::aabb_vertices;
use anput::{
    systems::SystemContext, third_party::intuicio_data::managed::ManagedLazy, universe::Res,
};
use anput_physics::{collisions::ContactsCache, third_party::vek::Rgba};
use send_wrapper::SendWrapper;
use spitfire_draw::{
    context::DrawContext,
    primitives::PrimitivesEmitter,
    utils::{Drawable, ShaderRef, Vertex},
};
use spitfire_glow::graphics::Graphics;
use std::error::Error;

const OVERLAP_COLOR: Rgba<f32> = Rgba::new(1.0, 0.0, 1.0, 0.5);
const CELL_COLOR: Rgba<f32> = Rgba::new(1.0, 0.0, 1.0, 1.0);

pub fn render_contacts(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (mut draw, graphics, contacts) = context.fetch::<(
        Res<true, &mut SendWrapper<DrawContext>>,
        Res<true, &mut SendWrapper<ManagedLazy<Graphics<Vertex>>>>,
        Res<true, &ContactsCache>,
    )>()?;

    let draw = &mut *draw;
    let graphics = &mut *graphics.write().unwrap();
    let primitives = PrimitivesEmitter::default().shader(ShaderRef::name("color"));

    for contact in contacts.any_contacts() {
        primitives
            .emit_lines(aabb_vertices(&contact.overlap_region))
            .tint(OVERLAP_COLOR)
            .looped(true)
            .draw(draw, graphics);

        for cell in contact.cells {
            primitives
                .emit_lines(aabb_vertices(&cell.region))
                .tint(CELL_COLOR)
                .looped(true)
                .draw(draw, graphics);
        }
    }

    Ok(())
}
