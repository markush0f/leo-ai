use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use crate::error::AudioError;

const PA_SAMPLE_FLOAT32LE: i32 = 5;
const PA_STREAM_PLAYBACK: i32 = 1;
const PA_STREAM_RECORD: i32 = 2;
const PA_UNSPECIFIED: u32 = u32::MAX;

#[repr(C)]
struct PaSampleSpec {
    format: i32,
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct PaBufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

enum PaSimple {}

unsafe extern "C" {
    fn pa_simple_new(
        server: *const c_char,
        name: *const c_char,
        dir: i32,
        dev: *const c_char,
        stream_name: *const c_char,
        ss: *const PaSampleSpec,
        map: *const c_void,
        attr: *const PaBufferAttr,
        error: *mut c_int,
    ) -> *mut PaSimple;
    fn pa_simple_free(s: *mut PaSimple);
    fn pa_simple_read(
        s: *mut PaSimple,
        data: *mut c_void,
        bytes: usize,
        error: *mut c_int,
    ) -> c_int;
    fn pa_simple_write(
        s: *mut PaSimple,
        data: *const c_void,
        bytes: usize,
        error: *mut c_int,
    ) -> c_int;
    fn pa_simple_flush(s: *mut PaSimple, error: *mut c_int) -> c_int;
    fn pa_strerror(error: c_int) -> *const c_char;
}

fn pulse_error(code: c_int) -> String {
    unsafe {
        let ptr = pa_strerror(code);
        if ptr.is_null() {
            format!("error de PulseAudio {code}")
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

pub struct PulseStream {
    ptr: *mut PaSimple,
}

unsafe impl Send for PulseStream {}

impl PulseStream {
    fn open(
        app_name: &str,
        stream_name: &str,
        device: &str,
        direction: i32,
        rate: u32,
        latency_bytes: u32,
    ) -> Result<Self, AudioError> {
        let app = CString::new(app_name).map_err(|e| AudioError::pulse(e.to_string()))?;
        let stream = CString::new(stream_name).map_err(|e| AudioError::pulse(e.to_string()))?;
        let dev = CString::new(device).map_err(|e| AudioError::pulse(e.to_string()))?;
        let spec = PaSampleSpec {
            format: PA_SAMPLE_FLOAT32LE,
            rate,
            channels: 1,
        };
        let attr = PaBufferAttr {
            maxlength: PA_UNSPECIFIED,
            tlength: latency_bytes,
            prebuf: PA_UNSPECIFIED,
            minreq: PA_UNSPECIFIED,
            fragsize: latency_bytes,
        };

        let mut error = 0;
        let ptr = unsafe {
            pa_simple_new(
                ptr::null(),
                app.as_ptr(),
                direction,
                dev.as_ptr(),
                stream.as_ptr(),
                &spec,
                ptr::null(),
                &attr,
                &mut error,
            )
        };
        if ptr.is_null() {
            return Err(AudioError::Device(pulse_error(error)));
        }
        Ok(Self { ptr })
    }

    pub fn record(
        app: &str,
        device: &str,
        rate: u32,
        latency_bytes: u32,
    ) -> Result<Self, AudioError> {
        Self::open(
            app,
            "micrófono",
            device,
            PA_STREAM_RECORD,
            rate,
            latency_bytes,
        )
    }

    pub fn playback(
        app: &str,
        device: &str,
        rate: u32,
        latency_bytes: u32,
    ) -> Result<Self, AudioError> {
        Self::open(
            app,
            "altavoces",
            device,
            PA_STREAM_PLAYBACK,
            rate,
            latency_bytes,
        )
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<(), AudioError> {
        let mut error = 0;
        let rc =
            unsafe { pa_simple_read(self.ptr, buf.as_mut_ptr().cast(), buf.len(), &mut error) };
        if rc < 0 {
            Err(AudioError::pulse(pulse_error(error)))
        } else {
            Ok(())
        }
    }

    pub fn write(&self, buf: &[u8]) -> Result<(), AudioError> {
        let mut error = 0;
        let rc = unsafe { pa_simple_write(self.ptr, buf.as_ptr().cast(), buf.len(), &mut error) };
        if rc < 0 {
            Err(AudioError::pulse(pulse_error(error)))
        } else {
            Ok(())
        }
    }

    pub fn flush(&self) -> Result<(), AudioError> {
        let mut error = 0;
        let rc = unsafe { pa_simple_flush(self.ptr, &mut error) };
        if rc < 0 {
            Err(AudioError::pulse(pulse_error(error)))
        } else {
            Ok(())
        }
    }
}

impl Drop for PulseStream {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { pa_simple_free(self.ptr) }
        }
    }
}
