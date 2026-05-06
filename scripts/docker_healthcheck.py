#!/usr/bin/env python3

import asyncio

from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client


async def main() -> int:
    try:
        async with streamable_http_client("http://localhost:8000/mcp") as (read, write, _):
            async with ClientSession(read, write) as session:
                await session.initialize()
                await session.list_tools()
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
