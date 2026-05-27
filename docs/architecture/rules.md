# RepoDesk Architecture Rules

1. `repodesk-core` не знає про Tauri, React або UI.
2. `apps/desktop/src-tauri` — тільки adapter layer.
3. `apps/desktop/src` не викликає `invoke` напряму за межами `api.ts`.
4. `App.tsx` не містить domain logic.
5. Кожна фіча має свій folder:
   - api
   - hooks
   - components
   - types
6. DB schema changes only through migrations.
7. New feature requires:
   - Rust unit test
   - Tauri command contract test where possible
   - Desktop smoke check
