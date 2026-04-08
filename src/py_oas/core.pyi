import typing as t

class _LogResetHandler:
    def reset(self) -> None: ...

log_reset_handler: _LogResetHandler

class OasClient:
    def __init__(self, base_url: str, doc_href: str) -> None: ...
    async def call(self, operation_id: str, /, body: t.Any = None) -> t.Any: ...
