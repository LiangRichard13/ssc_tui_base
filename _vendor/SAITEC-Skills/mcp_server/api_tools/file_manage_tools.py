"""File Manage Tools - MCP tools for file upload, download and management"""
import os
import httpx
from pathlib import Path
from mcp.server.fastmcp import FastMCP

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=30.0, connect=10.0)


def register_file_manage_tools(mcp: FastMCP):
    """Register file management tools to the MCP server."""

    def _headers() -> dict:
        api_key = os.getenv("SAITEC_API_KEY", "")
        return {"X-API-Key": api_key}

    # --- Tools ---

    @mcp.tool()
    async def upload_file(
        file_path: str,
        file_type: str,
    ) -> dict:
        """
        Upload a local file to the server (only supports image, video and dataset types).

        IMPORTANT: For any operation that requires reading a local file (image, video, dataset),
        you MUST first call this upload tool to upload the file to cloud storage, then use the
        returned file link (storage_uri) in the tool parameter for business execution.

        Args:
            file_path: Local path to the file, e.g., '/home/user/images/photo.png'.
            file_type: File type, must be 'image', 'video', or 'dataset'.

        Returns:
            File metadata including file_id, sha256, size_bytes, file_type, filename, created_at.
        """
        path = Path(file_path)
        if not path.exists():
            raise FileNotFoundError(f"File not found: {file_path}")

        filename = path.name
        file_bytes = path.read_bytes()

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/file-manage/upload",
                files={"file": (filename, file_bytes, "application/octet-stream")},
                data={"file_type": file_type},
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def download_file(file_id: str, file_path: str) -> dict:
        """
        Download a file by file_id and save to local path.

        Args:
            file_id: The UUID of the file to download.
            file_path: Local path to save the downloaded file, e.g., '/home/user/downloads/report.pdf'.

        Returns:
            Success message with saved file path and size.
        """
        path = Path(file_path)
        path.parent.mkdir(parents=True, exist_ok=True)

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/file-manage/files/{file_id}",
                headers=_headers(),
            )
            resp.raise_for_status()

            content_type = resp.headers.get("content-type", "application/octet-stream")
            if "application/json" in content_type:
                data = resp.json()
                raise ValueError(f"Unexpected JSON response (file may not exist): {data}")

            with open(path, "wb") as f:
                async for chunk in resp.aiter_bytes(chunk_size=8192):
                    f.write(chunk)

        size_bytes = path.stat().st_size
        return {
            "success": True,
            "file_id": file_id,
            "saved_path": str(path),
            "size_bytes": size_bytes,
        }

    @mcp.tool()
    async def list_files(
        skip: int = 0,
        limit: int = 100,
    ) -> dict:
        """
        List user's private files (excludes task-associated files).

        Args:
            skip: Number of records to skip, default 0.
            limit: Number of records to return, default 100, max 1000.

        Returns:
            Paginated file list with total count.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/file-manage/files",
                params={"skip": skip, "limit": limit},
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def list_task_files(task_id: str) -> dict:
        """
        List output files for a specific task.

        Args:
            task_id: The task ID to query.

        Returns:
            List of task output files with file_id, storage_uri, role, size_bytes, sha256, created_at.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/file-manage/tasks/{task_id}/files",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()
