#[pyo3::pymodule]
mod core {

    use oas::OasClient as RustOasClient;
    use pyo3::prelude::*;
    use pythonize::{depythonize, pythonize};
    use serde_json::Value;
    use std::sync::Arc;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let log_reset_handler = pyo3_log::init();
        let _ = m.add("log_reset_handler", LogResetHandler(log_reset_handler));
        Ok(())
    }

    #[pyclass]
    struct LogResetHandler(pyo3_log::ResetHandle);

    #[pymethods]
    impl LogResetHandler {
        /// When logging configuration is changed in Python, this callback should be called to
        /// reinitialize the Rust counterpart.
        fn reset(&self) -> PyResult<()> {
            self.0.reset();
            Ok(())
        }
    }

    #[pyclass]
    struct OasClient(Arc<RustOasClient>);

    #[pymethods]
    impl OasClient {
        #[new]
        fn new(base_url: &str, doc_href: &str) -> Self {
            Self(Arc::new(RustOasClient::new(base_url, doc_href)))
        }

        #[pyo3(signature = (operation_id, /, body = None))]
        fn call<'py>(
            &self,
            py: Python<'py>,
            operation_id: String,
            body: Option<&Bound<'py, PyAny>>,
        ) -> PyResult<Bound<'py, PyAny>> {
            let slf = Arc::clone(&self.0);
            let body: Option<Value> = match body {
                Some(v) => depythonize(v)?,
                None => None,
            };
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                let mut request = slf.request(&operation_id);
                if let Some(body) = body {
                    request = request.body(body);
                }
                let response = request.build().await?.send().await?;

                let res: PyResult<Py<PyAny>> =
                    Python::attach(|py| Ok(pythonize(py, &response.body)?.into()));
                res
            })
        }
    }
}
