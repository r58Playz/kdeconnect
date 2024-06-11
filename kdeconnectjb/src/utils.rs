#[cfg(target_os = "ios")]
use log::{LevelFilter, Log};
#[cfg(target_os = "ios")]
use simplelog::SharedLogger;
#[cfg(target_os = "ios")]
use oslog::OsLogger;

#[cfg(target_os = "ios")]
pub struct IosLogWrapper(pub OsLogger, pub LevelFilter);

#[cfg(target_os = "ios")]
impl Log for IosLogWrapper {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.enabled(metadata)
    }
    fn log(&self, record: &log::Record) {
        self.0.log(record)
    }
    fn flush(&self) {
        self.0.flush()
    }
}

#[cfg(target_os = "ios")]
impl SharedLogger for IosLogWrapper {
    fn level(&self) -> LevelFilter {
        self.1
    }

    fn as_log(self: Box<Self>) -> Box<dyn Log> {
        Box::new(self.0)
    }

    fn config(&self) -> Option<&simplelog::Config> {
        None
    }
}

#[macro_export]
macro_rules! build_runtime {
    () => {
        $crate::RUNTIME.get_or_try_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build())
    };
}

pub struct FFIVec<T> {
    array: *mut T,
    length: usize,
    capacity: usize,
}

impl<T> From<Vec<T>> for FFIVec<T> {
    fn from(mut value: Vec<T>) -> Self {
        Self {
            array: value.as_mut_ptr(),
            length: value.len(),
            capacity: value.capacity(),
        }
    }
}

impl<T> From<FFIVec<T>> for Vec<T> {
    fn from(value: FFIVec<T>) -> Self {
        unsafe { Vec::<T>::from_raw_parts(value.array, value.length, value.capacity) }
    }
}
