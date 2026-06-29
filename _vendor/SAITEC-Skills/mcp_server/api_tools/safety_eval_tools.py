"""Safety Eval Tools - MCP tools for LLM safety evaluation"""
import os
import httpx
from typing import Optional
from mcp.server.fastmcp import FastMCP

API_BASE = os.getenv("CORE_API_BASE", "http://127.0.0.1:8000")
HTTP_TIMEOUT = httpx.Timeout(timeout=60.0, connect=10.0)


def register_safety_tools(mcp: FastMCP):
    """Register safety evaluation tools to the MCP server."""

    def _headers() -> dict:
        api_key = os.getenv("SAITEC_API_KEY", "")
        return {"X-API-Key": api_key}

    # --- Tools ---

    @mcp.tool()
    async def create_safety_eval(
        task_name: str,
        model_name: str = "echo",
        judge_model_name: str = "echo",
        prompts: Optional[list[str]] = None,
        dataset: Optional[dict] = None,
        caller: Optional[dict] = None,
        judge_caller: Optional[dict] = None,
        attacks: Optional[dict] = None,
        safety_rules_text: str = "",
        risk_categories: Optional[list[str]] = None,
    ) -> dict:
        """
        Create and execute an LLM safety evaluation task.

        IMPORTANT: Parameters must be passed as native JSON types:
        - array parameters: pass as JSON array, not "[\"item\"]"
        - object parameters: pass as JSON object, not "{\"key\": \"value\"}"
        - bool parameters: pass as true/false, not "true"/"false"

        Args:
            task_name: Task ID, recommended to be unique.
            model_name: Model under test display name, default 'echo'.
            judge_model_name: Judge model display name, default 'echo'.
            prompts: String list for direct input (use with dataset or instead of it).
                Must be a JSON array, e.g., ["prompt1", "prompt2"].
            dataset: Batch input with source_type/path/file_format/data_format.
                Must be a JSON object.
            caller: Model call configuration, adapter_type supports 'openai'/'echo'.
                Must be a JSON object.
            judge_caller: Judge model call configuration. Must be a JSON object.
            attacks: Attack configuration, attack_names supports 'roleplay'/'ignore_previous'/'translation'.
                Must be a JSON object, e.g., {"attack_names": ["roleplay"]}.
            safety_rules_text: Custom judge rules text.
            risk_categories: List of risk categories to evaluate.
                Must be a JSON array, e.g., ["unsafe_content"].

        Returns:
            Task result including task_id, status, summary, risks, and artifacts.
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
        if attacks:
            payload["attacks"] = attacks
        if safety_rules_text:
            payload["safety_rules_text"] = safety_rules_text
        if risk_categories:
            payload["risk_categories"] = risk_categories

        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(
                f"{API_BASE}/api/v1/skills/safety-eval/eval",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def inject_safety_credentials(
        env_name: str,
        api_key: str,
        overwrite: bool = True,
    ) -> dict:
        """
        Inject runtime credentials for safety evaluation.

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
                f"{API_BASE}/api/v1/skills/safety-eval/credentials",
                json=payload,
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_safety_task(task_id: str) -> dict:
        """
        Get safety evaluation task details.

        Args:
            task_id: The task ID to query.

        Returns:
            Task details including task_id, task_type, status, summary, artifact_uris, and metadata.
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/safety-eval/tasks/{task_id}",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()

    @mcp.tool()
    async def get_safety_task_artifacts(task_id: str) -> dict:
        """
        Get safety evaluation task artifacts.

        Args:
            task_id: The task ID to query.

        Returns:
            Artifact paths list.
            Typical artifacts:
            - safety_eval_response.json: Structured evaluation result
            - safety_eval_report.md: Markdown format report
            - runtime/llm_safety_eval/{task_id}/{run_tag}/: Runtime traces
        """
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.get(
                f"{API_BASE}/api/v1/skills/safety-eval/tasks/{task_id}/artifacts",
                headers=_headers(),
            )
            resp.raise_for_status()
            return resp.json()
