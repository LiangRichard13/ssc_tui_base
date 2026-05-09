"""General Eval Tools - MCP tools for general LLM capability evaluation"""
import os
import httpx
from typing import Optional
from mcp.server.fastmcp import FastMCP

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=60.0, connect=10.0)


def register_general_tools(mcp: FastMCP):
    """Register general evaluation tools to the MCP server."""

    def _headers() -> dict:
        api_key = os.getenv("SAITEC_API_KEY", "")
        return {"X-API-Key": api_key}

    # --- Tools ---

    @mcp.tool()
    async def create_general_eval(
        task_name: str,
        model_name: str = "echo",
        judge_model_name: str = "echo",
        prompts: Optional[list[str]] = None,
        dataset: Optional[dict] = None,
        caller: Optional[dict] = None,
        judge_caller: Optional[dict] = None,
        dimensions: Optional[list[str]] = None,
        evaluation_rubric_text: str = "",
    ) -> dict:
        """
        Create a general LLM capability evaluation task.

        Args:
            task_name: Task ID, recommended to be unique.
            model_name: Model under test display name, default 'echo'.
            judge_model_name: Judge model display name, default 'echo'.
            prompts: String list for direct input (mutually exclusive with dataset).
            dataset: Batch input with source_type/path/file_format/data_format (mutually exclusive with prompts).
            caller: Model call configuration, adapter_type supports 'openai'/'echo'.
            judge_caller: Judge model call configuration.
            dimensions: Evaluation dimensions (compatibility field, not used as scoring standard).
            evaluation_rubric_text: Evaluation rubric text (compatibility field).

        Returns:
            Task result including task_id, status, summary, and artifacts.
        """
        payload = {"task_name": task_name, "model_name": model_name, "judge_model_name": judge_model_name}
        if prompts:
            payload["prompts"] = prompts
        if dataset:
            payload["dataset"] = dataset
        if caller:
            payload["caller"] = caller
        if judge_caller:
            payload["judge_caller"] = judge_caller
        if dimensions:
            payload["dimensions"] = dimensions
        if evaluation_rubric_text:
            payload["evaluation_rubric_text"] = evaluation_rubric_text

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/general-eval/eval",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def inject_general_credentials(
        env_name: str,
        api_key: str,
        overwrite: bool = True,
    ) -> dict:
        """
        Inject runtime credentials for general evaluation.

        Args:
            env_name: Environment variable name, e.g., 'DEEPSEEK_API_KEY'.
            api_key: API key plaintext.
            overwrite: Whether to overwrite existing variable, default True.

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
                f"{API_BASE}/api/v1/skills/general-eval/credentials",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_general_task(task_id: str) -> dict:
        """
        Get general evaluation task details.

        Args:
            task_id: The task ID to query.

        Returns:
            Task details including task_id, task_type, status, summary (averaged scores by field), artifact_uris, and metadata.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/general-eval/tasks/{task_id}",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_general_task_artifacts(task_id: str) -> dict:
        """
        Get general evaluation task artifacts.

        Args:
            task_id: The task ID to query.

        Returns:
            Artifact paths list.
            Typical artifacts:
            - common_eval_response.json: Structured evaluation result
            - common_eval_report.md: Markdown format report
            - runtime/llm_common_eval/{task_id}/{run_tag}/: Runtime traces
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/general-eval/tasks/{task_id}/artifacts",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()
