use log::{LevelFilter, Log};
use oslog::OsLogger;
use simplelog::SharedLogger;

pub struct IosLogWrapper(pub OsLogger, pub LevelFilter);

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
        RUNTIME.get_or_try_init(|| Builder::new_multi_thread().enable_all().build())
    };
}

#[macro_export]
macro_rules! check_str {
    ($var:ident) => {
        let $var = unsafe {
            if $var.is_null() {
                return false;
            }
            CStr::from_ptr($var).to_string_lossy().to_string()
        };
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
