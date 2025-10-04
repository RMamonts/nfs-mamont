pub struct Defer<'a> {
    call_back: Option<Box<dyn FnOnce() + 'a>>,
}

impl<'a> Defer<'a> {
    fn new(call_back: Box<dyn FnOnce() + 'a>) -> Defer<'a> {
        Self { call_back: Some(call_back) }
    }
}

impl<'a> Drop for Defer<'a> {
    fn drop(&mut self) {
        if let Some(call_back) = self.call_back.take() {
            call_back()
        }
    }
}

pub fn defer<'a>(call_back: impl FnOnce() + 'a) -> Defer<'a> {
    Defer::new(Box::new(call_back))
}
