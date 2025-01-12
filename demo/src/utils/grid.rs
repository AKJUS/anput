use vek::Vec2;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Grid<T: Default + Clone> {
    pub default_value: T,
    size: Vec2<usize>,
    buffer: Vec<T>,
}

impl<T: Default + Clone> Grid<T> {
    pub fn with_default_value(mut self, value: T) -> Self {
        self.default_value = value;
        self
    }

    pub fn with_size(mut self, size: Vec2<usize>) -> Self {
        self.set_size(size);
        self
    }

    pub fn new(size: Vec2<usize>, buffer: Vec<T>) -> Option<Self> {
        if buffer.len() != size.x * size.y {
            return None;
        }
        Some(Self {
            default_value: T::default(),
            size,
            buffer,
        })
    }

    pub fn size(&self) -> Vec2<usize> {
        self.size
    }

    pub fn set_size(&mut self, size: Vec2<usize>) {
        self.size = size;
        let size = self.size.x * self.size.y;
        self.buffer = Vec::with_capacity(size);
        for _ in 0..size {
            self.buffer.push(self.default_value.clone());
        }
    }

    pub fn buffer(&self) -> &[T] {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut [T] {
        &mut self.buffer
    }

    pub fn get(&self, position: Vec2<usize>) -> Option<&T> {
        let index = self.index(position)?;
        self.buffer.get(index)
    }

    pub fn get_mut(&mut self, position: Vec2<usize>) -> Option<&mut T> {
        let index = self.index(position)?;
        self.buffer.get_mut(index)
    }

    pub fn index(&self, position: Vec2<usize>) -> Option<usize> {
        if position.x < self.size.x && position.y < self.size.y {
            Some(self.size.x * position.y + position.x)
        } else {
            None
        }
    }

    pub fn copy_from(&mut self, position: Vec2<usize>, data: impl IntoIterator<Item = T>) {
        if let Some(index) = self.index(position) {
            for (cell, value) in self.buffer[index..(index + self.size.x)]
                .iter_mut()
                .zip(data)
            {
                *cell = value;
            }
        }
    }
}
