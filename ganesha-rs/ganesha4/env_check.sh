#!/usr/bin/env bash
# Print all environment variables that start with USER
env | grep '^USER' || echo "No USER-prefixed variables found"
