pub const COM_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// COM Automation Dispatch Engine (Win32 OLE Automation via IDispatch)
// -----------------------------------------------------------------------------
pub struct ComOptions<'a> {
    pub progid: &'a str,
    pub method: &'a str,
    pub args: &'a [Value],
    pub attach: bool,
    pub bring_to_front: bool,
    pub timeout: Duration,
}

#[cfg(not(windows))]
pub fn execute_com_request(_opts: ComOptions) -> Result<Value, String> {
    Err("Windows COM Automation (bind:com) is only supported on Windows hosts".to_string())
}

#[cfg(windows)]
pub fn execute_com_request(opts: ComOptions) -> Result<Value, String> {
    let (tx, rx) = mpsc::channel();
    let progid = opts.progid.to_string();
    let method = opts.method.to_string();
    let args = opts.args.to_vec();
    let attach = opts.attach;
    let bring_to_front = opts.bring_to_front;
    let timeout = opts.timeout;

    thread::spawn(move || {
        let res = com_native::invoke_com(&progid, &method, &args, attach, bring_to_front);
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(res) => res,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("COM execution timed out after {}ms ({}.{})", timeout.as_millis(), opts.progid, opts.method))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("COM execution worker thread disconnected or panicked ({}.{})", opts.progid, opts.method))
        }
    }
}

#[cfg(windows)]
mod com_native {
    use std::ffi::c_void;
    use std::ptr;
    use serde_json::{json, Value};

    type HRESULT = i32;
    type LCID = u32;
    type DISPID = i32;
    type VARTYPE = u16;

    const S_OK: HRESULT = 0;
    const DISPATCH_METHOD: u16 = 0x1;
    const DISPATCH_PROPERTYGET: u16 = 0x2;
    const DISPATCH_PROPERTYPUT: u16 = 0x4;
    const LOCALE_USER_DEFAULT: LCID = 0x0400;

