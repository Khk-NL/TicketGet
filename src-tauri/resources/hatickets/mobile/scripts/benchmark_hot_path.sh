#!/bin/bash
# 从详情页出发，模拟真实抢票流程（不提交订单）
# 使用方法: ./mobile/scripts/benchmark_hot_path.sh [--config mobile/config.local.jsonc] [--runs 1] [--price 580元] [--price-index 2] [--city 成都] [--date 04.18] [--json]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEFAULT_CONFIG_FILE="$REPO_ROOT/mobile/config.jsonc"
CONFIG_OVERRIDE=""

resolve_path() {
    local target="$1"
    if [[ "$target" = /* ]]; then
        printf '%s\n' "$target"
    else
        printf '%s\n' "$(cd "$(dirname "$target")" && pwd)/$(basename "$target")"
    fi
}

ARGS=("$@")
HAS_RUNS=false
for ((i=0; i<${#ARGS[@]}; i++)); do
    case "${ARGS[$i]}" in
        --runs|--runs=*)
            HAS_RUNS=true
            ;;
        --config)
            next_index=$((i + 1))
            if [ $next_index -ge ${#ARGS[@]} ]; then
                echo "❌ --config 需要一个文件路径"
                exit 1
            fi
            CONFIG_OVERRIDE="$(resolve_path "${ARGS[$next_index]}")"
            ;;
        --config=*)
            CONFIG_OVERRIDE="$(resolve_path "${ARGS[$i]#*=}")"
            ;;
    esac
done

if [ -n "$CONFIG_OVERRIDE" ]; then
    CONFIG_FILE="$CONFIG_OVERRIDE"
elif [ -n "$HATICKETS_CONFIG_PATH" ]; then
    CONFIG_FILE="$(resolve_path "$HATICKETS_CONFIG_PATH")"
else
    CONFIG_FILE="$DEFAULT_CONFIG_FILE"
fi

if ! adb devices 2>/dev/null | grep -q "device$"; then
    echo "❌ 未检测到已连接的 Android 设备"
    echo "   请通过 USB 连接设备并开启 USB 调试模式"
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo "❌ 配置文件不存在: $CONFIG_FILE"
    echo "   可先复制模板: cp mobile/config.example.jsonc mobile/config.jsonc"
    exit 1
fi

cd "$REPO_ROOT"

if [ "$HAS_RUNS" = false ]; then
    ARGS+=("--runs" "1")
fi

echo "⏱️  开始模拟抢票流程压测（不提交订单）..."
echo "   请确保手机已停在目标演出详情页（detail_page）"
echo "   当前配置文件: $CONFIG_FILE"
echo "   本脚本会强制使用安全模式: if_commit_order=false, auto_navigate=false, rush_mode=true"
echo "   会输出每一步日志和相邻步骤耗时（+Xs）"
echo ""

HATICKETS_CONFIG_PATH="$CONFIG_FILE" poetry run python mobile/hot_path_benchmark.py "${ARGS[@]}"
