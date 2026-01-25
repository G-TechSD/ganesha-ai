# Ganesha 4.0 Comprehensive Test Plan

## 100 Use Cases for Testing

### Category 1: Coding Tasks (1-25)

1. **Create a Python function** - "Write a function to calculate fibonacci numbers"
2. **Debug code** - "Why does this code throw an error: `def foo(): return bar`"
3. **Explain code** - "Explain what this regex does: `^[\w.-]+@[\w.-]+\.\w+$`"
4. **Refactor code** - "Refactor this code to use list comprehension: `result = []; for i in range(10): result.append(i*2)`"
5. **Write tests** - "Write unit tests for a function that validates email addresses"
6. **Code review** - "Review this code for potential bugs: `if x = 5: print('five')`"
7. **Convert between languages** - "Convert this Python to Rust: `def add(a, b): return a + b`"
8. **Optimize algorithm** - "Optimize this O(n²) sorting algorithm"
9. **Add type hints** - "Add type hints to this Python function"
10. **Create class** - "Create a Python class for a bank account with deposit/withdraw"
11. **Write API endpoint** - "Write a FastAPI endpoint for user registration"
12. **Parse JSON** - "Write code to parse this JSON structure and extract emails"
13. **Handle errors** - "Add proper error handling to this file reading code"
14. **Write regex** - "Write a regex to match phone numbers in format XXX-XXX-XXXX"
15. **Create CLI tool** - "Write a Python CLI tool to rename files in bulk"
16. **Database query** - "Write a SQL query to find duplicate entries"
17. **Write async code** - "Convert this sync function to async in Python"
18. **Create decorator** - "Write a Python decorator for timing function execution"
19. **Implement algorithm** - "Implement binary search in Python"
20. **Fix memory leak** - "This code has a memory leak, find and fix it"
21. **Write config parser** - "Write code to parse a TOML configuration file"
22. **Create data class** - "Create a dataclass for representing a User"
23. **Write validator** - "Write input validation for a form with email, phone, age"
24. **Implement caching** - "Add caching to this expensive function"
25. **Write migration** - "Write a database migration to add a new column"

### Category 2: System Administration (26-50)

26. **List files** - "ls -la"
27. **Change directory** - "cd /tmp"
28. **Check disk space** - "df -h"
29. **View processes** - "ps aux | head -20"
30. **Check memory** - "free -h"
31. **Find large files** - "find files larger than 100MB in home directory"
32. **Check network** - "Check network connectivity to google.com"
33. **View logs** - "Show last 50 lines of syslog"
34. **Check services** - "Check if nginx is running"
35. **Create directory** - "Create a directory structure for a new project"
36. **Set permissions** - "Make this script executable"
37. **Compress files** - "Create a tar.gz of this directory"
38. **Extract archive** - "Extract a zip file"
39. **Search in files** - "Find all files containing 'TODO' in this project"
40. **Count lines** - "Count lines of code in all Python files"
41. **Check ports** - "What's listening on port 8080?"
42. **View environment** - "Show relevant environment variables"
43. **Create symlink** - "Create a symbolic link"
44. **Check git status** - "git status"
45. **View git log** - "git log --oneline -10"
46. **Check disk usage** - "What's using the most space in /var?"
47. **Monitor resources** - "Show CPU and memory usage"
48. **List packages** - "List installed Python packages"
49. **Check uptime** - "System uptime"
50. **View kernel** - "Show kernel version"

### Category 3: Research & Web Tasks (51-70)

51. **Explain concept** - "Explain how TCP/IP works"
52. **Compare technologies** - "Compare REST vs GraphQL"
53. **Best practices** - "What are best practices for API design?"
54. **Troubleshoot** - "Why might a Docker container keep restarting?"
55. **Security advice** - "How to secure a Redis instance?"
56. **Architecture** - "Explain microservices vs monolith"
57. **Debug strategy** - "How to debug a memory leak in Python?"
58. **Performance** - "How to optimize PostgreSQL queries?"
59. **Learn concept** - "Explain async/await in JavaScript"
60. **Tool comparison** - "Compare Kubernetes vs Docker Swarm"
61. **Configuration** - "How to configure nginx as a reverse proxy?"
62. **Debugging** - "How to debug SSL certificate issues?"
63. **Error explanation** - "What does 'ECONNREFUSED' mean?"
64. **Protocol** - "Explain WebSocket protocol"
65. **Pattern** - "Explain the Observer design pattern"
66. **Algorithm** - "How does quicksort work?"
67. **Data structure** - "When to use a B-tree vs hash table?"
68. **Concurrency** - "Explain mutex vs semaphore"
69. **Networking** - "What is CIDR notation?"
70. **Cloud** - "Explain AWS Lambda cold starts"

### Category 4: File Operations (71-85)

71. **Read file** - "Read the contents of Cargo.toml"
72. **Write file** - "Create a simple README.md"
73. **Append to file** - "Add a line to a log file"
74. **Copy file** - "Copy a file to backup"
75. **Move file** - "Rename a file"
76. **Delete file** - "Remove a temp file"
77. **Search content** - "Find all imports in Python files"
78. **Replace content** - "Replace all occurrences of 'foo' with 'bar'"
79. **Merge files** - "Concatenate multiple files"
80. **Split file** - "Split a large file into chunks"
81. **Sort file** - "Sort lines in a file"
82. **Unique lines** - "Remove duplicate lines from file"
83. **Compare files** - "Diff two files"
84. **File info** - "Get detailed info about a file"
85. **File encoding** - "Check file encoding"

### Category 5: Edge Cases & Stress Tests (86-100)

86. **Empty input** - ""
87. **Very long input** - "[1000 character string]"
88. **Special characters** - "Test with émojis 🎉 and spëcial çharacters"
89. **Multi-line input** - "Process this\nmulti-line\nstring"
90. **Nested quotes** - "Handle 'nested \"quotes\"' properly"
91. **Command injection** - "ls; rm -rf /" (should be safe)
92. **Path traversal** - "cat ../../../etc/passwd" (should be handled)
93. **Rapid requests** - "Quick succession of commands"
94. **Long running task** - "Sleep for 5 seconds then echo done"
95. **Error recovery** - "Run nonexistent_command"
96. **Interrupt handling** - "Handle Ctrl+C gracefully"
97. **Large output** - "Generate a large amount of output"
98. **Binary data** - "Handle binary file content"
99. **Timeout** - "Handle a command that might hang"
100. **Session state** - "Remember context from previous messages"

## Testing Criteria

1. **Output Quality**: Is the response helpful and accurate?
2. **Error Handling**: Does it gracefully handle errors?
3. **Speed**: How fast is the response?
4. **Context**: Does it maintain conversation context?
5. **Safety**: Does it prevent dangerous operations?
6. **Formatting**: Is the output well-formatted in the Ganesha box?
