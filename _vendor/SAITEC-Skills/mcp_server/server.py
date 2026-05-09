"""FastMCP 启动入口"""
from dotenv import load_dotenv
from mcp.server.fastmcp import FastMCP

load_dotenv()

# 初始化唯一的 MCP 实例
mcp = FastMCP("SAITEC-Skills")

# 注册所有 tools
from api_tools.text_detect_tools import register_text_tools
from api_tools.image_detect_tools import register_image_tools
from api_tools.video_detect_tools import register_video_tools
from api_tools.safety_eval_tools import register_safety_tools
from api_tools.corpus_safety_eval_tools import register_corpus_tools
from api_tools.general_eval_tools import register_general_tools
from api_tools.file_manage_tools import register_file_manage_tools
from api_tools.skill_doc_tools import register_skill_doc_tools

register_text_tools(mcp)
register_image_tools(mcp)
register_video_tools(mcp)
register_safety_tools(mcp)
register_corpus_tools(mcp)
register_general_tools(mcp)
register_file_manage_tools(mcp)
register_skill_doc_tools(mcp)

if __name__ == "__main__":
    mcp.run()
