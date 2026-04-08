use std::os::raw::c_void;

use magnus::{Error, Ruby, Value, function, method, prelude::*};
use oas::OasClient;
use rb_sys::rb_thread_call_without_gvl;
use tokio::runtime;

#[magnus::wrap(class = "RbOas::OasClient", free_immediately, size)]
struct RbOasClient(OasClient);

struct CallData<'a> {
    client: &'a OasClient,
    operation_id: String,
    result: Option<anyhow::Result<serde_json::Value>>,
}

extern "C" fn call_without_gvl(data: *mut c_void) -> *mut c_void {
    let data = unsafe { &mut *(data as *mut CallData) };
    let rt = runtime::LocalRuntime::new().unwrap();
    data.result = Some(rt.block_on(async {
        Ok(data
            .client
            .request(&data.operation_id)
            .build()
            .await?
            .send()
            .await?
            .body
            .unwrap())
    }));
    std::ptr::null_mut()
}

impl RbOasClient {
    fn new(base_url: String, docs_href: String) -> Self {
        Self(OasClient::new(base_url.as_ref(), docs_href.as_ref()))
    }

    fn call(&self, operation_id: String) -> Result<Value, Error> {
        let mut data = CallData {
            client: &self.0,
            operation_id: operation_id.clone(),
            result: None,
        };

        unsafe {
            rb_thread_call_without_gvl(
                Some(call_without_gvl),
                &mut data as *mut _ as *mut c_void,
                None,
                std::ptr::null_mut(),
            );
        }
        let ruby = magnus::Ruby::get().unwrap();

        let result = data
            .result
            .unwrap()
            .map_err(|e| Error::new(ruby.exception_runtime_error(), format!("{}", e)))?;
        let result: Value = { serde_magnus::serialize(&ruby, &result).unwrap() };
        Ok(result)
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("RbOas")?;
    let oas_client = module.define_class("OasClient", ruby.class_object())?;
    oas_client.define_singleton_method("new", function!(RbOasClient::new, 2))?;
    oas_client.define_method("call", method!(RbOasClient::call, 1))?;
    Ok(())
}
