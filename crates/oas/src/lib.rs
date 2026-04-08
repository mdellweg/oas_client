use anyhow::{Result, anyhow, bail};
use log::{debug, info};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Semaphore;

mod oas;

#[derive(Debug)]
struct Spec {
    openapi: oas::OpenApi,
    operations: HashMap<String, (oas::Method, String)>,
}

impl Spec {
    fn try_new(raw_spec: &str) -> Result<Self> {
        let openapi: oas::OpenApi = serde_json::from_str(raw_spec)?;
        let mut operations = HashMap::new();
        if let Some(ref paths) = openapi.paths {
            for (path, path_item) in paths {
                for (method, operation) in &path_item.operations {
                    if let Some(operation_id) = &operation.operation_id {
                        operations.insert(operation_id.clone(), (method.clone(), path.clone()));
                    }
                }
            }
        }
        Ok(Self {
            openapi,
            operations,
        })
    }
}

pub struct OasClient {
    base_url: String,
    doc_href: Option<String>,
    client: reqwest::Client,
    parsed_spec: OnceLock<Spec>,
    spec_semaphore: Semaphore,
}

impl OasClient {
    pub fn new(base_url: &str, doc_href: &str) -> Self {
        Self {
            base_url: base_url.to_owned(),
            doc_href: Some(doc_href.to_owned()),
            client: reqwest::Client::new(),
            parsed_spec: OnceLock::new(),
            spec_semaphore: Semaphore::new(1),
        }
    }

    pub fn from_docs(base_url: &str, api_docs: &str) -> Result<Self> {
        let parsed_spec = OnceLock::new();
        parsed_spec.set(Spec::try_new(api_docs)?).unwrap();
        Ok(Self {
            base_url: base_url.to_owned(),
            doc_href: None,
            client: reqwest::Client::new(),
            parsed_spec,
            spec_semaphore: Semaphore::new(1),
        })
    }

    async fn fetch_raw_spec(&self) -> Result<String> {
        let url = &(self.base_url.clone() + self.doc_href.as_ref().unwrap());
        let response = self.client.get(url).send().await?.error_for_status()?;
        Ok(response.text().await?)
    }

    async fn parsed_spec(&self) -> Result<&Spec> {
        match self.parsed_spec.get() {
            Some(spec) => Ok(spec),
            None => {
                debug!("Fetch openapi spec from server.");
                let _lock = self.spec_semaphore.acquire().await.unwrap();
                let spec = Spec::try_new(&self.fetch_raw_spec().await?)?;
                self.parsed_spec.set(spec).unwrap();
                Ok(self.parsed_spec.get().unwrap())
            }
        }
    }

    pub fn request<'a>(&'a self, operation_id: &str) -> RequestBuilder<'a> {
        RequestBuilder {
            client: self,
            operation_id: Some(operation_id.to_string()),
            method: None,
            path: None,
            parameters: HashMap::new(),
            body: None,
        }
    }

    pub fn request_from_method_path<'a>(
        &'a self,
        method: oas::Method,
        path: &str,
    ) -> RequestBuilder<'a> {
        RequestBuilder {
            client: self,
            operation_id: None,
            method: Some(method),
            path: Some(path.to_owned()),
            parameters: HashMap::new(),
            body: None,
        }
    }
}

pub struct Response {
    // response: reqwest::Response,
    pub status: u16,
    pub body: Option<Value>,
}

pub struct RequestBuilder<'a> {
    client: &'a OasClient,
    operation_id: Option<String>,
    method: Option<oas::Method>,
    path: Option<String>,
    parameters: HashMap<String, Value>,
    body: Option<Value>,
}

pub struct Request<'a> {
    client: &'a OasClient,
    method: oas::Method,
    path: String,
    body: Option<Value>,
}

impl<'a> std::fmt::Debug for Request<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("body", &self.body)
            .finish()
    }
}

impl<'a> RequestBuilder<'a> {
    /// Set body to be built into the request.
    pub fn body(mut self, value: Value) -> Self {
        self.body = Some(value);
        self
    }

    /// Add a parameter to the request.
    pub fn parameter(mut self, name: &str, value: Value) -> Self {
        self.parameters.insert(name.to_owned(), value);
        self
    }

    /// Build request according to the OAS spec.
    pub async fn build(mut self) -> Result<Request<'a>> {
        let method: oas::Method;
        let mut path: String;
        let parsed_spec = self.client.parsed_spec().await?;
        if let Some(operation_id) = &self.operation_id {
            if let Some((found_method, found_path)) = parsed_spec.operations.get(operation_id) {
                method = found_method.clone();
                path = found_path.to_string();
            } else {
                bail!("Operation Id unknown.");
            }
            info!("Operation ID: '{}'", operation_id)
        } else {
            method = self.method.ok_or(anyhow!("Method not provided"))?;
            path = self.path.ok_or(anyhow!("Path not provided."))?;
        }
        let operation = &parsed_spec
            .openapi
            .paths
            .as_ref()
            .unwrap()
            .get(&path)
            .ok_or(anyhow!("Path unknown."))?
            .operations
            .get(&method)
            .ok_or(anyhow!("Method unknown."))?;

