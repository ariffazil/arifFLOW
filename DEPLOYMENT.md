# Deployment — arifFlow (Orchestration)

## Prerequisites

- Docker 24+ and Docker Compose v2
- 2 CPU cores, 4GB RAM
- Ports: `7073` (arifFlow organ)

## Quick Start

```bash
git clone https://github.com/arif-fazil/arifFlow.git
cd arifFlow
docker compose up -d

# Verify
curl http://localhost:7073/health
```

## Docker Compose

```yaml
services:
  arifflow:
    image: arifazil/arifflow:latest
    ports:
      - "7073:7073"
    environment:
      - ARIFLOW_ORGAN_REGISTRY=http://arifos-kernel:8088
    restart: unless-stopped
```

## Federation Role

arifFlow orchestrates workflows across all organs. Deploy after arifOS kernel
and AAA.
