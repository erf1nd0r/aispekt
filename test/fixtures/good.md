# acme-api — agent instructions

TypeScript API on Bun + Hono, deployed to Cloudflare Workers.

## Commands

- Build: `bun run build`
- Test: `bun test` (run before every commit)
- Typecheck: `bun run typecheck`
- Deploy (staging only): `bun run deploy:staging`

## Gotchas

- The dev server needs `MOCK_BILLING=1` or startup fails against prod billing.
- Migrations run through `bun run db:migrate`, not wrangler — wrangler skips the seed step.
- `src/legacy/` is frozen during the v2 migration; route new endpoints via `src/api/handlers/`.

## Conventions that differ from defaults

- Result types over thrown exceptions in `src/api/` (see `src/api/result.ts` for the pattern).
- Route files export a single `register(app)` function.

## Boundaries

- Never commit secrets or .env files.
- Ask before deploying to production or deleting migrations.
- Don't edit `src/legacy/` without asking first.
