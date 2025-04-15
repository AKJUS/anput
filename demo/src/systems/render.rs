use crate::{
    components::{
        drawable::{DrawOrder, Drawable, screen_rect},
        position::Position,
        sprite::Sprite,
        text::Text,
    },
    resources::camera::Camera,
};
use anput::prelude::*;
use std::{error::Error, io::stdout};

pub fn render_system(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, camera, sprite_query, text_query) = context.fetch::<(
        &World,
        Res<true, &Camera>,
        Query<true, (&Position, &Sprite, Option<&DrawOrder>)>,
        Query<true, (&Position, &Text, Option<&DrawOrder>)>,
    )>()?;

    let mut drawables = sprite_query
        .query(world)
        .map(|(position, sprite, order)| {
            (
                position.0,
                sprite as &dyn Drawable,
                order.copied().unwrap_or_default().0,
            )
        })
        .chain(text_query.query(world).map(|(position, text, order)| {
            (
                position.0,
                text as &dyn Drawable,
                order.copied().unwrap_or_default().0,
            )
        }))
        .collect::<Vec<_>>();
    drawables.sort_by(|(_, _, a), (_, _, b)| a.cmp(b));

    let mut stream = stdout();
    let screen_rect = screen_rect()?;
    for (position, drawable, _) in drawables {
        drawable.draw(&mut stream, position - camera.position, screen_rect)?;
    }

    Ok(())
}
