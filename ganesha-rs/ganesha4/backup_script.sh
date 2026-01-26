#!/bin/bash

# Set the directory to backup
source_dir="/path/to/your/directory"

# Set the backup directory
backup_dir="/path/to/your/backup/directory"

# Create a timestamp for the backup
timestamp=$(date +%Y%m%d%H%M%S)

# Create the backup filename
backup_file="backup_${timestamp}.tar.gz"

# Create the full path to the backup file
backup_path="${backup_dir}/${backup_file}"

# Check if the source directory exists
if [ ! -d "${source_dir}" ]; then
  echo "Error: Source directory '${source_dir}' does not exist."
  exit 1
fi

# Check if the backup directory exists, create it if it doesn't
if [ ! -d "${backup_dir}" ]; then
  echo "Backup directory '${backup_dir}' does not exist. Creating it..."
  mkdir -p "${backup_dir}"
  if [ $? -ne 0 ]; then
    echo "Error: Could not create backup directory '${backup_dir}'."
    exit 1
  fi
fi

# Create the backup using tar and gzip
echo "Creating backup '${backup_path}' from '${source_dir}'..."
tar -czvf "${backup_path}" -C "$(dirname "${source_dir}")" "$(basename "${source_dir}")"

# Check if the backup was successful
if [ $? -eq 0 ]; then
  echo "Backup created successfully: '${backup_path}'"
else
  echo "Error: Backup failed."
  exit 1
fi

exit 0