        if let Some(parameters) = operation.parameters.as_ref() {
            for parameter in parameters {
                if let Some(value) = self.parameters.remove(&parameter.name) {
                    if let Some(schema) = parameter.schema.as_ref() {
                        let validator = jsonschema::options().offline().build(schema)?;
                        if !validator.is_valid(&value) {
                            // TODO validator.validate would be nicer, but the Errors have a
                            // demanding lifetime.
                            bail!("Validating the body failed.");
                        }
                    }
                    match parameter.location {
                        oas::ParameterLocation::Path => {
                            let marker = "{".to_string() + &parameter.name + "}";
                            path = path.replace(&marker, &value.to_string());
                        }
                        _ => todo!(),
                    }
                } else {
                    if parameter.is_required() {
                        bail!("Required parameter '{}' missing.", parameter.name);
                    }
                }
            }
        }
        if !self.parameters.is_empty() {
            bail!("Unexpected parameters specified.");
        }
        if let Some(request_body) = operation.request_body.as_ref() {
            if let Some(body) = &self.body {
                if let Some(media_type) = request_body.content.get("application/json") {
                    if let Some(schema) = media_type.schema.as_ref() {
                        let validator = jsonschema::options().offline().build(schema)?;
                        if !validator.is_valid(body) {
                            // TODO validator.validate would be nicer, but the Errors have a
                            // demanding lifetime.
                            bail!("Validating the body failed.");
                        }
                    }
                } else {
                    bail!("No suitable content-type found.");
                }
            } else {
                if request_body.required.unwrap_or(false) {
                    bail!("Required request body is missing.")
                }
            }
        } else {
            if self.body.is_some() {
                bail!("No body expected.");
            }
        }
        Ok(Request {
            client: self.client,
            method,
            path,
            body: self.body,
        })
    }
}

