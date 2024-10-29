use std::{cell::RefCell, collections::HashMap, io::Error, path::PathBuf, rc::Rc};

use super::uicomponents::Buffer;

pub struct BufferManager {
    buffers: HashMap<PathBuf, Rc<RefCell<Buffer>>>,
}

impl BufferManager {
    pub fn new() -> Self {
        BufferManager {
            buffers: HashMap::new(),
        }
    }

    pub fn get_buffer(&mut self, file_path: &str) -> Result<Rc<RefCell<Buffer>>, Error> {
        let path = PathBuf::from(file_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(file_path));

        if let Some(buffer_rc) = self.buffers.get(&path) {
            Ok(buffer_rc.clone())
        } else {
            let buffer = Buffer::load(file_path)?;
            let buffer_rc = Rc::new(RefCell::new(buffer));
            self.buffers.insert(path, buffer_rc.clone());
            Ok(buffer_rc)
        }
    }
}
