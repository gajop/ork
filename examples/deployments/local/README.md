# Local Development

Run Ork locally using Docker Compose and the Firestore emulator.

Workers run as subprocesses in local mode, not as separate containers.

## Usage

Start the services:

```bash
docker compose up -d
```

Deploy a workflow:

```bash
ork workflow deploy workflows/etl.yaml
```

Start a run:

```bash
ork run start my_etl
```

Follow logs:

```bash
ork run logs my_etl --follow
```

Stop the services:

```bash
docker compose down
```
