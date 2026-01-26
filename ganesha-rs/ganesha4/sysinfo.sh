#!/bin/bash

# Script to display system information

echo "---------------------------------"
echo "        System Information       "
echo "---------------------------------"
echo "Hostname: $(hostname)"
echo "Operating System: $(uname -s) $(uname -r) $(uname -v)"
echo "Kernel Architecture: $(uname -m)"
echo "Uptime: $(uptime | awk '{print $3, $4, $5}')"
echo "Current Date and Time: $(date)"
echo "---------------------------------"
echo "        Hardware Information       "
echo "---------------------------------"
echo "CPU Model: $(cat /proc/cpuinfo | grep 'model name' | head -n 1 | awk -F: '{print $2}' | sed 's/^ //')"
echo "Number of CPU Cores: $(nproc)"
echo "Total Memory: $(free -h | grep Mem | awk '{print $2}')"
echo "Available Memory: $(free -h | grep Mem | awk '{print $7}')"
echo "Disk Space (root): $(df -h / | awk 'NR==2{print $2}')"
echo "Disk Used (root): $(df -h / | awk 'NR==2{print $3}')"
echo "Disk Available (root): $(df -h / | awk 'NR==2{print $4}')"
echo "---------------------------------"
echo "        Network Information        "
echo "---------------------------------"
echo "IP Address: $(hostname -I)"
echo "---------------------------------"

