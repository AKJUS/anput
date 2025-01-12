use vek::Vec2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub position: Vec2<isize>,
}
