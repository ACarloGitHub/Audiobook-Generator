//! Windows Job Object: ties child process lifetime to the parent.
//!
//! When the parent process exits (crash, normal exit, kill), Windows
//! automatically terminates every process assigned to the job. This
//! prevents orphaned `llama-server`, `qwen-tts`, and `voxcpm2-cli`
//! processes that hold GPU memory and network ports.
//!
//! On non-Windows platforms the functions are no-ops (Linux/macOS
//! already clean up child processes via process groups).

use std::sync::OnceLock;

/// Initialise the global Job Object. Call once at startup.
pub fn init() {
    #[cfg(windows)]
    {
        JOB.get_or_init(create_job);
    }
}

/// Assign a spawned child process to the global Job Object so it is
/// automatically killed when the parent exits. Call right after
/// `Command::spawn()`.
pub fn assign_child(child: &std::process::Child) {
    #[cfg(windows)]
    {
        let Some(job) = JOB.get() else {
            return;
        };
        if job.0.is_null() {
            return;
        }
        let pid = child.id();
        let handle = unsafe { ffi::OpenProcess(ffi::PROCESS_SET_QUOTA | ffi::PROCESS_TERMINATE, 0, pid) };
        if !handle.is_null() {
            unsafe {
                ffi::AssignProcessToJobObject(job.0, handle);
                ffi::CloseHandle(handle);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child;
    }
}

// ---------------------------------------------------------------------
// Windows implementation via raw FFI (no external crate needed)
// ---------------------------------------------------------------------

#[cfg(windows)]
#[allow(non_snake_case, non_upper_case_globals)]
mod ffi {
    use std::ffi::c_void;

    pub type HANDLE = *mut c_void;
    pub type BOOL = i32;

    pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    pub const JobObjectExtendedLimitInformation: u32 = 9;
    pub const PROCESS_SET_QUOTA: u32 = 0x0100;
    pub const PROCESS_TERMINATE: u32 = 0x0001;

    #[repr(C)]
    #[derive(Default)]
    pub struct IO_COUNTERS {
        pub QuadPart: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        pub PerProcessUserTimeLimit: IO_COUNTERS,
        pub PerJobUserTimeLimit: IO_COUNTERS,
        pub LimitFlags: u32,
        pub MinimumWorkingSetSize: usize,
        pub MaximumWorkingSetSize: usize,
        pub ActiveProcessLimit: u32,
        pub Affinity: usize,
        pub PriorityClass: u32,
        pub SchedulingClass: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        pub IoInfo: IO_COUNTERS,
        pub ProcessMemoryLimit: usize,
        pub JobMemoryLimit: usize,
        pub PeakProcessMemoryUsed: usize,
        pub PeakJobMemoryUsed: usize,
    }

    extern "system" {
        pub fn CreateJobObjectW(
            lpJobAttributes: *mut c_void,
            lpName: *const u16,
        ) -> HANDLE;
        pub fn SetInformationJobObject(
            hJob: HANDLE,
            info_class: u32,
            lpJobObjectInfo: *const c_void,
            cbJobObjectInfoLength: u32,
        ) -> BOOL;
        pub fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;
        pub fn OpenProcess(
            dwDesiredAccess: u32,
            bInheritHandle: BOOL,
            dwProcessId: u32,
        ) -> HANDLE;
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    }
}

/// Wrapper around a raw HANDLE that is safe to store in a static.
#[cfg(windows)]
#[repr(transparent)]
struct JobHandle(ffi::HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

#[cfg(windows)]
static JOB: OnceLock<JobHandle> = OnceLock::new();

#[cfg(windows)]
fn create_job() -> JobHandle {
    unsafe {
        let job = ffi::CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if job.is_null() {
            tracing::error!(
                "CreateJobObjectW failed, child processes will not be auto-killed on exit"
            );
            return JobHandle(std::ptr::null_mut());
        }
        let mut info: ffi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = ffi::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = ffi::SetInformationJobObject(
            job,
            ffi::JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<ffi::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            tracing::error!(
                "SetInformationJobObject failed, child processes will not be auto-killed on exit"
            );
        } else {
            tracing::info!(
                "Windows Job Object created (KILL_ON_JOB_CLOSE): child processes will be cleaned up on exit"
            );
        }
        JobHandle(job)
    }
}
