use crate::sync::SpinIrqLock;
use core::ops::Index;

pub const MAX_CPUS: usize = 128;

static CPUS: SpinIrqLock<Cpus<MAX_CPUS>> = SpinIrqLock::new(Cpus::empty());

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CpuId(usize);
impl CpuId {
    pub const fn as_usize(self) -> usize {
        self.0
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CpuState {
    Disabled,
    Waiting,
    Starting,
    Running,
}
#[derive(Copy, Clone, Debug)]
pub struct CpuInfo {
    pub apic_id: u32,
    pub is_bsp: bool,
    pub cpu_id: CpuId,
    pub state: CpuState,
}
pub struct Cpus<const N: usize> {
    cpus: [Option<CpuInfo>; N],
    count: usize,
}
impl<const N: usize> Cpus<N> {
    const fn empty() -> Self {
        Self {
            cpus: [const { None }; N],
            count: 0,
        }
    }
    pub fn push_with_id<F: FnOnce(CpuId) -> CpuInfo>(&mut self, f: F) -> Option<&CpuInfo> {
        if self.count >= self.cpus.len() {
            return None;
        }
        let id = CpuId(self.count);
        self.cpus[self.count] = Some(f(id));
        self.count += 1;
        self.cpus[self.count - 1].as_ref()
    }
    pub fn lookup(&self, apic_id: u32) -> Option<&CpuInfo> {
        for i in 0..self.count {
            if let Some(cpu) = &self.cpus[i]
                && cpu.apic_id == apic_id
            {
                return Some(cpu);
            }
        }
        None
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn iter(&self) -> impl Iterator<Item = &CpuInfo> {
        self.cpus[..self.count].iter().map(|v| v.as_ref().unwrap())
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut CpuInfo> {
        self.cpus[..self.count]
            .iter_mut()
            .map(|v| v.as_mut().unwrap())
    }
}
impl<const N: usize> Index<CpuId> for Cpus<N> {
    type Output = CpuInfo;

    fn index(&self, index: CpuId) -> &Self::Output {
        self.cpus[index.0].as_ref().unwrap()
    }
}
pub fn init<F: FnOnce(&mut Cpus<MAX_CPUS>)>(f: F) {
    CPUS.access(f)
}
pub fn access_mut<R, F: FnOnce(&mut Cpus<MAX_CPUS>) -> R>(f: F) -> R {
    CPUS.access(f)
}
pub fn get_cpu_id(apic_id: u32) -> Option<CpuId> {
    CPUS.access(|cpus| cpus.lookup(apic_id).map(|info| info.cpu_id))
}
