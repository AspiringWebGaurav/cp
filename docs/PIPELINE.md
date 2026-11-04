# GPHost Autonomous Release Pipeline

## Configuration
- **Engine Endpoint:** `https://gphost.eu.cc`
- **Application Target:** `cp`
- **Current Version:** `0.0.1`

## Publishing a Release
Run the automated shipping script:
```bash
npm run ship
```

This automates:
1. Production frontend build (`tsc && vite build`).
2. Release binary compilation with Cargo (`cargo build --release`).
3. SHA-256 binary checksum computation.
4. GPHost Cloud Engine manifest upload via API key (`gp_live_...`).

