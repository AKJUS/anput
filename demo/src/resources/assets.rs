use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

pub trait AssetFactory: Send + Sync {
    type Object;

    fn decode(&self, bytes: &[u8]) -> Result<Arc<Self::Object>, Box<dyn Error>>;
}

impl<T, F> AssetFactory for F
where
    F: Fn(&[u8]) -> Result<Arc<T>, Box<dyn Error>> + Send + Sync,
{
    type Object = T;

    fn decode(&self, bytes: &[u8]) -> Result<Arc<T>, Box<dyn Error>> {
        (self)(bytes)
    }
}

pub struct Assets<T> {
    pub root: PathBuf,
    factory: Box<dyn AssetFactory<Object = T>>,
    registry: HashMap<PathBuf, Weak<T>>,
}

impl<T> Assets<T> {
    pub fn new(factory: impl AssetFactory<Object = T> + 'static) -> Self {
        Self {
            root: ".".into(),
            factory: Box::new(factory),
            registry: Default::default(),
        }
    }

    pub fn with_root_absolute(mut self, path: impl AsRef<Path>) -> Self {
        self.root = path.as_ref().into();
        self
    }

    pub fn with_root_relative(mut self, path: impl AsRef<Path>) -> Self {
        self.root = std::env::current_dir().unwrap().join(path.as_ref());
        self
    }

    pub fn get(&mut self, path: impl AsRef<Path>) -> Result<Arc<T>, Box<dyn Error>> {
        let path = self.root.join(path);
        if let Some(handle) = self.registry.get(&path) {
            let handle = handle.upgrade();
            if let Some(handle) = handle {
                return Ok(handle);
            }
        }
        let buffer = std::fs::read(&path)
            .inspect_err(|_| println!("Could not load asset file: {:?}", path))?;
        let result = self.factory.decode(&buffer)?;
        self.registry.insert(path, Arc::downgrade(&result));
        Ok(result)
    }

    pub fn release(&mut self, path: impl AsRef<Path>) {
        self.registry.remove(path.as_ref());
    }

    pub fn maintain(&mut self) {
        let to_remove = self
            .registry
            .iter()
            .filter(|(_, handle)| handle.strong_count() == 0)
            .map(|(path, _)| path.to_path_buf())
            .collect::<Vec<_>>();
        for path in to_remove {
            self.registry.remove(&path);
        }
    }
}
