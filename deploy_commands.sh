#!/bin/bash

# Deploy backend

docker build -t craigsdevcontainers.azurecr.io/artbabo:latest .

docker push  craigsdevcontainers.azurecr.io/artbabo:latest

# Deploy frontend

