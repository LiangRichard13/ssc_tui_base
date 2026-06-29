#!/bin/bash
# MCP Server 启动脚本
# 使用方式: claude mcp add SAITEC-Skills /home/lcd/SAITEC-Skills/run_mcp.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source ~/miniconda3/etc/profile.d/conda.sh
# 这里假设你的环境名称为 skills 并在skills中安装了所需的依赖
conda activate skills

# 环境变量配置
export SAITEC_API_KEY=c03d790c-c82f-400a-84ae-1fb3cf4ff6cb
export CORE_API_BASE=http://127.0.0.1:8000

python "$SCRIPT_DIR/mcp_server/server.py"
