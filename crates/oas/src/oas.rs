use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

type TodoValue = Value;

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct License {
    name: String,
    identifier: Option<String>,
    url: Option<String>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contact {
    name: Option<String>,
    url: Option<String>,
    email: Option<String>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    title: String,
    version: String,
    description: Option<String>,
    terms_of_service: Option<String>,
    contact: Option<Contact>,
    license: Option<License>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Schema {}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaType {
    pub schema: Option<Value>,
    // schema: Option<Schema>,
    // example
    // examples
    // encoding
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    description: String,
    headers: Option<HashMap<String, TodoValue>>,
    content: Option<HashMap<String, MediaType>>,
    links: Option<HashMap<String, TodoValue>>,
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterLocation {
    Path,
    Header,
    Query,
    Cookie,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: ParameterLocation,
    pub required: Option<bool>,
    pub schema: Option<Value>,
}

impl Parameter {
    pub fn is_required(&self) -> bool {
        self.required.unwrap_or(false) || self.location == ParameterLocation::Path
    }
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    description: Option<String>,
    pub content: HashMap<String, MediaType>,
    pub required: Option<bool>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub operation_id: Option<String>,
    tags: Option<Vec<String>>,
    summary: Option<String>,
    pub parameters: Option<Vec<Parameter>>,
    pub request_body: Option<RequestBody>,
    responses: Option<HashMap<String, Response>>,
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    Get,
    Head,
    Delete,
    Post,
    Put,
    Patch,
    Trace,
}

impl From<Method> for reqwest::Method {
    fn from(val: Method) -> Self {
        match val {
            Method::Get => reqwest::Method::GET,
            Method::Head => reqwest::Method::HEAD,
            Method::Delete => reqwest::Method::DELETE,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Trace => reqwest::Method::TRACE,
        }
    }
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathItem {
    pub parameters: Option<Vec<Parameter>>,
    #[serde(flatten)]
    pub operations: HashMap<Method, Operation>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApi {
    openapi: String,
    info: Info,
    pub paths: Option<HashMap<String, PathItem>>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
    components: Option<TodoValue>,
}

#[cfg(test)]
mod tests {
    use crate::oas;

    const MOCK_API: &'static str = include_str!("../../../assets/mock_api.json");

    #[test]
    fn valid_schema_is_deserialized() {
        let _openapi: oas::OpenApi = serde_json::from_str(MOCK_API).expect("Parse API docs");
    }

    #[test]
    fn request_body_can_be_validated_with_jsonschema() {
        let openapi: oas::OpenApi = serde_json::from_str(MOCK_API).expect("Parse API docs");
        let paths_spec = openapi.paths.expect("Paths exists.");
        let path_spec = paths_spec.get("api/v1/hello/").expect("Path is defined.");
        let operation_spec = path_spec
            .operations
            .get(&oas::Method::Post)
            .expect("Method is defined.");
        let request_body_spec = operation_spec
            .request_body
            .as_ref()
            .expect("Body is defined.");
        let mediatype = request_body_spec
            .content
            .get("application/json")
            .expect("Content type is provided.");
        let schema = mediatype.schema.as_ref().expect("Schema is defined");
        let registry = jsonschema::Registry::new()
            .add("h", serde_json::json!({"components": openapi.components}))
            .unwrap()
            .prepare()
            .unwrap();
        println!("{:?}", registry);
        let validator = jsonschema::options()
            .offline()
            .with_registry(&registry)
            .build(&schema)
            .unwrap();
        println!("{:?}", validator);
        assert!(validator.is_valid(&serde_json::json!({"name": "Alice"})));
    }

    #[test]
    fn request_body_can_be_invalidated_with_jsonschema() {
        let openapi: oas::OpenApi = serde_json::from_str(MOCK_API).expect("Parse API docs");
        let paths_spec = openapi.paths.expect("Paths exists.");
        let path_spec = paths_spec.get("api/v1/hello/").expect("Path is defined.");
        let operation_spec = path_spec
            .operations
            .get(&oas::Method::Post)
            .expect("Method is defined.");
        let request_body_spec = operation_spec
            .request_body
            .as_ref()
            .expect("Body is defined.");
        let mediatype = request_body_spec
            .content
            .get("application/json")
            .expect("Content type is provided.");
        let schema = mediatype.schema.as_ref().expect("Schema is defined");
        assert!(!jsonschema::is_valid(
            &schema,
            &serde_json::json!({"firstname": "Alice"})
        ));
    }

    #[test]
    fn parameter_is_required_if_in_path() {
        let parameter: oas::Parameter =
            serde_json::from_str(r#"{"name": "test", "in": "path"}"#).unwrap();
        assert!(parameter.is_required());
    }
}

#[cfg(test)]
mod test_references {
    use jsonschema::Registry;
    use serde_json::json;

    #[test]
    #[ignore]
    fn validate_using_a_local_reference() -> Result<(), Box<dyn std::error::Error>> {
        let components = json!({"components": {"schemas": {"Name": {"type": "string"}}}});
        let schema = json!({"$ref": "#/components/schemas/Name"});
        let object = json!("Alice");

        let registry = Registry::new().add("", components)?.prepare()?;
        let validator = jsonschema::options()
            .offline()
            .with_registry(&registry)
            .build(&schema)?;

        assert!(validator.is_valid(&object));
        Ok(())
    }

    #[test]
    fn validate_using_inlined_reference() -> Result<(), Box<dyn std::error::Error>> {
        let schema = json!({"$ref": "#/components/schemas/Name", "components": {"schemas": {"Name": {"type": "string"}}}});
        let object = json!("Alice");

        let validator = jsonschema::options().offline().build(&schema)?;

        assert!(validator.is_valid(&object));
        Ok(())
    }

    #[test]
    fn validate_using_a_quasi_local_reference() -> Result<(), Box<dyn std::error::Error>> {
        let components = json!({"components": {"schemas": {"Name": {"type": "string"}}}});
        let schema = json!({"$ref": "a#/components/schemas/Name"});
        let object = json!("Alice");

        let registry = Registry::new().add("a", components)?.prepare()?;
        let validator = jsonschema::options()
            .offline()
            .with_registry(&registry)
            .build(&schema)?;

        assert!(validator.is_valid(&object));
        Ok(())
    }
}