impl<'a> Request<'a> {
    pub async fn send(self) -> Result<Response> {
        let url = self.client.base_url.clone() + &self.path;
        let method: reqwest::Method = self.method.into();
        info!("Request: {} {}", method, url);
        let mut request = self.client.client.request(method, url);
        if let Some(body) = self.body {
            request = request.json(&body);
        }

        let result = request.send().await?;
        info!("Response: {}", result.status());
        let result = result.error_for_status()?;
        Ok(Response {
            status: result.status().into(),
            body: result.json().await.ok(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use httpmock::{Method, MockServer};
    use serde_json::json;

    const MOCK_API: &'static str = include_str!("../../../assets/mock_api.json");

    fn mock_server() -> MockServer {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(Method::GET).path("/docs/api.json");
            then.status(200).body(MOCK_API);
        });
        server.mock(|when, then| {
            when.method(Method::GET).path("/api/v1/hello/");
            then.status(200).body(r#"{"msg": "Hello, World!"}"#);
        });
        server.mock(|when, then| {
            when.method(Method::DELETE).path("/api/v1/hello/");
            then.status(204);
        });
        server.mock(|when, then| {
            when.method(Method::POST)
                .path("/api/v1/hello/")
                .json_body(json!({"name": "Alice"}));
            then.status(201)
                .json_body_obj(&json!({"msg": "Hello, Alice!"}));
        });
        server.mock(|when, then| {
            when.method(Method::GET).path("/api/v1/user/42/");
            then.status(200).json_body_obj(&json!({"name": "Alice"}));
        });
        server.mock(|when, then| {
            when.method(Method::GET).path("/broken-docs/api.json");
            then.status(200).json_body_obj(&json!({}));
        });
        server
    }

    fn mock_client() -> OasClient {
        let server = mock_server();
        OasClient::new(&server.url("/"), "docs/api.json")
    }

    mod request_builder {
        use super::*;

        #[tokio::test]
        async fn calls_hello_with_provided_api_doc() {
            let server = mock_server();
            let client = OasClient::from_docs(&server.url("/"), MOCK_API).expect("parse api spec");

            let request = client.request("hello");

            assert_eq!(request.operation_id.as_ref().unwrap(), "hello");
            assert!(request.method.is_none());
            assert!(request.path.is_none());

            let request = request.build().await.expect("request validates");
            assert_eq!(request.method, oas::Method::Get);
            assert_eq!(request.path, "api/v1/hello/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 200);
            assert_eq!(
                response.body.unwrap(),
                serde_json::json!({"msg": "Hello, World!"})
            );
        }

        #[tokio::test]
        async fn errors_on_unknown_operation_id() {
            let client = mock_client();

            let request = client.request("heyhey");

            assert_eq!(request.operation_id.as_ref().unwrap(), "heyhey");
            request.build().await.expect_err("build request fails");
        }

        #[tokio::test]
        async fn errors_on_unexpected_body() {
            let client = mock_client();

            let request = client
                .request("hello")
                .body(serde_json::json!({"name": "Alice"}));

            assert_eq!(request.operation_id.as_ref().unwrap(), "hello");
            request.build().await.expect_err("build request fails");
        }

        #[tokio::test]
        async fn calls_hello() {
            let client = mock_client();

            let request = client.request("hello");

            assert_eq!(request.operation_id.as_ref().unwrap(), "hello");
            assert!(request.method.is_none());
            assert!(request.path.is_none());

            let request = request.build().await.expect("request validates");
            assert_eq!(request.method, oas::Method::Get);
            assert_eq!(request.path, "api/v1/hello/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 200);
            assert_eq!(
                response.body.unwrap(),
                serde_json::json!({"msg": "Hello, World!"})
            );
        }

        #[tokio::test]
        async fn errors_on_unknown_method() {
            let client = mock_client();

            let request = client.request_from_method_path(oas::Method::Trace, "api/v1/hello/");

            assert!(request.operation_id.is_none());
            assert_eq!(request.method.as_ref().unwrap(), &oas::Method::Trace);
            assert_eq!(request.path.as_ref().unwrap(), "api/v1/hello/");

            request.build().await.expect_err("build request fails");
        }

        #[tokio::test]
        async fn errors_on_unknown_path() {
            let client = mock_client();

            let request = client.request_from_method_path(oas::Method::Get, "api/v999/hello/");

            assert!(request.operation_id.is_none());
            assert_eq!(request.method.as_ref().unwrap(), &oas::Method::Get);
            assert_eq!(request.path.as_ref().unwrap(), "api/v999/hello/");

            request.build().await.expect_err("build request fails");
        }

        #[tokio::test]
        async fn calls_hello_from_method_path() {
            let client = mock_client();

            let request = client.request_from_method_path(oas::Method::Get, "api/v1/hello/");

            assert!(request.operation_id.is_none());
            assert_eq!(request.method.as_ref().unwrap(), &oas::Method::Get);
            assert_eq!(request.path.as_ref().unwrap(), "api/v1/hello/");

            let request = request.build().await.expect("request validates");
            assert_eq!(request.method, oas::Method::Get);
            assert_eq!(request.path, "api/v1/hello/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 200);
            assert_eq!(
                response.body.unwrap(),
                serde_json::json!({"msg": "Hello, World!"})
            );
        }

        #[tokio::test]
        async fn calls_goodbye() {
            let client = mock_client();

            let request = client.request("goodbye");

            assert_eq!(request.operation_id.as_ref().unwrap(), "goodbye");
            assert!(request.method.is_none());
            assert!(request.path.is_none());

            let request = request.build().await.expect("request validates");
            assert_eq!(request.method, oas::Method::Delete);
            assert_eq!(request.path, "api/v1/hello/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 204);
            assert!(response.body.is_none());
        }

        #[tokio::test]
        async fn calls_greet() {
            let client = mock_client();

            let request = client.request("greet").body(json!({"name": "Alice"}));

            assert_eq!(request.operation_id.as_ref().unwrap(), "greet");
            assert!(request.method.is_none());
            assert!(request.path.is_none());
            assert!(request.body.is_some());

            let request = request.build().await.expect("request validates");
            assert_eq!(request.method, oas::Method::Post);
            assert_eq!(request.path, "api/v1/hello/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 201);
            assert_eq!(
                response.body.unwrap(),
                serde_json::json!({"msg": "Hello, Alice!"})
            );
        }

        #[tokio::test]
        async fn greet_fails_for_invalid_body() {
            let client = mock_client();

            let request = client.request("greet").body(json!({"firstname": "Alice"}));

            assert_eq!(request.operation_id.as_ref().unwrap(), "greet");
            assert!(request.method.is_none());
            assert!(request.path.is_none());
            assert!(request.body.is_some());

            request.build().await.expect_err("request is invalid");
        }

        #[tokio::test]
        async fn list_user_uses_path_parameter() {
            let client = mock_client();

            let request = client.request("show_user").parameter("id", 42.into());
            let request = request.build().await.expect("request is valid");
            assert_eq!(request.path, "api/v1/user/42/");

            let response = request.send().await.expect("request calls to server");
            assert_eq!(response.status, 200);
            assert_eq!(response.body.unwrap(), serde_json::json!({"name": "Alice"}));
        }

        #[tokio::test]
        async fn list_user_errors_on_excess_parameter() {
            let client = mock_client();

            let request = client
                .request("show_user")
                .parameter("id", 42.into())
                .parameter("wtf", "wtf".into());
            request.build().await.expect_err("request is invalid");
        }

        #[tokio::test]
        async fn list_user_errors_on_wrong_type_parameter() {
            let client = mock_client();

            let request = client.request("show_user").parameter("id", "alice".into());
            request.build().await.expect_err("request is invalid");
        }
    }
}
