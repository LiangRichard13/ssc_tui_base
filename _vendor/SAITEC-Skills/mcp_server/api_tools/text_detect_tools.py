"""Text Detect Tools - MCP tools for text AIGC detection"""
import os
import httpx
from typing import Optional
from mcp.server.fastmcp import FastMCP

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=120.0, connect=10.0)


def register_text_tools(mcp: FastMCP):
    """Register text detection tools to the MCP server."""

    def _headers() -> dict:
        api_key = os.getenv("SAITEC_API_KEY", "")
        return {"X-API-Key": api_key}

    # --- Tools ---

    @mcp.tool()
    async def detect_text(
        text: str,
        method: str = "sample",
        threshold: float = 0.55,
        language: str = "zh",
        task_name: Optional[str] = None,
    ) -> dict:
        """
        Detect if a text is AI-generated (AIGC detection).

        Args:
            text: The text content to detect.
            method: Detection method, 'sample' or 'radar'. Default 'sample'.
            threshold: Decision threshold, default 0.55.
            language: Text language, default 'zh'.
            task_name: Optional task name.

        Returns:
            Detection result including label, confidence, and is_aigc flag.
        """
        payload = {
            "text": text,
            "method": method,
            "threshold": threshold,
            "language": language,
        }
        if task_name:
            payload["task_name"] = task_name

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/text-detect/detect",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def batch_detect_texts(
        items: list[dict],
        method: str = "sample",
        threshold: float = 0.55,
        task_name: Optional[str] = None,
    ) -> dict:
        """
        Batch detect if multiple texts are AI-generated.

        Args:
            items: List of text items, each containing 'text' key.
            method: Detection method, 'sample' or 'radar'. Default 'sample'.
            threshold: Decision threshold, default 0.55.
            task_name: Optional task name.

        Returns:
            Batch detection result with task_id and status.
        """
        payload = {
            "method": method,
            "threshold": threshold,
            "items": items,
        }
        if task_name:
            payload["task_name"] = task_name

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/text-detect/batch",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_text_task(task_id: str) -> dict:
        """
        Get text detection task details.

        Args:
            task_id: The task ID to query.

        Returns:
            Task details including status, summary, and artifact URIs.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/text-detect/tasks/{task_id}",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_text_task_artifacts(task_id: str) -> dict:
        """
        Get text detection task artifacts.

        Args:
            task_id: The task ID to query.

        Returns:
            Artifact paths list.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/text-detect/tasks/{task_id}/artifacts",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()
