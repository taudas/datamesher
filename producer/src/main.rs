use std::ffi::CStr;
use std::io;
use std::ptr;

use rdma_sys::*;

/// A DTMSHR producer node: one RDMA device opened, with a protection domain
/// and completion queue ready for queue pairs to be built on top of.
struct DtmshrNode {
    device_list: *mut *mut ibv_device,
    context: *mut ibv_context,
    pd: *mut ibv_pd,
    cq: *mut ibv_cq,
}

impl DtmshrNode {
    /// Opens the first available RDMA device on this host and brings up
    /// the minimal state (protection domain, completion queue) a producer
    /// needs before it can accept queue pairs from consumers.
    fn open_first_device() -> io::Result<Self> {
        unsafe {
            let mut num_devices: i32 = 0;
            let device_list = ibv_get_device_list(&mut num_devices);
            if device_list.is_null() || num_devices == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no RDMA devices found (is rdma-core installed and a device present?)",
                ));
            }

            let device = *device_list;
            let name = CStr::from_ptr(ibv_get_device_name(device))
                .to_string_lossy()
                .into_owned();
            eprintln!("dtmshr-producer: opening device {name}");

            let context = ibv_open_device(device);
            if context.is_null() {
                ibv_free_device_list(device_list);
                return Err(io::Error::last_os_error());
            }

            let pd = ibv_alloc_pd(context);
            if pd.is_null() {
                let err = io::Error::last_os_error();
                ibv_close_device(context);
                ibv_free_device_list(device_list);
                return Err(err);
            }

            const CQ_DEPTH: i32 = 16;
            let cq = ibv_create_cq(context, CQ_DEPTH, ptr::null_mut(), ptr::null_mut(), 0);
            if cq.is_null() {
                let err = io::Error::last_os_error();
                ibv_dealloc_pd(pd);
                ibv_close_device(context);
                ibv_free_device_list(device_list);
                return Err(err);
            }

            Ok(Self {
                device_list,
                context,
                pd,
                cq,
            })
        }
    }
}

impl Drop for DtmshrNode {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_cq(self.cq);
            ibv_dealloc_pd(self.pd);
            ibv_close_device(self.context);
            ibv_free_device_list(self.device_list);
        }
    }
}

fn main() -> io::Result<()> {
    let node = DtmshrNode::open_first_device()?;
    eprintln!("dtmshr-producer: node up, protection domain and completion queue ready");
    drop(node);
    Ok(())
}
