use crate::{resources::assets::AssetFactory, utils::grid::Grid};
use std::sync::Arc;
use vek::Vec2;

pub type ImageContent = Grid<char>;
pub type ImageHandle = Arc<ImageContent>;

#[derive(Debug, Clone)]
pub enum Image {
    Content(ImageContent),
    Handle(ImageHandle),
}

impl Default for Image {
    fn default() -> Self {
        Self::Content(Default::default())
    }
}

impl Image {
    pub fn content(&self) -> &ImageContent {
        match self {
            Self::Content(content) => content,
            Self::Handle(handle) => handle,
        }
    }

    pub fn content_mut(&mut self) -> Option<&mut ImageContent> {
        match self {
            Self::Content(content) => Some(content),
            Self::Handle(handle) => Arc::get_mut(handle),
        }
    }
}

impl From<ImageContent> for Image {
    fn from(value: ImageContent) -> Self {
        Self::Content(value)
    }
}

impl From<ImageHandle> for Image {
    fn from(value: ImageHandle) -> Self {
        Self::Handle(value)
    }
}

pub fn image_content_from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> ImageContent {
    let lines = lines.into_iter().collect::<Vec<_>>();
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    let mut result = ImageContent::default()
        .with_default_value(' ')
        .with_size(Vec2::new(width, lines.len()));
    for (row, line) in lines.into_iter().enumerate() {
        result.copy_from(Vec2::new(0, row), line.chars());
    }
    result
}

pub struct ImageAssetFactory;

impl AssetFactory for ImageAssetFactory {
    type Object = ImageContent;

    fn decode(&self, bytes: &[u8]) -> Result<Arc<Self::Object>, Box<dyn std::error::Error>> {
        let content = std::str::from_utf8(bytes)?;
        Ok(Arc::new(image_content_from_lines(content.lines())))
    }
}
