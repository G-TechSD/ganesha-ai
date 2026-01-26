#!/usr/bin/env bash

# -------------------------------------------------
# Backup Folder Script
# -------------------------------------------------
#
# Usage:
#   ./backup_folder.sh /path/to/source /path/to/backup/destination
#
# This script creates a timestamped tar.gz archive of the
# specified source directory and stores it in the backup
# destination. It also logs the operation to a log file.
#
# Requirements:
#   - Bash 4.x or newer
#   - tar, gzip, date utilities

set -euo pipefail

# ---------- Configuration ----------
LOG_FILE="/var/log/backup_folder.log"
BACKUP_PREFIX="backup_"

# ---------- Functions ----------
log() {
    local msg="$1"
    echo "$(date '+%Y-%m-%d %H:%M:%S') : $msg" | tee -a "$LOG_FILE"
}

error_exit() {
    log "ERROR: $1"
    exit 1
}

# ---------- Argument Validation ----------
if [[ $# -ne 2 ]]; then
    error_exit "Invalid arguments.\nUsage: $0 /path/to/source /path/to/backup/destination"
fi

SRC_DIR="$1"
DEST_DIR="$2"

# Resolve absolute paths
SRC_DIR="$(realpath "$SRC_DIR")" || error_exit "Source directory does not exist."
DEST_DIR="$(realpath "$DEST_DIR")" || error_exit "Destination directory does not exist."

log "Starting backup of '$SRC_DIR' to '$DEST_DIR'."

# ---------- Backup Process ----------
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
BACKUP_NAME="${BACKUP_PREFIX}${TIMESTAMP}.tar.gz"
BACKUP_PATH="$DEST_DIR/$BACKUP_NAME"

log "Creating archive: $BACKUP_PATH"

if tar -czf "$BACKUP_PATH" -C "$(dirname "$SRC_DIR")" "$(basename "$SRC_DIR")"; then
    log "Backup successful: $BACKUP_PATH"
else
    error_exit "Tar command failed."
fi

# ---------- Completion ----------
log "Backup operation completed successfully."

exit 0
