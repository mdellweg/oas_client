import logging

import pytest

from py_oas import OasClient
from py_oas.core import log_reset_handler


class TestOasClient:
    async def test_errors_on_unknown_operation_id(self, client: OasClient) -> None:
        with pytest.raises(RuntimeError):
            await client.call("heyhey")

    async def test_errors_on_unexpected_body(self, client: OasClient) -> None:
        with pytest.raises(RuntimeError):
            await client.call("hello", body={"name": "Alice"})

    async def test_calls_hello(self, client: OasClient) -> None:
        result = await client.call("hello")
        assert result == {"msg": "Hello, World!"}

    async def test_calls_greet(self, client: OasClient) -> None:
        result = await client.call("greet", body={"name": "Alice"})
        assert result == {"msg": "Hello, Alice!"}

    async def test_calls_goodbye(self, client: OasClient) -> None:
        result = await client.call("goodbye")
        assert result is None

    async def test_loglevel_info_reveals_traces(
        self, base_url: str, client: OasClient, caplog: pytest.LogCaptureFixture
    ) -> None:
        caplog.set_level(logging.INFO)
        log_reset_handler.reset()

        await client.call("hello")
        assert ("oas", 20, "Operation ID: 'hello'") in caplog.record_tuples
        assert (
            "oas",
            20,
            f"Request: GET {base_url}api/v1/hello/",
        ) in caplog.record_tuples
        assert ("oas", 20, "Response: 200 OK") in caplog.record_tuples
