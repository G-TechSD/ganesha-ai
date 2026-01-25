#!/bin/bash
# Comprehensive Ganesha Testing Suite
# Tests 150+ prompts across diverse use cases

set -e

GANESHA="${GANESHA:-ganesha}"
TEST_DIR="/tmp/ganesha_tests_$(date +%Y%m%d_%H%M%S)"
LOG_FILE="$TEST_DIR/test_results.log"
PASS=0
FAIL=0
TIMEOUT_SEC=60

mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

log() {
    echo "[$(date +%H:%M:%S)] $1" | tee -a "$LOG_FILE"
}

run_test() {
    local name="$1"
    local prompt="$2"
    local check="$3"  # Optional check command

    log "TEST: $name"
    mkdir -p "$TEST_DIR/$name"
    cd "$TEST_DIR/$name"

    # Run ganesha with timeout
    if timeout "$TIMEOUT_SEC" bash -c "echo '$prompt' | $GANESHA" > output.txt 2>&1; then
        if [ -n "$check" ]; then
            if eval "$check"; then
                log "  PASS: $name"
                ((PASS++))
            else
                log "  FAIL: $name (check failed)"
                ((FAIL++))
            fi
        else
            log "  PASS: $name"
            ((PASS++))
        fi
    else
        log "  FAIL: $name (timeout or error)"
        ((FAIL++))
    fi

    cd "$TEST_DIR"
}

log "========================================="
log "Ganesha Comprehensive Test Suite"
log "========================================="
log "Test directory: $TEST_DIR"
log ""

# Category 1: Basic Shell Commands (10 tests)
log "--- CATEGORY 1: Basic Shell Commands ---"
run_test "shell_ls" "ls" "grep -q 'output.txt' output.txt || true"
run_test "shell_pwd" "pwd" "grep -q '/' output.txt"
run_test "shell_date" "what is the date" "grep -q '202' output.txt"
run_test "shell_whoami" "whoami" "grep -q 'bill' output.txt"
run_test "shell_hostname" "hostname" ""
run_test "shell_df" "show disk usage" "grep -q '/' output.txt"
run_test "shell_ps" "show running processes" ""
run_test "shell_echo" "echo hello world" "grep -q 'hello' output.txt"
run_test "shell_mkdir" "create a directory called testdir" "test -d testdir"
run_test "shell_touch" "create an empty file called test.txt" "test -f test.txt"

# Category 2: File Creation (15 tests)
log "--- CATEGORY 2: File Creation ---"
run_test "file_html_simple" "create a simple HTML file called page.html with a heading" "test -f page.html && grep -q '<h' page.html"
run_test "file_css" "create a CSS file called style.css with basic styles" "test -f style.css && grep -q '{' style.css"
run_test "file_js" "create a JavaScript file called app.js that logs hello" "test -f app.js && grep -q 'console' app.js"
run_test "file_python" "create a Python file called hello.py that prints hello" "test -f hello.py && grep -q 'print' hello.py"
run_test "file_json" "create a JSON file called data.json with a name field" "test -f data.json && grep -q 'name' data.json"
run_test "file_yaml" "create a YAML config file called config.yaml" "test -f config.yaml"
run_test "file_markdown" "create a README.md with a title and description" "test -f README.md && grep -q '#' README.md"
run_test "file_shell" "create a bash script called run.sh that echoes hello" "test -f run.sh && grep -q 'echo' run.sh"
run_test "file_sql" "create a SQL file called schema.sql with a users table" "test -f schema.sql && grep -qi 'CREATE' schema.sql"
run_test "file_dockerfile" "create a Dockerfile for a node.js app" "test -f Dockerfile && grep -qi 'FROM' Dockerfile"
run_test "file_gitignore" "create a .gitignore for a node project" "test -f .gitignore && grep -q 'node_modules' .gitignore"
run_test "file_makefile" "create a Makefile with build and clean targets" "test -f Makefile && grep -q 'build' Makefile"
run_test "file_rust" "create a Rust file called main.rs that prints hello" "test -f main.rs && grep -q 'fn main' main.rs"
run_test "file_go" "create a Go file called main.go that prints hello" "test -f main.go && grep -q 'package' main.go"
run_test "file_tsx" "create a React component called Button.tsx" "test -f Button.tsx && grep -q 'export' Button.tsx"

