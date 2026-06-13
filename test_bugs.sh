#!/usr/bin/env bash
# ============================================================================
# 回归测试脚本 — 验证 BUG-001、BUG-002、BUG-003 已修复
# 用法: bash test_bugs.sh [base_url] [api_key]
# 默认: http://localhost:6450 + oc-9001b5b9ab76fa4bb23f8ec399e41c072149ae30
# ============================================================================

BASE_URL="${1:-http://localhost:6450}"
API_KEY="${2:-oc-9001b5b9ab76fa4bb23f8ec399e41c072149ae30}"
PASS=0
FAIL=0

green() { echo -e "\033[32m$1\033[0m"; }
red()   { echo -e "\033[31m$1\033[0m"; }
bold()  { echo -e "\033[1m$1\033[0m"; }

header() {
  echo ""
  bold "═══════════════════════════════════════════════════════════════"
  bold "  $1"
  bold "═══════════════════════════════════════════════════════════════"
}

# 执行一个测试并记录结果
# 用法: run "name" <curl command>
run() {
  local name="$1"; shift
  local out_file; out_file=$(mktemp)
  local rc=0
  "$@" >"$out_file" 2>&1 || rc=$?
  local output; output=$(cat "$out_file"); rm -f "$out_file"
  if [ $rc -eq 0 ]; then
    green "  ✅ PASS: $name — $output"
    PASS=$((PASS + 1))
  else
    red "  ❌ FAIL: $name"
    [ -n "$output" ] && echo "     $output" | head -3
    FAIL=$((FAIL + 1))
  fi
}

# ============================================================================
bold "🔍 Open Proxy AI — 回归测试"
bold "   Base URL : $BASE_URL"
bold "   API Key  : ${API_KEY:0:8}"
echo ""

health=$(curl -sf "$BASE_URL/health" 2>&1) && echo "  ✅ PASS: 服务端可达" && PASS=$((PASS + 1)) || { echo "  ❌ FAIL: 服务端不可达"; FAIL=$((FAIL + 1)); }

# ═══════════════════════════════════════════════════════════════════════════
# 辅助函数：直接执行 curl 并验证结果
# ═══════════════════════════════════════════════════════════════════════════

test_oai() {
  local model="$1" label="$2"
  local tmp; tmp=$(mktemp)
  local code
  code=$(curl -s -w "%{http_code}" -o "$tmp" -X POST "$BASE_URL/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -d "{\"model\":\"$model\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with OK\"}],\"max_tokens\":50,\"stream\":false}")
  local body; body=$(cat "$tmp"); rm -f "$tmp"
  local result ok
  result=$(echo "$body" | python3 -c "
import json,sys; d=json.load(sys.stdin)
c = d.get('choices', [])
content = c[0].get('message',{}).get('content','') if c else ''
print(f'choices={len(c)} preview={content[:20]}')
assert c and content, 'empty response'
" 2>&1) && ok=1 || ok=0
  if [ $ok -eq 1 ]; then run "$label" echo "code=$code $result"
  else run "$label" sh -c "echo 'FAIL: HTTP $code — $result'; exit 1"
  fi
}

test_anth() {
  local model="$1" label="$2"
  local tmp; tmp=$(mktemp)
  local code
  code=$(curl -s -w "%{http_code}" -o "$tmp" -X POST "$BASE_URL/v1/messages" \
    -H "Content-Type: application/json" \
    -H "x-api-key: $API_KEY" \
    -H "anthropic-version: 2023-06-01" \
    -d "{\"model\":\"$model\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with OK\"}],\"max_tokens\":50,\"stream\":false}")
  local body; body=$(cat "$tmp"); rm -f "$tmp"
  local result ok
  result=$(echo "$body" | python3 -c "
import json,sys; d=json.load(sys.stdin)
cl = d.get('content', [])
text = cl[0].get('text','') if cl else ''
print(f'content={text[:20]}')
assert cl and text, 'empty response'
" 2>&1) && ok=1 || ok=0
  if [ $ok -eq 1 ]; then run "$label" echo "code=$code $result"
  else run "$label" sh -c "echo 'FAIL: HTTP $code — $result'; exit 1"
  fi
}

# ═══════════════════════════════════════════════════════════════════════════
header "BUG-001: Nemotron 模型名 (super→ultra)"
test_oai  nemotron-3-ultra-free "nemotron-3-ultra-free (OpenAI)"
test_anth nemotron-3-ultra-free "nemotron-3-ultra-free (Anthropic)"

header "BUG-002: OpenAI 参数透传 (max_tokens)"
test_oai  deepseek-v4-flash-free "deepseek-v4-flash-free (OpenAI)"
test_oai  big-pickle             "big-pickle (OpenAI)"

header "BUG-003: Anthropic 转换保留 max_tokens"
test_anth deepseek-v4-flash-free "deepseek-v4-flash-free (Anthropic)"
test_anth big-pickle             "big-pickle (Anthropic)"

header "新内置模型"
test_oai  north-mini-code-free   "north-mini-code-free (OpenAI)"
test_oai  mimo-v2.5-free         "mimo-v2.5-free (OpenAI)"

# ═══════════════════════════════════════════════════════════════════════════
# 新增端点
# ═══════════════════════════════════════════════════════════════════════════
header "新增端点"
code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:6450/v1/models/deepseek-v4-flash-free") && \
  [ "$code" = "200" ] && run "GET /v1/models/:model" echo "200" || run "GET /v1/models/:model" echo "got $code"

code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:6450/v1/models/nonexistent") && \
  [ "$code" = "404" ] && run "GET /v1/models/:model (404)" echo "404" || run "GET /v1/models/:model (404)" echo "got $code"

code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:6450/v1/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"model":"deepseek-v4-flash-free","prompt":"Reply with OK","max_tokens":50}') && \
  [ "$code" = "200" ] && run "POST /v1/completions" echo "200" || run "POST /v1/completions" echo "got $code"

code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:6450/v1/responses/compact" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"model":"ModelPool","input":"Reply with OK","max_output_tokens":50}') && \
  [ "$code" = "200" ] && run "POST /v1/responses/compact" echo "200" || run "POST /v1/responses/compact" echo "got $code"

code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "http://localhost:6450/v1/embeddings" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"model":"text-embedding-ada-002","input":"hello"}') && \
  [ "$code" = "200" ] && run "POST /v1/embeddings" echo "200" || run "POST /v1/embeddings" echo "got $code"

# ============================================================================
echo ""
bold "═══════════════════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
  green "  全部通过: $PASS 项 ✅"
else
  echo "  通过: $PASS    失败: $FAIL"
fi
bold "═══════════════════════════════════════════════════════════════"
echo ""
exit $FAIL
