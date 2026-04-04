use log::info;

pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(f: F) -> Self {
        Defer { f: Some(f) }
    }

    // 取消执行
    pub fn cancel(&mut self) {
        self.f = None;
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}

pub fn try_dump(addr: u64, len: usize) {
    let addr = addr as *mut u64;
    for i in 0..len {
        unsafe {
            let ptr = addr.sub(i);
            if ptr.is_null() {
                break;
            }
            info!("addr: {:#x?}, val: {:#x?}", ptr, ptr.read());
        }
    }
}
