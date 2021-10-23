use crate::app::App;
use rimui::UIEvent;

pub struct MouseOperation {
    pub(crate) operation: Option<Box<dyn FnMut(&mut App, &UIEvent)>>,
    pub(crate) button: i32,
}

impl MouseOperation {
    pub fn new() -> MouseOperation {
        MouseOperation {
            operation: None,
            button: 0,
        }
    }
    pub fn start<F>(&mut self, operation: F, button: i32)
    where
        for<'a> F: FnMut(&'a mut App, &UIEvent) + 'static,
    {
        self.operation = Some(Box::new(operation));
        self.button = button;
    }

    pub fn reset(&mut self) {
        self.operation = None;
        self.button = 0;
    }
}
