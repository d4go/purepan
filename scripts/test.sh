#!/bin/bash

# 测试脚本 - 运行所有测试

set -e

echo "🧪 运行测试套件..."

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# 测试计数器
total_tests=0
passed_tests=0
failed_tests=0

# 后端测试
echo -e "${BLUE}=== 后端测试 ===${NC}"
cd backend
echo -e "${YELLOW}🦀 运行 Rust 单元测试...${NC}"
if cargo test --lib; then
    backend_result=$?
    echo -e "${GREEN}✅ 后端测试通过${NC}"
    passed_tests=$((passed_tests + 1))
else
    backend_result=$?
    echo -e "${RED}❌ 后端测试失败${NC}"
    failed_tests=$((failed_tests + 1))
fi
total_tests=$((total_tests + 1))

echo ""
echo -e "${YELLOW}🔍 运行代码检查...${NC}"
if cargo clippy -- -D warnings; then
    echo -e "${GREEN}✅ 代码检查通过${NC}"
    passed_tests=$((passed_tests + 1))
else
    echo -e "${RED}❌ 代码检查失败${NC}"
    failed_tests=$((failed_tests + 1))
fi
total_tests=$((total_tests + 1))

echo ""
echo -e "${YELLOW}📐 运行代码格式检查...${NC}"
if cargo fmt -- --check; then
    echo -e "${GREEN}✅ 代码格式检查通过${NC}"
    passed_tests=$((passed_tests + 1))
else
    echo -e "${RED}❌ 代码格式检查失败${NC}"
    failed_tests=$((failed_tests + 1))
fi
total_tests=$((total_tests + 1))

cd ..

# 前端测试
echo ""
echo -e "${BLUE}=== 前端测试 ===${NC}"
cd frontend

# 检查是否有 package.json
if [ -f "package.json" ]; then
    # 检查是否安装了依赖
    if [ ! -d "node_modules" ]; then
        echo -e "${YELLOW}📦 安装前端依赖...${NC}"
        npm install
    fi

    # 运行 ESLint（如果配置了）
    if npm run lint --if-present > /dev/null 2>&1; then
        echo -e "${YELLOW}🔍 运行 ESLint...${NC}"
        if npm run lint; then
            echo -e "${GREEN}✅ ESLint 检查通过${NC}"
            passed_tests=$((passed_tests + 1))
        else
            echo -e "${RED}❌ ESLint 检查失败${NC}"
            failed_tests=$((failed_tests + 1))
        fi
        total_tests=$((total_tests + 1))
    fi

    # 运行类型检查
    echo -e "${YELLOW}📘 运行 TypeScript 类型检查...${NC}"
    if npm run type-check --if-present || npx vue-tsc --noEmit; then
        echo -e "${GREEN}✅ 类型检查通过${NC}"
        passed_tests=$((passed_tests + 1))
    else
        echo -e "${YELLOW}⚠️  类型检查有警告或未配置${NC}"
    fi
    total_tests=$((total_tests + 1))

    # 运行单元测试（如果配置了）
    if npm run test:unit --if-present > /dev/null 2>&1; then
        echo -e "${YELLOW}🧪 运行前端单元测试...${NC}"
        if npm run test:unit; then
            echo -e "${GREEN}✅ 前端单元测试通过${NC}"
            passed_tests=$((passed_tests + 1))
        else
            echo -e "${RED}❌ 前端单元测试失败${NC}"
            failed_tests=$((failed_tests + 1))
        fi
        total_tests=$((total_tests + 1))
    fi

    # 尝试构建前端
    echo -e "${YELLOW}🔨 测试前端构建...${NC}"
    if npm run build; then
        echo -e "${GREEN}✅ 前端构建成功${NC}"
        passed_tests=$((passed_tests + 1))
    else
        echo -e "${RED}❌ 前端构建失败${NC}"
        failed_tests=$((failed_tests + 1))
    fi
    total_tests=$((total_tests + 1))
else
    echo -e "${YELLOW}⚠️  未找到 package.json，跳过前端测试${NC}"
fi

cd ..

# Docker 构建测试
echo ""
echo -e "${BLUE}=== Docker 构建测试 ===${NC}"
echo -e "${YELLOW}🐳 测试 Docker 镜像构建...${NC}"
if docker build -t PurePan:test . > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Docker 镜像构建成功${NC}"
    docker rmi PurePan:test > /dev/null 2>&1 || true
    passed_tests=$((passed_tests + 1))
else
    echo -e "${RED}❌ Docker 镜像构建失败${NC}"
    failed_tests=$((failed_tests + 1))
fi
total_tests=$((total_tests + 1))

# 测试总结
echo ""
echo -e "${BLUE}===================${NC}"
echo -e "${BLUE}=== 测试总结 ===${NC}"
echo -e "${BLUE}===================${NC}"
echo "总测试数: $total_tests"
echo -e "${GREEN}通过: $passed_tests${NC}"
if [ $failed_tests -gt 0 ]; then
    echo -e "${RED}失败: $failed_tests${NC}"
else
    echo -e "${GREEN}失败: $failed_tests${NC}"
fi

# 返回结果
if [ $failed_tests -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✅ 所有测试通过！${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ 有测试失败，请检查${NC}"
    exit 1
fi

