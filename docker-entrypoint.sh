#!/bin/sh
set -e

echo "Running database migrations..."
diesel migration run

echo "Starting server..."
exec ./deesl
