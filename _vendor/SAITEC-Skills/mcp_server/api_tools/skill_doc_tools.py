"""Skill Documentation Tools - MCP tools for accessing skill documentation

IMPORTANT: Before executing any business operation (detect_image, detect_video,
create_safety_eval, etc.), you should first call list_skills() or get_skill_doc()
to read the relevant skill documentation and understand the correct workflow.
"""
from pathlib import Path
from mcp.server.fastmcp import FastMCP

SKILLS_DIR = Path(__file__).parent.parent.parent / "skills"

SKILL_FILES = {
    "text_detect": "text_detect.md",
    "image_detect": "image_detect.md",
    "video_detect": "video_detect.md",
    "safety_eval": "safety_eval.md",
    "corpus_safety_eval": "corpus_safety_eval.md",
    "general_eval": "general_eval.md",
}


def register_skill_doc_tools(mcp: FastMCP):
    """Register skill documentation tools to the MCP server."""

    @mcp.tool()
    def list_skills() -> dict:
        """
        List all available skill documentations.

        Returns:
            A list of available skills with their names and descriptions.
        """
        skills = []
        for name, filename in SKILL_FILES.items():
            filepath = SKILLS_DIR / filename
            if filepath.exists():
                content = filepath.read_text(encoding="utf-8")
                first_line = content.split("\n")[0].strip("# ").strip()
                skills.append({
                    "name": name,
                    "filename": filename,
                    "description": first_line,
                })
        return {"skills": skills}

    @mcp.tool()
    def get_skill_doc(skill_name: str) -> dict:
        """
        Get the content of a specific skill documentation.

        Args:
            skill_name: Skill name (e.g., 'text_detect', 'image_detect').

        Returns:
            The full content of the skill documentation.
        """
        if skill_name not in SKILL_FILES:
            available = list(SKILL_FILES.keys())
            raise ValueError(f"Unknown skill: {skill_name}. Available: {available}")

        filepath = SKILLS_DIR / SKILL_FILES[skill_name]
        if not filepath.exists():
            raise FileNotFoundError(f"Skill file not found: {filepath}")

        content = filepath.read_text(encoding="utf-8")
        return {
            "skill_name": skill_name,
            "content": content,
        }

    @mcp.tool()
    def search_skills(query: str) -> dict:
        """
        Search skill documentations for a keyword.

        Args:
            query: The search keyword.

        Returns:
            Matching skills with line numbers and context.
        """
        results = []
        for name, filename in SKILL_FILES.items():
            filepath = SKILLS_DIR / filename
            if not filepath.exists():
                continue

            content = filepath.read_text(encoding="utf-8")
            lines = content.split("\n")
            matches = []
            for i, line in enumerate(lines, 1):
                if query.lower() in line.lower():
                    matches.append({
                        "line": i,
                        "context": line.strip(),
                    })

            if matches:
                results.append({
                    "skill": name,
                    "matches": matches,
                })

        return {
            "query": query,
            "results": results,
        }