    const VT_EMPTY: VARTYPE = 0;
    const VT_NULL: VARTYPE = 1;
    const VT_I2: VARTYPE = 2;
    const VT_I4: VARTYPE = 3;
    const VT_R4: VARTYPE = 4;
    const VT_R8: VARTYPE = 5;
    const VT_BSTR: VARTYPE = 8;
    const VT_DISPATCH: VARTYPE = 9;
    const VT_BOOL: VARTYPE = 11;
    const VT_I1: VARTYPE = 16;
    const VT_UI1: VARTYPE = 17;
    const VT_UI2: VARTYPE = 18;
    const VT_UI4: VARTYPE = 19;
    const VT_I8: VARTYPE = 20;
    const VT_UI8: VARTYPE = 21;
    const VT_INT: VARTYPE = 22;
    const VT_UINT: VARTYPE = 23;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct GUID {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    const IID_NULL: GUID = GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    const IID_IDISPATCH: GUID = GUID {
        data1: 0x00020400,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    #[repr(C)]
    pub struct VARIANT {
        pub vt: VARTYPE,
        pub w_reserved1: u16,
        pub w_reserved2: u16,
        pub w_reserved3: u16,
        pub data: [u8; 16],
    }

    #[repr(C)]
    pub struct DISPPARAMS {
        pub rgvarg: *mut VARIANT,
        pub rgdispid_named_args: *mut DISPID,
        pub c_args: u32,
        pub c_named_args: u32,
    }

    #[repr(C)]
    pub struct IUnknownVtbl {
        pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    pub struct IDispatchVtbl {
        pub query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        pub get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
        pub get_type_info: unsafe extern "system" fn(*mut c_void, u32, LCID, *mut *mut c_void) -> HRESULT,
        pub get_ids_of_names: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            *const *const u16,
            u32,
            LCID,
            *mut DISPID,
        ) -> HRESULT,
        pub invoke: unsafe extern "system" fn(
            *mut c_void,
            DISPID,
            *const GUID,
            LCID,
            u16,
            *mut DISPPARAMS,
            *mut VARIANT,
            *mut c_void,
            *mut u32,
        ) -> HRESULT,
    }

    #[link(name = "ole32")]
    #[link(name = "oleaut32")]
    extern "system" {
        fn CoInitializeEx(pv_reserved: *mut c_void, co_init: u32) -> HRESULT;
        fn CoUninitialize();
        fn CLSIDFromProgID(lpsz_prog_id: *const u16, lpclsid: *mut GUID) -> HRESULT;
        fn CoCreateInstance(
            rclsid: *const GUID,
            p_unk_outer: *mut c_void,
            dw_cls_context: u32,
            riid: *const GUID,
            ppv: *mut *mut c_void,
        ) -> HRESULT;
        fn GetActiveObject(rclsid: *const GUID, pv_reserved: *mut c_void, pp_unk: *mut *mut c_void) -> HRESULT;
        fn SysAllocStringLen(str_in: *const u16, ui: u32) -> *mut u16;
        fn SysStringLen(bstr: *const u16) -> u32;
        fn SysFreeString(bstr_string: *mut u16);
        fn VariantInit(pvarg: *mut VARIANT);
        fn VariantClear(pvarg: *mut VARIANT) -> HRESULT;
    }

    pub fn invoke_com(
        progid: &str,
        method_path: &str,
        args: &[Value],
        attach: bool,
        bring_to_front: bool,
    ) -> Result<Value, String> {
        unsafe {
            let _ = CoInitializeEx(ptr::null_mut(), 2 /* COINIT_APARTMENTTHREADED */);
            let res = invoke_com_inner(progid, method_path, args, attach, bring_to_front);
            CoUninitialize();
            res
        }
    }

    unsafe fn invoke_com_inner(
        progid: &str,
        method_path: &str,
        args: &[Value],
        attach: bool,
        bring_to_front: bool,
    ) -> Result<Value, String> {
        let mut clsid = GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
        let wide_progid: Vec<u16> = progid.encode_utf16().chain(std::iter::once(0)).collect();

        let hr = CLSIDFromProgID(wide_progid.as_ptr(), &mut clsid);
        if hr != S_OK {
            return Err(format!("CLSIDFromProgID failed for ProgID '{}' (HRESULT 0x{:08X})", progid, hr as u32));
        }

        let mut dispatch_ptr: *mut c_void = ptr::null_mut();

        if attach {
            let mut unk_ptr: *mut c_void = ptr::null_mut();
            if GetActiveObject(&clsid, ptr::null_mut(), &mut unk_ptr) == S_OK && !unk_ptr.is_null() {
                let vtbl = *(unk_ptr as *mut *mut IUnknownVtbl);
                ((*vtbl).query_interface)(unk_ptr, &IID_IDISPATCH, &mut dispatch_ptr);
                ((*vtbl).release)(unk_ptr);
            }
        }

        if dispatch_ptr.is_null() {
            let hr = CoCreateInstance(&clsid, ptr::null_mut(), 5 /* CLSCTX_SERVER */, &IID_IDISPATCH, &mut dispatch_ptr);
            if hr != S_OK || dispatch_ptr.is_null() {
                return Err(format!("Failed to instantiate COM object '{}' (HRESULT 0x{:08X})", progid, hr as u32));
            }
        }

        if bring_to_front {
            let mut vis_id: DISPID = 0;
            let wide_vis: Vec<u16> = "Visible".encode_utf16().chain(std::iter::once(0)).collect();
            let vis_name_ptrs = [wide_vis.as_ptr()];
            let vtbl = *(dispatch_ptr as *mut *mut IDispatchVtbl);
            if ((*vtbl).get_ids_of_names)(dispatch_ptr, &IID_NULL, vis_name_ptrs.as_ptr(), 1, LOCALE_USER_DEFAULT, &mut vis_id) == S_OK {
                let mut true_var = VARIANT { vt: VT_BOOL, w_reserved1: 0, w_reserved2: 0, w_reserved3: 0, data: [0; 16] };
                let val: i16 = -1;
                ptr::copy_nonoverlapping(&val, true_var.data.as_mut_ptr() as *mut i16, 1);
                let mut prop_put_id: DISPID = -3;
                let mut vis_params = DISPPARAMS {
                    rgvarg: &mut true_var,
                    rgdispid_named_args: &mut prop_put_id,
                    c_args: 1,
                    c_named_args: 1,
                };
                let mut dummy_res = VARIANT { vt: VT_EMPTY, w_reserved1: 0, w_reserved2: 0, w_reserved3: 0, data: [0; 16] };
                let _ = ((*vtbl).invoke)(dispatch_ptr, vis_id, &IID_NULL, LOCALE_USER_DEFAULT, DISPATCH_PROPERTYPUT, &mut vis_params, &mut dummy_res, ptr::null_mut(), ptr::null_mut());
            }
        }

        let segments: Vec<&str> = method_path.split('.').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            let vtbl = *(dispatch_ptr as *mut *mut IDispatchVtbl);
            ((*vtbl).release)(dispatch_ptr);
            return Ok(json!({ "status": "connected", "progid": progid }));
        }

        let mut current_dispatch = dispatch_ptr;

        for &seg in &segments[..segments.len() - 1] {
            let vtbl = *(current_dispatch as *mut *mut IDispatchVtbl);
            let wide_name: Vec<u16> = seg.encode_utf16().chain(std::iter::once(0)).collect();
            let name_ptrs = [wide_name.as_ptr()];
            let mut dispid: DISPID = 0;

            let hr = ((*vtbl).get_ids_of_names)(
                current_dispatch,
                &IID_NULL,
                name_ptrs.as_ptr(),
                1,
                LOCALE_USER_DEFAULT,
                &mut dispid,
            );
            if hr != S_OK {
                ((*vtbl).release)(current_dispatch);
                return Err(format!("Property/Method '{}' not found on COM object '{}' (HRESULT 0x{:08X})", seg, progid, hr as u32));
            }

            let mut empty_params = DISPPARAMS {
                rgvarg: ptr::null_mut(),
                rgdispid_named_args: ptr::null_mut(),
                c_args: 0,
                c_named_args: 0,
            };
            let mut sub_var = VARIANT { vt: VT_EMPTY, w_reserved1: 0, w_reserved2: 0, w_reserved3: 0, data: [0; 16] };
            VariantInit(&mut sub_var);

            let hr = ((*vtbl).invoke)(
                current_dispatch,
                dispid,
                &IID_NULL,
                LOCALE_USER_DEFAULT,
                DISPATCH_PROPERTYGET | DISPATCH_METHOD,
                &mut empty_params,
                &mut sub_var,
                ptr::null_mut(),
                ptr::null_mut(),
            );

            if hr != S_OK {
                VariantClear(&mut sub_var);
                ((*vtbl).release)(current_dispatch);
                return Err(format!("Failed to retrieve property '{}' on COM object '{}' (HRESULT 0x{:08X})", seg, progid, hr as u32));
            }

            if sub_var.vt != VT_DISPATCH {
                VariantClear(&mut sub_var);
                ((*vtbl).release)(current_dispatch);
                return Err(format!("Property '{}' on COM object '{}' did not return an IDispatch object", seg, progid));
            }

            let next_dispatch = *(sub_var.data.as_ptr() as *const *mut c_void);
            if next_dispatch.is_null() {
                VariantClear(&mut sub_var);
                ((*vtbl).release)(current_dispatch);
                return Err(format!("Property '{}' on COM object '{}' returned NULL dispatch pointer", seg, progid));
            }

            ((*vtbl).release)(current_dispatch);
            current_dispatch = next_dispatch;
        }

        let last_segment = segments[segments.len() - 1];
        let vtbl = *(current_dispatch as *mut *mut IDispatchVtbl);
        let wide_name: Vec<u16> = last_segment.encode_utf16().chain(std::iter::once(0)).collect();
        let name_ptrs = [wide_name.as_ptr()];
        let mut dispid: DISPID = 0;

        let hr = ((*vtbl).get_ids_of_names)(
            current_dispatch,
            &IID_NULL,
            name_ptrs.as_ptr(),
            1,
            LOCALE_USER_DEFAULT,
            &mut dispid,
        );
        if hr != S_OK {
            ((*vtbl).release)(current_dispatch);
            return Err(format!("Method/Property '{}' not found on COM object '{}' (HRESULT 0x{:08X})", last_segment, progid, hr as u32));
        }

        let mut var_args: Vec<VARIANT> = Vec::new();
        for arg in args.iter().rev() {
            let mut v = VARIANT { vt: VT_EMPTY, w_reserved1: 0, w_reserved2: 0, w_reserved3: 0, data: [0; 16] };
            VariantInit(&mut v);
            match arg {
                Value::String(s) => {
                    v.vt = VT_BSTR;
                    let wide_s: Vec<u16> = s.encode_utf16().collect();
                    let bstr = SysAllocStringLen(wide_s.as_ptr(), wide_s.len() as u32);
                    ptr::copy_nonoverlapping(&bstr, v.data.as_mut_ptr() as *mut *mut u16, 1);
                }
                Value::Number(n) if n.is_i64() => {
                    let val = n.as_i64().unwrap();
                    if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
                        v.vt = VT_I4;
                        let val32 = val as i32;
                        ptr::copy_nonoverlapping(&val32, v.data.as_mut_ptr() as *mut i32, 1);
                    } else {
                        v.vt = VT_I8;
                        ptr::copy_nonoverlapping(&val, v.data.as_mut_ptr() as *mut i64, 1);
                    }
                }
                Value::Number(n) => {
                    v.vt = VT_R8;
                    let val = n.as_f64().unwrap_or(0.0);
                    ptr::copy_nonoverlapping(&val, v.data.as_mut_ptr() as *mut f64, 1);
                }
                Value::Bool(b) => {
                    v.vt = VT_BOOL;
                    let val: i16 = if *b { -1 } else { 0 };
                    ptr::copy_nonoverlapping(&val, v.data.as_mut_ptr() as *mut i16, 1);
                }
                Value::Null => {
                    v.vt = VT_NULL;
                }
                _ => {
                    let s = arg.to_string();
                    v.vt = VT_BSTR;
                    let wide_s: Vec<u16> = s.encode_utf16().collect();
                    let bstr = SysAllocStringLen(wide_s.as_ptr(), wide_s.len() as u32);
                    ptr::copy_nonoverlapping(&bstr, v.data.as_mut_ptr() as *mut *mut u16, 1);
                }
            }
            var_args.push(v);
        }

        let mut params = DISPPARAMS {
            rgvarg: if var_args.is_empty() { ptr::null_mut() } else { var_args.as_mut_ptr() },
            rgdispid_named_args: ptr::null_mut(),
            c_args: var_args.len() as u32,
            c_named_args: 0,
        };

        let mut result_var = VARIANT { vt: VT_EMPTY, w_reserved1: 0, w_reserved2: 0, w_reserved3: 0, data: [0; 16] };
        VariantInit(&mut result_var);

        let hr = ((*vtbl).invoke)(
            current_dispatch,
            dispid,
            &IID_NULL,
            LOCALE_USER_DEFAULT,
            DISPATCH_METHOD | DISPATCH_PROPERTYGET,
            &mut params,
            &mut result_var,
            ptr::null_mut(),
            ptr::null_mut(),
        );

        for mut v in var_args {
            VariantClear(&mut v);
        }
        ((*vtbl).release)(current_dispatch);

        if hr != S_OK {
            VariantClear(&mut result_var);
            return Err(format!("IDispatch::Invoke for '{}.{}' failed (HRESULT 0x{:08X})", progid, last_segment, hr as u32));
        }

        let out_val = match result_var.vt {
            VT_EMPTY | VT_NULL => Value::Null,
            VT_BSTR => {
                let bstr_ptr = *(result_var.data.as_ptr() as *const *mut u16);
                if bstr_ptr.is_null() {
                    Value::String(String::new())
                } else {
                    let len = SysStringLen(bstr_ptr) as usize;
                    let slice = std::slice::from_raw_parts(bstr_ptr, len);
                    Value::String(String::from_utf16_lossy(slice))
                }
            }
            VT_I1 => json!(*(result_var.data.as_ptr() as *const i8)),
            VT_UI1 => json!(*(result_var.data.as_ptr() as *const u8)),
            VT_I2 => json!(*(result_var.data.as_ptr() as *const i16)),
            VT_UI2 => json!(*(result_var.data.as_ptr() as *const u16)),
            VT_I4 | VT_INT => json!(*(result_var.data.as_ptr() as *const i32)),
            VT_UI4 | VT_UINT => json!(*(result_var.data.as_ptr() as *const u32)),
            VT_I8 => json!(*(result_var.data.as_ptr() as *const i64)),
            VT_UI8 => json!(*(result_var.data.as_ptr() as *const u64)),
            VT_R4 => json!(*(result_var.data.as_ptr() as *const f32)),
            VT_R8 => json!(*(result_var.data.as_ptr() as *const f64)),
            VT_BOOL => json!(*(result_var.data.as_ptr() as *const i16) != 0),
            VT_DISPATCH => {
                let disp = *(result_var.data.as_ptr() as *const *mut c_void);
                json!({ "_type": "IDispatch", "ptr": format!("0x{:x}", disp as usize) })
            }
            _ => Value::Null,
        };

        VariantClear(&mut result_var);
        Ok(out_val)
    }
}
"###;