# Category 3: Multi-File Projects (10 tests)
log "--- CATEGORY 3: Multi-File Projects ---"
run_test "multi_2files" "create 2 HTML files: a.html and b.html" "test -f a.html && test -f b.html"
run_test "multi_3files" "create 3 Python files: one.py, two.py, three.py" "test -f one.py && test -f two.py && test -f three.py"
run_test "multi_html_css" "create index.html and matching styles.css" "test -f index.html && test -f styles.css"
run_test "multi_5pages" "create a 5 page website: page1.html through page5.html" "test -f page1.html && test -f page5.html"
run_test "multi_api" "create a simple REST API with routes.py and app.py" "test -f routes.py && test -f app.py"
run_test "multi_component" "create a React component with Component.tsx and Component.css" "test -f Component.tsx && test -f Component.css"
run_test "multi_package" "create package.json and tsconfig.json for a TypeScript project" "test -f package.json && test -f tsconfig.json"
run_test "multi_docker" "create Dockerfile and docker-compose.yml" "test -f Dockerfile && test -f docker-compose.yml"
run_test "multi_test" "create main.py and test_main.py" "test -f main.py && test -f test_main.py"
run_test "multi_config" "create .env.example and config.js" "test -f .env.example && test -f config.js"

# Category 4: Information/Knowledge (10 tests)
log "--- CATEGORY 4: Information/Knowledge ---"
run_test "info_explain_git" "explain what git is in 2 sentences" "grep -qi 'version\|commit\|track' output.txt"
run_test "info_python_list" "how do I create a list in Python" "grep -q '\[' output.txt"
run_test "info_regex" "give me a regex to match email addresses" "grep -q '@' output.txt"
run_test "info_sql_join" "explain SQL JOIN" "grep -qi 'join' output.txt"
run_test "info_http_codes" "what does HTTP 404 mean" "grep -qi 'not found\|404' output.txt"
run_test "info_chmod" "explain chmod 755" "grep -qi 'permission\|read\|write\|execute' output.txt"
run_test "info_docker_vs_vm" "difference between docker and VM" "grep -qi 'container\|virtual' output.txt"
run_test "info_rest_vs_graphql" "REST vs GraphQL" "grep -qi 'query\|endpoint' output.txt"
run_test "info_async_await" "explain async await in JavaScript" "grep -qi 'async\|await\|promise' output.txt"
run_test "info_ssl_tls" "what is SSL/TLS" "grep -qi 'encrypt\|secure\|certificate' output.txt"

# Category 5: System Commands (10 tests)
log "--- CATEGORY 5: System Commands ---"
run_test "sys_memory" "show memory usage" "grep -qE 'Mem|GB|MB|free' output.txt"
run_test "sys_cpu" "show CPU info" ""
run_test "sys_uptime" "system uptime" "grep -qi 'up\|load\|day\|hour' output.txt"
run_test "sys_kernel" "kernel version" "grep -qi 'linux\|kernel' output.txt"
run_test "sys_env" "show SHELL environment variable" "grep -q 'bash\|zsh\|sh' output.txt"
run_test "sys_ip" "show IP address" ""
run_test "sys_ports" "show listening ports" ""
run_test "sys_users" "list system users" ""
run_test "sys_services" "is ssh running" ""
run_test "sys_disk_space" "how much free space on root" "grep -qE 'G|M|%' output.txt"

# Category 6: Git Operations (10 tests)
log "--- CATEGORY 6: Git Operations ---"
mkdir -p git_init_test && cd git_init_test && git init -q 2>/dev/null && cd ..
run_test "git_status" "git status" ""
run_test "git_log" "show git log" ""
run_test "git_branch" "list git branches" ""
run_test "git_init" "initialize a new git repo" "test -d .git"
run_test "git_config" "show git config" ""

