use log::info;

pub struct PushStack<'a> {
    stack: &'a mut [u64],
    rsp: usize,
}

impl<'a> PushStack<'a> {
    pub fn new(stack: &'a mut [u64]) -> PushStack<'a> {
        PushStack {
            rsp: stack.len(),
            stack,
        }
    }

    pub fn rsp(&self) -> usize {
        self.rsp
    }

    pub fn push_u64(&mut self, data: u64) -> usize {
        info!("push {:X?}", data);
        if self.rsp > 0 {
            self.stack[self.rsp - 1] = data;
            self.rsp -= 1;
        } else {
            panic!("push stack overflow");
        }
        self.rsp
    }
}
