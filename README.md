# Ganesha
**The First Cross-Platform AI-Powered System Administration Tool**

OPENAI CODEX CLI IS ALMOST EXACTLY THIS - but they developed it about 6 months later

https://www.youtube.com/watch?v=FUq9qRwrDrI

---
WARNING - EXPERIMENTAL - CURRENTLY IN BETA
---

### Overview
Ganesha, the first advanced cross-platform AI-powered tool of its kind, empowers system administrators across Linux, Mac, and Windows with intuitive, plain English task requests in the terminal. Simply describe your desired actions, and Ganesha—backed by GPT-4o-mini translates them into commands, scripts, or even rollbacks, simplifying system management and troubleshooting.

---

## YOU NEED AN OPENAI API KEY TO USE THIS VERSION OF GANESHA
**visit platform.openai.com and obtain an API key with access to gpt-4o-mini**


## Get Started: Add your OpenAI API key to ganesha.py
**Open ganesha.py and locate #OPENAI_API_KEY = "", uncomment and paste your key**
```shell
OPENAI_API_KEY = "YOUR KEY HERE"
```

## Installation
**The following command will install all dependencies and make the "ganesha" command available**
```shell
python ganesha.py --setup
```
Unfortunately this may not work, and you might need to install the modules like so:
```shell
pip3 install openai==0.28 colorama psutil requests zipfile
```

### Key Features

- **Natural Language Processing**: Ganesha interprets plain English task requests, transforming your intentions into executable commands without requiring complex syntax.
- **Automated Execution**: With user approval, Ganesha executes commands directly, streamlining workflows and minimizing manual intervention.
- **Intelligent Troubleshooting**: Upon encountering an error, Ganesha analyzes output, identifies the issue, and offers alternative solutions or commands.
- **Rollback Capability**: Track and undo executed commands easily with an automated rollback feature.
- **Clear Explanations**: Each command includes a description, empowering users with understanding and serving as a learning tool.

### Requirements

- **OpenAI API Key**: Ganesha currently requires an OpenAI API key (not included) for GPT-4 functionality.
- **Alternative Setup**: Ganesha can potentially run with local models like GPT4All, although this setup is experimental and untested.

---

### Core Benefits

1. **Unmatched AI Integration**: Ganesha is the only cross-platform tool to leverage GPT-4 for intuitive, natural language-based system management.
2. **Versatility**: Perform administrative tasks across Linux, Mac, and Windows, covering software management, security, network setup, and more.
3. **User-Friendly**: Designed for all skill levels, Ganesha makes advanced system administration accessible and intuitive.
4. **Time-Saving Automation**: Speeds up routine tasks by automating command execution, while reducing human error.
5. **Educational Insights**: Clear explanations enhance user understanding, making it an ideal learning resource.

---

### Key Capabilities

#### Troubleshooting and Recovery
- Describe a problem, and Ganesha diagnoses and executes commands to resolve it.
- Utilize rollback commands to reverse changes made in previous sessions, with cross-platform reliability.

#### Installation and Configuration
- Automate complex software installations and dependency configurations with straightforward task requests.

#### Security and System Optimization
- **Security Audits**: Run security assessments and apply configuration updates for enhanced protection.
- **Performance Tuning**: Optimize CPU, memory, and disk usage to improve system performance.

#### Network and Connectivity Management
- Configure network settings, troubleshoot connectivity issues, and manage firewall rules without requiring in-depth networking knowledge.

#### Data Management and Backup
- Set up automated backups and restore routines, or manage disk space efficiently by identifying large or unused files.

#### Task Automation and Scripting
- Generate custom scripts to handle repetitive tasks, enhancing productivity.

---

### Rollback and Recovery System

#### Rollback Features
- **Automated Rollback Commands**: Logs all executed commands and generates inverse commands to undo applied changes.
- **Multi-Platform Reliability**: Rollback functionality is tested and verified on Linux, Mac, and Windows.
- **Context-Aware Rollbacks**: Tailors rollback actions based on the platform, restoring configurations or unmuting sound as necessary.

#### Usage Example
1. **Execution**: `ganesha "Mute system volume" --A`
2. **Rollback**: `ganesha --rollback last` – automatically restores the prior settings or installation.

---

### Real-World Applications

Ganesha simplifies a wide variety of tasks, empowering users with efficient and intuitive system management:

#### Routine Administration
- **Service Control**: Start, stop, restart, or troubleshoot system services.
- **System Updates**: Automate package updates for security and stability.
- **Resource Monitoring**: Set up monitoring and alerting for system resources like CPU and memory.

#### Developer and Compliance Support
- **Environment Setup**: Quickly configure development environments with required tools and dependencies.
- **Compliance Enforcement**: Automate compliance checks to maintain organizational standards.

#### Prompts for Sysadmins
- **"Install Docker and configure it to start on boot."**
- **"Generate a list of all active services and restart any failed ones."**
- **"Set up Nginx as a reverse proxy for a local service on port 5000."**
- **"Rollback the most recent changes and summarize the undone actions."**
- **"Install Fail2ban for SSH brute-force protection."**

#### Beginner Prompts
- **"List all files in my current folder."**
- **"Move 'notes.txt' to the 'Documents' folder."**
- **"Display my system's IP address."**
- **"Install a software package using apt."**
- **"Change file permissions to make 'script.sh' executable."**


## Command-Line Usage

```shell
ganesha [options] [plain English task request]
```

### Options:

- **`--setup`**  
  Installs all required dependencies, then adds "ganesha" command to the system.
  *Example*: `python ganesha.py --setup"`
  
- **`--rollback [session_id | last]`**  
  Reverts changes made in the most recent session or a specified session by session ID. Use `last` to roll back the latest session.  
  *Example*: `ganesha --rollback last`

- **`--summary`**  
  Provides a summary of the activities from the most recent session.  
  *Example*: `ganesha --summary`

- **`--report [criteria]`**  
  Generates a report based on specified criteria, such as a security audit or disk usage.  
  *Example*: `ganesha --report "full security audit of open ports"`

- **`--interactive`**  
  Launches an interactive menu for guided task execution, session summaries, or rollbacks.  
  *Example*: `ganesha --interactive`

- **`--debug`**  
  Activates debug mode, displaying raw responses from GPT-4 for detailed insights and troubleshooting.  
  *Example*: `ganesha --execute "Optimize system memory usage" --debug`

- **`--A`**  
  Automatically approves all commands, bypassing manual confirmation prompts.  
  *Example*: `ganesha --execute "Update all system packages" --A`

---