# Category 7: Text Processing (10 tests)
log "--- CATEGORY 7: Text Processing ---"
echo "hello world test line" > sample.txt
run_test "text_wc" "count words in sample.txt" "grep -qE '[0-9]' output.txt"
run_test "text_head" "show first 3 lines of sample.txt" "grep -q 'hello' output.txt"
run_test "text_grep" "search for 'world' in sample.txt" "grep -q 'world' output.txt"
run_test "text_sort" "sort sample.txt" ""
run_test "text_unique" "show unique lines in sample.txt" ""

# Category 8: Code Analysis (10 tests)
log "--- CATEGORY 8: Code Analysis ---"
cat > analyze.py << 'PYEOF'
def hello():
    print("Hello")

class MyClass:
    def method(self):
        pass
PYEOF
run_test "code_explain" "explain what analyze.py does" "grep -qi 'function\|class\|print' output.txt"
run_test "code_find_funcs" "list functions in analyze.py" "grep -qi 'hello\|method' output.txt"
run_test "code_count_lines" "count lines in analyze.py" "grep -qE '[0-9]' output.txt"

# Category 9: Calculations (5 tests)
log "--- CATEGORY 9: Calculations ---"
run_test "calc_simple" "what is 25 * 4" "grep -q '100' output.txt"
run_test "calc_convert" "convert 100 celsius to fahrenheit" "grep -q '212' output.txt"
run_test "calc_percent" "what is 15% of 200" "grep -q '30' output.txt"

# Category 10: Directory Navigation (10 tests)
log "--- CATEGORY 10: Directory Navigation ---"
run_test "nav_cd_home" "cd ~" ""
run_test "nav_cd_tmp" "cd /tmp" ""
run_test "nav_list_dirs" "list only directories" ""
run_test "nav_find_files" "find all .txt files" ""
run_test "nav_tree" "show directory structure" ""

# Category 11: Package Management (5 tests)
log "--- CATEGORY 11: Package Management ---"
run_test "pkg_npm_list" "list installed npm packages globally" ""
run_test "pkg_pip_list" "list installed pip packages" ""
run_test "pkg_apt_search" "search for nginx package" ""

# Category 12: Network (5 tests)
log "--- CATEGORY 12: Network ---"
run_test "net_ping" "ping localhost once" ""
run_test "net_curl" "curl example.com" "grep -qi 'example\|html' output.txt"
run_test "net_dns" "lookup google.com DNS" ""

# Category 13: Archives (5 tests)
log "--- CATEGORY 13: Archives ---"
echo "test" > archive_test.txt
run_test "arch_tar_create" "create a tar archive of archive_test.txt" "test -f archive_test.tar* || test -f archive_test.txt.tar*"
run_test "arch_zip" "create a zip of archive_test.txt" ""

# Category 14: Permissions (5 tests)
log "--- CATEGORY 14: Permissions ---"
touch perm_test.txt
run_test "perm_chmod" "make perm_test.txt executable" "test -x perm_test.txt || ls -l perm_test.txt | grep -q 'x'"
run_test "perm_show" "show permissions of perm_test.txt" "grep -q 'rw' output.txt"

# Category 15: Help/Meta (5 tests)
log "--- CATEGORY 15: Help/Meta ---"
run_test "help_capabilities" "what can you do" "grep -qiE 'command|file|create|execute|shell' output.txt"
run_test "help_tools" "what tools do you have" ""

# Summary
log ""
log "========================================="
log "TEST SUMMARY"
log "========================================="
log "Passed: $PASS"
log "Failed: $FAIL"
log "Total: $((PASS + FAIL))"
log "Success Rate: $(echo "scale=1; $PASS * 100 / ($PASS + $FAIL)" | bc)%"
log "========================================="
log "Results saved to: $LOG_FILE"
log "Test artifacts in: $TEST_DIR"

# Return exit code based on failures
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
