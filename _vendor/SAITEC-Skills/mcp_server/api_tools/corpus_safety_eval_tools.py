"""Corpus Safety Eval Tools - MCP tools for corpus safety evaluation"""
import os
import httpx
from typing import Optional
from mcp.server.fastmcp import FastMCP

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=60.0, connect=10.0)


def register_corpus_tools(mcp: FastMCP):
    """Register corpus safety evaluation tools to the MCP server."""

    def _headers() -> dict:
        api_key = os.getenv("SAITEC_API_KEY", "")
        return {"X-API-Key": api_key}

    # --- Tools ---

    @mcp.tool()
    async def create_corpus_safety_eval(
        task_name: str,
        texts: Optional[list[str]] = None,
        dataset: Optional[dict] = None,
        judge_model_name: str = "echo",
        judge_caller: Optional[dict] = None,
        chunking: Optional[dict] = None,
        safety_rules_text: str = "",
    ) -> dict:
        """
        Create a corpus safety evaluation task.

        IMPORTANT: Parameters must be passed as native JSON types:
        - array parameters: pass as JSON array, not "[\"item\"]"
        - object parameters: pass as JSON object, not "{\"key\": \"value\"}"

        Args:
            task_name: Task ID, recommended to be unique.
            texts: List of texts to evaluate (mutually exclusive with dataset).
                Must be a JSON array, e.g., ["text1", "text2"].
            dataset: Batch input with source_type/path/file_format/data_format (mutually exclusive with texts).
                Must be a JSON object.
            judge_model_name: Judge model display name, default 'echo'.
            judge_caller: Judge model call configuration, adapter_type supports 'openai'/'echo'.
                Must be a JSON object.
            chunking: Text chunking configuration with enabled/max_chars/overlap_chars.
                Must be a JSON object, e.g., {"enabled": true, "max_chars": 4000, "overlap_chars": 200}.
            safety_rules_text: Custom safety rules text.

        Returns:
            Task result including task_id, status, and artifacts.
        """
        payload = {"task_name": task_name, "judge_model_name": judge_model_name}
        if texts:
            payload["texts"] = texts
        if dataset:
            payload["dataset"] = dataset
        if judge_caller:
            payload["judge_caller"] = judge_caller
        if chunking:
            payload["chunking"] = chunking
        if safety_rules_text:
            payload["safety_rules_text"] = safety_rules_text

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/corpus-safety-eval/eval",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def inject_corpus_credentials(
        env_name: str,
        api_key: str,
        overwrite: bool = True,
    ) -> dict:
        """
        Inject runtime credentials for corpus safety evaluation.

        IMPORTANT: Parameters must be passed as native JSON types:
        - bool parameters: pass as true/false, not "true"/"false"

        Args:
            env_name: Environment variable name, e.g., 'DEEPSEEK_API_KEY'.
            api_key: API key plaintext.
            overwrite: Whether to overwrite existing variable, default True. Must be true/false.

        Returns:
            Result of credential injection.
        """
        payload = {
            "env_name": env_name,
            "api_key": api_key,
            "overwrite": overwrite,
        }

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/corpus-safety-eval/credentials",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_corpus_safety_task(task_id: str) -> dict:
        """
        Get corpus safety evaluation task details.

        Args:
            task_id: The task ID to query.

        Returns:
            Task details including task_id, task_type, status, summary, artifact_uris, and metadata.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/corpus-safety-eval/tasks/{task_id}",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_corpus_safety_task_artifacts(task_id: str) -> dict:
        """
        Get corpus safety evaluation task artifacts.

        Args:
            task_id: The task ID to query.

        Returns:
            Artifact paths list.
            Typical artifacts:
            - corpus_safety_eval_response.json: Structured evaluation result
            - corpus_safety_eval_report.md: Markdown format report
            - runtime/corpus_safety_eval/{task_id}/{run_tag}/: Runtime traces
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/corpus-safety-eval/tasks/{task_id}/artifacts",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()
