use crate::RenderInterface;

pub struct Gui {
    
}

impl Gui {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderInterface for Gui {
    fn queue(&self, _command: crate::RenderCommand) {
        todo!()
    }

    fn flush(&self) {
        todo!()
    }

    fn kill(&self) {
        todo!()
    }

    fn init(&self) {
        todo!()
    }

    fn size(&self) -> (usize, usize) {
        todo!()
    }
}

