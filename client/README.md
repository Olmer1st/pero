# Pero web client

Svelte 5 + Vite + TypeScript. It is thin: no business logic, no LLM calls, no keys; every call goes to the Pero
server. The built files are served by the Rust server (`web_dir` in `/etc/pero/server.toml`).

## Build and check (Node 22 LTS)
```bash
cd client
npm ci            # pinned versions from package-lock.json; install scripts are disabled (.npmrc)
npm run check     # svelte-check: types of the whole client
npm test          # unit tests of the API module (Node's built-in runner)
npm run build     # writes dist/
npm audit         # known issues in the packages
```
`src/generated/api.ts` is generated from the Rust types in `shared/` (see `docs/data-model.md`); never edit it by hand.

## Try it against a real server
```bash
cargo build -p pero-server && npm run build      # once
client/e2e/run-local.sh                          # throw-away server + the client's API module, end to end
PERO_SERVE=1 client/e2e/run-local.sh             # keep that server running; prints a temporary token
PERO_DEV_SERVER=https://127.0.0.1:18443 npm run dev   # dev page at http://localhost:5173, API forwarded
```

## Rules the code follows
- The device token lives only in memory (`src/lib/api.ts`): never local or session storage, cookies or files. Reloading signs out.
- Requests carry no cookies and are never cached. A 401 with a token signs the app out.
- Screens use no inline scripts or styles, so the server's strict Content-Security-Policy applies.
