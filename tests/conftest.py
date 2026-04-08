import json
import typing as t
from pathlib import Path

import pytest
from pytest_httpserver import HTTPServer

from py_oas import OasClient

MOCK_API = {
    "openapi": "3.1.2",
    "info": {
        "title": "Rust PY test API",
        "version": "0.0.1",
    },
    "paths": {
        "/status/": {"get": {"operationId": "status_read"}},
        "/object/": {"get": {"operationId": "object_list"}},
        "/object/{id}/": {
            "get": {
                "operationId": "object_read",
                "parameters": {"name": "id", "in": "path", "required": True},
            }
        },
    },
}


@pytest.fixture(scope="session")
def mock_api() -> t.Any:
    mock_api_path = Path(__file__).parent.parent / "assets" / "mock_api.json"
    return json.loads(mock_api_path.read_text())


@pytest.fixture
def base_url(httpserver: HTTPServer, mock_api: t.Any) -> str:
    httpserver.expect_request("/nowhere/").respond_with_json(
        {"error": "'/nowhere/' not found"}, status=404
    )
    httpserver.expect_request("/status/").respond_with_json("Hello, World!")
    httpserver.expect_request("/docs/api.json").respond_with_json(mock_api)
    httpserver.expect_request("/api/v1/hello/", method="get").respond_with_json(
        {"msg": "Hello, World!"}
    )
    httpserver.expect_request("/api/v1/hello/", method="delete").respond_with_json(
        None, status=204
    )
    httpserver.expect_request(
        "/api/v1/hello/", method="post", json={"name": "Alice"}
    ).respond_with_json({"msg": "Hello, Alice!"}, status=201)
    return httpserver.url_for("")


@pytest.fixture
def client(base_url: str) -> OasClient:
    return OasClient(base_url=base_url, doc_href="/docs/api.json")
