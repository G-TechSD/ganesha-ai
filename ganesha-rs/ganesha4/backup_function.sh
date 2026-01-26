#!/bin/bash

# Function: backup_files
# Description: Backs up files to a specified directory.
# Usage: backup_files <source_directory> <destination_directory>

backup_files() {
  # Check if the correct number of arguments is provided
  if [ $# -ne 2 ]; then
    echo "Usage: backup_files <source_directory> <destination_directory>"
    return 1
  fi

  # Assign arguments to variables
  source_dir="$1"
  dest_dir="$2"

  # Check if the source directory exists
  if [ ! -d "$source_dir" ]; then
    echo "Error: Source directory '$source_dir' does not exist."
    return 1
  fi

  # Check if the destination directory exists, create if it doesn't
  if [ ! -d "$dest_dir" ]; then
    echo "Destination directory '$dest_dir' does not exist. Creating it..."
    mkdir -p "$dest_dir"
    if [ $? -ne 0 ]; then
      echo "Error: Failed to create destination directory '$dest_dir'."
      return 1
    fi
  fi

  # Create a timestamped archive
  timestamp=$(date +%Y%m%d%H%M%S)
  backup_file="backup_${timestamp}.tar.gz"
  backup_path="$dest_dir/$backup_file"

  # Create the archive
  echo "Creating backup archive: $backup_path"
  tar -czvf "$backup_path" -C "$source_dir" .

  if [ $? -ne 0 ]; then
    echo "Error: Failed to create backup archive."
    return 1
  fi

  echo "Backup completed successfully."
  echo "Archive saved to: $backup_path"
  return 0
}

# Example usage (you can uncomment this for testing):
# backup_files "/path/to/your/source/directory" "/path/to/your/backup/directory"

