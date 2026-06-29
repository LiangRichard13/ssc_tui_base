"""General Eval Tools - MCP tools for general LLM capability evaluation"""
import os
import httpx
from typing import Optional
from mcp.server.fastmcp import FastMCP
from api_tools.auth_headers import build_auth_headers
from api_tools.http_errors import raise_for_status_with_body

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=60.0, connect=10.0)


def register_general_tools(mcp: FastMCP):
    """Register general evaluation tools to the MCP server."""

    def _headers() -> dict:
        return build_auth_headers()

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

        IMPORTANT: Parameters must be passed as native JSON types:
        - array parameters: pass as JSON array, not "[\"item\"]"
        - object parameters: pass as JSON object, not "{\"key\": \"value\"}"

        Args:
            task_name: Task ID, recommended to be unique.
            model_name: Model under test display name, default 'echo'.
            judge_model_name: Judge model display name, default 'echo'.
            prompts: String list for direct input (mutually exclusive with dataset).
                Must be a JSON array, e.g., ["prompt1", "prompt2"].
            dataset: Batch input with source_type/path/file_format/data_format (mutually exclusive with prompts).
                Must be a JSON object.
            caller: Model call configuration, adapter_type supports 'openai'/'echo'.
                Must be a JSON object.
            judge_caller: Judge model call configuration. Must be a JSON object.
            dimensions: Evaluation dimensions (compatibility field, not used as scoring standard).
                Must be a JSON array, e.g., ["math_reasoning", "code_generation"].
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
            raise_for_status_with_body(resp)
            return resp.json()

    @mcp.tool()
    async def inject_general_credentials(
        env_name: str,
        api_key: str,
        overwrite: bool = True,
    ) -> dict:
        """
        Inject runtime credentials for general evaluation.

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
                f"{API_BASE}/api/v1/skills/general-eval/credentials",
                json=payload,
                headers=_headers(),
            )
            raise_for_status_with_body(resp)
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
            raise_for_status_with_body(resp)
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
            raise_for_status_with_body(resp)
            return resp.json()

    @mcp.tool()
    async def get_tested_models(
        skip: int = 0,
        limit: int = 100,
    ) -> dict:
        """
        Query current user's tested model list. Returns configured models with their
        model_name, api_key (masked), and base_url. Useful before running safety_eval
        or general_eval tasks to find which model configurations are available.

        Args:
            skip: Number of records to skip, default 0.
            limit: Number of records to return, default 100.

        Returns:
            List of tested models with id, model_name, base_url, created_at, etc.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/tested-models",
                params={"skip": skip, "limit": limit},
                headers=_headers(),
            )
            raise_for_status_with_body(resp)
            return resp.json()
